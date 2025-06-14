import * as anchor from "@coral-xyz/anchor";
import {
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
  Keypair,
  SYSVAR_CLOCK_PUBKEY,
  Transaction,
} from "@solana/web3.js";
import { Program, BN } from "@coral-xyz/anchor";
import {
  AuthorityType,
  createSetAuthorityInstruction,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import {
  mplTokenMetadata,
  createFungible,
} from "@metaplex-foundation/mpl-token-metadata";
import {
  createTokenIfMissing,
  findAssociatedTokenPda,
  getSplAssociatedTokenProgramId,
  mintTokensTo,
} from "@metaplex-foundation/mpl-toolbox";
import {
  createSignerFromKeypair,
  generateSigner,
  percentAmount,
  signerIdentity,
} from "@metaplex-foundation/umi";
import { Ponzimon } from "../target/types/ponzimon";

import fs from "fs";
import { Command } from "commander";
import { base58 } from "@metaplex-foundation/umi/serializers";

const GLOBAL_STATE_SEED = "global_state";
const GOVERNANCE_TOKEN_SEED = "governance_token";
// Add metadata configuration
const TOKEN_METADATA = {
  name: "testing",
  symbol: "test",
  uri: "https://jade-brilliant-frog-932.mypinata.cloud/ipfs/bafkreigbbqqdqkga7cf7ylf67zc2mm6mua6ndvk7q4e2gpueevcwcfpgcq",
  external_url: "https://testing.gg",
};

const TOKEN_DECIMALS = 6;

function serializeAccount(obj: any): any {
  if (obj instanceof anchor.BN || obj instanceof BN) {
    // Use toString() to avoid JS number overflow
    return obj.toString();
  }
  if (obj instanceof PublicKey) {
    return obj.toBase58();
  }
  if (Array.isArray(obj)) {
    return obj.map(serializeAccount);
  }
  if (obj && typeof obj === "object") {
    const out: any = {};
    for (const k in obj) {
      out[k] = serializeAccount(obj[k]);
    }
    return out;
  }
  return obj;
}

async function mintToken(
  keypairPath: string,
  network: string,
  mintKeypairPath?: string,
  amount: bigint = 210_000_000000n
) {
  if (!keypairPath) {
    console.error("Please provide path to keypair file as argument");
    process.exit(1);
  }

  try {
    // Load keypair from file
    const keypairData = JSON.parse(fs.readFileSync(keypairPath, "utf-8"));
    const wallet = Keypair.fromSecretKey(new Uint8Array(keypairData));

    const umi = createUmi(network);
    const authorityKey = umi.eddsa.createKeypairFromSecretKey(
      new Uint8Array(keypairData)
    );
    const updateAuthoritySigner = createSignerFromKeypair(umi, authorityKey);
    umi.use(mplTokenMetadata()).use(signerIdentity(updateAuthoritySigner));

    // Setup connection and provider
    const connection = new anchor.web3.Connection(network);
    const provider = new anchor.AnchorProvider(
      connection,
      new anchor.Wallet(wallet),
      { commitment: "confirmed" }
    );
    anchor.setProvider(provider);

    // Get program
    const program = anchor.workspace.Ponzimon as Program<Ponzimon>;

    // Create token mint - either from custom keypair or generate new one
    console.log("Creating token mint...");
    let mint;
    if (mintKeypairPath) {
      const mintKeypairData = JSON.parse(
        fs.readFileSync(mintKeypairPath, "utf-8")
      );
      const mintKeypair = umi.eddsa.createKeypairFromSecretKey(
        new Uint8Array(mintKeypairData)
      );
      mint = createSignerFromKeypair(umi, mintKeypair);
      console.log("Using custom mint keypair from:", mintKeypairPath);
    } else {
      mint = generateSigner(umi);
      console.log("Generated new mint keypair");
    }

    // Create the fungible token and mint tokens
    const createFungibleIx = createFungible(umi, {
      ...TOKEN_METADATA,
      mint,
      sellerFeeBasisPoints: percentAmount(0),
      decimals: TOKEN_DECIMALS,
    });
    const createTokenIx = createTokenIfMissing(umi, {
      mint: mint.publicKey,
      owner: umi.identity.publicKey,
      ataProgram: getSplAssociatedTokenProgramId(umi),
    });
    const mintTokensIx = mintTokensTo(umi, {
      mint: mint.publicKey,
      token: findAssociatedTokenPda(umi, {
        mint: mint.publicKey,
        owner: umi.identity.publicKey,
      }),
      amount,
    });
    const minttx = await createFungibleIx
      .add(createTokenIx)
      .add(mintTokensIx)
      .sendAndConfirm(umi);

    console.log({
      mint: mint.publicKey.toString(),
      tx: base58.deserialize(minttx.signature)[0],
    });

    const tokenMint = new PublicKey(mint.publicKey);
    // ... you can continue with further logic if needed ...
  } catch (error) {
    console.error("Error:", error);
    process.exit(1);
  }
}

async function initializeProgram(
  keypairPath: string,
  mintPubkey: string,
  feesWalletAddress: string,
  halvingIntervalArg: string,
  totalSupplyArg: string,
  initialRewardRateArg: string,
  cooldownSlotsArg: string,
  network: string
) {
  if (!keypairPath || !mintPubkey || !feesWalletAddress) {
    console.error(
      "Usage: ts-node initializeProgram.ts <KEYPAIR.json> <TOKEN_MINT> <FEES_WALLET>"
    );
    process.exit(1);
  }

  try {
    // Load keypair from file
    const keypairData = JSON.parse(fs.readFileSync(keypairPath, "utf-8"));
    const wallet = Keypair.fromSecretKey(new Uint8Array(keypairData));

    // Setup connection and provider
    const connection = new anchor.web3.Connection(network);
    const provider = new anchor.AnchorProvider(
      connection,
      new anchor.Wallet(wallet),
      { commitment: "confirmed" }
    );
    anchor.setProvider(provider);

    // Get program
    const program = anchor.workspace.Ponzimon as Program<Ponzimon>;

    // Get current slot
    const currentSlot = new BN(await provider.connection.getSlot());

    // Get the global state key and fees wallet
    const tokenMint = new PublicKey(mintPubkey);
    const feesWallet = new PublicKey(feesWalletAddress);
    const [globalStateKey] = PublicKey.findProgramAddressSync(
      [Buffer.from(GLOBAL_STATE_SEED), tokenMint.toBuffer()],
      program.programId
    );

    // Derive fees token account (Associated Token Account for fees wallet)
    const [feesTokenAccount] = PublicKey.findProgramAddressSync(
      [
        feesWallet.toBuffer(),
        TOKEN_PROGRAM_ID.toBuffer(),
        tokenMint.toBuffer(),
      ],
      ASSOCIATED_TOKEN_PROGRAM_ID
    );

    console.log("Initializing program...", totalSupplyArg);
    const HALVING_INTERVAL = new BN(halvingIntervalArg);
    const TOTAL_SUPPLY = new BN(Number(totalSupplyArg) * 10 ** TOKEN_DECIMALS);
    const INITIAL_REWARD_RATE = new BN(initialRewardRateArg);
    const COOLDOWN_SLOTS = new BN(cooldownSlotsArg);
    const tx = await program.methods
      .initializeProgram(
        currentSlot,
        HALVING_INTERVAL,
        TOTAL_SUPPLY,
        INITIAL_REWARD_RATE,
        COOLDOWN_SLOTS
      )
      .accountsStrict({
        globalState: globalStateKey,
        authority: wallet.publicKey,
        feesWallet: feesWallet,
        feesTokenAccount: feesTokenAccount,
        tokenMint: tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    console.log("Transaction signature:", tx);
    console.log("Program initialized successfully!");
    console.log("GLOBAL_STATE_KEY =", globalStateKey.toString());
    console.log("TOKEN_MINT =", tokenMint.toString());
    console.log("FEES_WALLET =", feesWallet.toString());
    console.log(
      "FEES_TOKEN_ACCOUNT =",
      feesTokenAccount.toString()
    );
  } catch (error) {
    console.error("Error:", error);
    process.exit(1);
  }
}

async function updateParameters(
  network: string,
  keypairPath: string,
  tokenMintAddress: string,
  referralFee: number | null,
  burnRate: number | null,
  cooldownSlots: number | null,
  halvingInterval: number | null,
  dustThresholdDivisor: number | null
) {
  if (!keypairPath) {
    console.error("Please provide path to keypair file as argument");
    process.exit(1);
  }

  try {
    // Load keypair from file
    const keypairData = JSON.parse(fs.readFileSync(keypairPath, "utf-8"));
    const wallet = Keypair.fromSecretKey(new Uint8Array(keypairData));

    // Setup connection and provider
    const connection = new anchor.web3.Connection(network);
    const provider = new anchor.AnchorProvider(
      connection,
      new anchor.Wallet(wallet),
      { commitment: "confirmed" }
    );
    anchor.setProvider(provider);

    // Get program
    const program = anchor.workspace.Ponzimon as Program<Ponzimon>;

    // Get the global state key - you'll need to pass in the token mint address
    const tokenMint = new PublicKey(tokenMintAddress);
    const [globalStateKey] = PublicKey.findProgramAddressSync(
      [Buffer.from(GLOBAL_STATE_SEED), tokenMint.toBuffer()],
      program.programId
    );

    console.log("Updating parameters...");
    const tx = await program.methods
      .updateParameters(
        referralFee,
        burnRate,
        cooldownSlots ? new BN(cooldownSlots) : null,
        halvingInterval ? new BN(halvingInterval) : null,
        dustThresholdDivisor ? new BN(dustThresholdDivisor) : null
      )
      .accountsStrict({
        authority: wallet.publicKey,
        globalState: globalStateKey,
      })
      .rpc();

    console.log("Transaction signature:", tx);
    console.log("Parameters updated successfully!");
    console.log({
      referralFee: referralFee !== null ? referralFee : "unchanged",
      burnRate: burnRate !== null ? burnRate : "unchanged",
      cooldownSlots: cooldownSlots !== null ? cooldownSlots : "unchanged",
      halvingInterval: halvingInterval !== null ? halvingInterval : "unchanged",
    });
  } catch (error) {
    console.error("Error:", error);
    process.exit(1);
  }
}

async function updatePoolManual(
  network: string,
  keypairPath: string,
  tokenMintAddress: string
) {
  if (!keypairPath) {
    console.error("Please provide path to keypair file as argument");
    process.exit(1);
  }

  try {
    // Load keypair from file
    const keypairData = JSON.parse(fs.readFileSync(keypairPath, "utf-8"));
    const wallet = Keypair.fromSecretKey(new Uint8Array(keypairData));

    // Setup connection and provider
    const connection = new anchor.web3.Connection(network);
    const provider = new anchor.AnchorProvider(
      connection,
      new anchor.Wallet(wallet),
      { commitment: "confirmed" }
    );
    anchor.setProvider(provider);

    // Get program
    const program = anchor.workspace.Ponzimon as Program<Ponzimon>;

    // Get the global state key - you'll need to pass in the token mint address
    const tokenMint = new PublicKey(tokenMintAddress);
    const [globalStateKey] = PublicKey.findProgramAddressSync(
      [Buffer.from(GLOBAL_STATE_SEED), tokenMint.toBuffer()],
      program.programId
    );

    console.log("Updating pool manually...");
    const tx = await program.methods
      .updatePoolManual()
      .accountsStrict({
        authority: wallet.publicKey,
        globalState: globalStateKey,
      })
      .rpc();

    console.log("Transaction signature:", tx);
    console.log("Pool updated successfully!");
  } catch (error) {
    console.error("Error:", error);
    process.exit(1);
  }
}

async function transferMintAuthority(
  network: string,
  keypairPath: string,
  mintArg: string
) {
  if (!keypairPath || !mintArg) {
    console.error(
      "Usage: ts-node transferMintAuthority.ts <KEYPAIR.json> <TOKEN_MINT>"
    );
    process.exit(1);
  }

  const keypairData = JSON.parse(fs.readFileSync(keypairPath, "utf8"));
  const wallet = Keypair.fromSecretKey(new Uint8Array(keypairData));
  const tokenMint = new PublicKey(mintArg);

  // --- 2. Anchor provider (needed only for PDA calc / compatibility) ---
  const connection = new anchor.web3.Connection(network);
  const provider = new anchor.AnchorProvider(
    connection,
    new anchor.Wallet(wallet),
    { commitment: "confirmed" }
  );
  anchor.setProvider(provider);
  const program = anchor.workspace.Ponzimon as anchor.Program<Ponzimon>;

  // --- 3. Derive the global-state PDA that should receive authority ---
  const [globalStateKey] = PublicKey.findProgramAddressSync(
    [Buffer.from(GLOBAL_STATE_SEED), tokenMint.toBuffer()],
    program.programId
  );

  try {
    // --- 4. Build & send SetAuthority tx ---
    const ixMint = createSetAuthorityInstruction(
      tokenMint, // account whose authority we're changing (the mint)
      wallet.publicKey, // current authority (must sign)
      AuthorityType.MintTokens,
      globalStateKey, // new authority
      [], // multisig signers (none)
      TOKEN_PROGRAM_ID
    );

    // Set freeze authority to null
    const ixFreeze = createSetAuthorityInstruction(
      tokenMint,
      wallet.publicKey,
      AuthorityType.FreezeAccount,
      null, // new authority is null
      [],
      TOKEN_PROGRAM_ID
    );

    const tx = new Transaction().add(ixMint).add(ixFreeze);
    const sig = await connection.sendTransaction(tx, [wallet], {
      skipPreflight: false,
      preflightCommitment: "confirmed",
    });
    await connection.confirmTransaction(sig, "confirmed");

    console.log("✅ Mint and freeze authority transferred. Tx:", sig);
  } catch (error) {
    console.error("Error:", error);
    process.exit(1);
  }
}

async function resetPlayer(
  network: string,
  keypairPath: string,
  token: string,
  player: string
) {
  if (!keypairPath) {
    console.error("Please provide path to keypair file as argument");
    process.exit(1);
  }
  if (!token) {
    console.error("Please provide token mint address as argument");
    process.exit(1);
  }
  if (!player) {
    console.error("Please provide player wallet address as argument");
    process.exit(1);
  }

  try {
    // Load keypair from file
    const keypairData = JSON.parse(fs.readFileSync(keypairPath, "utf-8"));
    const wallet = Keypair.fromSecretKey(new Uint8Array(keypairData));

    // Setup connection and provider
    const connection = new anchor.web3.Connection(network);
    const provider = new anchor.AnchorProvider(
      connection,
      new anchor.Wallet(wallet),
      { commitment: "confirmed" }
    );
    anchor.setProvider(provider);

    // Get program
    const program = anchor.workspace.Ponzimon as Program<Ponzimon>;

    // Get the global state key and player key - you'll need to pass in the token mint address and player wallet
    const tokenMint = new PublicKey(token); // Token mint should be passed as third argument
    const playerWallet = new PublicKey(player); // Player wallet should be passed as fourth argument

    const [globalStateKey] = PublicKey.findProgramAddressSync(
      [Buffer.from(GLOBAL_STATE_SEED), tokenMint.toBuffer()],
      program.programId
    );

    const [playerKey] = PublicKey.findProgramAddressSync(
      [Buffer.from("player"), playerWallet.toBuffer()],
      program.programId
    );

    console.log("Resetting player...");
    const tx = await program.methods
      .resetPlayer()
      .accountsStrict({
        authority: wallet.publicKey,
        globalState: globalStateKey,
        player: playerKey,
        playerWallet: playerWallet,
        tokenMint: tokenMint,
      })
      .rpc();

    console.log("Transaction signature:", tx);
    console.log("Player reset successfully!");
  } catch (error) {
    console.error("Error:", error);
    process.exit(1);
  }
}

/**
 * Withdraw both SOL and SPL tokens from the governance account.
 *
 * @param program - The Anchor program instance
 * @param authority - The authority's Keypair
 * @param globalStateKey - The global state PDA
 * @param governanceTokenAccount - The governance token account (SPL)
 * @param tokenMint - The SPL token mint address
 * @param destinationTokenAccount - The destination SPL token account
 * @param destinationSolAccount - The destination SOL account (system account)
 * @param withdrawTokenAmount - Amount of SPL tokens to withdraw (in smallest units)
 * @param withdrawSolLamports - Amount of SOL to withdraw (in lamports)
 */
export async function withdrawSolAndToken(
  program: any,
  authority: any,
  globalStateKey: PublicKey,
  governanceTokenAccount: PublicKey,
  tokenMint: PublicKey,
  destinationTokenAccount: PublicKey,
  destinationSolAccount: PublicKey,
  withdrawTokenAmount: number | bigint,
  withdrawSolLamports: number | bigint
) {
  // Withdraw SPL tokens
  if (withdrawTokenAmount && withdrawTokenAmount > 0) {
    await program.methods
      .withdrawFees(new BN(withdrawTokenAmount.toString()))
      .accountsStrict({
        authority: authority.publicKey,
        globalState: globalStateKey,
        governanceTokenAccount,
        destination: destinationTokenAccount,
        tokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .signers([authority])
      .rpc();
    console.log(
      `Withdrew ${withdrawTokenAmount} tokens to ${destinationTokenAccount.toBase58()}`
    );
  }

  // Withdraw SOL
  if (withdrawSolLamports && withdrawSolLamports > 0) {
    await program.methods
      .withdrawSolFees(new BN(withdrawSolLamports.toString()))
      .accountsStrict({
        authority: authority.publicKey,
        globalState: globalStateKey,
        destination: destinationSolAccount,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();
    console.log(
      `Withdrew ${withdrawSolLamports} lamports to ${destinationSolAccount.toBase58()}`
    );
  }
}

// Modify the bottom of the file to include the new command
const program = new Command();

program
  .name("ponzimon-cmd")
  .description("CLI for Ponzimon program management")
  .version("1.0.0");

program
  .command("mint")
  .description("Create a new token mint and mint initial supply")
  .requiredOption("-k, --keypair <path>", "Path to keypair file")
  .option(
    "-m, --mint-keypair <path>",
    "Path to custom mint keypair file (optional)",
    ""
  )
  .option(
    "-a, --amount <number>",
    "Amount to mint to the owner (in whole tokens, default: 210000)",
    "210000"
  )
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .action(async (opts) => {
    // Parse amount as a number and convert to smallest units (assuming 6 decimals)
    const amount = BigInt(Math.floor(Number(opts.amount) * 1_000_000));
    await mintToken(opts.keypair, opts.network, opts.mintKeypair, amount);
  });

program
  .command("initialize-program")
  .description("Initialize the program with an existing mint")
  .requiredOption("-k, --keypair <path>", "Path to keypair file")
  .requiredOption("-m, --mint <address>", "Token mint address")
  .requiredOption(
    "-f, --fees-wallet <address>",
    "Fees recipient wallet address"
  )
  .requiredOption("-h, --halving-interval <number>", "Halving interval (slots)")
  .requiredOption(
    "-t, --total-supply <number>",
    "Total supply (integer, e.g. 21000000000000)"
  )
  .requiredOption(
    "-i, --initial-reward-rate <number>",
    "Initial reward rate (integer, e.g. 50000000)"
  )
  .requiredOption("-c, --cooldown-slots <number>", "Cooldown slots")
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .action(async (opts) => {
    await initializeProgram(
      opts.keypair,
      opts.mint,
      opts.feesWallet,
      opts.halvingInterval,
      opts.totalSupply,
      opts.initialRewardRate,
      opts.cooldownSlots,
      opts.network
    );
  });

program
  .command("update-params")
  .description("Update program parameters")
  .requiredOption("-k, --keypair <path>", "Path to keypair file")
  .requiredOption("-m, --mint <address>", "Token mint address")
  .option("--referral-fee <number>", "Referral fee", null)
  .option("--burn-rate <number>", "Burn rate", null)
  .option("--cooldown-slots <number>", "Cooldown slots", null)
  .option("--halving-interval <number>", "Halving interval", null)
  .option("--dust-threshold-divisor <number>", "Dust threshold divisor", null)
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .action(async (opts) => {
    await updateParameters(
      opts.network,
      opts.keypair,
      opts.mint,
      opts.referralFee !== null ? parseInt(opts.referralFee) : null,
      opts.burnRate !== null ? parseInt(opts.burnRate) : null,
      opts.cooldownSlots !== null ? parseInt(opts.cooldownSlots) : null,
      opts.halvingInterval !== null ? parseInt(opts.halvingInterval) : null,
      opts.dustThresholdDivisor !== null
        ? parseInt(opts.dustThresholdDivisor)
        : null
    );
  });

program
  .command("update-pool")
  .description("Manually update the pool")
  .requiredOption("-k, --keypair <path>", "Path to keypair file")
  .requiredOption("-m, --mint <address>", "Token mint address")
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .action(async (opts) => {
    await updatePoolManual(opts.network, opts.keypair, opts.mint);
  });

program
  .command("transfer-authority")
  .description("Transfer mint and freeze authority to global state PDA")
  .requiredOption("-k, --keypair <path>", "Path to keypair file")
  .requiredOption("-m, --mint <address>", "Token mint address")
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .action(async (opts) => {
    await transferMintAuthority(opts.network, opts.keypair, opts.mint);
  });

program
  .command("reset-player")
  .description("Reset a player account")
  .requiredOption("-k, --keypair <path>", "Path to keypair file")
  .requiredOption("-m, --mint <address>", "Token mint address")
  .requiredOption("-p, --player <address>", "Player wallet address")
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .action(async (opts) => {
    await resetPlayer(opts.network, opts.keypair, opts.mint, opts.player);
  });

program
  .command("withdraw")
  .description("Withdraw both SOL and SPL tokens from the governance account")
  .requiredOption("-k, --keypair <path>", "Path to keypair file")
  .requiredOption("-m, --mint <address>", "Token mint address")
  .requiredOption(
    "--destination-token <address>",
    "Destination SPL token account"
  )
  .requiredOption(
    "--destination-sol <address>",
    "Destination SOL account (system account)"
  )
  .option(
    "--token-amount <number>",
    "Amount of SPL tokens to withdraw (in whole tokens)",
    "0"
  )
  .option("--sol-amount <number>", "Amount of SOL to withdraw (in SOL)", "0")
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .action(async (opts) => {
    // Load keypair
    const keypairData = JSON.parse(fs.readFileSync(opts.keypair, "utf-8"));
    const wallet = Keypair.fromSecretKey(new Uint8Array(keypairData));

    // Setup connection and provider
    const connection = new anchor.web3.Connection(opts.network);
    const provider = new anchor.AnchorProvider(
      connection,
      new anchor.Wallet(wallet),
      { commitment: "confirmed" }
    );
    anchor.setProvider(provider);

    // Get program
    const programInstance = anchor.workspace.Ponzimon as Program<Ponzimon>;

    // Derive global state and governance token account
    const tokenMint = new PublicKey(opts.mint);
    const [globalStateKey] = PublicKey.findProgramAddressSync(
      [Buffer.from(GLOBAL_STATE_SEED), tokenMint.toBuffer()],
      programInstance.programId
    );
    const [governanceTokenAccount] = PublicKey.findProgramAddressSync(
      [
        Buffer.from(GOVERNANCE_TOKEN_SEED),
        globalStateKey.toBuffer(),
        tokenMint.toBuffer(),
      ],
      programInstance.programId
    );

    // Parse destination accounts
    const destinationTokenAccount = new PublicKey(opts.destinationToken);
    const destinationSolAccount = new PublicKey(opts.destinationSol);

    // --- Fetch mint decimals ---
    const mintAccountInfo = await connection.getParsedAccountInfo(tokenMint);
    let decimals = 6; // fallback
    if (
      mintAccountInfo.value &&
      typeof mintAccountInfo.value.data === "object" &&
      mintAccountInfo.value.data !== null &&
      "program" in mintAccountInfo.value.data &&
      (mintAccountInfo.value.data as any).program === "spl-token"
    ) {
      decimals = (mintAccountInfo.value.data as any).parsed.info.decimals;
    } else if (
      mintAccountInfo.value &&
      mintAccountInfo.value.data instanceof Buffer
    ) {
      decimals = mintAccountInfo.value.data[44];
    }

    // --- Convert user input ---
    const withdrawTokenAmount = BigInt(
      Math.floor(Number(opts.tokenAmount) * 10 ** decimals)
    );
    const withdrawSolLamports = BigInt(
      Math.floor(Number(opts.solAmount) * anchor.web3.LAMPORTS_PER_SOL)
    );

    // Call the withdraw function
    await withdrawSolAndToken(
      programInstance,
      wallet,
      globalStateKey,
      governanceTokenAccount,
      tokenMint,
      destinationTokenAccount,
      destinationSolAccount,
      withdrawTokenAmount,
      withdrawSolLamports
    );
  });

program
  .command("set-authority")
  .description(
    "Set mint authority to global state PDA and freeze authority to null"
  )
  .requiredOption("-k, --keypair <path>", "Path to keypair file")
  .requiredOption("-m, --mint <address>", "Token mint address")
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .action(async (opts) => {
    const keypairData = JSON.parse(fs.readFileSync(opts.keypair, "utf8"));
    const wallet = Keypair.fromSecretKey(new Uint8Array(keypairData));
    const tokenMint = new PublicKey(opts.mint);

    // Setup connection and provider
    const connection = new anchor.web3.Connection(opts.network);
    const provider = new anchor.AnchorProvider(
      connection,
      new anchor.Wallet(wallet),
      { commitment: "confirmed" }
    );
    anchor.setProvider(provider);
    const programInstance = anchor.workspace.Ponzimon as Program<Ponzimon>;

    // Derive the global state PDA
    const [globalStateKey] = PublicKey.findProgramAddressSync(
      [Buffer.from(GLOBAL_STATE_SEED), tokenMint.toBuffer()],
      programInstance.programId
    );

    // Create instructions
    const ixMint = createSetAuthorityInstruction(
      tokenMint,
      wallet.publicKey,
      AuthorityType.MintTokens,
      globalStateKey,
      [],
      TOKEN_PROGRAM_ID
    );
    const ixFreeze = createSetAuthorityInstruction(
      tokenMint,
      wallet.publicKey,
      AuthorityType.FreezeAccount,
      null,
      [],
      TOKEN_PROGRAM_ID
    );

    // Send transaction
    const tx = new Transaction().add(ixMint).add(ixFreeze);
    const sig = await connection.sendTransaction(tx, [wallet], {
      skipPreflight: false,
      preflightCommitment: "confirmed",
    });
    await connection.confirmTransaction(sig, "confirmed");

    console.log(
      "✅ Mint authority set to global state PDA and freeze authority set to null. Tx:",
      sig
    );
  });

program
  .command("list-globalstate")
  .description(
    "Fetch and log all GlobalState accounts in the program, including token and SOL balances"
  )
  .requiredOption("-k, --keypair <path>", "Path to keypair file")
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .action(async (opts) => {
    // Load keypair
    const keypairData = JSON.parse(fs.readFileSync(opts.keypair, "utf-8"));
    const wallet = Keypair.fromSecretKey(new Uint8Array(keypairData));

    // Setup connection and provider
    const connection = new anchor.web3.Connection(opts.network);
    const provider = new anchor.AnchorProvider(
      connection,
      new anchor.Wallet(wallet),
      { commitment: "confirmed" }
    );
    anchor.setProvider(provider);

    // Get program
    const programInstance = anchor.workspace.Ponzimon as Program<Ponzimon>;

    // Fetch all GlobalState accounts
    const globalStates = await programInstance.account.globalState.all();

    console.log("GlobalState accounts:");
    for (const acc of globalStates) {
      // Derive governanceTokenAccount PDA
      const tokenMint = acc.account.tokenMint;
      const [governanceTokenAccount] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("governance_token"),
          acc.publicKey.toBuffer(),
          tokenMint.toBuffer(),
        ],
        programInstance.programId
      );

      // Derive authority's associated token account
      const authorityTokenAcc = await PublicKey.findProgramAddressSync(
        [
          wallet.publicKey.toBuffer(),
          TOKEN_PROGRAM_ID.toBuffer(),
          tokenMint.toBuffer(),
        ],
        ASSOCIATED_TOKEN_PROGRAM_ID
      );

      // Fetch SPL token balance
      let tokenBalance = 0;
      try {
        const tokenAccInfo = await connection.getTokenAccountBalance(
          governanceTokenAccount
        );
        tokenBalance = tokenAccInfo.value.uiAmount;
      } catch (e) {
        tokenBalance = NaN;
      }

      // Fetch SOL balance of the global state PDA
      let solBalance = 0;
      try {
        const solLamports = await connection.getBalance(acc.publicKey);
        solBalance = solLamports / anchor.web3.LAMPORTS_PER_SOL;
      } catch (e) {
        solBalance = NaN;
      }

      console.log({
        pubkey: acc.publicKey.toBase58(),
        authorityTokenAcc: authorityTokenAcc[0].toBase58(),
        ...serializeAccount(acc.account),
        governanceTokenAccount: governanceTokenAccount.toBase58(),
        tokenBalance,
        solBalance,
      });
    }
  });

// If no command is given, show help
if (!process.argv.slice(2).length) {
  program.outputHelp();
} else {
  program.parseAsync(process.argv);
}
