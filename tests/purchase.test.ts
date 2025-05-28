import * as anchor from "@coral-xyz/anchor";
import {
    PublicKey,
    SYSVAR_RENT_PUBKEY,
    SYSVAR_CLOCK_PUBKEY,
    Keypair,
    SystemProgram,
} from "@solana/web3.js";
import { Program, BN } from "@coral-xyz/anchor";
import {
    TOKEN_PROGRAM_ID,
    getAccount,
    getMint,
    ASSOCIATED_TOKEN_PROGRAM_ID,
    getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { Bitsol } from "../target/types/bitsol";
import {
    FACILITY_CONFIGS,
    RASPBERRY_PI,
    MINER_CONFIGS,
    TOASTER,
    PLAYER_SEED,
    LOW_PROFILE_STORAGE,
    CRAMPED_BEDROOM,
    createTestAccount,
    createTokenAccount,
    drainPlayerTokenAccountToGovernance,
    setupFullTestPlayerAndProgram,
    setupTestPlayer,
    setupTestProgram,
    sleepSlots,
} from "./test-helpers";
import { SYSTEM_PROGRAM_ID } from "@coral-xyz/anchor/dist/cjs/native/system";

describe("Bitsol Purchase Instructions", () => {
    // Common setup code from existing describe blocks
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const program = anchor.workspace.Bitsol as Program<Bitsol>;

    // Setup variables
    let tokenMint: PublicKey;
    let globalStateKey: PublicKey;
    let playerWallet: Keypair;
    let playerKey: PublicKey;
    let playerTokenAccount: PublicKey;
    let governanceTokenAccount: PublicKey;

    const HALVING_INTERVAL = 10;
    const TOTAL_SUPPLY = new BN(21_000_000_000000);
    const INITIAL_REWARD_RATE = new BN(50_000000);
    const COOLDOWN_SLOTS = 10;

    beforeAll(async () => {
        ({ tokenMint, globalStateKey, governanceTokenAccount } =
            await setupTestProgram(provider, program, HALVING_INTERVAL, TOTAL_SUPPLY, INITIAL_REWARD_RATE, COOLDOWN_SLOTS));
    });

    beforeEach(async () => {
        ({ playerWallet, playerKey, playerTokenAccount } =
            await setupTestPlayer(provider, program, tokenMint, globalStateKey));
    });

    describe("Initial Facility Purchase", () => {
        test("should successfully purchase initial facility", async () => {
            // Verify player account initialization
            const player = await program.account.player.fetch(playerKey);
            const globalState = await program.account.globalState.fetch(
                globalStateKey
            );

            expect(player.owner.toString()).toBe(playerWallet.publicKey.toString());
            expect(player.facility.facilityType).toBe(CRAMPED_BEDROOM);
            expect(player.facility.totalMiners).toBe(
                FACILITY_CONFIGS[CRAMPED_BEDROOM][0]
            );
            expect(player.facility.powerOutput.toString()).toBe(
                FACILITY_CONFIGS[CRAMPED_BEDROOM][1].toString()
            );
            expect(player.miners.length).toBe(1);
            expect(player.miners[0].minerType).toBe(TOASTER);
            expect(player.miners[0].hashrate.toString()).toBe("1500");
            expect(player.miners[0].powerConsumption.toString()).toBe("3");
            expect(player.hashpower.toString()).toBe("1500");
            expect(player.referrer).toBe(null);
            expect(Number(player.lastAccBitsPerHash)).toBeCloseTo(Number(globalState.accBitsPerHash));
            expect(BN.isBN(player.lastClaimSlot)).toBe(true);
            expect(BN.isBN(player.lastUpgradeSlot)).toBe(true);
            expect(globalState.totalHashpower.toString()).toBe("1500");
        });

        test("should fail when production is disabled", async () => {
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
                .toggleProduction(false)
                .accountsStrict({
                    authority: provider.wallet.publicKey,
                    globalState: globalStateKey,
                })
                .rpc();

            await expect(
                program.methods
                    .purchaseInitialFacility(null)
                    .accountsStrict({
                        playerWallet: playerWallet.publicKey,
                        player: playerKey,
                        globalState: globalStateKey,
                        playerTokenAccount: playerTokenAccount,
                        systemProgram: SystemProgram.programId,
                        rent: SYSVAR_RENT_PUBKEY,
                        clock: SYSVAR_CLOCK_PUBKEY,
                        tokenMint: tokenMint,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
                    })
                    .signers([playerWallet])
                    .rpc()
            ).rejects.toThrow(/ProductionDisabled/);

            await program.methods
                .toggleProduction(true)
                .accountsStrict({
                    authority: provider.wallet.publicKey,
                    globalState: globalStateKey,
                })
                .rpc();
        });

        test("should fail with insufficient SOL", async () => {
            const emptyWallet = Keypair.generate();
            const emptyTokenAccount = await createTokenAccount(
                provider,
                tokenMint,
                emptyWallet.publicKey
            );

            const [emptyPlayerKey] = PublicKey.findProgramAddressSync(
                [Buffer.from(PLAYER_SEED), emptyWallet.publicKey.toBuffer()],
                program.programId
            );

            await expect(
                program.methods
                    .purchaseInitialFacility(null)
                    .accountsStrict({
                        playerWallet: emptyWallet.publicKey,
                        player: emptyPlayerKey,
                        globalState: globalStateKey,
                        playerTokenAccount: emptyTokenAccount,
                        systemProgram: SystemProgram.programId,
                        rent: SYSVAR_RENT_PUBKEY,
                        clock: SYSVAR_CLOCK_PUBKEY,
                        tokenMint: tokenMint,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
                    })
                    .signers([emptyWallet])
                    .rpc()
            ).rejects.toThrow();
        });
    });

    describe("Miner Purchase", () => {
        beforeEach(async () => {
            ({ playerWallet, playerKey, playerTokenAccount, globalStateKey, governanceTokenAccount, tokenMint } =
                await setupFullTestPlayerAndProgram(provider, program));
        });

        test("should successfully buy a miner", async () => {
            const initialGlobalState = await program.account.globalState.fetch(
                globalStateKey
            );
            const minerType = MINER_CONFIGS[RASPBERRY_PI];
            const minerCost = minerType[2];

            console.log({
                provider: provider.wallet.publicKey.toBase58(),
                tokenMint: tokenMint.toBase58(),
                playerTokenAccount: playerTokenAccount.toBase58(),
                globalStateKey: globalStateKey.toBase58(),
                minerCost,
                playerWallet: playerWallet.publicKey.toBase58(),
                playerKey: playerKey.toBase58(),
                governanceTokenAccount: governanceTokenAccount.toBase58(),
            });

            const initialPlayerBalance = (
                await getAccount(provider.connection, playerTokenAccount)
            ).amount;
            const initialGovernanceBalance = (
                await getAccount(provider.connection, governanceTokenAccount)
            ).amount;

            try {
                await sleepSlots(1);
                await program.methods
                    .buyMiner(RASPBERRY_PI, 0)
                    .accountsStrict({
                        playerWallet: playerWallet.publicKey,
                        player: playerKey,
                        globalState: globalStateKey,
                        playerTokenAccount: playerTokenAccount,
                        governanceTokenAccount: governanceTokenAccount,
                        tokenMint: tokenMint,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        clock: SYSVAR_CLOCK_PUBKEY,
                        systemProgram: SystemProgram.programId,
                    })
                    .signers([playerWallet])
                    .rpc();
            } catch (error) {
                console.log(error);
            }

            const player = await program.account.player.fetch(playerKey);
            const globalState = await program.account.globalState.fetch(
                globalStateKey
            );

            // Check token balances
            const finalPlayerBalance = (
                await getAccount(provider.connection, playerTokenAccount)
            ).amount;
            const finalGovernanceBalance = (
                await getAccount(provider.connection, governanceTokenAccount)
            ).amount;

            const burnAmount = (BigInt(minerCost) * BigInt(75)) / BigInt(100); // 75% burn rate
            const governanceAmount = BigInt(minerCost) - burnAmount; // 25% to governance

            // Verify balances
            // NOTE: The difference between spent and minerCost is due to rewards being claimed
            // automatically during buy_miner (see settle_and_mint_rewards in the program).
            // This means the player receives pending rewards before paying for the miner,
            // so the net spent is slightly less than minerCost.
            const spent = BigInt(initialPlayerBalance) - BigInt(finalPlayerBalance);

            expect(minerCost).toBe(120000000);
            expect(spent).toBe(119985000n); // slight rounding error due to settle_and_mint_rewards
            expect(Number(governanceAmount)).toBe(30000000);
            expect(Number(finalGovernanceBalance - initialGovernanceBalance)).toBe(30004999);

            //   we add an extra 1500 to the initial hashpower because the initial facility has 1500 hashpower
            const expectedHashpower = Number(initialGlobalState.totalHashpower) + Number(player.miners[1].hashrate);
            expect(globalState.totalHashpower.toString()).toBe(expectedHashpower.toString());

            expect(player.miners.length).toBe(2);
            expect(player.miners[1].minerType).toBe(RASPBERRY_PI);
            expect(player.miners[1].hashrate.toString()).toBe(minerType[0].toString());
            expect(player.miners[1].powerConsumption.toString()).toBe(minerType[1].toString());
            expect(player.hashpower.toString()).toBe("7500");

            const totalPower = player.miners.reduce(
                (sum, miner) => sum + miner.powerConsumption.toNumber(),
                0
            );
            expect(totalPower).toBeLessThanOrEqual(
                player.facility.powerOutput.toNumber()
            );
        });

        test("should correctly distribute tokens between burn and governance", async () => {
            const minerCost = MINER_CONFIGS[TOASTER][2];

            const initialPlayerBalance = (
                await getAccount(provider.connection, playerTokenAccount)
            ).amount;
            const initialGovernanceBalance = (
                await getAccount(provider.connection, governanceTokenAccount)
            ).amount;
            const initialSupply = (await getMint(provider.connection, tokenMint))
                .supply;

            await sleepSlots(1);
            await program.methods
                .buyMiner(TOASTER, 0)
                .accountsStrict({
                    playerWallet: playerWallet.publicKey,
                    player: playerKey,
                    globalState: globalStateKey,
                    playerTokenAccount: playerTokenAccount,
                    governanceTokenAccount: governanceTokenAccount,
                    tokenMint: tokenMint,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    clock: SYSVAR_CLOCK_PUBKEY,
                    systemProgram: SystemProgram.programId,
                })
                .signers([playerWallet])
                .rpc();

            const finalPlayerBalance = (
                await getAccount(provider.connection, playerTokenAccount)
            ).amount;
            const finalGovernanceBalance = (
                await getAccount(provider.connection, governanceTokenAccount)
            ).amount;
            const finalSupply = (await getMint(provider.connection, tokenMint))
                .supply;

            const burnAmount = (BigInt(minerCost) * BigInt(75)) / BigInt(100); // 75% burn rate
            const governanceAmount = BigInt(minerCost) - burnAmount;

            // Check player spent correct amount
            const spent = Number(initialPlayerBalance - finalPlayerBalance);
            expect(spent).toBe(39985000); // slight rounding error due to settle_and_mint_rewards

            // Check governance received correct amount
            expect(Number(governanceAmount)).toBe(10000000);
            expect(Number(finalGovernanceBalance - initialGovernanceBalance)).toBe(10004999); // slight rounding error due to settle_and_mint_rewards

            // Check correct amount was burned
            expect(burnAmount).toBe(30000000n);
            expect(Number(initialSupply - finalSupply)).toBe(29980001);
        });

        test("should fail when facility capacity is exceeded", async () => {
            await sleepSlots(1);
            await program.methods
                .buyMiner(TOASTER, 0)
                .accountsStrict({
                    playerWallet: playerWallet.publicKey,
                    player: playerKey,
                    globalState: globalStateKey,
                    playerTokenAccount: playerTokenAccount,
                    governanceTokenAccount: governanceTokenAccount,
                    tokenMint: tokenMint,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    clock: SYSVAR_CLOCK_PUBKEY,
                    systemProgram: SystemProgram.programId,
                })
                .signers([playerWallet])
                .rpc();

            // Try to exceed capacity
            await expect(
                program.methods
                    .buyMiner(TOASTER, 0)
                    .accountsStrict({
                        playerWallet: playerWallet.publicKey,
                        player: playerKey,
                        globalState: globalStateKey,
                        playerTokenAccount: playerTokenAccount,
                        governanceTokenAccount: governanceTokenAccount,
                        tokenMint: tokenMint,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        clock: SYSVAR_CLOCK_PUBKEY,
                        systemProgram: SystemProgram.programId,
                    })
                    .signers([playerWallet])
                    .rpc()
            ).rejects.toThrow(/MinerCapacityExceeded/);
        });

        test("should fail with insufficient BITS", async () => {
            // Drain all tokens from the player's token account
            await drainPlayerTokenAccountToGovernance({
                provider,
                playerTokenAccount,
                governanceTokenAccount,
                playerWallet,
            });
            await sleepSlots(1);
            await expect(
                program.methods
                    .buyMiner(TOASTER, 0)
                    .accountsStrict({
                        playerWallet: playerWallet.publicKey,
                        player: playerKey,
                        globalState: globalStateKey,
                        playerTokenAccount: playerTokenAccount,
                        governanceTokenAccount: governanceTokenAccount,
                        tokenMint: tokenMint,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        clock: SYSVAR_CLOCK_PUBKEY,
                        systemProgram: SystemProgram.programId,
                    })
                    .signers([playerWallet])
                    .rpc()
            ).rejects.toThrow(/InsufficientBits/);
        });
    });

    describe("Facility Upgrade", () => {
        beforeEach(async () => {
            ({ playerWallet, playerKey, playerTokenAccount, globalStateKey, governanceTokenAccount, tokenMint } =
                await setupFullTestPlayerAndProgram(provider, program));
        });

        test("should enforce facility upgrade cooldown", async () => {
            await sleepSlots(1);
            await expect(
                program.methods
                    .upgradeFacility(LOW_PROFILE_STORAGE)
                    .accountsStrict({
                        playerWallet: playerWallet.publicKey,
                        player: playerKey,
                        globalState: globalStateKey,
                        playerTokenAccount: playerTokenAccount,
                        governanceTokenAccount: governanceTokenAccount,
                        tokenMint: tokenMint,
                        tokenProgram: TOKEN_PROGRAM_ID,
                        clock: SYSVAR_CLOCK_PUBKEY,
                    })
                    .signers([playerWallet])
                    .rpc()
            ).rejects.toThrow(/CooldownNotExpired/);
        });
    });
});
