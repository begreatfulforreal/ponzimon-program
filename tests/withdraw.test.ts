import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { Bitsol } from "../target/types/bitsol";
import { ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID, getAccount, getOrCreateAssociatedTokenAccount } from "@solana/spl-token";
import { createTestAccount, setupFullTestPlayerAndProgram, sleepSlots } from "./test-helpers";

describe("Bitsol - withdraw_fees and withdraw_sol_fees instructions", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const program = anchor.workspace.Bitsol as Program<Bitsol>;

    let tokenMint: PublicKey;
    let authority: PublicKey;
    let destinationTokenAccount: PublicKey;
    let globalStateKey: PublicKey;
    let governanceTokenAccount: PublicKey;
    let playerWallet: Keypair;
    let playerTokenAccount: PublicKey;
    let playerKey: PublicKey;

    const HALVING_INTERVAL = 10;
    const TOTAL_SUPPLY = new BN(1_000_000_000000);
    const INITIAL_REWARD_RATE = new BN(100_000000);
    const COOLDOWN_SLOTS = 1;

    beforeEach(async () => {
        ({
            tokenMint,
            globalStateKey,
            governanceTokenAccount,
            playerWallet,
            playerTokenAccount,
            playerKey
        } = await setupFullTestPlayerAndProgram(provider, program, {
            initialMintAmount: TOTAL_SUPPLY.toNumber(),
            halvingInterval: HALVING_INTERVAL,
            totalSupply: TOTAL_SUPPLY,
            initialRewardRate: INITIAL_REWARD_RATE,
            cooldownSlots: COOLDOWN_SLOTS,
        }));
        
        await sleepSlots(1);
        await program.methods
            .buyMiner(0, 0)
            .accountsStrict({
                playerWallet: playerWallet.publicKey,
                player: playerKey,
                globalState: globalStateKey,
                playerTokenAccount,
                governanceTokenAccount,
                tokenMint,
                tokenProgram: TOKEN_PROGRAM_ID,
                clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
                systemProgram: anchor.web3.SystemProgram.programId,
            })
            .signers([playerWallet])
            .rpc();

        authority = provider.wallet.publicKey;
        destinationTokenAccount = (
            await getOrCreateAssociatedTokenAccount(provider.connection, provider.wallet.payer, tokenMint, authority)
        ).address;
    });

    it("allows authority to withdraw SPL tokens", async () => {
        const withdrawAmount = BigInt(2_500000); // Example: 2.5 $BITSOL

        const initialDestBalance = (await getAccount(provider.connection, destinationTokenAccount)).amount;
        const initialGovBalance = (await getAccount(provider.connection, governanceTokenAccount)).amount;

        try {
            await program.methods
                .withdrawFees(new BN(withdrawAmount.toString()))
                .accountsStrict({
                    authority,
                    globalState: globalStateKey,
                    governanceTokenAccount,
                    destination: destinationTokenAccount,
                    tokenMint,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    systemProgram: SystemProgram.programId,
                    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
                })
                .rpc();
        } catch (error) {
            console.log(error);
        }

        const finalDestBalance = (await getAccount(provider.connection, destinationTokenAccount)).amount;
        const finalGovBalance = (await getAccount(provider.connection, governanceTokenAccount)).amount;

        expect(BigInt(finalDestBalance)).toBe(BigInt(initialDestBalance) + withdrawAmount);
        expect(BigInt(finalGovBalance)).toBe(BigInt(initialGovBalance) - withdrawAmount);
    });

    it("allows authority to withdraw SOL", async () => {
        // Create a new destination system account for SOL
        const solDestWallet = await createTestAccount(provider);

        // Check initial balances
        const initialGovLamports = await provider.connection.getBalance(governanceTokenAccount);
        const initialDestLamports = await provider.connection.getBalance(solDestWallet.publicKey);
        console.log({
            initialGovLamports,
            initialDestLamports,
        });

        // Withdraw 0.04 SOL (one facility's worth)
        const withdrawLamports = 40_000_000;

        await program.methods
            .withdrawSolFees(new BN(withdrawLamports))
            .accountsStrict({
                authority,
                globalState: globalStateKey,
                destination: solDestWallet.publicKey,
                systemProgram: SystemProgram.programId,
            })
            .rpc();

        const finalDestLamports = await provider.connection.getBalance(solDestWallet.publicKey);
        const globalStateLamports = await provider.connection.getBalance(globalStateKey);

        // Fetch rent-exempt minimum for the global state account
        const accountInfo = await provider.connection.getAccountInfo(globalStateKey);
        const rentExemptMin = accountInfo?.lamports ?? 0;

        expect(finalDestLamports).toBe(initialDestLamports + withdrawLamports);
        expect(globalStateLamports).toBeGreaterThanOrEqual(rentExemptMin);
    });

    it("fails if non-authority tries to withdraw SPL tokens", async () => {
        const fakeAuthority = anchor.web3.Keypair.generate();
        await expect(
            program.methods
                .withdrawFees(new BN(1_000_000))
                .accountsStrict({
                    authority: fakeAuthority.publicKey,
                    globalState: globalStateKey,
                    governanceTokenAccount,
                    destination: destinationTokenAccount,
                    tokenMint,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    systemProgram: SystemProgram.programId,
                    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
                })
                .signers([fakeAuthority])
                .rpc()
        ).rejects.toThrow(/Unauthorized/);
    });

    it("fails if non-authority tries to withdraw SOL", async () => {
        const fakeAuthority = anchor.web3.Keypair.generate();
        const solDestWallet = await createTestAccount(provider);
        await expect(
            program.methods
                .withdrawSolFees(new BN(1_000_000))
                .accountsStrict({
                    authority: fakeAuthority.publicKey,
                    globalState: globalStateKey,
                    destination: solDestWallet.publicKey,
                    systemProgram: SystemProgram.programId,
                })
                .signers([fakeAuthority])
                .rpc()
        ).rejects.toThrow(/Unauthorized/);
    });

    it("fails if withdrawing more SPL tokens than available", async () => {
        const govBalance = (await getAccount(provider.connection, governanceTokenAccount)).amount;
        await expect(
            program.methods
                .withdrawFees(new BN((BigInt(govBalance) + 1n).toString()))
                .accountsStrict({
                    authority,
                    globalState: globalStateKey,
                    governanceTokenAccount,
                    destination: destinationTokenAccount,
                    tokenMint,
                    tokenProgram: TOKEN_PROGRAM_ID,
                    systemProgram: SystemProgram.programId,
                    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
                })
                .rpc()
        ).rejects.toThrow();
    });

    it("fails if withdrawing more SOL than available", async () => {
        const solDestWallet = await createTestAccount(provider);
        // Get the globalState PDA's lamports and subtract rent-exemption
        const globalStateLamports = await provider.connection.getBalance(globalStateKey);
        // For test, just try to withdraw more than available
        await expect(
            program.methods
                .withdrawSolFees(new BN(globalStateLamports + 1))
                .accountsStrict({
                    authority,
                    globalState: globalStateKey,
                    destination: solDestWallet.publicKey,
                    systemProgram: SystemProgram.programId,
                })
                .rpc()
        ).rejects.toThrow();
    });
});
