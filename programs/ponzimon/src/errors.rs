use anchor_lang::prelude::*;

#[error_code]
pub enum PonzimonError {
    #[msg("Wallet age is less than 7 days")]
    WalletTooNew,
    #[msg("Farm power capacity exceeded")]
    PowerCapacityExceeded,
    #[msg("Farm card capacity exceeded")]
    MachineCapacityExceeded,
    #[msg("Insufficient tokens balance")]
    InsufficientTokens,
    #[msg("Insufficient lamports")]
    InsufficientLamports,
    #[msg("Cooldown not expired")]
    CooldownNotExpired,
    #[msg("Production is disabled")]
    ProductionDisabled,
    #[msg("Invalid card type")]
    InvalidMachineType,
    #[msg("Invalid farm type")]
    InvalidFarmType,
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Initial farm already purchased")]
    InitialFarmAlreadyPurchased,
    #[msg("Invalid referrer")]
    InvalidReferrer,
    #[msg("The referrer's token account was not provided when required.")]
    ReferrerAccountMissing,
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
    #[msg("Current farm is not at maximum machine capacity for upgrade")]
    FarmNotFull,

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

    // New error variants for staked cards and booster packs
    #[msg("This card is currently staked and cannot be discarded.")]
    CardIsStaked,
    #[msg("This card is not staked.")]
    CardNotStaked,
    #[msg("This card is pending recycling and cannot be used.")]
    CardPendingRecycling,
    #[msg("Player already has a pending booster pack request.")]
    BoosterAlreadyPending,
    #[msg("Player does not have a pending booster pack to settle.")]
    NoBoosterPending,
    #[msg("Player already has a pending card recycle request.")]
    RecycleAlreadyPending,
    #[msg("Player does not have a pending card recycle to settle.")]
    NoRecyclePending,
    #[msg("Must provide between 1 and 20 cards for recycling.")]
    InvalidRecycleCardCount,
    #[msg("Duplicate card indices not allowed in recycle.")]
    DuplicateRecycleCardIndices,

    // Security-related error variants
    #[msg("Minimum delay not met for randomness commitment")]
    RandomnessDelayNotMet,
    #[msg("Invalid token account owner")]
    InvalidTokenAccountOwner,
    #[msg("Arithmetic overflow in berry calculation")]
    ArithmeticOverflow,
    #[msg("Card index out of bounds")]
    CardIndexOutOfBounds,
    #[msg("Invalid farm type for operation")]
    InvalidFarmTypeForOperation,
    #[msg("Program does not have mint authority over the token")]
    InvalidMintAuthority,

    // New error variants for staking feature
    #[msg("Amount must be greater than zero.")]
    ZeroAmount,
    #[msg("Insufficient staked amount.")]
    InsufficientStake,
    #[msg("Staked tokens are locked.")]
    StakeLocked,

    // New error variant for invalid parameter index
    #[msg("Invalid Parameter Index")]
    InvalidParameterIndex,
    #[msg("No pending action to cancel.")]
    NoPendingAction,
    #[msg("Cannot cancel yet. A timeout period is required.")]
    CancelTimeoutNotExpired,
}
