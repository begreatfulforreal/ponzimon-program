import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  SYSVAR_CLOCK_PUBKEY,
  SystemProgram,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createAssociatedTokenAccount,
  getAccount,
} from "@solana/spl-token";

import { Weedminer } from "../target/types/weedminer";
import {
  setupTestProgram,
  setupTestPlayer,
  pendingRewardsOnChain,
  expectedDistributionSummary,
  calculateInitialRewardRate,
  setupFullTestPlayerAndProgram,
  TOASTER,
  MINER_CONFIGS,
  sleepSlots,
} from "./test-helpers";

describe("Weedminer - claim instruction", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Weedminer as Program<Weedminer>;

  /* configurable test params */
  const HALVING_INTERVAL = 5;                        // 5 slots
  const TOTAL_SUPPLY = new BN(21_000_000_000000);    // 21 M @ 6dp - *10^6
  const INITIAL_REWARD_RATE = new BN(50_000000);   // 50 @ 6dp
  const DEFAULT_COOLDOWN_SLOTS = 2;

  /* derived expectations */
  console.log(
    "Global state distribution summary →",
    expectedDistributionSummary({
      halvingInterval: BigInt(HALVING_INTERVAL),
      totalSupply: BigInt(TOTAL_SUPPLY.toString()),
      initialRewardRate: BigInt(INITIAL_REWARD_RATE.toString()),
    }),
  );

  /* handles reused by inner tests (reset in beforeEach) */
  let tokenMint: PublicKey,
    globalStateKey: PublicKey,
    governanceTokenAccount: PublicKey,
    playerWallet: anchor.web3.Keypair,
    playerTokenAccount: PublicKey,
    playerKey: PublicKey;

  describe("Params checks", () => {
    it("it correctly calculates target emissions", async () => {
      const HALVING_INTERVAL = 3_024_000;                  // 2 weeks at 400ms blocks
      const TOTAL_SUPPLY = new BN(21_000_000_000000);    // 21 M @ 6dp - *10^6

      const { initialRewardRate, distributionSummary } = calculateInitialRewardRate({
        halvingInterval: HALVING_INTERVAL,
        totalSupply: TOTAL_SUPPLY,
        numberOfHalvings: 22,
      });

      console.log(
        "Expected distribution summary →",
        { initialRewardRate: BigInt(initialRewardRate.toString()), distributionSummary }
      );

      // Verify initial reward rate
      expect(initialRewardRate.toString()).toBe("3472224");

      // Verify distribution summary properties
      expect(distributionSummary.totalPredictedHalvings).toBe(22);
      expect(distributionSummary.halvingRewardsArr[0]).toBe(10500005376000n);
      expect(distributionSummary.halvingRewardsArr[1]).toBe(5250002688000n);
      expect(distributionSummary.halvingRewardsArr[2]).toBe(2625001344000n);

      // Verify reward rates array (first few values)
      expect(distributionSummary.rewardRatesArr[0]).toBe(3472224n);
      expect(distributionSummary.rewardRatesArr[1]).toBe(1736112n);

      // Verify time calculations
      expect(distributionSummary.timePerHalvingDays).toBe(14);
      expect(distributionSummary.totalBlockTimeDays).toBe(308);

      // Verify supply calculations
      expect(distributionSummary.totalSupplyE).toBe(20999974464000n);
    });


  });

  describe("checks for halvings", () => {
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
    });

    it("steps halvings correctly & clamps at supply-cap", async () => {
      const HALVINGS: { slot: number; reward: string; h: string }[] = [];

      const capture = async () => {
        const gs = await program.account.globalState.fetch(globalStateKey);
        HALVINGS.push({
          slot: await provider.connection.getSlot(),
          reward: gs.currentRewardRate.toString(),
          h: gs.lastProcessedHalvings.toString(),
        });
      };
      await sleepSlots(1);
      await capture(); // halving 0

      while (HALVINGS.length < 6) {
        await sleepSlots(1, 600);
        await program.methods.claimRewards().accountsStrict({
          playerWallet: playerWallet.publicKey,
          player: playerKey,
          globalState: globalStateKey,
          playerTokenAccount,
          referrerTokenAccount: null,
          tokenMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          clock: SYSVAR_CLOCK_PUBKEY,
        }).signers([playerWallet]).rpc();

        const gs = await program.account.globalState.fetch(globalStateKey);
        if (HALVINGS.at(-1)!.h !== gs.lastProcessedHalvings.toString()) {
          await capture();
        }
      }

      HALVINGS.forEach((v, i) => {
        if (i === 0) return;
        // console.log({
        //   reward: BigInt(v.reward).toString(),
        //   prev: BigInt(HALVINGS[i - 1].reward).toString(),
        //   halving: i,
        //   processedHalvings: v.h,
        //   prevProcessedHalvings: HALVINGS[i - 1].h
        // });
        expect(BigInt(v.reward)).toBe(BigInt(HALVINGS[i - 1].reward) / 2n);
        expect(Number(v.h)).toBe(Number(HALVINGS[i - 1].h) + 1);
      });
    });

    it("claims reward matches on-chain formula", async () => {
      await sleepSlots(2);

      const slotNow = BigInt(await provider.connection.getSlot());
      const gsBefore = await program.account.globalState.fetch(globalStateKey);
      const plBefore = await program.account.player.fetch(playerKey);
      const beforeBal = (await getAccount(provider.connection, playerTokenAccount)).amount;

      await program.methods.claimRewards().accountsStrict({
        playerWallet: playerWallet.publicKey,
        player: playerKey,
        globalState: globalStateKey,
        playerTokenAccount,
        referrerTokenAccount: null,
        governancePda: governanceTokenAccount,
        tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: SYSVAR_CLOCK_PUBKEY,
      }).signers([playerWallet]).rpc();

      const { playerAmount: expected } = pendingRewardsOnChain(slotNow,
        {
          hashpower: BigInt(plBefore.hashpower.toString()),
          lastAccBitsPerHash: BigInt(plBefore.lastAccBitsPerHash.toString())
        },
        {
          accBitsPerHash: BigInt(gsBefore.accBitsPerHash.toString()),
          lastRewardSlot: BigInt(gsBefore.lastRewardSlot.toString()),
          startSlot: BigInt(gsBefore.startSlot.toString()),
          halvingInterval: BigInt(gsBefore.halvingInterval.toString()),
          initialRewardRate: BigInt(gsBefore.initialRewardRate.toString()),
          currentRewardRate: BigInt(gsBefore.currentRewardRate.toString()),
          totalHashpower: BigInt(gsBefore.totalHashpower.toString()),
          totalSupply: BigInt(gsBefore.totalSupply.toString()),
          burnedTokens: BigInt(gsBefore.burnedTokens.toString()),
          cumulativeRewards: BigInt(gsBefore.cumulativeRewards.toString()),
          referralFee: BigInt(gsBefore.referralFee.toString())
        });

      const rewards = BigInt((await getAccount(provider.connection, playerTokenAccount)).amount)
        - BigInt(beforeBal);

      // Calc tolerance based on proximity to halving
      const nextHalvingSlot = BigInt(gsBefore.startSlot.toString()) +
        (BigInt(gsBefore.lastProcessedHalvings.toString()) + 1n) * BigInt(HALVING_INTERVAL);
      const slotsTillHalving = nextHalvingSlot - slotNow;
      const tolerance = slotsTillHalving < 3n ? expected / 2n : expected / 20n;

      expect(rewards >= expected - tolerance && rewards <= expected + tolerance).toBeTruthy();
    });
  });

  describe("checks for claims", () => {
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

    it("claims rewards and updates player/global state", async () => {
      await sleepSlots(2);
      const beforeBal = (await getAccount(provider.connection, playerTokenAccount)).amount;
      const gs0 = await program.account.globalState.fetch(globalStateKey);

      await program.methods.claimRewards().accountsStrict({
        playerWallet: playerWallet.publicKey,
        player: playerKey,
        globalState: globalStateKey,
        playerTokenAccount,
        referrerTokenAccount: null,
        governancePda: governanceTokenAccount,
        tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: SYSVAR_CLOCK_PUBKEY,
      }).signers([playerWallet]).rpc();

      const afterBal = (await getAccount(provider.connection, playerTokenAccount)).amount;
      const pl1 = await program.account.player.fetch(playerKey);
      const gs1 = await program.account.globalState.fetch(globalStateKey);
      expect(BigInt(afterBal)).toBeGreaterThan(BigInt(beforeBal));
      expect(Number(pl1.lastAccBitsPerHash)).toBeGreaterThan(0);
      expect(gs1.cumulativeRewards.toNumber()).toBeGreaterThan(gs0.cumulativeRewards.toNumber());
      expect(pl1.lastAccBitsPerHash.toString()).toBe(gs1.accBitsPerHash.toString());
    });

    it("blocks immediate double claim by same player", async () => {
      await sleepSlots(2);
      await program.methods.claimRewards().accountsStrict({
        playerWallet: playerWallet.publicKey,
        player: playerKey,
        globalState: globalStateKey,
        playerTokenAccount,
        referrerTokenAccount: null,
        governancePda: governanceTokenAccount,
        tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: SYSVAR_CLOCK_PUBKEY,
      }).signers([playerWallet]).rpc();

      await expect(
        program.methods.claimRewards().accountsStrict({
          playerWallet: playerWallet.publicKey,
          player: playerKey,
          globalState: globalStateKey,
          playerTokenAccount,
          referrerTokenAccount: null,
          governancePda: governanceTokenAccount,
          tokenMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          clock: SYSVAR_CLOCK_PUBKEY,
        }).signers([playerWallet]).rpc(),
      ).rejects.toThrow(/CooldownNotExpired/);

      await sleepSlots(2);
      await program.methods.claimRewards().accountsStrict({
        playerWallet: playerWallet.publicKey,
        player: playerKey,
        globalState: globalStateKey,
        playerTokenAccount,
        referrerTokenAccount: null,
        governancePda: governanceTokenAccount,
        tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: SYSVAR_CLOCK_PUBKEY,
      }).signers([playerWallet]).rpc();
    });

    it("mints referral fee when player has a referrer", async () => {
      const ref = anchor.web3.Keypair.generate();
      await provider.connection.requestAirdrop(ref.publicKey, 1e9);
      const refAta = await createAssociatedTokenAccount(
        provider.connection,
        playerWallet,
        tokenMint,
        ref.publicKey,
      );

      const { playerWallet: p2W, playerTokenAccount: p2ATA, playerKey: p2Key } =
        await setupTestPlayer(provider, program, tokenMint, globalStateKey, ref.publicKey);

      await sleepSlots(2);
      const before = (await getAccount(provider.connection, refAta)).amount;

      await program.methods.claimRewards().accountsStrict({
        playerWallet: p2W.publicKey,
        player: p2Key,
        globalState: globalStateKey,
        playerTokenAccount: p2ATA,
        referrerTokenAccount: refAta,
        governancePda: governanceTokenAccount,
        tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: SYSVAR_CLOCK_PUBKEY,
      }).signers([p2W]).rpc();

      const after = (await getAccount(provider.connection, refAta)).amount;
      expect(BigInt(after)).toBeGreaterThan(BigInt(before));
    });
  });

  describe("emission edge-cases (tiny param runs)", () => {
    /* reuse inner handles but re-init fresh per test */
    beforeEach(async () => {
      /* each test sets its own params */
    });

    it("audits and checks the first 5 claims", async () => {
      const CAP = new BN(400_000000);
      const RATE = new BN(100_000000);
      const INT = 2;

      ({ tokenMint, globalStateKey, governanceTokenAccount, playerWallet, playerKey, playerTokenAccount } =
        await setupFullTestPlayerAndProgram(provider, program, {
          totalSupply: CAP,
          halvingInterval: INT,
          initialRewardRate: RATE,
          initialMintAmount: 0,
        }));

      await sleepSlots(1, 500);

      let totalMinted = 0n;
      let claimRewardsSum = 0n;

      const ACC_SCALE = BigInt("1000000000000"); // match the Rust constant

      for (let i = 0; i < 5; i++) {
        // Fetch global state before claim
        const gsBefore = await program.account.globalState.fetch(globalStateKey);
        const plBefore = await program.account.player.fetch(playerKey);

        // Claim rewards
        await program.methods.claimRewards().accountsStrict({
          playerWallet: playerWallet.publicKey,
          player: playerKey,
          globalState: globalStateKey,
          playerTokenAccount,
          referrerTokenAccount: null,
          governancePda: governanceTokenAccount,
          tokenMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          clock: SYSVAR_CLOCK_PUBKEY,
        }).signers([playerWallet]).rpc();

        const gsAfter = await program.account.globalState.fetch(globalStateKey);
        const plAfter = await program.account.player.fetch(playerKey);

        const accDelta = BigInt(gsAfter.accBitsPerHash.toString()) - BigInt(plBefore.lastAccBitsPerHash.toString());
        const hashpower = BigInt(plBefore.hashpower.toString());
        const pending = (hashpower * accDelta) / ACC_SCALE;

        const fee = BigInt(gsBefore.referralFee?.toString() ?? "0");
        const expectedNetReward = pending - (pending * fee / 100n);

        const minted = BigInt((await getAccount(provider.connection, playerTokenAccount)).amount);
        const rewardThisClaim = minted - totalMinted;
        totalMinted = minted;
        claimRewardsSum += rewardThisClaim;

        // Log all relevant values as an object
        console.log({
          claimNumber: i + 1,
          rewardRateBefore: BigInt(gsBefore.currentRewardRate.toString()),
          rewardRateAfter: BigInt(gsAfter.currentRewardRate.toString()),
          halvingBefore: BigInt(gsBefore.lastProcessedHalvings.toString()),
          halvingAfter: BigInt(gsAfter.lastProcessedHalvings.toString()),
          fee,
          expectedNetReward,
          actualRewardThisClaim: rewardThisClaim,
          playerTokenAccountBalance: minted,
          globalState_cumulativeRewards: gsAfter.cumulativeRewards.toString(),
          globalState_totalHashpower: gsAfter.totalHashpower.toString(),
          globalState_totalSupply: gsAfter.totalSupply.toString(),
        });

        // --- EXPECTS ---

        // 1. Actual reward is close to expected net reward (allow 2.5% tolerance)
        const mintedMinusBurn = BigInt(gsAfter.cumulativeRewards.toString()) - BigInt(gsAfter.burnedTokens.toString());
        const remainingSupply = BigInt(gsAfter.totalSupply.toString()) - mintedMinusBurn;
        const clampedExpectedNetReward = expectedNetReward > remainingSupply ? remainingSupply : expectedNetReward;
        const tolerance = clampedExpectedNetReward / 40n + 1n;
        expect(
          rewardThisClaim >= clampedExpectedNetReward - tolerance &&
          rewardThisClaim <= clampedExpectedNetReward + tolerance
        ).toBeTruthy();

        // 2. Reward rate should halve for each halving that occurred
        const numHalvings = BigInt(gsAfter.lastProcessedHalvings.toString()) - BigInt(gsBefore.lastProcessedHalvings.toString());
        const expectedRewardRateAfter = numHalvings > 0n
          ? BigInt(gsBefore.currentRewardRate.toString()) / (2n ** numHalvings)
          : BigInt(gsBefore.currentRewardRate.toString());
        expect(BigInt(gsAfter.currentRewardRate.toString())).toBe(expectedRewardRateAfter);

        // 3. Cumulative rewards should be monotonically increasing and not exceed cap
        expect(gsAfter.cumulativeRewards.gte(new BN(0))).toBeTruthy();
        expect(gsAfter.cumulativeRewards.lte(CAP)).toBeTruthy();

        // 4. Player's token account balance should match sum of all rewards so far
        expect(minted).toBe(claimRewardsSum);

        await sleepSlots(1, 500);
      }
    });

    it("halves reward rate every interval", async () => {
      const RATE = new BN(64_000_000);
      const CAP = new BN(1_000_000_000);
      const INT = 2;

      ({ tokenMint, globalStateKey, governanceTokenAccount } =
        await setupTestProgram(provider, program, INT, CAP, RATE, 1));
      ({ playerWallet, playerKey, playerTokenAccount } =
        await setupTestPlayer(provider, program, tokenMint, globalStateKey));

      const seen: string[] = [];
      for (let i = 0; i < 4; i++) {
        await sleepSlots(INT, 400);

        // Use update_pool_manual instead of claimRewards
        await program.methods.updatePoolManual().accountsStrict({
          authority: provider.wallet.publicKey,
          globalState: globalStateKey,
          clock: SYSVAR_CLOCK_PUBKEY,
        }).rpc();

        const gs = await program.account.globalState.fetch(globalStateKey);
        seen.push(gs.currentRewardRate.toString());
      }
      seen.forEach((v, i) => {
        if (i === 0) return;
        // console.log({
        //   expected: BigInt(seen[i - 1]) / 2n,
        //   actual: BigInt(v),
        //   diff: BigInt(v) > BigInt(seen[i - 1]) ? BigInt(v) - BigInt(seen[i - 1]) : BigInt(seen[i - 1]) - BigInt(v),
        //   tolerance: BigInt(seen[i - 1]) / 2n / 100n,
        // });

        const expected = BigInt(seen[i - 1]) / 2n;
        const actual = BigInt(v);
        const diff = actual > expected ? actual - expected : expected - actual;
        const tolerance = expected / 100n;
        expect(diff <= tolerance).toBeTruthy();
      });
    });

    it("emits nothing with zero total hashpower", async () => {
      const RATE = new BN(1_000_000);
      const CAP = new BN(5_000_000);
      const INT = 3;
      ({ tokenMint, globalStateKey } =
        await setupTestProgram(provider, program, INT, CAP, RATE, 1)); // no player
      await sleepSlots(4);
      const b = await program.account.globalState.fetch(globalStateKey);
      await sleepSlots(2);
      const a = await program.account.globalState.fetch(globalStateKey);
      expect(a.accBitsPerHash.eq(b.accBitsPerHash)).toBeTruthy();
      expect(a.cumulativeRewards.eq(b.cumulativeRewards)).toBeTruthy();
    });

    it("burns decrease remaining supply correctly", async () => {
      const TOTAL_SUPPLY = new BN(1_000_000_000000); // 1M @ 6dp
      ({ playerWallet, playerKey, playerTokenAccount, globalStateKey, governanceTokenAccount, tokenMint } =
        await setupFullTestPlayerAndProgram(provider, program, { totalSupply: TOTAL_SUPPLY }));

      await sleepSlots(2);
      await program.methods.claimRewards().accountsStrict({
        playerWallet: playerWallet.publicKey,
        player: playerKey,
        globalState: globalStateKey,
        playerTokenAccount,
        governancePda: governanceTokenAccount,
        tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: SYSVAR_CLOCK_PUBKEY,
        referrerTokenAccount: null,
      }).signers([playerWallet]).rpc();

      await sleepSlots(2);

      const miner = MINER_CONFIGS[TOASTER]
      await program.methods.buyMiner(TOASTER, 0).accountsStrict({
        playerWallet: playerWallet.publicKey,
        player: playerKey,
        globalState: globalStateKey,
        playerTokenAccount,
        governanceTokenAccount: governanceTokenAccount,
        tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: SYSVAR_CLOCK_PUBKEY,
        systemProgram: SystemProgram.programId,
      }).signers([playerWallet]).rpc();

      const gs = await program.account.globalState.fetch(globalStateKey);
      const net = gs.cumulativeRewards.sub(gs.burnedTokens);
      expect(net.lte(gs.totalSupply)).toBeTruthy();
      expect(gs.burnedTokens.eq(new BN(miner[2] * 0.75))).toBeTruthy();
    });

    it("handles halving boundary transitions smoothly", async () => {
      const RATE = new BN(8_000_000);
      const CAP = new BN(50_000_000);
      const INT = 5;
      ({ tokenMint, globalStateKey, governanceTokenAccount } =
        await setupTestProgram(provider, program, INT, CAP, RATE, 1));
      ({ playerWallet, playerKey, playerTokenAccount } =
        await setupTestPlayer(provider, program, tokenMint, globalStateKey));

      await sleepSlots(INT, 400);
      await program.methods.updatePoolManual().accountsStrict({
        authority: provider.wallet.publicKey,
        globalState: globalStateKey,
        clock: SYSVAR_CLOCK_PUBKEY,
      }).rpc();

      const pre = await program.account.globalState.fetch(globalStateKey);
      const expectedRate = RATE.divn(2);
      const rateDiff = pre.currentRewardRate.sub(expectedRate).abs().toNumber();
      // console.log({
      //   expectedRate: expectedRate.toString(),
      //   currentRate: pre.currentRewardRate.toString(),
      //   rateDiff: rateDiff,
      // });

      expect(rateDiff <= expectedRate.toNumber() * 0.05).toBeTruthy();
    });
  });
});
