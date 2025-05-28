# BitSol Mining Program

### Testing

- use `anchor test` to run full suite automagically

or for debugging:

- Start a local validator using `solana-test-validator`
- Keep the validator running and then run `yarn test` to run the sdk tests
- You can inspect all addresses and tx at `https://explorer.solana.com/?cluster=custom&customUrl=http%3A%2F%2Flocalhost%3A8899`

### Deployment

#### Initialize program instance

- Devnet: `anchor deploy --provider.cluster devnet --provider.wallet ~/keypair.json`

### Command line commands

All commands are run via:

```
ts-node cli/program.ts <command> [options]
```

#### Commands

- **mint**  
  Create a new token mint and mint initial supply.
  ```
  ts-node cli/program.ts mint \
    -k <KEYPAIR.json> \
    --mint-keypair <MINT_KEYPAIR.json> \
    -a 210000 \
    -n <network-url>
  ```
  - `-k, --keypair <path>`: Path to keypair file (required)
  - `--mint-keypair <path>`: Path to mint keypair file (optional)
  - `-n, --network <url>`: Solana network URL (default: devnet)

- **initialize-program**  
  Initialize the program with an existing mint.
  ```
  ts-node cli/program.ts initialize-program \
    -k <KEYPAIR.json> \
    -m <TOKEN_MINT_ADDRESS> \
    --halving-interval <number> \
    --total-supply <number> \
    --initial-reward-rate <number> \
    --cooldown-slots <number> \
    -n <network-url>
  ```
  - `-k, --keypair <path>`: Path to keypair file (required)
  - `-m, --mint <address>`: Token mint address (required)
  - `--halving-interval <number>`: Halving interval (required)
  - `--total-supply <number>`: Total supply (required)
  - `--initial-reward-rate <number>`: Initial reward rate (required)
  - `--cooldown-slots <number>`: Cooldown slots (required)
  - `-n, --network <url>`: Solana network URL (default: devnet)

- **update-params**  
  Update program parameters (only include flags you want to update).
  ```
  ts-node cli/program.ts update-params \
    -k <KEYPAIR.json> \
    -m <TOKEN_MINT_ADDRESS> \
    [--referral-fee <number>] \
    [--burn-rate <number>] \
    [--cooldown-slots <number>] \
    [--halving-interval <number>] \
    [-n <network-url>]
  ```
  - `-k, --keypair <path>`: Path to keypair file (required)
  - `-m, --mint <address>`: Token mint address (required)
  - `--referral-fee <number>`: Referral fee (optional)
  - `--burn-rate <number>`: Burn rate (optional)
  - `--cooldown-slots <number>`: Cooldown slots (optional)
  - `--halving-interval <number>`: Halving interval (optional)
  - `-n, --network <url>`: Solana network URL (default: devnet)

- **update-pool**  
  Manually update the pool.
  ```
  ts-node cli/program.ts update-pool \
    -k <KEYPAIR.json> \
    -m <TOKEN_MINT_ADDRESS> \
    [-n <network-url>]
  ```
  - `-k, --keypair <path>`: Path to keypair file (required)
  - `-m, --mint <address>`: Token mint address (required)
  - `-n, --network <url>`: Solana network URL (default: devnet)

- **transfer-authority**  
  Transfer mint and freeze authority to global state PDA.
  ```
  ts-node cli/program.ts transfer-authority \
    -k <KEYPAIR.json> \
    -m <TOKEN_MINT_ADDRESS> \
    [-n <network-url>]
  ```
  - `-k, --keypair <path>`: Path to keypair file (required)
  - `-m, --mint <address>`: Token mint address (required)
  - `-n, --network <url>`: Solana network URL (default: devnet)

- **set-authority**  
  Set the mint authority to the global state PDA and the freeze authority to null.
  ```
  ts-node cli/program.ts set-authority \
    -k <KEYPAIR.json> \
    -m <TOKEN_MINT_ADDRESS> \
    [-n <network-url>]
  ```
  - `-k, --keypair <path>`: Path to keypair file (required)
  - `-m, --mint <address>`: Token mint address (required)
  - `-n, --network <url>`: Solana network URL (default: devnet)

- **reset-player**  
  Reset a player account.
  ```
  ts-node cli/program.ts reset-player \
    -k <KEYPAIR.json> \
    -m <TOKEN_MINT_ADDRESS> \
    -p <PLAYER_WALLET_ADDRESS> \
    [-n <network-url>]
  ```
  - `-k, --keypair <path>`: Path to keypair file (required)
  - `-m, --mint <address>`: Token mint address (required)
  - `-p, --player <address>`: Player wallet address (required)
  - `-n, --network <url>`: Solana network URL (default: devnet)

- **withdraw**  
  Withdraw both SOL and SPL tokens from the governance account.
  ```
  ts-node cli/program.ts withdraw \
    -k <KEYPAIR.json> \
    -m <TOKEN_MINT_ADDRESS> \
    --destination-token <DEST_TOKEN_ACCOUNT> \
    --destination-sol <DEST_SOL_ACCOUNT> \
    [--token-amount <AMOUNT_OF_TOKENS>] \
    [--sol-amount <AMOUNT_OF_SOL>] \
    [-n <network-url>]
  ```
  - `-k, --keypair <path>`: Path to keypair file (required)
  - `-m, --mint <address>`: Token mint address (required)
  - `--destination-token <address>`: Destination SPL token account (required)
  - `--destination-sol <address>`: Destination SOL account (required)
  - `--token-amount <number>`: Amount of SPL tokens to withdraw (in whole tokens, default: 0)
  - `--sol-amount <number>`: Amount of SOL to withdraw (in SOL, default: 0)
  - `-n, --network <url>`: Solana network URL (default: devnet)

- **list-globalstate**  
  Fetch and log all GlobalState accounts in the program, including token and SOL balances.
  ```
  ts-node cli/program.ts list-globalstate \
    -k <KEYPAIR.json> \
    [-n <network-url>]
  ```
  - `-k, --keypair <path>`: Path to keypair file (required)
  - `-n, --network <url>`: Solana network URL (default: devnet)

- **generate-global-reward**  
  Generate a global random reward (admin only).
  ```
  ts-node cli/program.ts generate-global-reward \
    -k <KEYPAIR.json> \
    -m <TOKEN_MINT_ADDRESS> \
    -a <AMOUNT> \
    -e <EXPIRY_SLOTS> \
    -n <network-url>
  ```
  - `-k, --keypair <path>`: Path to keypair file (required)
  - `-m, --mint <address>`: Token mint address (required)
  - `-a, --amount <number>`: Reward amount (in smallest units, e.g. 1000000 for 1 token if 6 decimals) (required)
  - `-e, --expiry-slots <number>`: Expiry slots (number of slots until reward expires) (required)
  - `-n, --network <url>`: Solana network URL (default: devnet)

---

You can specify a custom network with `-n <network-url>` for any command (defaults to devnet).

**Examples:**

ts-node cli/program.ts archive/atDaDk4wxwRtZTYq99A97px6wAty4WnUii3t6DAR26Q.json HuBoMcQdrqufHFL6A1VoKmwgLK5SkzxXxY2zYH1Jv7jt update-pool
ts-node cli/program.ts archive/atDaDk4wxwRtZTYq99A97px6wAty4WnUii3t6DAR26Q.json HuBoMcQdrqufHFL6A1VoKmwgLK5SkzxXxY2zYH1Jv7jt transfer-authority
