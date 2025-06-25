import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { Ponzimon } from "../target/types/ponzimon";
import {
  Keypair,
  SystemProgram,
  Connection,
  Transaction,
} from "@solana/web3.js";
import {
  createMint,
  getAssociatedTokenAddress,
  createSetAuthorityInstruction,
  AuthorityType,
} from "@solana/spl-token";

export async function setupTestProgram() {
  const provider = anchor.AnchorProvider.local();
  anchor.setProvider(provider);
  const program = anchor.workspace.Ponzimon as Program<Ponzimon>;
  const authority = Keypair.generate();

  const airdropSig = await provider.connection.requestAirdrop(
    authority.publicKey,
    10000000000
  );
  const latestBlockhash = await provider.connection.getLatestBlockhash();
  await provider.connection.confirmTransaction({
    blockhash: latestBlockhash.blockhash,
    lastValidBlockHeight: latestBlockhash.lastValidBlockHeight,
    signature: airdropSig,
  });

  const mint = await createMint(
    provider.connection,
    authority,
    authority.publicKey,
    null,
    6
  );

  const [globalStatePda] = await anchor.web3.PublicKey.findProgramAddress(
    [Buffer.from("global_state"), mint.toBuffer()],
    program.programId
  );

  const transferAuthTx = new Transaction().add(
    createSetAuthorityInstruction(
      mint,
      authority.publicKey,
      AuthorityType.MintTokens,
      globalStatePda
    )
  );
  await provider.sendAndConfirm(transferAuthTx as any, [authority]);

  const [solRewardsWalletPda] = await anchor.web3.PublicKey.findProgramAddress(
    [Buffer.from("sol_rewards_wallet"), mint.toBuffer()],
    program.programId
  );
  const [stakingVaultPda] = await anchor.web3.PublicKey.findProgramAddress(
    [Buffer.from("staking_vault"), mint.toBuffer()],
    program.programId
  );
  const feesTokenAccount = await getAssociatedTokenAddress(
    mint,
    authority.publicKey
  );

  await program.methods
    .initializeProgram(
      new BN(0), // startSlot
      new BN("1000000000000000"), // totalSupply (1B with 6 decimals)
      new BN(352733915), // initialRewardRate
      null,
      null,
      null,
      null,
      new BN(100), // staking lockup
      new BN(5) // token reward rate
    )
    .accounts({
      authority: authority.publicKey,
      globalState: globalStatePda,
      feesWallet: authority.publicKey,
      solRewardsWallet: solRewardsWalletPda,
      feesTokenAccount: feesTokenAccount,
      stakingVault: stakingVaultPda,
      tokenMint: mint,
    } as any)
    .signers([authority])
    .rpc();

  return {
    program,
    provider,
    connection: provider.connection,
    mint,
    authority,
    globalState: globalStatePda,
    feesWallet: authority.publicKey,
    solRewardsWallet: solRewardsWalletPda,
    stakingVault: stakingVaultPda,
  };
}

export async function advanceSlots(
  provider: anchor.AnchorProvider,
  slots: number
) {
  const currentSlot = await provider.connection.getSlot();
  const targetSlot = currentSlot + slots;
  for (let i = 0; i < slots; i++) {
    await provider.connection.requestAirdrop(
      Keypair.generate().publicKey,
      1 // A single lamport is enough to process a transaction
    );
  }
  let newSlot = await provider.connection.getSlot();
  while (newSlot < targetSlot) {
    await new Promise((resolve) => setTimeout(resolve, 500));
    newSlot = await provider.connection.getSlot();
  }
}

export async function airdrop(
  provider: anchor.AnchorProvider,
  address: anchor.web3.PublicKey,
  amount: number
) {
  const airdropSig = await provider.connection.requestAirdrop(
    address,
    amount * anchor.web3.LAMPORTS_PER_SOL
  );
  const latestBlockhash = await provider.connection.getLatestBlockhash();
  await provider.connection.confirmTransaction({
    blockhash: latestBlockhash.blockhash,
    lastValidBlockHeight: latestBlockhash.lastValidBlockHeight,
    signature: airdropSig,
  });
}

export async function setupTestPlayer(
  program: Program<Ponzimon>,
  provider: anchor.AnchorProvider,
  mint: anchor.web3.PublicKey,
  authority: Keypair,
  globalState: anchor.web3.PublicKey
) {
  const playerWallet = Keypair.generate();
  const airdropSigPlayer = await provider.connection.requestAirdrop(
    playerWallet.publicKey,
    10000000000
  );
  const latestBlockhashPlayer = await provider.connection.getLatestBlockhash();
  await provider.connection.confirmTransaction({
    blockhash: latestBlockhashPlayer.blockhash,
    lastValidBlockHeight: latestBlockhashPlayer.lastValidBlockHeight,
    signature: airdropSigPlayer,
  });

  const [playerPda] = await anchor.web3.PublicKey.findProgramAddress(
    [Buffer.from("player"), playerWallet.publicKey.toBuffer(), mint.toBuffer()],
    program.programId
  );

  const randomnessAccount = Keypair.generate();
  const gs_account = await program.account.globalState.fetch(globalState);
  const playerTokenAccount = await getAssociatedTokenAddress(
    mint,
    playerWallet.publicKey
  );

  await program.methods
    .purchaseInitialFarm()
    .accounts({
      playerWallet: playerWallet.publicKey,
      player: playerPda,
      globalState: globalState,
      feesWallet: gs_account.feesWallet,
      referrerWallet: null,
      tokenMint: mint,
      playerTokenAccount: playerTokenAccount,
      randomnessAccountData: randomnessAccount.publicKey,
    } as any)
    .signers([playerWallet])
    .rpc();

  return {
    player: playerPda,
    playerWallet,
    playerTokenAccount,
  };
}

export async function createTestMint(
  connection: Connection,
  authority: Keypair
) {
  return await createMint(
    connection as any,
    authority,
    authority.publicKey,
    null,
    9
  );
}

export async function createTestTokenAccount(
  provider: anchor.AnchorProvider,
  mint: anchor.web3.PublicKey,
  owner: anchor.web3.PublicKey,
  isAssociated = false
) {
  if (isAssociated) {
    const address = await getAssociatedTokenAddress(mint, owner, false);
    return { address };
  }
  const account = Keypair.generate();
  return {
    address: account.publicKey,
    keypair: account,
  };
}

// You may need to add more helper functions here as needed for your tests
