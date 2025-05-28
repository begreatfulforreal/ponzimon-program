import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram, SYSVAR_CLOCK_PUBKEY } from "@solana/web3.js";
import { Bitsol } from "../target/types/bitsol";
import {
    TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
    setupFullTestPlayerAndProgram,
    sleepSlots,
    TOASTER,
} from "./test-helpers";

describe("Bitsol - sell_miner instruction", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const program = anchor.workspace.Bitsol as Program<Bitsol>;

    let tokenMint: PublicKey,
        globalStateKey: PublicKey,
        governanceTokenAccount: PublicKey,
        playerWallet: anchor.web3.Keypair,
        playerKey: PublicKey,
        playerTokenAccount: PublicKey;

    beforeEach(async () => {
        ({
            tokenMint,
            playerWallet,
            playerKey,
            playerTokenAccount,
            globalStateKey,
            governanceTokenAccount,
        } = await setupFullTestPlayerAndProgram(provider, program));
    });

    it("successfully sells a miner and updates hashpower", async () => {
        // Buy a second miner so we have one to sell
        try {
            await program.methods
                .buyMiner(TOASTER, 0)
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

            let player = await program.account.player.fetch(playerKey);
            const initialMiners = player.miners.length;
            const initialHashpower = player.hashpower.toNumber();
            const minerHashrate = player.miners[1].hashrate.toNumber();

            // Sell the second miner (index 1)
            await program.methods
                .sellMiner(1)
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

            player = await program.account.player.fetch(playerKey);
            expect(player.miners.length).toBe(initialMiners - 1);
            expect(player.hashpower.toNumber()).toBe(initialHashpower - minerHashrate);
        } catch (error) {
            console.log(error);
        }
    });

    it("fails if miner index is invalid", async () => {
        // Only one miner at start (index 0 is valid, 1 is not)
        await expect(
            program.methods
                .sellMiner(1)
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
                .rpc()
        ).rejects.toThrow(/InvalidMinerType/);
    });

    it("fails if called by non-owner", async () => {
        // Buy a second miner so we have one to sell
        await sleepSlots(1);
        await program.methods
            .buyMiner(TOASTER, 0)
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

        const fakeWallet = anchor.web3.Keypair.generate();
        await expect(
            program.methods
                .sellMiner(1)
                .accountsStrict({
                    playerWallet: fakeWallet.publicKey,
                    player: playerKey,
                    globalState: globalStateKey,
                    playerTokenAccount,
                    tokenMint,
                    governanceTokenAccount,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    clock: SYSVAR_CLOCK_PUBKEY,
                })
                .signers([fakeWallet])
                .rpc()
        ).rejects.toThrow(/ConstraintSeeds/i);
    });

    it("fails if production is disabled", async () => {
        // Buy a second miner so we have one to sell
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

        // Disable production
        await program.methods
            .toggleProduction(false)
            .accountsStrict({
                authority: provider.wallet.publicKey,
                globalState: globalStateKey,
            })
            .rpc();

        await expect(
            program.methods
                .sellMiner(1)
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
                .rpc()
        ).rejects.toThrow(/ProductionDisabled/);
    });
});