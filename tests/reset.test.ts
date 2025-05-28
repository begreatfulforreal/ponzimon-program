import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { PublicKey, Keypair } from "@solana/web3.js";
import { Bitsol } from "../target/types/bitsol";
import { setupFullTestPlayerAndProgram, sleepSlots } from "./test-helpers";

describe("Bitsol - reset_player instruction", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Bitsol as Program<Bitsol>;

  let tokenMint: PublicKey;
  let globalStateKey: PublicKey;
  let playerKey: PublicKey;
  let playerTokenAccount: PublicKey;
  let playerWallet: Keypair;

  const HALVING_INTERVAL = 10;
  const TOTAL_SUPPLY = new BN(1_000_000_000000);
  const INITIAL_REWARD_RATE = new BN(100_000000);
  const COOLDOWN_SLOTS = 5;

  beforeEach(async () => {
    try {
      // Setup program and get initial state
      ({ tokenMint, globalStateKey, playerWallet, playerKey, playerTokenAccount } = await setupFullTestPlayerAndProgram(
        provider,
        program,
        {
          halvingInterval: HALVING_INTERVAL,
          totalSupply: TOTAL_SUPPLY,
          initialRewardRate: INITIAL_REWARD_RATE,
          cooldownSlots: COOLDOWN_SLOTS,
        }
      ));

      console.log("playerWallet", playerWallet.publicKey.toBase58());
      console.log("playerKey", playerKey.toBase58());
      console.log("playerTokenAccount", playerTokenAccount.toBase58());
      console.log("globalStateKey", globalStateKey.toBase58());
      console.log("tokenMint", tokenMint.toBase58());

      await sleepSlots(1);
    } catch (error) {
      console.error("Setup failed:", error);
      throw error;
    }
  });

  it("resets player hashpower and facility to initial state", async () => {
    try {
      // First verify initial state
      let player = await program.account.player.fetch(playerKey);
      expect(player.hashpower.toString()).not.toBe("0");
      expect(player.facility.facilityType).toBe(0);

      // Reset the player
      await program.methods
        .resetPlayer()
        .accountsStrict({
          authority: provider.wallet.publicKey,
          globalState: globalStateKey,
          player: playerKey,
          playerWallet: playerWallet.publicKey,
          clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
        })
        .rpc();

      // Verify reset state
      player = await program.account.player.fetch(playerKey);
      expect(player.hashpower.toString()).toBe("0");
      expect(player.facility.facilityType).toBe(0);
      expect(player.facility.totalMiners).toBe(2);
      expect(player.facility.powerOutput.toString()).toBe("15");
      expect(player.miners.length).toBe(0);
    } catch (error) {
      console.error("Test failed:", error);
      throw error;
    }
  });

  it("updates global hashpower correctly", async () => {
    try {
      // Get initial global state
      let globalState = await program.account.globalState.fetch(globalStateKey);
      const initialHashpower = globalState.totalHashpower;

      // Reset the player
      await program.methods
        .resetPlayer()
        .accountsStrict({
          authority: provider.wallet.publicKey,
          globalState: globalStateKey,
          player: playerKey,
          playerWallet: playerWallet.publicKey,
          clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
        })
        .rpc();

      // Verify global hashpower was reduced
      globalState = await program.account.globalState.fetch(globalStateKey);
      expect(globalState.totalHashpower.toString()).toBe("0");
    } catch (error) {
      console.error("Test failed:", error);
      throw error;
    }
  });

  it("fails if called by non-authority", async () => {
    try {
      const fakeAuthority = anchor.web3.Keypair.generate();
      await expect(
        program.methods
          .resetPlayer()
          .accountsStrict({
            authority: fakeAuthority.publicKey,
            globalState: globalStateKey,
            player: playerKey,
            playerWallet: playerWallet.publicKey,
            clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
          })
          .signers([fakeAuthority])
          .rpc()
      ).rejects.toThrow(/Unauthorized/);
    } catch (error) {
      console.error("Test failed:", error);
      throw error;
    }
  });

  it("updates player's last claim slot and accumulator", async () => {
    try {
      // Get initial state
      let player = await program.account.player.fetch(playerKey);
      const initialLastClaimSlot = player.lastClaimSlot;
      const initialLastAccBitsPerHash = player.lastAccBitsPerHash;

      // Reset the player
      await program.methods
        .resetPlayer()
        .accountsStrict({
          authority: provider.wallet.publicKey,
          globalState: globalStateKey,
          player: playerKey,
          playerWallet: playerWallet.publicKey,
          clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
        })
        .rpc();

      // Verify updated slots and accumulator
      player = await program.account.player.fetch(playerKey);
      expect(Number(player.lastClaimSlot.toString())).toBeGreaterThan(Number(initialLastClaimSlot.toString()));
      expect(Number(player.lastAccBitsPerHash.toString())).toBeGreaterThan(
        Number(initialLastAccBitsPerHash.toString())
      );
    } catch (error) {
      console.error("Test failed:", error);
      throw error;
    }
  });
});
