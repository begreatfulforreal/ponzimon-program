import {
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  SYSVAR_CLOCK_PUBKEY,
} from "@solana/web3.js";
import { AnchorProvider, Wallet, web3, BN, Program } from "@coral-xyz/anchor";
import {
  getAssociatedTokenAddressSync,
  createMint,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountInstruction,
  setAuthority,
  AuthorityType,
  createAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";
import { Bitsol } from "../target/types/bitsol";

/* ── fixed-point constants — mirror on-chain ───────────────────*/
const ACC_SCALE = 1_000_000_000_000n;     // 1 e12
const REFERRAL_FEE_PPM = 25n;                    // 25/1000  = 2.5 %

// Seeds
export const GLOBAL_STATE_SEED = "global_state";
export const PLAYER_SEED = "player";
export const FACILITY_TYPE_SEED = "facility_type";
export const GOVERNANCE_TOKEN_SEED = "governance_token";

// Facility Types
export const CRAMPED_BEDROOM = 0;
export const LOW_PROFILE_STORAGE = 1;
export const HIDDEN_POWERHOUSE = 2;
export const CUSTOM_GARAGE = 3;
export const HIGH_RISE_APARTMENT = 4;

// Miner Types
export const TOASTER = 0;
export const RASPBERRY_PI = 1;
export const NOTEBOOK = 2;
export const GAMER_RIG = 3;
export const GPU_RACK = 4;
export const ASIC_SOLO = 5;
export const ASIC_RACK = 6;
export const HYDRO_FARM = 7;
export const TERRA_MINER = 8;
export const QUANTUM_CLUSTER = 9;

// Facility Configurations
export const FACILITY_CONFIGS = [
  // [totalMiners, powerOutput, cost]
  [2, 15, 80_000000],      // Cramped Bedroom (80 $BITSOL)
  [4, 60, 240_000000],     // Low Profile Storage (240 $BITSOL)
  [6, 200, 720_000000],    // Hidden Powerhouse (720 $BITSOL)
  [9, 600, 1800_000000],   // Custom Garage (1800 $BITSOL)
  [12, 2000, 4800_000000], // High Rise Apartment (4800 $BITSOL)
] as const;

// Miner Configurations
export const MINER_CONFIGS = [
  // [hashrate, powerConsumption, cost]
  [1_500, 3, 40_000000],            // Toaster (40 $BITSOL)
  [6_000, 6, 120_000000],           // Raspberry Pi (120 $BITSOL)
  [25_000, 15, 350_000000],         // Notebook (350 $BITSOL)
  [60_000, 30, 700_000000],         // Gamer Rig (700 $BITSOL)
  [150_000, 60, 1_300_000000],      // GPU Rack (1300 $BITSOL)
  [400_000, 120, 2_500_000000],     // ASIC Solo (2500 $BITSOL)
  [800_000, 200, 5_000_000000],     // ASIC Rack (5000 $BITSOL)
  [1_500_000, 400, 9_000_000000],   // Hydro Farm (9000 $BITSOL)
  [3_500_000, 800, 18_000_000000],  // Terra Miner (18000 $BITSOL)
  [6_000_000, 1500, 40_000_000000], // Quantum Cluster (40000 $BITSOL)
] as const;

export async function createTokenAccount(
  provider: AnchorProvider,
  mint: PublicKey,
  owner: PublicKey
): Promise<PublicKey> {
  const tokenAccount = getAssociatedTokenAddressSync(mint, owner);
  const tx = new web3.Transaction().add(
    createAssociatedTokenAccountInstruction(
      provider.wallet.publicKey,
      tokenAccount,
      owner,
      mint
    )
  );
  await provider.sendAndConfirm(tx);
  return tokenAccount;
}

/* ── convenience: "air-drop me some SOL" ───────────────────────*/
export async function createTestAccount(
  provider: AnchorProvider,
): Promise<Keypair> {
  const kp = Keypair.generate();
  const sig = await provider.connection.requestAirdrop(
    kp.publicKey,
    web3.LAMPORTS_PER_SOL,
  );
  const { blockhash, lastValidBlockHeight } =
    await provider.connection.getLatestBlockhash();
  await provider.connection.confirmTransaction({
    signature: sig,
    blockhash,
    lastValidBlockHeight,
  });
  return kp;
}

/**
 * Create a fresh mint but keep YOUR wallet as mint authority.
 * You can now manually mint to any ATA before handing off to the PDA.
 */
export async function createTestMint(
  provider: AnchorProvider,
  decimals = 6,
): Promise<Keypair> {
  const mintKP = Keypair.generate();
  await createMint(
    provider.connection,
    provider.wallet.payer,
    provider.wallet.publicKey,
    null,
    decimals,
    mintKP,
  );
  return mintKP;
}

/**
 * Spins up your program, **handing authority** to the PDA just in time.
 */
export async function setupTestProgram(
  provider: AnchorProvider,
  program: Program<Bitsol>,
  halvingInterval: number,
  totalSupply: BN,
  initialRewardRate: BN,
  cooldownSlots = 10,
  mintKP?: Keypair,
) {
  const wallet = provider.wallet as Wallet;

  if (!mintKP) {
    mintKP = await createTestMint(provider);
  }
  let tokenMint = mintKP.publicKey;

  // derive PDAs
  const [globalStateKey] = PublicKey.findProgramAddressSync(
    [Buffer.from(GLOBAL_STATE_SEED), tokenMint.toBuffer()],
    program.programId,
  );
  const [governanceTokenAccount] = PublicKey.findProgramAddressSync(
    [
      Buffer.from(GOVERNANCE_TOKEN_SEED),
      globalStateKey.toBuffer(),
      tokenMint.toBuffer(),
    ],
    program.programId,
  );

  // hand mint authority off to your on-chain PDA
  await setAuthority(
    provider.connection,
    provider.wallet.payer,
    tokenMint,
    wallet.publicKey,
    AuthorityType.MintTokens,
    globalStateKey,
  );

  // initialize your program
  await program.methods
    .initializeProgram(
      new BN(await provider.connection.getSlot()), // start-slot
      new BN(halvingInterval),
      totalSupply,
      initialRewardRate,
      new BN(cooldownSlots),
    )
    .accountsStrict({
      authority: wallet.publicKey,
      tokenMint,
      globalState: globalStateKey,
      governanceTokenAccount,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      rent: SYSVAR_RENT_PUBKEY,
    })
    .rpc();

  return { tokenMint, globalStateKey, governanceTokenAccount };
}

/* ──────────────────────────────────────────────────────────────
   setupTestPlayer()
   ──────────────────────────────────────────────────────────── */
export async function setupTestPlayer(
  provider: AnchorProvider,
  program: Program<Bitsol>,
  tokenMint: PublicKey,
  globalStateKey: PublicKey,
  referrer: PublicKey | null = null,
) {
  const playerWallet = await createTestAccount(provider);
  const playerTokenAccount = getAssociatedTokenAddressSync(
    tokenMint,
    playerWallet.publicKey,
  );
  const [playerKey] = PublicKey.findProgramAddressSync(
    [Buffer.from(PLAYER_SEED), playerWallet.publicKey.toBuffer()],
    program.programId,
  );

  await program.methods
    .purchaseInitialFacility(referrer)
    .accountsStrict({
      playerWallet: playerWallet.publicKey,
      player: playerKey,
      globalState: globalStateKey,
      playerTokenAccount,
      tokenMint,
      tokenProgram: TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      rent: SYSVAR_RENT_PUBKEY,
      clock: SYSVAR_CLOCK_PUBKEY,
    })
    .signers([playerWallet])
    .rpc();

  return { playerWallet, playerTokenAccount, playerKey };
}

/* ──────────────────────────────────────────────────────────────
   pendingRewardsOnChain()
   (used by tests for 'oracle' expectations)
   ──────────────────────────────────────────────────────────── */
export function pendingRewardsOnChain(
  slotNow: bigint,
  player: { hashpower: bigint; lastAccBitsPerHash: bigint },
  gs: {
    accBitsPerHash: bigint;
    lastRewardSlot: bigint;
    startSlot: bigint;
    halvingInterval: bigint;
    initialRewardRate: bigint;
    currentRewardRate: bigint;
    totalHashpower: bigint;
    totalSupply: bigint;
    burnedTokens: bigint;
    cumulativeRewards: bigint;
    referralFee: bigint;
  },
): { playerAmount: bigint; referralAmount: bigint } {
  /* 1️⃣ replicate update_pool() */
  let acc = gs.accBitsPerHash;

  if (slotNow > gs.lastRewardSlot && gs.totalHashpower > 0n) {
    const slotsElapsed = slotNow - gs.lastRewardSlot;
    const halvings =
      (slotNow - gs.startSlot) / gs.halvingInterval;
    const rateNow = gs.initialRewardRate >> halvings;

    /* clamp to remaining supply */
    const mintedMinusBurn = gs.cumulativeRewards - gs.burnedTokens;
    const remaining = gs.totalSupply > mintedMinusBurn
      ? gs.totalSupply - mintedMinusBurn
      : 0n;
    const reward = BigInt(slotsElapsed) * BigInt(rateNow);
    const rewardClamped = reward > remaining ? remaining : reward;

    acc += rewardClamped * ACC_SCALE / gs.totalHashpower;
  }

  /* 2️⃣ player pending */
  const accumulated = player.hashpower * (acc - player.lastAccBitsPerHash) / ACC_SCALE;
  const pending = accumulated > 0n ? accumulated : 0n;

  /* 3️⃣ referral split (use gs.referralFee if dynamic, else 25n for 2.5‰) */
  const referralFee = gs.referralFee ?? 25n;
  const referral = pending * referralFee / 1000n;
  return { playerAmount: pending - referral, referralAmount: referral };
}

/* ──────────────────────────────────────────────────────────────
   expectedDistributionSummary()
   – quick deterministic calculator for front-end / tests
   ──────────────────────────────────────────────────────────── */
export function expectedDistributionSummary(params: {
  halvingInterval: bigint;
  totalSupply: bigint;
  initialRewardRate: bigint;
}): {
  totalPredictedHalvings: number;
  halvingRewardsArr: bigint[];
  rewardRatesArr: bigint[];
  totalBlocks: bigint;
  totalBlockTimeMs: bigint;
  totalBlockTimeMinutes: number;
  totalBlockTimeHours: number;
  totalBlockTimeDays: number;
  timePerHalvingMinutes: number;
  timePerHalvingHours: number;
  timePerHalvingDays: number;
  totalSupplyT: bigint;
  totalSupplyE: bigint;
} {
  let remaining = params.totalSupply;
  let rewardPerSlot = params.initialRewardRate;
  let halvings = 0;
  const perHalvingTotal: bigint[] = [];
  const rewardRatePerSlot: bigint[] = [];
  const BLOCK_TIME_MS = 400n; // Solana's 400ms block time

  // Defensive: avoid infinite loop if params are zero
  if (params.halvingInterval <= 0n || params.initialRewardRate <= 0n) {
    return {
      totalPredictedHalvings: 0,
      halvingRewardsArr: [],
      rewardRatesArr: [],
      totalBlocks: 0n,
      totalBlockTimeMs: 0n,
      totalBlockTimeMinutes: 0,
      totalBlockTimeHours: 0,
      totalBlockTimeDays: 0,
      timePerHalvingMinutes: 0,
      timePerHalvingHours: 0,
      timePerHalvingDays: 0,
      totalSupplyT: params.totalSupply,
      totalSupplyE: 0n,
    };
  }

  while (rewardPerSlot > 0n && remaining > 0n) {
    rewardRatePerSlot.push(rewardPerSlot);

    const halvingReward = rewardPerSlot * params.halvingInterval;
    const actualReward = halvingReward > remaining ? remaining : halvingReward;
    perHalvingTotal.push(actualReward);
    remaining -= actualReward;
    rewardPerSlot >>= 1n;
    halvings++;
  }

  // Calculate total blocks and time metrics
  const totalBlocks = params.halvingInterval * BigInt(halvings);
  const totalBlockTimeMs = totalBlocks * BLOCK_TIME_MS;

  // Convert to more readable time units
  const msPerMinute = 60n * 1000n;
  const msPerHour = msPerMinute * 60n;
  const msPerDay = msPerHour * 24n;

  const totalBlockTimeMinutes = Number(totalBlockTimeMs / msPerMinute);
  const totalBlockTimeHours = Number(totalBlockTimeMs / msPerHour);
  const totalBlockTimeDays = Number(totalBlockTimeMs / msPerDay);

  // Calculate time per halving
  const timePerHalvingMs = params.halvingInterval * BLOCK_TIME_MS;
  const timePerHalvingMinutes = Number(timePerHalvingMs / msPerMinute);
  const timePerHalvingHours = Number(timePerHalvingMs / msPerHour);
  const timePerHalvingDays = Number(timePerHalvingMs / msPerDay);

  const totalSupplyEmitted = perHalvingTotal.reduce((a, v) => a + v, 0n);

  return {
    totalPredictedHalvings: halvings,
    halvingRewardsArr: perHalvingTotal.slice(0, 3), // for summary, or use full array if needed
    rewardRatesArr: rewardRatePerSlot.slice(0, 10), // for summary, or use full array if needed
    totalBlocks,
    totalBlockTimeMs,
    totalBlockTimeMinutes,
    totalBlockTimeHours,
    totalBlockTimeDays,
    timePerHalvingMinutes,
    timePerHalvingHours,
    timePerHalvingDays,
    totalSupplyT: params.totalSupply,
    totalSupplyE: totalSupplyEmitted,
  };
}

function initialRewardRateExact(
  halvingInterval: number,
  totalSupply: BN,
  numberOfHalvings: number,
): BN {
  // Convert inputs to BN
  const intervalBN = new BN(halvingInterval);

  // Use BN's pow method instead of bit shifting
  const twoBN = new BN(2);
  const powBN = twoBN.pow(new BN(numberOfHalvings));

  // Calculate numerator
  const numerator = totalSupply.mul(powBN);

  // Calculate denominator: 2 * halvingInterval * (2^n - 1)
  const denominator = twoBN.mul(intervalBN).mul(powBN.sub(new BN(1)));

  // Ceiling division
  const result = numerator.add(denominator.sub(new BN(1))).div(denominator);

  return result;
}

/**
 * Calculates the initial reward rate to distribute totalSupply over exact number of halvings
 * @param halvingInterval - The number of slots between halvings
 * @param totalSupply - The total supply of the mint
 * @param numberOfHalvings - The number of halvings to distribute the supply over
 * @returns {
 *   initialRewardRate: bigint;
 *   distributionSummary: ReturnType<typeof expectedDistributionSummary>;
 * }
 */
export function calculateInitialRewardRate(params: {
  halvingInterval: number;
  totalSupply: BN;
  numberOfHalvings: number;
}) {
  const { halvingInterval, totalSupply, numberOfHalvings } = params;

  if (halvingInterval <= 0n || numberOfHalvings <= 0) {
    throw new Error("halvingInterval and numberOfHalvings must be > 0");
  }

  const initialRewardRate = initialRewardRateExact(
    halvingInterval,
    totalSupply,
    numberOfHalvings,
  );

  const distributionSummary = expectedDistributionSummary({
    halvingInterval: BigInt(halvingInterval),
    totalSupply: BigInt(totalSupply.toString()),
    initialRewardRate: BigInt(initialRewardRate.toString()),
  });

  // Sanity check—should always pass
  if (
    distributionSummary.totalPredictedHalvings !== numberOfHalvings ||
    Math.abs(Number(distributionSummary.totalSupplyE - BigInt(totalSupply.toString()))) / Number(BigInt(totalSupply.toString())) > 0.001
  ) {
    throw new Error("Emission schedule mismatch—check your inputs.");
  }

  return { initialRewardRate, distributionSummary };
}

/**
 * Sets up a fresh test player, mint, and program state for purchase/upgrade/miner tests.
 * Returns all relevant keys and accounts.
 * @param provider 
 * @param program 
 * @param param2 
 * @returns 
 */
export async function setupFullTestPlayerAndProgram(
  provider: AnchorProvider,
  program: Program<Bitsol>,
  {
    initialMintAmount = 75_000_000000,
    halvingInterval = 100,
    totalSupply = new BN(1_000_000_000),
    initialRewardRate = new BN(10_000),
    cooldownSlots = 10,
    mintDecimals = 6,
  }: {
    initialMintAmount?: number,
    halvingInterval?: number,
    totalSupply?: BN,
    initialRewardRate?: BN,
    cooldownSlots?: number,
    mintDecimals?: number,
  } = {}
) {
  // 1. Create mint
  const mintKP = await createTestMint(provider, mintDecimals);
  const tokenMint = mintKP.publicKey;

  // 2. Create player wallet and token account
  const playerWallet = await createTestAccount(provider);
  const [playerKey] = PublicKey.findProgramAddressSync(
    [Buffer.from(PLAYER_SEED), playerWallet.publicKey.toBuffer()],
    program.programId
  );
  const playerTokenAccount = await createAssociatedTokenAccount(
    provider.connection,
    provider.wallet.payer,
    tokenMint,
    playerWallet.publicKey
  );

  // 3. Mint tokens to player
  await mintTo(
    provider.connection,
    provider.wallet.payer,
    tokenMint,
    playerTokenAccount,
    provider.wallet.publicKey,
    initialMintAmount,
  );

  // 4. Setup program state (globalState, governanceTokenAccount)
  const { globalStateKey, governanceTokenAccount } = await setupTestProgram(
    provider,
    program,
    halvingInterval,
    totalSupply,
    initialRewardRate,
    cooldownSlots,
    mintKP,
  );

  // 5. Purchase initial facility for player
  await program.methods
    .purchaseInitialFacility(null)
    .accountsStrict({
      playerWallet: playerWallet.publicKey,
      player: playerKey,
      globalState: globalStateKey,
      playerTokenAccount,
      tokenMint,
      tokenProgram: TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      rent: SYSVAR_RENT_PUBKEY,
      clock: SYSVAR_CLOCK_PUBKEY,
    })
    .signers([playerWallet])
    .rpc();

  return {
    tokenMint,
    playerWallet,
    playerKey,
    playerTokenAccount,
    globalStateKey,
    governanceTokenAccount,
  };
}

export async function drainPlayerTokenAccountToGovernance({
  provider,
  playerTokenAccount,
  governanceTokenAccount,
  playerWallet,
}: {
  provider: AnchorProvider,
  playerTokenAccount: PublicKey,
  governanceTokenAccount: PublicKey,
  playerWallet: Keypair,
}) {
  const { getAccount, createTransferInstruction } = await import("@solana/spl-token");
  const { Transaction } = await import("@solana/web3.js");

  const playerTokenInfo = await getAccount(provider.connection, playerTokenAccount);
  if (playerTokenInfo.amount > 0n) {
    const transferIx = createTransferInstruction(
      playerTokenAccount,
      governanceTokenAccount,
      playerWallet.publicKey,
      playerTokenInfo.amount
    );
    const tx = new Transaction().add(transferIx);
    await provider.sendAndConfirm(tx, [playerWallet]);
  }
}

export const sleepSlots = (n = 1, t = 500) => new Promise(r => setTimeout(r, n * t));

export function calculateEmissionSchedule(params: {
  halvingInterval: number;
  totalSupply: BN;
  initialRewardRate: BN;
}) {
  const { halvingInterval, totalSupply, initialRewardRate } = params;

  if (halvingInterval <= 0 || initialRewardRate.lte(new BN(0))) {
    throw new Error("halvingInterval and initialRewardRate must be > 0");
  }

  const distributionSummary = expectedDistributionSummary({
    halvingInterval: BigInt(halvingInterval),
    totalSupply: BigInt(totalSupply.toString()),
    initialRewardRate: BigInt(initialRewardRate.toString()),
  });

  // Optionally, you can still check if totalSupplyE is close to totalSupply
  if (
    Math.abs(Number(distributionSummary.totalSupplyE - BigInt(totalSupply.toString()))) / Number(BigInt(totalSupply.toString())) > 0.001
  ) {
    throw new Error("Emission schedule mismatch—check your inputs.");
  }

  return distributionSummary; // includes totalPredictedHalvings, etc.
}
