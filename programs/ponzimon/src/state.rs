use crate::constants::*;
use anchor_lang::prelude::*;

#[account]
pub struct GlobalState {
    /* ── governance ─────────────────────────────── */
    pub authority: Pubkey,   // Governance authority
    pub token_mint: Pubkey,  // Token mint
    pub fees_wallet: Pubkey, // Wallet that receives SOL and token fees

    /* ── emission mechanics ─────────────────────── */
    pub total_supply: u64,       // Hard cap (mint-burn accounting)
    pub burned_tokens: u64,      // Total tokens destroyed with `token::burn`
    pub cumulative_rewards: u64, // Total tokens ever minted as rewards
    pub start_slot: u64,         // Genesis slot
    pub halving_interval: u64,   // Slots between halvings
    pub last_processed_halvings: u64,
    pub initial_reward_rate: u64,       // Reward per slot at genesis
    pub current_reward_rate: u64,       // Cached reward per slot "now"
    pub acc_tokens_per_hashpower: u128, // 1e12-scaled accumulator (renamed from acc_tokens_per_berry)
    pub last_reward_slot: u64,          // When `acc_tokens_per_hashpower` was last bumped

    /* ── economic params ────────────────────────── */
    pub burn_rate: u8,               // % of token cost burned (default 75)
    pub referral_fee: u8,            // % of rewards to referrer (default 25)
    pub production_enabled: bool,    // Global kill-switch
    pub cooldown_slots: u64,         // Farm upgrade cooldown
    pub dust_threshold_divisor: u64, // Divisor for total_supply to get dust_threshold (default 1000 for 0.1%)

    /* ── fee configuration ──────────────────────── */
    pub initial_farm_purchase_fee_lamports: u64, // 0.3 SOL in lamports (was constant)
    pub booster_pack_cost_microtokens: u64,      // 10 tokens in microtokens (was constant)
    pub gamble_fee_lamports: u64,                // 0.1 SOL in lamports (was constant)

    /* ── gameplay stats ─────────────────────────── */
    pub total_berries: u64, // Σ player berry consumption (for capacity tracking)
    pub total_hashpower: u64, // Σ player hashpower (for reward distribution)

    /* ── gambling stats ───────────────────────── */
    pub total_global_gambles: u64, // Total number of gambles across all players
    pub total_global_gamble_wins: u64, // Total number of wins across all players

    /* ── booster pack stats ──────────────────────── */
    pub total_booster_packs_opened: u64, // Total number of booster packs opened
    pub total_card_recycling_attempts: u64, // Total number of card recycling attempts
    pub total_successful_card_recycling: u64, // Total number of successful card recycling

    /* ── staking pool ───────────────────────────── */
    pub sol_rewards_wallet: Pubkey,
    pub total_staked_tokens: u64,
    pub staking_lockup_slots: u64,
    pub acc_sol_rewards_per_token: u128, // SOL deposited per staked token (scaled by ACC_SCALE)
    pub acc_token_rewards_per_token: u128,
    pub last_staking_reward_slot: u64,
    pub token_reward_rate: u64,   // per slot
    pub total_sol_deposited: u64, // Track total SOL ever deposited for rewards
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum PendingRandomAction {
    None,
    Gamble {
        amount: u64,
    },
    Booster,
    Recycle {
        card_count: u8,
        total_hashpower: u64,
    },
}

impl Default for PendingRandomAction {
    fn default() -> Self {
        PendingRandomAction::None
    }
}

#[account]
pub struct Player {
    pub owner: Pubkey,
    pub farm: Farm,
    pub cards: [Card; MAX_CARDS_PER_PLAYER as usize], // Support up to 200 cards total
    pub card_count: u8,                               // Track actual number of cards
    pub staked_cards_bitset: u64, // Bitset tracking which cards are staked (supports up to 64 cards)
    pub berries: u64,             // Total berry consumption by staked cards (for capacity limiting)
    pub total_hashpower: u64,     // Total hashpower of staked cards (for reward calculation)
    pub referrer: Option<Pubkey>,
    pub last_acc_tokens_per_hashpower: u128, // Renamed from last_acc_tokens_per_berry
    pub last_claim_slot: u64,
    pub last_upgrade_slot: u64,
    pub total_rewards: u64,
    pub total_gambles: u64,     // Total number of times player has gambled
    pub total_gamble_wins: u64, // Total number of times player has won gambling

    // Switchboard randomness fields
    pub pending_action: PendingRandomAction,
    pub randomness_account: Pubkey, // Reference to the Switchboard randomness account
    pub commit_slot: u64,           // The slot at which the randomness was committed

    /* ── additional player stats ──────────────────────── */
    pub total_earnings_for_referrer: u64, // Total tokens this player generated for their referrer
    pub total_booster_packs_opened: u64,  // Total booster packs opened by this player
    pub total_cards_recycled: u64,        // Total cards recycled by this player
    pub successful_card_recycling: u64,   // Successful card recycling attempts
    pub total_sol_spent: u64,             // Total SOL spent by this player (in lamports)
    pub total_tokens_spent: u64,          // Total tokens spent by this player (in microtokens)

    /* ── staking stats ──────────────────────────── */
    pub staked_tokens: u64,
    pub last_stake_slot: u64,
    pub last_acc_sol_rewards_per_token: u128, // Track user's last SOL accumulator checkpoint
    pub last_acc_token_rewards_per_token: u128,
    pub claimed_token_rewards: u64,
}

impl Player {
    // The is_card_pending_recycling function is removed as cards are now removed immediately
    // in the commit step, making this check obsolete.
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Farm {
    pub farm_type: u8,
    pub total_cards: u8,     // Max number of cards this farm can hold
    pub berry_capacity: u64, // Total berry capacity of this farm
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
pub struct Card {
    pub id: u16,               // Card ID from the Pokemon card list
    pub rarity: u8, // Card rarity (0=Common, 1=Uncommon, 2=Rare, 3=VeryRare, 4=SuperRare, 5=MegaRare)
    pub hashpower: u16, // Hashpower level of the card for rewards (max 65535 is enough)
    pub berry_consumption: u8, // How many berries this card consumes per slot (max 255 is enough)
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CardHashpowerCheckpoint {
    pub slot: u64,
    pub card_hashpower: u64, // Total card hashpower at this checkpoint
    pub accumulated_rewards: u64,
}
