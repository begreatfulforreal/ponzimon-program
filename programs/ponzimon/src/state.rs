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
    pub cooldown_slots: u64,         // Farm upgrade cooldown
    pub dust_threshold_divisor: u64, // Divisor for total_supply to get dust_threshold (default 1000 for 0.1%)

    /* ── gameplay stats ─────────────────────────── */
    pub total_berries: u64, // Σ player berry consumption

    /* ── gambling stats ───────────────────────── */
    pub total_global_gambles: u64, // Total number of gambles across all players
    pub total_global_gamble_wins: u64, // Total number of wins across all players
}

#[account]
pub struct Player {
    pub owner: Pubkey,
    pub farm: Farm,
    pub cards: Vec<Card>,
    pub berries: u64, // Total berry consumption by all cards
    pub referrer: Option<Pubkey>,
    pub last_acc_bits_per_hash: u128,
    pub last_claim_slot: u64,
    pub last_upgrade_slot: u64,
    pub total_rewards: u64,
    pub total_gambles: u64,     // Total number of times player has gambled
    pub total_gamble_wins: u64, // Total number of times player has won gambling

    // Switchboard randomness fields
    pub randomness_account: Pubkey, // Reference to the Switchboard randomness account
    pub commit_slot: u64,           // The slot at which the randomness was committed
    pub current_gamble_amount: u64, // Amount currently being gambled
    pub has_pending_gamble: bool,   // Whether there's a pending gamble to settle
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Farm {
    pub farm_type: u8,
    pub total_cards: u8,     // Max number of cards this farm can hold
    pub berry_capacity: u64, // Total berry capacity of this farm
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Card {
    pub card_type: u8,          // Type of Pokemon card
    pub card_power: u64,        // Power level of the card for rewards
    pub berry_consumption: u64, // How many berries this card consumes per slot
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CardPowerCheckpoint {
    pub slot: u64,
    pub card_power: u64, // Total card power at this checkpoint
    pub accumulated_rewards: u64,
}
