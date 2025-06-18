use crate::{constants::*, errors::PonzimonError};
use anchor_lang::prelude::*;

pub fn calculate_halvings(current_slot: u64, start_slot: u64, halving_interval: u64) -> u64 {
    current_slot.saturating_sub(start_slot) / halving_interval
}

pub fn calculate_max_halvings(initial_reward_rate: u64) -> u64 {
    if initial_reward_rate == 0 {
        return 0;
    }
    // Find position of highest set bit (effectively log2)
    64 - initial_reward_rate.leading_zeros() as u64
}

pub fn reward_after_halvings(initial: u64, halvings: u64) -> u64 {
    initial.checked_shr(halvings as u32).unwrap_or(0)
}

// Security helper functions

/// Validates that a card index is within bounds for a player's cards
pub fn validate_card_index(card_index: u8, cards_len: usize) -> Result<()> {
    require!(
        (card_index as usize) < cards_len,
        PonzimonError::CardIndexOutOfBounds
    );
    Ok(())
}

/// Validates that a farm type is within acceptable bounds
pub fn validate_farm_type(farm_type: u8) -> Result<()> {
    require!(
        farm_type <= MAX_FARM_TYPE,
        PonzimonError::InvalidFarmTypeForOperation
    );
    Ok(())
}

/// Safely adds berry consumption to total, checking for overflow
pub fn safe_add_berries(current: u64, to_add: u64) -> Result<u64> {
    current
        .checked_add(to_add)
        .ok_or(PonzimonError::ArithmeticOverflow.into())
}

/// Safely subtracts berry consumption from total, checking for underflow
pub fn safe_sub_berries(current: u64, to_sub: u64) -> Result<u64> {
    current
        .checked_sub(to_sub)
        .ok_or(PonzimonError::ArithmeticOverflow.into())
}

/// Safely adds hashpower to total, checking for overflow
pub fn safe_add_hashpower(current: u64, to_add: u64) -> Result<u64> {
    current
        .checked_add(to_add)
        .ok_or(PonzimonError::ArithmeticOverflow.into())
}

/// Safely subtracts hashpower from total, checking for underflow
pub fn safe_sub_hashpower(current: u64, to_sub: u64) -> Result<u64> {
    current
        .checked_sub(to_sub)
        .ok_or(PonzimonError::ArithmeticOverflow.into())
}

/// Validates minimum delay for randomness operations
pub fn validate_randomness_delay(commit_slot: u64, current_slot: u64) -> Result<()> {
    require!(
        current_slot > commit_slot.saturating_add(MIN_RANDOMNESS_DELAY_SLOTS),
        PonzimonError::RandomnessDelayNotMet
    );
    Ok(())
}
