import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Ponzimon } from "../target/types/ponzimon";
import * as assert from "assert";
import {
  airdrop,
  setupTestProgram,
  createTestTokenAccount,
} from "./test-helpers";

describe("Ponzimon Basic Flow", () => {
  let program: Program<Ponzimon>;
  let provider: anchor.AnchorProvider;
  let connection: anchor.web3.Connection;
  let mint: anchor.web3.PublicKey;
  let authority: anchor.web3.Keypair;
  let globalState: anchor.web3.PublicKey;
  let feesWallet: anchor.web3.PublicKey;
  let solRewardsWallet: anchor.web3.PublicKey;
  let stakingVault: anchor.web3.PublicKey;
  let feesTokenAccount: anchor.web3.PublicKey;

  beforeAll(async () => {
    // --- Program Setup ---
    const setup = await setupTestProgram();
    program = setup.program;
    provider = setup.provider;
    connection = setup.connection;
    mint = setup.mint;
    authority = setup.authority as any;
    globalState = setup.globalState;
    feesWallet = setup.feesWallet;
    solRewardsWallet = setup.solRewardsWallet;
    stakingVault = setup.stakingVault;

    // --- Create Associated Token Accounts ---
    const feesAta = await createTestTokenAccount(
      provider,
      mint,
      feesWallet,
      true
    );
    feesTokenAccount = feesAta.address;
  });

  it("should create a player, stake cards, and claim rewards", async () => {
    // --- Player Setup ---
    const playerWallet = anchor.web3.Keypair.generate();
    await airdrop(provider, playerWallet.publicKey, 10); // 10 SOL

    const [playerPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [
        Buffer.from("player"),
        playerWallet.publicKey.toBuffer(),
        mint.toBuffer(),
      ],
      program.programId
    );

    const playerTokenAccount = await createTestTokenAccount(
      provider,
      mint,
      playerWallet.publicKey,
      true
    );

    // --- Purchase Initial Farm ---
    await program.methods
      .purchaseInitialFarm()
      .accounts({
        playerWallet: playerWallet.publicKey,
        feesWallet: feesWallet,
        referrerWallet: null,
        tokenMint: mint,
        // Mock randomness account, not used in this test
        randomnessAccountData: anchor.web3.Keypair.generate().publicKey,
      })
      .signers([playerWallet])
      .rpc();

    let playerAccount = await program.account.player.fetch(playerPda);
    assert.strictEqual(
      playerAccount.cardCount,
      3,
      "Player should have 3 starter cards"
    );

    const maxStakedCardsFirstFarm = 2;
    // --- Stake Starter Cards ---
    for (let i = 0; i < maxStakedCardsFirstFarm; i++) {
      await program.methods
        .stakeCard(i)
        .accounts({
          playerWallet: playerWallet.publicKey,
          tokenMint: mint,
        })
        .signers([playerWallet])
        .rpc();
    }

    playerAccount = await program.account.player.fetch(playerPda);
    assert.strictEqual(
      playerAccount.stakedCardsBitset.bitLength(),
      maxStakedCardsFirstFarm,
      "All 2 starter cards should be staked"
    );
    assert.ok(
      playerAccount.totalHashpower.gtn(0),
      "Player hashpower should be greater than 0"
    );

    // --- Simulate Time and Claim Rewards ---
    const initialBalance = (
      await connection.getTokenAccountBalance(playerTokenAccount.address)
    ).value.uiAmount;
    assert.strictEqual(initialBalance, 0, "Player should start with 0 tokens");

    // Simulate ~200 slots passing
    for (let i = 0; i < 20; i++) {
      await airdrop(provider, anchor.web3.Keypair.generate().publicKey, 0.0001);
    }

    await program.methods
      .claimRewards()
      .accounts({
        playerWallet: playerWallet.publicKey,
        playerTokenAccount: playerTokenAccount.address,
        tokenMint: mint,
      })
      .signers([playerWallet])
      .rpc();

    const finalBalance = (
      await connection.getTokenAccountBalance(playerTokenAccount.address)
    ).value.uiAmount;
    assert.ok(
      finalBalance > 0,
      "Player token balance should increase after claiming rewards"
    );
  });
});
