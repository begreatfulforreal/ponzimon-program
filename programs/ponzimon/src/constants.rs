pub const GLOBAL_STATE_SEED: &[u8] = b"global_state";
pub const PLAYER_SEED: &[u8] = b"player";

// Fixed variables
pub const ACC_SCALE: u128 = 1_000_000_000_000; // 1e12

// Security constants
pub const MIN_RANDOMNESS_DELAY_SLOTS: u64 = 3; // Minimum slots between commit and settle
pub const MAX_CARDS_PER_PLAYER: u8 = 50; // Maximum cards a player can have
pub const MAX_FARM_TYPE: u8 = 9; // Maximum valid farm type (MASTER_ARENA)

// Farm Types (10 farms now)
pub const STARTER_HUT: u8 = 0;
pub const COZY_CABIN: u8 = 1;
pub const POKEMON_CENTER: u8 = 2;
pub const TRAINER_ACADEMY: u8 = 3;
pub const GYM_FARM: u8 = 4;
pub const POKEMON_LAB: u8 = 5;
pub const ELITE_TOWER: u8 = 6;
pub const CHAMPION_HALL: u8 = 7;
pub const LEGENDARY_SANCTUARY: u8 = 8;
pub const MASTER_ARENA: u8 = 9;

// Card Types (Pokemon cards)
pub const COMMON_CARD: u8 = 0;
pub const UNCOMMON_CARD: u8 = 1;
pub const RARE_CARD: u8 = 2;
pub const HOLO_RARE_CARD: u8 = 3;
pub const ULTRA_RARE_CARD: u8 = 4;
pub const SECRET_RARE_CARD: u8 = 5;
pub const LEGENDARY_CARD: u8 = 6;
pub const MYTHICAL_CARD: u8 = 7;
pub const PROMO_CARD: u8 = 8;
pub const FIRST_EDITION_CARD: u8 = 9;

// === Farm configurations =================================================
// format: (total_cards, berry_capacity, cost_in_microtokens)
pub const FARM_CONFIGS: [(u8, u64, u64); 10] = [
    (3, 15, 50_000_000),       // Starter Hut       –  50  tokens, 15 berry capacity
    (5, 30, 120_000_000),      // Cozy Cabin        – 120  tokens, 30 berry capacity
    (8, 60, 300_000_000),      // Pokemon Center    – 300  tokens, 60 berry capacity
    (12, 120, 600_000_000),    // Trainer Academy   – 600  tokens, 120 berry capacity
    (16, 240, 1200_000_000),   // Gym Farm          – 1200 tokens, 240 berry capacity
    (20, 480, 2400_000_000),   // Pokemon Lab       – 2400 tokens, 480 berry capacity
    (25, 960, 4800_000_000),   // Elite Tower       – 4800 tokens, 960 berry capacity
    (30, 1920, 9600_000_000),  // Champion Hall     – 9600 tokens, 1920 berry capacity
    (40, 3840, 19200_000_000), // Legendary Sanctuary – 19200 tokens, 3840 berry capacity
    (50, 7680, 38400_000_000), // Master Arena      – 38400 tokens, 7680 berry capacity
];

// === Card configurations ====================================================
// format: (card_power, berry_consumption, cost_in_microtokens)
pub const CARD_CONFIGS: [(u64, u64, u64); 10] = [
    (100, 3, 10_000_000),       // Common Card - 3 berries/slot
    (250, 5, 25_000_000),       // Uncommon Card - 5 berries/slot
    (500, 8, 50_000_000),       // Rare Card - 8 berries/slot
    (1000, 12, 100_000_000),    // Holo Rare Card - 12 berries/slot
    (2000, 20, 200_000_000),    // Ultra Rare Card - 20 berries/slot
    (4000, 32, 400_000_000),    // Secret Rare Card - 32 berries/slot
    (8000, 50, 800_000_000),    // Legendary Card - 50 berries/slot
    (16000, 80, 1600_000_000),  // Mythical Card - 80 berries/slot
    (32000, 120, 3200_000_000), // Promo Card - 120 berries/slot
    (64000, 200, 6400_000_000), // First Edition Card - 200 berries/slot
];
