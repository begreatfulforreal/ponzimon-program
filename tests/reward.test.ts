// ... existing code ...
import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  getAccount,
} from "@solana/spl-token";

import { Bitsol } from "../target/types/bitsol";
import {
  setupTestProgram,
  setupTestPlayer,
  sleepSlots,
} from "./test-helpers";

describe("Bitsol - global random reward", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Bitsol as Program<Bitsol>;

  const HALVING_INTERVAL = 5;
  const TOTAL_SUPPLY = new BN(21_000_000_000000);
  const INITIAL_REWARD_RATE = new BN(50_000000);
  const DEFAULT_COOLDOWN_SLOTS = 2;

  let tokenMint: PublicKey,
    globalStateKey: PublicKey,
    governanceTokenAccount: PublicKey,
    playerWallet: anchor.web3.Keypair,
    playerTokenAccount: PublicKey,
    playerKey: PublicKey;

  beforeEach(async () => {
    ({ tokenMint, globalStateKey, governanceTokenAccount } =
      await setupTestProgram(
        provider,
        program,
        HALVING_INTERVAL,
        TOTAL_SUPPLY,
        INITIAL_REWARD_RATE,
        DEFAULT_COOLDOWN_SLOTS,
      ));
    ({ playerWallet, playerKey, playerTokenAccount } =
      await setupTestPlayer(provider, program, tokenMint, globalStateKey));
    await sleepSlots(1);
  });

  it("admin can generate a global random reward", async () => {
    const amount = new BN(123_000000);
    const expirySlots = new BN(5);

    await program.methods.generateGlobalRandomReward(amount, expirySlots)
      .accountsStrict({
        authority: provider.wallet.publicKey,
        globalState: globalStateKey,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .rpc();

    const gs = await program.account.globalState.fetch(globalStateKey);
    expect(gs.globalRandomReward).toBeTruthy();
    expect(gs.globalRandomReward.amount.toString()).toBe(amount.toString());
    expect(gs.globalRandomReward.rewardId.toString()).toBe("1");
    expect(gs.globalRewardCounter.toString()).toBe("1");
  });

  it("player can claim the global random reward", async () => {
    const amount = new BN(456_000000);
    const expirySlots = new BN(5);

    // Admin generates reward
    await program.methods.generateGlobalRandomReward(amount, expirySlots)
      .accountsStrict({
        authority: provider.wallet.publicKey,
        globalState: globalStateKey,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .rpc();

    const beforeBal = (await getAccount(provider.connection, playerTokenAccount)).amount;

    // Player claims reward
    await program.methods.claimGlobalRandomReward()
      .accountsStrict({
        playerWallet: playerWallet.publicKey,
        player: playerKey,
        globalState: globalStateKey,
        playerTokenAccount,
        tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .signers([playerWallet])
      .rpc();

    const afterBal = (await getAccount(provider.connection, playerTokenAccount)).amount;
    expect(BigInt(afterBal)).toBe(BigInt(beforeBal) + BigInt(amount.toString()));

    // Player cannot claim again
    await expect(
      program.methods.claimGlobalRandomReward()
        .accountsStrict({
          playerWallet: playerWallet.publicKey,
          player: playerKey,
          globalState: globalStateKey,
          playerTokenAccount,
          tokenMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          clock: SYSVAR_CLOCK_PUBKEY,
        })
        .signers([playerWallet])
        .rpc()
    ).rejects.toThrow(/RewardAlreadyClaimed/);
  });

  it("claim fails after expiry", async () => {
    const amount = new BN(789_000000);
    const expirySlots = new BN(2);

    // Admin generates reward
    await program.methods.generateGlobalRandomReward(amount, expirySlots)
      .accountsStrict({
        authority: provider.wallet.publicKey,
        globalState: globalStateKey,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .rpc();

    // Wait until after expiry
    await sleepSlots(expirySlots.toNumber() + 1);

    // Player tries to claim
    await expect(
      program.methods.claimGlobalRandomReward()
        .accountsStrict({
          playerWallet: playerWallet.publicKey,
          player: playerKey,
          globalState: globalStateKey,
          playerTokenAccount,
          tokenMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          clock: SYSVAR_CLOCK_PUBKEY,
        })
        .signers([playerWallet])
        .rpc()
    ).rejects.toThrow(/RewardExpired/);
  });

  it("counter system tracks multiple rewards correctly", async () => {
    const amount1 = new BN(100_000000);
    const amount2 = new BN(200_000000);
    const amount3 = new BN(300_000000);
    const expirySlots = new BN(10);

    // Generate first reward
    await program.methods.generateGlobalRandomReward(amount1, expirySlots)
      .accountsStrict({
        authority: provider.wallet.publicKey,
        globalState: globalStateKey,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .rpc();

    let gs = await program.account.globalState.fetch(globalStateKey);
    expect(gs.globalRewardCounter.toString()).toBe("1");
    expect(gs.globalRandomReward.rewardId.toString()).toBe("1");

    // Player claims first reward
    await program.methods.claimGlobalRandomReward()
      .accountsStrict({
        playerWallet: playerWallet.publicKey,
        player: playerKey,
        globalState: globalStateKey,
        playerTokenAccount,
        tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .signers([playerWallet])
      .rpc();

    let player = await program.account.player.fetch(playerKey);
    expect(player.lastClaimedGlobalRewardId.toString()).toBe("1");

    // Generate second reward
    await program.methods.generateGlobalRandomReward(amount2, expirySlots)
      .accountsStrict({
        authority: provider.wallet.publicKey,
        globalState: globalStateKey,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .rpc();

    gs = await program.account.globalState.fetch(globalStateKey);
    expect(gs.globalRewardCounter.toString()).toBe("2");
    expect(gs.globalRandomReward.rewardId.toString()).toBe("2");

    // Player claims second reward
    await program.methods.claimGlobalRandomReward()
      .accountsStrict({
        playerWallet: playerWallet.publicKey,
        player: playerKey,
        globalState: globalStateKey,
        playerTokenAccount,
        tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .signers([playerWallet])
      .rpc();

    player = await program.account.player.fetch(playerKey);
    expect(player.lastClaimedGlobalRewardId.toString()).toBe("2");

    // Generate third reward
    await program.methods.generateGlobalRandomReward(amount3, expirySlots)
      .accountsStrict({
        authority: provider.wallet.publicKey,
        globalState: globalStateKey,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .rpc();

    gs = await program.account.globalState.fetch(globalStateKey);
    expect(gs.globalRewardCounter.toString()).toBe("3");
    expect(gs.globalRandomReward.rewardId.toString()).toBe("3");

    // Player can still claim the third reward
    await program.methods.claimGlobalRandomReward()
      .accountsStrict({
        playerWallet: playerWallet.publicKey,
        player: playerKey,
        globalState: globalStateKey,
        playerTokenAccount,
        tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: SYSVAR_CLOCK_PUBKEY,
      })
      .signers([playerWallet])
      .rpc();

    player = await program.account.player.fetch(playerKey);
    expect(player.lastClaimedGlobalRewardId.toString()).toBe("3");

    // Player cannot claim again
    await expect(
      program.methods.claimGlobalRandomReward()
        .accountsStrict({
          playerWallet: playerWallet.publicKey,
          player: playerKey,
          globalState: globalStateKey,
          playerTokenAccount,
          tokenMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          clock: SYSVAR_CLOCK_PUBKEY,
        })
        .signers([playerWallet])
        .rpc()
    ).rejects.toThrow(/RewardAlreadyClaimed/);
  });
});