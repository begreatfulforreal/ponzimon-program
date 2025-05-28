# Airdrop CLI Usage Examples

## 1. Extract Payees

Extract all SOL payees who sent at least 0.1 SOL to a given wallet before a specific slot.  
This will output a `payees.json` file in the `./airdrop/` directory.

```sh
ts-node cli/airdrop.ts extract-payees \
  --wallet <WALLET_ADDRESS> \
  --before-slot <SLOT_NUMBER> \
  --network https://api.devnet.solana.com
```

**Example:**
```sh
ts-node cli/airdrop.ts extract-payees \
  --wallet 7Gk...YourWalletAddress...9dF \
  --before-slot 250000000
```

---

## 2. Send SPL Tokens to Payees

Send SPL tokens to all payees listed in a JSON file (default: `./scripts/payees.json`).  
Requires a payer keypair and the SPL token mint address.

```sh
ts-node cli/airdrop.ts send-tokens \
  --keypair <PATH_TO_KEYPAIR_JSON> \
  --payees <PATH_TO_PAYEES_JSON> \
  --mint <MINT_ADDRESS> \
  --network https://api.devnet.solana.com
```

**Example:**
```sh
ts-node cli/airdrop.ts send-tokens \
  --keypair ~/.config/solana/devnet.json \
  --payees ./airdrop/payees.json \
  --mint BiTSZoHtcVJK42BgZ7DtvHv6RHbTFuSewdgxLB5AjHLm
```

---

## 3. Send SOL to Payees

Send native SOL to all payees listed in a JSON file (default: `./scripts/payees.json`).  
Requires a payer keypair.

```sh
ts-node cli/airdrop.ts send-sol \
  --keypair <PATH_TO_KEYPAIR_JSON> \
  --payees <PATH_TO_PAYEES_JSON> \
  --network https://api.devnet.solana.com
```

**Example:**
```sh
ts-node cli/airdrop.ts send-sol \
  --keypair ~/.config/solana/devnet.json \
  --payees ./airdrop/payees.json
```

**Options:**
- `--keypair`, `-k` (required): Path to payer keypair file.
- `--payees`, `-p` (optional): Path to payees JSON.  
  _Default:_ `./scripts/payees.json`
- `--network`, `-n` (optional): Solana network URL.  
  _Default:_ `https://api.devnet.solana.com`

---

## 4. Extract Player Snapshots

Extract all player accounts and snapshot their data from the on-chain program.  
This will output a `player_snapshots.json` file in the `./cli/extract/` directory by default, or to a custom path if specified.

```sh
ts-node cli/airdrop.ts extract-players \
  --network <NETWORK_URL> \
  --output <OUTPUT_FILE>
```

**Options:**
- `--network`, `-n` (optional): Solana network URL.  
  _Default:_ `https://api.devnet.solana.com`
- `--output`, `-o` (optional): Output file path.  
  _Default:_ `cli/extract/player_snapshots.json`

**Example:**
```sh
ts-node cli/airdrop.ts extract-players
```
or with custom output:
```sh
ts-node cli/airdrop.ts extract-players --output ./airdrop/snapshots.json
```

---

## 5. Show Help

Show the CLI help and all available commands:

```sh
ts-node cli/airdrop.ts --help
```

---

## Notes

- The `--network` option is optional for all commands and defaults to the mainnet or devnet as appropriate.
- The `--payees` and `--mint` options for `send-tokens` have defaults, but you can override them.
- The `--output` option for `extract-players` lets you specify a custom file path.
- Make sure your payer keypair has enough SOL to pay for transaction fees and token transfers.
