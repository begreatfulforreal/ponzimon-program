import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Ponzimon } from "../target/types/ponzimon";
import * as assert from "assert";
import {
  setupTestProgram,
  setupTestPlayer,
  createTestMint,
  createTestAccount,
} from "./test-helpers";

describe("Ponzimon Staking Limits", () => {
  let program: Program<Ponzimon>;
  let provider: anchor.AnchorProvider;
  let connection: anchor.web3.Connection;
  let mint: any;
  let authority: any;
  let globalState: anchor.web3.PublicKey;

  beforeAll(async () => {
    const setup = await setupTestProgram();
    program = setup.program;
    provider = setup.provider;
    connection = setup.connection;
    mint = setup.mint;
    authority = setup.authority;

    const [globalStatePda] = await anchor.web3.PublicKey.findProgramAddress(
      [Buffer.from("global_state"), mint.toBuffer()],
      program.programId
    );
    globalState = globalStatePda;
  });

  it("should allow staking a card with an index of 127", async () => {
    const { player, playerWallet } = await setupTestPlayer(
      program,
      provider,
      mint,
      authority,
      globalState
    );

    // Add a card at index 127, which also sets card_count to 128
    await (program.methods as any)
      .addTestCardAtIndex(127, 1) // index, card_id
      .accounts({
        player: player,
        authority: playerWallet.publicKey,
      })
      .signers([playerWallet])
      .rpc();

    const cardIndexToStake = 127;
    await program.methods
      .stakeCard(cardIndexToStake)
      .accounts({
        playerWallet: playerWallet.publicKey,
        player: player,
        globalState: globalState,
        tokenMint: mint,
      } as any)
      .signers([playerWallet])
      .rpc();

    const playerAccount = await program.account.player.fetch(player);
    const mask = BigInt(1) << BigInt(cardIndexToStake);
    assert.ok(
      (BigInt(playerAccount.stakedCardsBitset.toString()) & mask) !== BigInt(0),
      "Card 127 should be staked"
    );
  });

  it("should not allow staking a card with an index of 128 or greater", async () => {
    const { player, playerWallet } = await setupTestPlayer(
      program,
      provider,
      mint,
      authority,
      globalState
    );

    // Add a card at index 127, which also sets card_count to 128
    await (program.methods as any)
      .addTestCardAtIndex(127, 1) // index, card_id
      .accounts({
        player: player,
        authority: playerWallet.publicKey,
      })
      .signers([playerWallet])
      .rpc();

    const cardIndexToStake = 128;

    await assert.rejects(
      async () => {
        await program.methods
          .stakeCard(cardIndexToStake)
          .accounts({
            playerWallet: playerWallet.publicKey,
            player: player,
            globalState: globalState,
            tokenMint: mint,
          } as any)
          .signers([playerWallet])
          .rpc();
      },
      (err: any) => {
        assert.ok(
          err.toString().includes("CardIndexOutOfBounds"),
          `Unexpected error: ${err}`
        );
        return true;
      },
      "Staking card 128 should fail"
    );
  });
});
