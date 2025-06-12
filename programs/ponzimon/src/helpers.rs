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
