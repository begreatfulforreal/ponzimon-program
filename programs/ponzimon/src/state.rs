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
    pub initial_reward_rate: u64,   // Reward per slot at genesis
    pub current_reward_rate: u64,   // Cached reward per slot "now"
    pub acc_tokens_per_berry: u128, // 1e12-scaled accumulator
    pub last_reward_slot: u64,      // When `acc_tokens_per_berry` was last bumped

    /* ── economic params ────────────────────────── */
    pub burn_rate: u8,               // % of token cost burned (default 75)
    pub referral_fee: u8,            // % of rewards to referrer (default 25)
    pub production_enabled: bool,    // Global kill-switch
    pub cooldown_slots: u64,         // Farm upgrade cooldown
    pub dust_threshold_divisor: u64, // Divisor for total_supply to get dust_threshold (default 1000 for 0.1%)

    /* ── gameplay stats ─────────────────────────── */
    pub total_berries: u64, // Σ player berry consumption

    /* ── gambling stats ───────────────────────── */
    pub total_global_gambles: u64, // Total number of gambles across all players
    pub total_global_gamble_wins: u64, // Total number of wins across all players

    /* ── booster pack stats ──────────────────────── */
    pub total_booster_packs_opened: u64, // Total number of booster packs opened
    pub total_card_recycling_attempts: u64, // Total number of card recycling attempts
    pub total_successful_card_recycling: u64, // Total number of successful card recycling
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum PendingRandomAction {
    None,
    Gamble { amount: u64 },
    Booster,
    Recycle { indices: [u8; 10] },
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
    pub berries: u64,             // Total berry consumption by all cards
    pub referrer: Option<Pubkey>,
    pub last_acc_tokens_per_berry: u128,
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
}

impl Player {
    /// Check if a card index is pending recycling
    pub fn is_card_pending_recycling(&self, index: u8) -> bool {
        if let PendingRandomAction::Recycle { indices } = &self.pending_action {
            indices.contains(&index)
        } else {
            false
        }
    }
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
    pub power: u16, // Power level of the card for rewards (max 65535 is enough)
    pub berry_consumption: u8, // How many berries this card consumes per slot (max 255 is enough)
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CardPowerCheckpoint {
    pub slot: u64,
    pub card_power: u64, // Total card power at this checkpoint
    pub accumulated_rewards: u64,
}
