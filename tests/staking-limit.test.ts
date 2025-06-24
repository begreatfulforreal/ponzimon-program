import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Ponzimon } from "../target/types/ponzimon";
import * as assert from "assert";
import {
  airdrop,
  setupTestProgram,
  createTestTokenAccount,
  advanceSlots,
} from "./test-helpers";
import { BN } from "@coral-xyz/anchor";

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

    // --- Stake Starter Cards ---
    await program.methods
      .stakeCard(0)
      .accounts({
        playerWallet: playerWallet.publicKey,
        tokenMint: mint,
      })
      .signers([playerWallet])
      .rpc();

    playerAccount = await program.account.player.fetch(playerPda);
    // The player's `lastClaimSlot` is not updated on stake, so we need to get the
    // `lastRewardSlot` from the global state to correctly calculate elapsed time for rewards.
    const globalStateAfterStake = await program.account.globalState.fetch(
      globalState
    );
    const startedSlot = globalStateAfterStake.lastRewardSlot;

    const stakedCardCount = (
      playerAccount.stakedCardsBitset.toString(2).match(/1/g) || []
    ).length;
    assert.strictEqual(
      stakedCardCount,
      1,
      "All 1 starter cards should be staked"
    );
    assert.ok(
      playerAccount.totalHashpower.gtn(0),
      "Player hashpower should be greater than 0"
    );

    // --- Simulate Time and Claim Rewards ---
    const initialBalance = new BN(
      (
        await connection.getTokenAccountBalance(playerTokenAccount.address)
      ).value.amount
    );
    assert.ok(initialBalance.eqn(0), "Player should start with 0 tokens");

    // Simulate slots passing
    const slotsToAdvance = 10;
    await advanceSlots(provider, slotsToAdvance);

    await program.methods
      .claimRewards()
      .accounts({
        playerWallet: playerWallet.publicKey,
        playerTokenAccount: playerTokenAccount.address,
        tokenMint: mint,
      })
      .signers([playerWallet])
      .rpc();

    const finalBalance = new BN(
      (
        await connection.getTokenAccountBalance(playerTokenAccount.address)
      ).value.amount
    );

    // --- Verification ---
    const globalStateAccount = await program.account.globalState.fetch(
      globalState
    );
    playerAccount = await program.account.player.fetch(playerPda);

    const slotsAdvanced = playerAccount.lastClaimSlot.sub(startedSlot);
    // Expected rewards should be based on the player's hashpower contribution
    const expectedRewards = new BN(slotsAdvanced)
      .mul(globalStateAccount.initialRewardRate)
      .mul(playerAccount.totalHashpower)
      .div(globalStateAccount.totalHashpower);

    // Allow for a small tolerance in case of rounding differences
    const tolerance = new BN(1);
    const difference = finalBalance.sub(expectedRewards).abs();

    assert.ok(
      difference.lte(tolerance),
      `Final balance should be close to expected rewards. Got: ${finalBalance}, Expected: ${expectedRewards}`
    );

    // --- In-Test Emission Log ---
    const totalTestTimeSeconds = slotsAdvanced.toNumber() * 0.4;
    const emittedTokens = finalBalance.toNumber() / Math.pow(10, 6);
    console.log("\n--- In-Test Emissions ---");
    console.log(
      `Emitted ${emittedTokens.toFixed(
        6
      )} tokens over ${totalTestTimeSeconds.toFixed(
        1
      )} seconds (${slotsAdvanced.toNumber()} slots).`
    );
    console.log("-------------------------\n");

    // --- Real-world Issuance Calculation ---
    const slotsPerHour = 3600 / 0.4; // 9000
    const mintDecimals = 6;

    // Calculate issuance per hour for a single user
    const tokensPerHourRaw = new BN(slotsPerHour).mul(
      globalStateAccount.initialRewardRate
    );
    const tokensPerHour =
      tokensPerHourRaw.toNumber() / Math.pow(10, mintDecimals);

    // Calculate issuance in 6 hours
    const tokensIn6Hours = tokensPerHour * 6;

    console.log("--- Token Issuance Simulation (1 User) ---");
    console.log(`Tokens issued per hour: ${tokensPerHour.toFixed(4)}`);
    console.log(`Tokens issued in 6 hours: ${tokensIn6Hours.toFixed(4)}`);
    console.log("-----------------------------------------");
  });
});
