import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import { Bitsol } from "../target/types/bitsol";
import { setupTestProgram } from "./test-helpers";

describe("Bitsol - update_parameters instruction", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    const program = anchor.workspace.Bitsol as Program<Bitsol>;

    let tokenMint: PublicKey;
    let globalStateKey: PublicKey;
    let governanceTokenAccount: PublicKey;

    const HALVING_INTERVAL = 10;
    const TOTAL_SUPPLY = new BN(1_000_000_000000);
    const INITIAL_REWARD_RATE = new BN(100_000000);
    const COOLDOWN_SLOTS = 5;

    beforeEach(async () => {
        ({ tokenMint, globalStateKey, governanceTokenAccount } = await setupTestProgram(
            provider,
            program,
            HALVING_INTERVAL,
            TOTAL_SUPPLY,
            INITIAL_REWARD_RATE,
            COOLDOWN_SLOTS
        ));
    });

    it("updates referral fee, burn rate, cooldown slots, and halving interval", async () => {
        // Update all parameters
        await program.methods
            .updateParameters(30, 60, new BN(20), new BN(12345))
            .accountsStrict({
                authority: provider.wallet.publicKey,
                globalState: globalStateKey,
            })
            .rpc();

        const globalState = await program.account.globalState.fetch(globalStateKey);
        expect(globalState.referralFee).toBe(30);
        expect(globalState.burnRate).toBe(60);
        expect(globalState.cooldownSlots.toString()).toBe("20");
        expect(globalState.halvingInterval.toString()).toBe("12345");
    });

    it("updates only one parameter at a time", async () => {
        // Only referral fee
        await program.methods
            .updateParameters(10, null, null, null)
            .accountsStrict({
                authority: provider.wallet.publicKey,
                globalState: globalStateKey,
            })
            .rpc();
        let globalState = await program.account.globalState.fetch(globalStateKey);
        expect(globalState.referralFee).toBe(10);

        // Only burn rate
        await program.methods
            .updateParameters(null, 40, null, null)
            .accountsStrict({
                authority: provider.wallet.publicKey,
                globalState: globalStateKey,
            })
            .rpc();
        globalState = await program.account.globalState.fetch(globalStateKey);
        expect(globalState.burnRate).toBe(40);

        // Only cooldown slots
        await program.methods
            .updateParameters(null, null, new BN(99), null)
            .accountsStrict({
                authority: provider.wallet.publicKey,
                globalState: globalStateKey,
            })
            .rpc();
        globalState = await program.account.globalState.fetch(globalStateKey);
        expect(globalState.cooldownSlots.toString()).toBe("99");

        // Only halving interval
        await program.methods
            .updateParameters(null, null, null, new BN(8888))
            .accountsStrict({
                authority: provider.wallet.publicKey,
                globalState: globalStateKey,
            })
            .rpc();
        globalState = await program.account.globalState.fetch(globalStateKey);
        expect(globalState.halvingInterval.toString()).toBe("8888");
    });

    it("fails if referral fee is too high", async () => {
        await expect(
            program.methods
                .updateParameters(51, null, null, null)
                .accountsStrict({
                    authority: provider.wallet.publicKey,
                    globalState: globalStateKey,
                })
                .rpc()
        ).rejects.toThrow(/InvalidMinerType/);
    });

    it("fails if cooldown slots is zero", async () => {
        await expect(
            program.methods
                .updateParameters(null, null, new BN(0), null)
                .accountsStrict({
                    authority: provider.wallet.publicKey,
                    globalState: globalStateKey,
                })
                .rpc()
        ).rejects.toThrow(/InvalidMinerType/);
    });

    it("fails if halving interval is zero", async () => {
        await expect(
            program.methods
                .updateParameters(null, null, null, new BN(0))
                .accountsStrict({
                    authority: provider.wallet.publicKey,
                    globalState: globalStateKey,
                })
                .rpc()
        ).rejects.toThrow(/InvalidMinerType/);
    });

    it("fails if called by non-authority", async () => {
        const fakeAuthority = anchor.web3.Keypair.generate();
        await expect(
            program.methods
                .updateParameters(10, 10, new BN(10), null)
                .accountsStrict({
                    authority: fakeAuthority.publicKey,
                    globalState: globalStateKey,
                })
                .signers([fakeAuthority])
                .rpc()
        ).rejects.toThrow(/Unauthorized/);
    });
});