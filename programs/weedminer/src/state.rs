use anchor_lang::prelude::*;

#[account]
pub struct GlobalState {
    /* ── governance ─────────────────────────────── */
    pub authority: Pubkey,   // Governance authority
    pub token_mint: Pubkey,  // BITS token mint
    pub fees_wallet: Pubkey, // Wallet that receives SOL and token fees

    /* ── emission mechanics ─────────────────────── */
    pub total_supply: u64,       // Hard cap (mint-burn accounting)
    pub burned_tokens: u64,      // Total tokens destroyed with `token::burn`
    pub cumulative_rewards: u64, // Total tokens ever minted as rewards
    pub start_slot: u64,         // Genesis slot
    pub halving_interval: u64,   // Slots between halvings
    pub last_processed_halvings: u64,
    pub initial_reward_rate: u64, // Reward per slot at genesis
    pub current_reward_rate: u64, // Cached reward per slot "now"
    pub acc_bits_per_hash: u128,  // 1e12-scaled accumulator
    pub last_reward_slot: u64,    // When `acc_bits_per_hash` was last bumped

    /* ── economic params ────────────────────────── */
    pub burn_rate: u8,               // % of BITS cost burned (default 75)
    pub referral_fee: u8,            // ‰ (per-mille) paid to referrer (default 25 => 2.5 %)
    pub production_enabled: bool,    // Global kill-switch
    pub cooldown_slots: u64,         // Facility upgrade cooldown
    pub dust_threshold_divisor: u64, // Divisor for total_supply to get dust_threshold (default 1000 for 0.1%)

    /* ── gameplay stats ─────────────────────────── */
    pub total_hashpower: u64, // Σ player hash-rate

    /* ── global random rewards ─────────────────── */
    pub global_random_reward: Option<GlobalRandomReward>,
    pub global_reward_counter: u64, // Increments each time admin creates a new reward

    /* ── gambling stats ───────────────────────── */
    pub total_global_gambles: u64, // Total number of gambles across all players
    pub total_global_gamble_wins: u64, // Total number of wins across all players
}

#[account]
pub struct Player {
    pub owner: Pubkey,
    pub facility: Facility,
    pub machines: Vec<Machine>,
    pub hashpower: u64,
    pub referrer: Option<Pubkey>,
    pub last_acc_bits_per_hash: u128,
    pub last_claim_slot: u64,
    pub last_upgrade_slot: u64,
    pub total_rewards: u64,
    pub last_claimed_global_reward_id: u64, // ID of the last global reward claimed by this player
    pub total_gambles: u64,                 // Total number of times player has gambled
    pub total_gamble_wins: u64,             // Total number of times player has won gambling
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Facility {
    pub facility_type: u8,
    pub total_machines: u8,
    pub power_output: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Machine {
    pub machine_type: u8,
    pub hashrate: u64,
    pub power_consumption: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct HashpowerCheckpoint {
    pub slot: u64,
    pub hashpower: u64,
    pub accumulated_rewards: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct GlobalRandomReward {
    pub reward_id: u64, // Unique ID for this reward (from global_reward_counter)
    pub amount: u64,
    pub generated_slot: u64,
    pub expiry_slot: u64,
}
