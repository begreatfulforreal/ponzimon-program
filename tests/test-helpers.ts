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
  TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
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
    9
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
      new BN(100), // halvingInterval
      new BN(1000000), // totalSupply
      new BN(10), // initialRewardRate
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
  };
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

export async function createTestAccount(
  connection: Connection,
  payer: Keypair,
  mint: anchor.web3.PublicKey,
  owner: anchor.web3.PublicKey
) {
  return await createAccount(connection as any, payer, mint, owner);
}

// You may need to add more helper functions here as needed for your tests
