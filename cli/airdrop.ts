import {
  Connection,
  PublicKey,
  ParsedTransactionWithMeta,
  Keypair,
  sendAndConfirmTransaction,
  Transaction,
  SystemProgram,
} from "@solana/web3.js";
import fs from "fs";
import {
  getOrCreateAssociatedTokenAccount,
  createTransferInstruction,
  getMint,
  AccountLayout,
} from "@solana/spl-token";
import { Command } from "commander";
import * as anchor from "@coral-xyz/anchor";
import path from "path";
import * as ponzimonIdl from "../target/idl/ponzimon.json";
import { Ponzimon } from "../target/types/ponzimon";

// Helper: sleep
function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
// Helper to load payees
function loadPayees(path: string) {
  return JSON.parse(fs.readFileSync(path, "utf-8"));
}

// Helper to ensure directory exists
function ensureDirExists(filePath: string) {
  const dir = path.dirname(filePath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
}

// --- Standardized Batch/Retry Vars ---
const BATCH_SIZE = 10;
const DELAY_BETWEEN_BATCHES = 1000;
const MAX_RETRIES = 3;
const RETRY_DELAY = 2000;

// Extract SOL payees
async function extractSolPayees(
  wallet: string,
  beforeSlot: number,
  networkUrl: string
) {
  const connection = new Connection(networkUrl, "confirmed");
  const walletPubkey = new PublicKey(wallet);

  let before: string | undefined = undefined;
  let keepGoing = true;
  const payees: Record<string, number> = {};
  let totalSignatures = 0;
  let totalTransfers = 0;

  while (keepGoing) {
    const signatures = await connection.getSignaturesForAddress(walletPubkey, {
      before,
      limit: 1000,
    });
    await sleep(0);

    if (signatures.length === 0) break;

    console.log(
      `Fetched ${signatures.length} signatures (batch), total so far: ${
        totalSignatures + signatures.length
      }`
    );

    for (const sig of signatures) {
      if (sig.slot >= beforeSlot) continue;

      let tx: ParsedTransactionWithMeta = await connection.getParsedTransaction(
        sig.signature,
        {
          commitment: "confirmed",
          maxSupportedTransactionVersion: 0,
        }
      );
      await sleep(0);

      if (!tx) continue;

      for (const ix of tx.transaction.message.instructions) {
        if (
          "parsed" in ix &&
          ix.program === "system" &&
          ix.parsed.type === "transfer" &&
          ix.parsed.info.destination === wallet
        ) {
          const from = ix.parsed.info.source;
          const amount = Number(ix.parsed.info.lamports) / 1e9;

          if (!payees[from]) {
            payees[from] = 0;
            console.log(`New payer found: ${from}`);
          }
          payees[from] += amount;
          totalTransfers++;
          console.log(`Transfer: ${amount} SOL from ${from}`);
        }
      }
      totalSignatures++;
    }

    before = signatures[signatures.length - 1].signature;
    if (signatures.length < 1000) keepGoing = false;
    console.log(
      `Processed ${totalSignatures} signatures so far, ${totalTransfers} transfers found.`
    );
  }

  const filteredPayees: Record<string, number> = {};
  for (const [key, value] of Object.entries(payees)) {
    if (value >= 0.1) {
      filteredPayees[key] = value;
    }
  }

  const outputArray = Object.entries(filteredPayees).map(([address, sol]) => ({
    address,
    sol,
    airdrop: Number(((sol / 0.0115) * 1.2).toFixed(2)),
  }));

  const outputPath = path.join(__dirname, "./airdrop/payees.json");
  ensureDirExists(outputPath);
  fs.writeFileSync(outputPath, JSON.stringify(outputArray, null, 2), "utf-8");
  console.log(`Saved ${outputArray.length} payers to ${outputPath}`);
}

// Send SPL tokens to payees
async function sendSplAirdrop(
  payerKeypairPath: string,
  mintAddress: string,
  payeesPath: string,
  networkUrl: string
) {
  const connection = new Connection(networkUrl, "confirmed");
  const payer = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(payerKeypairPath, "utf-8")))
  );
  const mint = new PublicKey(mintAddress);
  const payees = loadPayees(payeesPath);

  let successCount = 0;
  let failCount = 0;
  const failedPayees: any[] = [];
  const successPayees: any[] = [];

  // Get rent-exempt minimum for a token account
  const rent = await connection.getMinimumBalanceForRentExemption(
    AccountLayout.span
  );

  for (let i = 0; i < payees.length; i += BATCH_SIZE) {
    const batch = payees.slice(i, i + BATCH_SIZE);

    await Promise.all(
      batch.map(async ({ address, airdrop }) => {
        let recipient: PublicKey;
        try {
          recipient = new PublicKey(address);
        } catch (err) {
          // Fail silently for invalid public key
          failCount++;
          failedPayees.push({
            address,
            airdrop,
            error: "Invalid public key",
            network: networkUrl,
          });
          console.error(
            `❌ Invalid public key for address ${address}:`,
            err.message || err
          );
          return; // Skip to next payee
        }

        try {
          console.log(
            `\n---\nPreparing to send ${airdrop} tokens to ${address}`
          );

          console.log(`Getting/creating payer ATA...`);
          const payerAta = await getOrCreateAssociatedTokenAccount(
            connection,
            payer,
            mint,
            payer.publicKey
          );
          console.log(`Payer ATA: ${payerAta.address.toBase58()}`);

          console.log(`Getting/creating recipient ATA...`);
          const recipientAta = await getOrCreateAssociatedTokenAccount(
            connection,
            payer,
            mint,
            recipient
          );
          console.log(`Recipient ATA: ${recipientAta.address.toBase58()}`);
          if (recipientAta.amount === BigInt(rent)) {
            console.log(
              `Recipient ATA was created for ${address} (account was unfunded)`
            );
          }
          await sleep(3000);

          const mintInfo = await getMint(connection, mint);
          const decimals = mintInfo.decimals;
          const amount = Math.round(Number(airdrop) * 10 ** decimals);
          console.log(
            `Transferring ${amount} tokens (raw amount) to ${address} with ${decimals} decimals`
          );

          console.log(`Building transfer transaction...`);
          const tx = new Transaction().add(
            createTransferInstruction(
              payerAta.address,
              recipientAta.address,
              payer.publicKey,
              amount
            )
          );

          let attempts = 0;
          let sent = false;
          let lastError = null;

          while (attempts < MAX_RETRIES && !sent) {
            try {
              attempts++;
              console.log(`Sending transaction (attempt ${attempts})...`);
              const sig = await sendAndConfirmTransaction(connection, tx, [
                payer,
              ]);
              console.log(`✅ Sent ${airdrop} tokens to ${address}: ${sig}`);
              successCount++;
              sent = true;
              successPayees.push({ address, airdrop, signature: sig });
            } catch (err: any) {
              lastError = err;
              if (err.message?.includes("429")) {
                console.log(
                  `Rate limited, retrying in ${RETRY_DELAY}ms... (Attempt ${attempts}/${MAX_RETRIES})`
                );
                await sleep(RETRY_DELAY);
              }
              if (attempts < MAX_RETRIES) {
                await sleep(RETRY_DELAY * attempts);
              }
            }
          }
          if (!sent) {
            failCount++;
            failedPayees.push({
              address,
              airdrop,
              error: lastError?.message || String(lastError),
              network: networkUrl,
            });
          }
        } catch (err: any) {
          failCount++;
          failedPayees.push({
            address,
            airdrop,
            error: err?.message || String(err),
            network: networkUrl,
          });
        }
      })
    );

    if (i + BATCH_SIZE < payees.length) {
      console.log(`Waiting ${DELAY_BETWEEN_BATCHES}ms before next batch...`);
      await sleep(DELAY_BETWEEN_BATCHES);
    }
  }

  const failOutputPath = path.join(__dirname, "/airdrop/spl-drop-fail.json");
  ensureDirExists(failOutputPath);
  fs.writeFileSync(
    failOutputPath,
    JSON.stringify({ network: networkUrl, txs: failedPayees }, null, 2),
    "utf-8"
  );

  const successOutputPath = path.join(
    __dirname,
    "/airdrop/spl-drop-success.json"
  );
  ensureDirExists(successOutputPath);
  fs.writeFileSync(
    successOutputPath,
    JSON.stringify(
      { network: networkUrl, count: successCount, txs: successPayees },
      null,
      2
    ),
    "utf-8"
  );

  console.log(
    `\nAirdrop complete. Success: ${successCount}, Failed: ${failCount}`
  );
  if (failedPayees.length > 0) {
    console.log(`Failed sends written to ${failOutputPath}`);
  }
  if (successPayees.length > 0) {
    console.log(`Successful sends written to ${successOutputPath}`);
  }
}

// Send SOL to payees
async function sendSolAirdrop(
  payerKeypairPath: string,
  payeesPath: string,
  networkUrl: string
) {
  const connection = new Connection(networkUrl, "confirmed");
  const payer = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(payerKeypairPath, "utf-8")))
  );
  const payees = loadPayees(payeesPath);

  let successCount = 0;
  let failCount = 0;
  const failedPayees: any[] = [];
  const successPayees: any[] = [];

  for (let i = 0; i < payees.length; i += BATCH_SIZE) {
    const batch = payees.slice(i, i + BATCH_SIZE);

    await Promise.all(
      batch.map(async ({ address, sol }) => {
        let recipient: PublicKey;
        try {
          recipient = new PublicKey(address);
        } catch (err) {
          // Fail silently for invalid public key
          failCount++;
          failedPayees.push({
            address,
            sol,
            error: "Invalid public key",
            network: networkUrl,
          });
          console.error(
            `❌ Invalid public key for address ${address}:`,
            err.message || err
          );
          return; // Skip to next payee
        }
        const amountLamports = Math.round(Number(sol) * 1e9);

        console.log(`\n---\nPreparing to send ${sol} SOL to ${address}`);

        const tx = new Transaction().add(
          SystemProgram.transfer({
            fromPubkey: payer.publicKey,
            toPubkey: recipient,
            lamports: amountLamports,
          })
        );

        let attempts = 0;
        let sent = false;
        let lastError = null;

        while (attempts < MAX_RETRIES && !sent) {
          try {
            attempts++;
            console.log(`Sending transaction (attempt ${attempts})...`);
            const sig = await sendAndConfirmTransaction(connection, tx, [
              payer,
            ]);
            console.log(`✅ Sent ${sol} SOL to ${address}: ${sig}`);
            successCount++;
            sent = true;
            successPayees.push({ address, sol, signature: sig });
          } catch (err) {
            lastError = err;
            if (err.message?.includes("429")) {
              console.log(
                `Rate limited, retrying in ${RETRY_DELAY}ms... (Attempt ${attempts}/${MAX_RETRIES})`
              );
              await sleep(RETRY_DELAY);
            }
            if (attempts < MAX_RETRIES) {
              await sleep(RETRY_DELAY * attempts);
            }
          }
        }
        if (!sent) {
          failCount++;
          failedPayees.push({
            address,
            sol,
            error: lastError?.message || String(lastError),
            network: networkUrl,
          });
        }
      })
    );

    if (i + BATCH_SIZE < payees.length) {
      console.log(`Waiting ${DELAY_BETWEEN_BATCHES}ms before next batch...`);
      await sleep(DELAY_BETWEEN_BATCHES);
    }
  }

  const failOutputPath = path.join(__dirname, "/airdrop/sol_fail.json");
  ensureDirExists(failOutputPath);
  fs.writeFileSync(
    failOutputPath,
    JSON.stringify({ network: networkUrl, txs: failedPayees }, null, 2),
    "utf-8"
  );

  const successOutputPath = path.join(__dirname, "/airdrop/sol_success.json");
  ensureDirExists(successOutputPath);
  fs.writeFileSync(
    successOutputPath,
    JSON.stringify(
      { network: networkUrl, count: successCount, txs: successPayees },
      null,
      2
    ),
    "utf-8"
  );

  console.log(
    `\nSOL Airdrop complete. Success: ${successCount}, Failed: ${failCount}`
  );
  if (failedPayees.length > 0) {
    console.log(`Failed sends written to ${failOutputPath}`);
  }
  if (successPayees.length > 0) {
    console.log(`Successful sends written to ${successOutputPath}`);
  }
}

interface PlayerSnapshot {
  pubkey: string;
  tokenAcc: string;
  cardCount: number;
  farmType: number;
  berries: string;
  totalRewards: string;
}

async function processAccountWithRetry(
  program: anchor.Program<Ponzimon>,
  account: { pubkey: PublicKey },
  retryCount = 0
): Promise<PlayerSnapshot | null> {
  try {
    const playerData = await program.account.player.fetch(account.pubkey);
    if (!playerData) return null;
    return {
      pubkey: playerData.owner.toString(),
      tokenAcc: account.pubkey.toString(),
      cardCount: playerData.cards.length,
      farmType: playerData.farm.farmType,
      berries: playerData.berries.toString(),
      totalRewards: playerData.totalRewards.toString(),
    };
  } catch (error: any) {
    if (error.message?.includes("429") && retryCount < MAX_RETRIES) {
      console.log(
        `Rate limited, retrying in ${RETRY_DELAY}ms... (Attempt ${
          retryCount + 1
        }/${MAX_RETRIES})`
      );
      await sleep(RETRY_DELAY);
      return processAccountWithRetry(program, account, retryCount + 1);
    }
    console.error(
      `Error processing account ${account.pubkey.toString()}:`,
      error
    );
    return null;
  }
}

async function extractPlayers(networkUrl: string, outputFile?: string) {
  try {
    const OUTPUT_FILE =
      outputFile || path.join(__dirname, "extract/player_snapshots.json");
    console.log("Starting player account extraction...");
    console.log("Output file will be:", OUTPUT_FILE);

    const connection = new Connection(networkUrl, "confirmed");
    const provider = new anchor.AnchorProvider(connection, null, {});
    const program = new anchor.Program<Ponzimon>(
      ponzimonIdl as Ponzimon,
      provider
    );

    console.log("Fetching program accounts...");
    const accounts = await connection.getProgramAccounts(program.programId);
    console.log(`Found ${accounts.length} total program accounts`);

    const playerSnapshots: (PlayerSnapshot | null)[] = [];
    for (let i = 0; i < accounts.length; i += BATCH_SIZE) {
      const batch = accounts.slice(i, i + BATCH_SIZE);
      console.log(
        `Processing batch ${Math.floor(i / BATCH_SIZE) + 1}/${Math.ceil(
          accounts.length / BATCH_SIZE
        )}`
      );
      const batchResults = await Promise.all(
        batch.map((account) => processAccountWithRetry(program, account))
      );
      playerSnapshots.push(...batchResults);
      if (i + BATCH_SIZE < accounts.length) {
        await sleep(DELAY_BETWEEN_BATCHES);
      }
    }
    const validSnapshots = playerSnapshots.filter(
      (snapshot) => snapshot !== null
    ) as PlayerSnapshot[];

    // Collect failed pubkeys
    const failedSnapshots = playerSnapshots
      .map((snapshot, idx) =>
        snapshot === null ? accounts[idx].pubkey.toBase58() : null
      )
      .filter((pubkey): pubkey is string => pubkey !== null);

    console.log(
      `Successfully processed ${validSnapshots.length} player accounts`
    );
    if (failedSnapshots.length > 0) {
      const failFile = OUTPUT_FILE.replace(/(\.json)?$/, "_failed.json");
      ensureDirExists(failFile);
      fs.writeFileSync(
        failFile,
        JSON.stringify(
          {
            failedCount: failedSnapshots.length,
            failedPubkeys: failedSnapshots,
          },
          null,
          2
        )
      );
      console.log(
        `Failed to process ${failedSnapshots.length} accounts. See: ${failFile}`
      );
    }

    validSnapshots.sort((a, b) => b.farmType - a.farmType);

    ensureDirExists(OUTPUT_FILE);
    fs.writeFileSync(
      OUTPUT_FILE,
      JSON.stringify(
        {
          lastRefreshed: new Date().toISOString(),
          count: validSnapshots.length,
          players: validSnapshots,
        },
        null,
        2
      )
    );
    console.log(`Results written to ${OUTPUT_FILE}`);
    return validSnapshots;
  } catch (error) {
    console.error("Fatal error:", error);
    process.exit(1);
  }
}

// --- Commander CLI setup ---
const program = new Command();

program
  .name("ponzimon-airdrop")
  .description("Airdrop CLI for extracting payees and sending SPL tokens")
  .version("1.0.0");

// Extract payees command
program
  .command("extract-payees")
  .description("Extract SOL payees from a wallet before a given slot")
  .requiredOption("-w, --wallet <address>", "Wallet address to scan")
  .requiredOption(
    "-s, --before-slot <number>",
    "Only include transactions before this slot",
    parseInt
  )
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .action(async (opts) => {
    await extractSolPayees(opts.wallet, opts.beforeSlot, opts.network);
  });

// Send SPL tokens command
program
  .command("send-tokens")
  .description("Send SPL tokens to payees from a keypair")
  .requiredOption("-k, --keypair <path>", "Path to payer keypair file")
  .option("-p, --payees <path>", "Path to payees JSON", "./scripts/payees.json")
  .option(
    "-m, --mint <address>",
    "SPL token mint address",
    "BiTSZoHtcVJK42BgZ7DtvHv6RHbTFuSewdgxLB5AjHLm"
  )
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .action(async (opts) => {
    await sendSplAirdrop(opts.keypair, opts.mint, opts.payees, opts.network);
  });

// Send SOL command
program
  .command("send-sol")
  .description("Send SOL to payees from a keypair")
  .requiredOption("-k, --keypair <path>", "Path to payer keypair file")
  .option(
    "-p, --payees <path>",
    "Path to payees JSON",
    "./cli/airdrop/payees.json"
  )
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .action(async (opts) => {
    await sendSolAirdrop(opts.keypair, opts.payees, opts.network);
  });

// Extract players command
program
  .command("extract-players")
  .description(
    "Extract ponzimon program player accounts and snapshot their data"
  )
  .option(
    "-n, --network <url>",
    "Solana network URL",
    "https://api.devnet.solana.com"
  )
  .option("-o, --output <path>", "Output file path")
  .action(async (opts) => {
    await extractPlayers(opts.network, opts.output);
  });

// Show help if no command is given
if (!process.argv.slice(2).length) {
  program.outputHelp();
} else {
  program.parseAsync(process.argv);
}
