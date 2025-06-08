use anchor_lang::prelude::*;

#[error_code]
pub enum WeedMinerError {
    #[msg("Wallet age is less than 7 days")]
    WalletTooNew,
    #[msg("Facility power capacity exceeded")]
    PowerCapacityExceeded,
    #[msg("Facility machine capacity exceeded")]
    MachineCapacityExceeded,
    #[msg("Insufficient $WEED balance")]
    InsufficientBits,
    #[msg("Insufficient lamports")]
    InsufficientLamports,
    #[msg("Cooldown not expired")]
    CooldownNotExpired,
    #[msg("Production is disabled")]
    ProductionDisabled,
    #[msg("Invalid machine type")]
    InvalidMachineType,
    #[msg("Invalid facility type")]
    InvalidFacilityType,
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Initial facility already purchased")]
    InitialFacilityAlreadyPurchased,
    #[msg("Invalid referrer")]
    InvalidReferrer,
    #[msg("Invalid token mint")]
    InvalidTokenMint,
    #[msg("New wallet restricted")]
    NewWalletRestricted,
    #[msg("No pending reward")]
    NoPendingReward,
    #[msg("Reward already claimed")]
    RewardAlreadyClaimed,
    #[msg("Reward expired")]
    RewardExpired,
    #[msg("Self-referral is not allowed")]
    SelfReferralNotAllowed,
    #[msg("Invalid referral fee, must be between 0 and 50 (5.0%)")]
    InvalidReferralFee,
    #[msg("Invalid burn rate, must be between 0 and 100")]
    InvalidBurnRate,
    #[msg("Invalid cooldown slots, must be > 0")]
    InvalidCooldownSlots,
    #[msg("Invalid halving interval, must be > 0")]
    InvalidHalvingInterval,
    #[msg("Invalid dust threshold divisor, must be > 0")]
    InvalidDustThresholdDivisor,
    #[msg("Current facility is not at maximum machine capacity for upgrade")]
    FacilityNotFull,

    // Switchboard randomness errors
    #[msg("Randomness already revealed")]
    RandomnessAlreadyRevealed,
    #[msg("Randomness not resolved")]
    RandomnessNotResolved,
    #[msg("Randomness expired")]
    RandomnessExpired,
    #[msg("Invalid randomness account")]
    InvalidRandomnessAccount,
    #[msg("No pending gamble to settle")]
    NoPendingGamble,
    #[msg("Player already has a pending gamble")]
    AlreadyHasPendingGamble,
}
