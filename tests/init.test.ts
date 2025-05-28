import * as anchor from "@coral-xyz/anchor";
import { PublicKey, SystemProgram, SYSVAR_CLOCK_PUBKEY, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { Program, BN } from "@coral-xyz/anchor";
import { createMint, getOrCreateAssociatedTokenAccount, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { Weedminer } from "../target/types/weedminer";
import { GLOBAL_STATE_SEED, GOVERNANCE_TOKEN_SEED, PLAYER_SEED } from "./test-helpers";

describe("Weedminer Initialization", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Weedminer as Program<Weedminer>;

  // Setup variables
  let tokenMint: PublicKey;
  let globalStateKey: PublicKey;
  let globalStateBump: number;
  let playerKey: PublicKey;
  let playerBump: number;
  let playerTokenAccount: PublicKey;
  let governanceTokenAccount: PublicKey;
  let currentSlot: BN;

  // Test parameters
  const HALVING_INTERVAL = new BN(100000); // 100k slots
  const TOTAL_SUPPLY = new BN(21_000_000_000000); // 21 M @ 6dp - *10^6
  const INITIAL_REWARD_RATE = new BN(50_000000); // 50 $BITSOL at 6 dp
  const COOLDOWN_SLOTS = new BN(10); // 10 slots

  beforeAll(async () => {
    // Get current slot
    currentSlot = new BN(await provider.connection.getSlot());

    // Create token mint
    const wallet = provider.wallet as anchor.Wallet;
    tokenMint = await createMint(
      provider.connection,
      wallet.payer,
      wallet.publicKey,
      null,
      6 // decimals
    );

    // Create player token account
    const tokenAccount = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      wallet.payer,
      tokenMint,
      wallet.publicKey
    );
    playerTokenAccount = tokenAccount.address;

    // Derive PDAs
    [globalStateKey, globalStateBump] = PublicKey.findProgramAddressSync(
      [Buffer.from(GLOBAL_STATE_SEED), tokenMint.toBuffer()],
      program.programId
    );

    [playerKey, playerBump] = PublicKey.findProgramAddressSync(
      [Buffer.from(PLAYER_SEED), wallet.publicKey.toBuffer()],
      program.programId
    );

    [governanceTokenAccount] = PublicKey.findProgramAddressSync(
      [Buffer.from(GOVERNANCE_TOKEN_SEED), globalStateKey.toBuffer(), tokenMint.toBuffer()],
      program.programId
    );
  });

  test("should initialize program with correct parameters", async () => {
    try {
      const tx = await program.methods
        .initializeProgram(currentSlot, HALVING_INTERVAL, TOTAL_SUPPLY, INITIAL_REWARD_RATE, COOLDOWN_SLOTS)
        .accountsStrict({
          globalState: globalStateKey,
          authority: provider.wallet.publicKey,
          tokenMint: tokenMint,
          governanceTokenAccount: governanceTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      await provider.connection.confirmTransaction(tx);
    } catch (error) {
      console.error(error);
      throw error;
    }

    const globalState = await program.account.globalState.fetch(globalStateKey);

    // Assertions
    expect(globalState.authority.toString()).toBe(provider.wallet.publicKey.toString());
    expect(globalState.tokenMint.toString()).toBe(tokenMint.toString());
    expect(globalState.startSlot.toString()).toBe(currentSlot.toString());
    expect(globalState.halvingInterval.toString()).toBe(HALVING_INTERVAL.toString());
    expect(globalState.initialRewardRate.toString()).toBe(INITIAL_REWARD_RATE.toString());
    expect(globalState.totalSupply.toString()).toBe(TOTAL_SUPPLY.toString());
    expect(globalState.burnedTokens.toString()).toBe("0");
    expect(globalState.referralFee).toBe(25);
    expect(globalState.productionEnabled).toBe(true);
    expect(globalState.cooldownSlots.toString()).toBe(COOLDOWN_SLOTS.toString());
    expect(globalState.totalHashpower.toString()).toBe("0");
    expect(globalState.cumulativeRewards.toString()).toBe("0");
  });

  test("should prevent re-initialization", async () => {
    await expect(
      program.methods
        .initializeProgram(currentSlot, HALVING_INTERVAL, TOTAL_SUPPLY, INITIAL_REWARD_RATE, COOLDOWN_SLOTS)
        .accountsStrict({
          globalState: globalStateKey,
          authority: provider.wallet.publicKey,
          tokenMint: tokenMint,
          governanceTokenAccount: governanceTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .rpc()
    ).rejects.toThrow();
  });
});
