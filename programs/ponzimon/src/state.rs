use crate::constants::*;
use crate::PonzimonError;
use anchor_lang::prelude::*;

#[account]
pub struct GlobalState {
    /* ── governance ─────────────────────────────── */
    pub authority: Pubkey,   // Governance authority
    pub token_mint: Pubkey,  // Token mint
    pub fees_wallet: Pubkey, // Wallet that receives SOL and token fees

    /* ── emission mechanics ─────────────────────── */
    pub total_supply: u64,              // Hard cap (mint-burn accounting)
    pub burned_tokens: u64,             // Total tokens destroyed with `token::burn`
    pub cumulative_rewards: u64,        // Total tokens ever minted as rewards
    pub start_slot: u64,                // Genesis slot
    pub reward_rate: u64,               // Reward per slot
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

    /* ── dynamic rewards ────────────────────────── */
    pub reward_rate_multiplier: u64, // Scaled by 1000, e.g., 1000 = 1x, 100 = 0.1x, 10000 = 10x
    pub last_rate_update_slot: u64,  // The slot when the multiplier was last updated

    /* ── future expansion ───────────────────────── */
    pub padding: [u8; 64], // Reserved space for future fields
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum PendingRandomAction {
    None,
    Gamble {
        amount: u64,
    },
    Booster,
    Recycle {
        card_indices: [u8; 128], // Array of card indices to recycle
        card_count: u8,          // Number of valid indices in the array
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
    pub staked_cards_bitset: u128,                    // Changed from u64 to u128
    pub berries: u64, // Total berry consumption by staked cards (for capacity limiting)
    pub total_hashpower: u64, // Total hashpower of staked cards (for reward calculation)
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

    /* ── future expansion ───────────────────────── */
    pub padding: [u8; 64], // Reserved space for future fields
}

/// Helper functions for working with fixed-size arrays
impl Player {
    pub fn add_card(&mut self, card: Card) -> Result<()> {
        require!(
            (self.card_count as usize) < MAX_CARDS_PER_PLAYER as usize,
            PonzimonError::MachineCapacityExceeded
        );
        self.cards[self.card_count as usize] = card;
        self.card_count += 1;
        Ok(())
    }

    pub fn is_card_being_recycled(&self, card_index: u8) -> bool {
        if let PendingRandomAction::Recycle {
            card_indices,
            card_count,
        } = &self.pending_action
        {
            for i in 0..*card_count {
                if card_indices[i as usize] == card_index {
                    return true;
                }
            }
        }
        false
    }

    pub fn remove_card(&mut self, index: u8) -> Result<()> {
        let index_usize = index as usize;
        require!(
            index_usize < (self.card_count as usize),
            PonzimonError::CardIndexOutOfBounds
        );

        // Shift all cards after the removed card to fill the gap
        for i in index_usize..(self.card_count as usize - 1) {
            self.cards[i] = self.cards[i + 1];
        }

        // Update bitset - shift down any staked cards that were after the removed card
        let original_card_count = self.card_count;

        // Clear the last slot (set to default/zero values)
        self.cards[(self.card_count - 1) as usize] = Card::default();
        self.card_count -= 1;

        let mut new_bitset = 0u128;

        for i in 0..original_card_count {
            let old_mask = 1u128 << i;
            if self.staked_cards_bitset & old_mask != 0 {
                if i < index {
                    // Cards before the removed card stay in the same position
                    new_bitset |= old_mask;
                } else if i > index {
                    // Cards after the removed card shift down by 1
                    new_bitset |= 1u128 << (i - 1);
                }
                // Cards at the removed index are automatically unstaked
            }
        }

        self.staked_cards_bitset = new_bitset;

        Ok(())
    }

    pub fn stake_card(&mut self, index: u8) -> Result<()> {
        require!(index < 128, PonzimonError::CardIndexOutOfBounds);
        let mask = 1u128 << index;
        require!(
            self.staked_cards_bitset & mask == 0,
            PonzimonError::CardIsStaked
        );
        self.staked_cards_bitset |= mask;
        Ok(())
    }

    pub fn unstake_card(&mut self, index: u8) -> Result<()> {
        require!(index < 128, PonzimonError::CardIndexOutOfBounds);
        let mask = 1u128 << index;
        require!(
            self.staked_cards_bitset & mask != 0,
            PonzimonError::CardNotStaked
        );
        self.staked_cards_bitset &= !mask;
        Ok(())
    }

    pub fn is_card_staked(&self, index: u8) -> bool {
        if index >= 128 {
            return false;
        }
        (self.staked_cards_bitset & (1u128 << index)) != 0
    }

    pub fn count_staked_cards(&self) -> u8 {
        self.staked_cards_bitset.count_ones() as u8
    }

    pub fn calculate_total_berry_consumption(&self) -> u64 {
        let mut total = 0u64;
        for i in 0..self.card_count {
            if self.is_card_staked(i) {
                let card = &self.cards[i as usize];
                total += card.berry_consumption as u64;
            }
        }
        total
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
    pub hashpower: u16, // Hashpower level of the card for rewards (max 65535 is enough)
    pub berry_consumption: u8, // How many berries this card consumes per slot (max 255 is enough)
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct CardHashpowerCheckpoint {
    pub slot: u64,
    pub card_hashpower: u64, // Total card hashpower at this checkpoint
    pub accumulated_rewards: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::prelude::Pubkey;

    fn new_player() -> Player {
        Player {
            owner: Pubkey::new_unique(),
            farm: Farm {
                farm_type: 1,
                total_cards: 128,
                berry_capacity: 100,
            },
            cards: [Card::default(); MAX_CARDS_PER_PLAYER as usize],
            card_count: 0,
            staked_cards_bitset: 0,
            berries: 0,
            total_hashpower: 0,
            referrer: None,
            last_acc_tokens_per_hashpower: 0,
            last_claim_slot: 0,
            last_upgrade_slot: 0,
            total_rewards: 0,
            total_gambles: 0,
            total_gamble_wins: 0,
            pending_action: PendingRandomAction::None,
            randomness_account: Pubkey::new_unique(),
            commit_slot: 0,
            total_earnings_for_referrer: 0,
            total_booster_packs_opened: 0,
            total_cards_recycled: 0,
            successful_card_recycling: 0,
            total_sol_spent: 0,
            total_tokens_spent: 0,
            staked_tokens: 0,
            last_stake_slot: 0,
            last_acc_sol_rewards_per_token: 0,
            last_acc_token_rewards_per_token: 0,
            claimed_token_rewards: 0,
            padding: [0; 64],
        }
    }

    #[test]
    fn test_stake_unstake_card() {
        let mut player = new_player();

        // Test staking card 127
        assert!(player.stake_card(127).is_ok());
        assert_eq!(player.staked_cards_bitset, 1u128 << 127);
        assert!(player.is_card_staked(127));

        // Test staking card 128 (should fail)
        assert!(player.stake_card(128).is_err());
        assert!(!player.is_card_staked(128));

        // Test unstaking card 127
        assert!(player.unstake_card(127).is_ok());
        assert_eq!(player.staked_cards_bitset, 0);
        assert!(!player.is_card_staked(127));

        // Test unstaking already unstaked card
        assert!(player.unstake_card(127).is_err());

        // Test unstaking out of bounds card
        assert_eq!(
            player.unstake_card(128).unwrap_err(),
            error!(PonzimonError::CardIndexOutOfBounds)
        );
    }

    #[test]
    fn test_count_staked_cards() {
        let mut player = new_player();
        assert_eq!(player.count_staked_cards(), 0);

        player.stake_card(0).unwrap();
        player.stake_card(5).unwrap();
        player.stake_card(127).unwrap();
        assert_eq!(player.count_staked_cards(), 3);

        player.unstake_card(5).unwrap();
        assert_eq!(player.count_staked_cards(), 2);

        player.stake_card(5).unwrap();
        assert_eq!(player.count_staked_cards(), 3);
    }

    #[test]
    fn test_remove_card_bitset_shifting() {
        let mut player = new_player();
        player.add_card(Card::default()).unwrap();
        player.add_card(Card::default()).unwrap();
        player.add_card(Card::default()).unwrap();
        player.add_card(Card::default()).unwrap();
        player.add_card(Card::default()).unwrap();
        assert_eq!(player.card_count, 5);

        // Stake cards at indices 0, 2, 4
        player.stake_card(0).unwrap();
        player.stake_card(2).unwrap();
        player.stake_card(4).unwrap();
        // Bitset should be ...00010101
        assert_eq!(player.staked_cards_bitset, (1 << 0) | (1 << 2) | (1 << 4));
        assert_eq!(player.count_staked_cards(), 3);

        // 1. Remove an unstaked card (index 1)
        player.remove_card(1).unwrap();
        assert_eq!(player.card_count, 4);

        // Staked cards were at indices 0, 2, 4. After removing unstaked card at index 1,
        // the cards at 2 and 4 shift down to 1 and 3.
        // Staked indices should now be 0, 1, 3
        let expected_bitset = (1 << 0) | (1 << 1) | (1 << 3);
        assert_eq!(player.staked_cards_bitset, expected_bitset);

        assert!(player.is_card_staked(0)); // Was 0, still 0
        assert!(player.is_card_staked(1)); // Was 2, now 1
        assert!(!player.is_card_staked(2));
        assert!(player.is_card_staked(3)); // Was 4, now 3
        assert_eq!(player.count_staked_cards(), 3);

        // 2. Remove a staked card (new index 1, was originally index 2)
        // This unstakes and removes the card.
        player.remove_card(1).unwrap();
        assert_eq!(player.card_count, 3);

        // Staked cards were at 0, 3. After removing card at index 1,
        // the card at 3 shifts down to 2.
        // Staked indices should now be 0, 2
        let expected_bitset = (1 << 0) | (1 << 2);
        assert_eq!(player.staked_cards_bitset, expected_bitset);

        assert!(player.is_card_staked(0)); // Was 0, still 0
        assert!(!player.is_card_staked(1));
        assert!(player.is_card_staked(2)); // Was 3, now 2
        assert_eq!(player.count_staked_cards(), 2);
    }
}
