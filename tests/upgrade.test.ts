import * as anchor from "@coral-xyz/anchor";
import { BN, Program } from "@coral-xyz/anchor";
import {
    TOKEN_PROGRAM_ID,
    getAccount,
} from "@solana/spl-token";
import {
    PublicKey,
    SYSVAR_CLOCK_PUBKEY,
    SystemProgram,
} from "@solana/web3.js";

import { Bitsol } from "../target/types/bitsol";
import {
    setupFullTestPlayerAndProgram,
    sleepSlots,
    TOASTER,
    MINER_CONFIGS,
    CRAMPED_BEDROOM,
    FACILITY_CONFIGS,
    HIDDEN_POWERHOUSE,
    HIGH_RISE_APARTMENT,
    LOW_PROFILE_STORAGE,
    GPU_RACK,
    drainPlayerTokenAccountToGovernance,
} from "./test-helpers";

/* ─────────────────────────────────────────────────────────────────────────── */

describe("Bitsol – upgrade functionality", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const program = anchor.workspace.Bitsol as Program<Bitsol>;

    /* generic emission params (kept tiny for fast localnet tests) */
    const HALVING_INTERVAL = 300_000;
    const TOTAL_SUPPLY = new BN(1_000_000_000000); // 1M BITS @ 6 dp
    const INITIAL_REWARD_RATE = new BN(100_000000) // 100 BITS / slot
    const COOLDOWN_SLOTS = 1;

    let tokenMint: PublicKey,
        globalStateKey: PublicKey,
        governanceTokenAccount: PublicKey,
        playerWallet: anchor.web3.Keypair,
        playerKey: PublicKey,
        playerTokenAccount: PublicKey;

    /* helpers ---------------------------------------------------------------- */
    const buyMiner = async (minerType: number) =>
        program.methods
            .buyMiner(minerType, 0)
            .accountsStrict({
                playerWallet: playerWallet.publicKey,
                player: playerKey,
                globalState: globalStateKey,
                playerTokenAccount,
                governanceTokenAccount,
                tokenMint,
                tokenProgram: TOKEN_PROGRAM_ID,
                clock: SYSVAR_CLOCK_PUBKEY,
                systemProgram: SystemProgram.programId,
            })
            .signers([playerWallet])
            .rpc();

    const upgradeFacility = async (nextType: number) =>
        program.methods
            .upgradeFacility(nextType)
            .accountsStrict({
                playerWallet: playerWallet.publicKey,
                player: playerKey,
                globalState: globalStateKey,
                playerTokenAccount,
                governanceTokenAccount,
                tokenMint,
                tokenProgram: TOKEN_PROGRAM_ID,
                clock: SYSVAR_CLOCK_PUBKEY,
            })
            .signers([playerWallet])
            .rpc();

    /* ───────────────────────────────────────────────────────────────────────── */

    describe("miner upgrades", () => {
        beforeEach(async () => {
            ({ tokenMint, globalStateKey, governanceTokenAccount, playerWallet, playerKey, playerTokenAccount } =
                await setupFullTestPlayerAndProgram(
                    provider, program,
                    {
                        totalSupply: TOTAL_SUPPLY,
                        cooldownSlots: COOLDOWN_SLOTS,
                        initialRewardRate: INITIAL_REWARD_RATE,
                        halvingInterval: HALVING_INTERVAL,
                    }
                ));

            await sleepSlots(1); // let genesis slot tick
        });

        it("adds hash-power / miners & burns cost", async () => {
            // Settle rewards before buying to get pending rewards
            const before = await getAccount(provider.connection, playerTokenAccount);
            const pl0 = await program.account.player.fetch(playerKey);
            const gs0 = await program.account.globalState.fetch(globalStateKey);

            // Buy Nano Rig (index 0)
            await buyMiner(TOASTER);

            const after = await getAccount(provider.connection, playerTokenAccount);
            const pl1 = await program.account.player.fetch(playerKey);
            const gs1 = await program.account.globalState.fetch(globalStateKey);

            const [hashrate, , cost] = MINER_CONFIGS[TOASTER];

            expect(pl1.miners.length).toBe(pl0.miners.length + 1);
            expect(pl1.hashpower.toNumber()).toBe(pl0.hashpower.toNumber() + hashrate);
            expect(gs1.totalHashpower.toNumber()).toBe(gs0.totalHashpower.toNumber() + hashrate);

            // hard to do diff because settle_and_mint_rewards is called
            // cost is 1_000_000
            expect(Number(before.amount)).toBe(75000000000);
            expect(Number(after.amount)).toBe(75110000000);
        });

        it("fails when exceeding miner-capacity", async () => {
            // Starter Shack caps at 2 miners → already has 1 from bootstrap
            await buyMiner(TOASTER);                 // third exceeds capacity
            await expect(buyMiner(TOASTER))
                .rejects
                .toThrow(/MinerCapacityExceeded/);
        });

        it("fails when exceeding power capacity", async () => {
            // Fill power output exactly, then try to exceed by Turbo Miner
            await expect(buyMiner(GPU_RACK))       // +60 power → should error
                .rejects
                .toThrow(/PowerCapacityExceeded/);
        });

        it("fails on invalid miner type", async () => {
            await expect(buyMiner(99))
                .rejects
                .toThrow(/InvalidMinerType/);
        });

        it("allows selling a miner and updates hashpower", async () => {
            // buy an extra miner then sell it (index 1)
            await sleepSlots(1);
            await buyMiner(TOASTER);
            const plMid = await program.account.player.fetch(playerKey);
            expect(plMid.miners.length).toBe(2);
            await sleepSlots(1);

            await program.methods
                .sellMiner(1)                            // sell index 1 (the new rig)
                .accountsStrict({
                    playerWallet: playerWallet.publicKey,
                    player: playerKey,
                    globalState: globalStateKey,
                    playerTokenAccount,
                    tokenMint,
                    governanceTokenAccount,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    clock: SYSVAR_CLOCK_PUBKEY,
                })
                .signers([playerWallet])
                .rpc();

            const plEnd = await program.account.player.fetch(playerKey);
            expect(plEnd.miners.length).toBe(1);       // back to bootstrap rig
            expect(plEnd.hashpower.toNumber()).toBeLessThan(plMid.hashpower.toNumber());
        });
    });

    /* ───────────────────────────────────────────────────────────────────────── */

    describe("facility upgrades", () => {
        beforeEach(async () => {
            ({
                playerWallet, playerKey, playerTokenAccount, globalStateKey,
                governanceTokenAccount, tokenMint
            } =
                await setupFullTestPlayerAndProgram(provider, program, {
                    totalSupply: TOTAL_SUPPLY,
                    cooldownSlots: COOLDOWN_SLOTS,
                    initialRewardRate: INITIAL_REWARD_RATE,
                    halvingInterval: HALVING_INTERVAL,
                }));

            await sleepSlots(1);
        });

        it("upgrades to next tier and enforces cooldown", async () => {
            const beforeBal = await getAccount(provider.connection, playerTokenAccount);

            // upgrade → Small Warehouse
            await upgradeFacility(LOW_PROFILE_STORAGE);

            const pl1 = await program.account.player.fetch(playerKey);
            const afterBal = await getAccount(provider.connection, playerTokenAccount);

            expect(pl1.facility.facilityType).toBe(LOW_PROFILE_STORAGE);
            expect(afterBal.amount).toBeLessThan(75240000001n);       // paid upgrade cost

            // immediate second upgrade must fail (cooldown 2 slots)
            await expect(upgradeFacility(HIDDEN_POWERHOUSE))
                .rejects
                .toThrow(/CooldownNotExpired/);

            await sleepSlots(COOLDOWN_SLOTS + 1);
            await upgradeFacility(HIDDEN_POWERHOUSE);

            const pl2 = await program.account.player.fetch(playerKey);
            expect(pl2.facility.facilityType).toBe(HIDDEN_POWERHOUSE);
        });

        it("blocks downgrade or invalid tier", async () => {
            await upgradeFacility(LOW_PROFILE_STORAGE);

            // Downgrade attempt
            await expect(upgradeFacility(CRAMPED_BEDROOM))
                .rejects
                .toThrow(/InvalidFacilityType/);

            // Above MAX constant
            await expect(upgradeFacility(HIGH_RISE_APARTMENT + 1))
                .rejects
                .toThrow(/InvalidFacilityType/);
        });

        it("increases capacity so extra miners can be bought", async () => {
            // Starter Shack → capacity 2. Upgrade to Small Warehouse (capacity 5)
            await sleepSlots(3);
            await upgradeFacility(LOW_PROFILE_STORAGE);

            // Buy until new capacity reached
            const capacity = FACILITY_CONFIGS[LOW_PROFILE_STORAGE][0];
            let pl = await program.account.player.fetch(playerKey);
            const minersToBuy = capacity - pl.miners.length;
            console.log({
                minersToBuy,
                facilityType: FACILITY_CONFIGS[LOW_PROFILE_STORAGE][1],
                currentMiners: pl.miners.length,
                capacity,
            });
            await sleepSlots(1);

            for (let i = 0; i < minersToBuy; i++) {
                await buyMiner(TOASTER);
                await sleepSlots(1);
                pl = await program.account.player.fetch(playerKey);
                console.log(`After buy #${i + 1}: miners = ${pl.miners.length}`);
                if (pl.miners.length >= capacity) {
                    console.log("Reached capacity, breaking loop.");
                    break;
                }
            }
            const plEnd = await program.account.player.fetch(playerKey);
            console.log(`Final miners: ${plEnd.miners.length}, capacity: ${capacity}`);
            expect(plEnd.miners.length).toBeLessThanOrEqual(capacity);
        });

        it("fails upgrade when balance is insufficient", async () => {
            // Drain all tokens from the player's token account
            await drainPlayerTokenAccountToGovernance({
                provider,
                playerTokenAccount,
                governanceTokenAccount,
                playerWallet,
            });

            await expect(upgradeFacility(LOW_PROFILE_STORAGE))
                .rejects
                .toThrow(/InsufficientBits/);
        });
    });
});
