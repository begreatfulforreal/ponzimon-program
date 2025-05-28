pub const GLOBAL_STATE_SEED: &[u8] = b"global_state";
pub const PLAYER_SEED: &[u8] = b"player";

// Fixed variables
pub const ACC_SCALE: u128 = 1_000_000_000_000; // 1e12

// Facility Types
pub const CRAMPED_BEDROOM: u8 = 0;
pub const LOW_PROFILE_STORAGE: u8 = 1;
pub const HIDDEN_POWERHOUSE: u8 = 2;
pub const CUSTOM_GARAGE: u8 = 3;
pub const HIGH_RISE_APARTMENT: u8 = 4;

// Machine Types
pub const TOASTER: u8 = 0;
pub const RASPBERRY_PI: u8 = 1;
pub const NOTEBOOK: u8 = 2;
pub const GAMER_RIG: u8 = 3;
pub const GPU_RACK: u8 = 4;
pub const ASIC_SOLO: u8 = 5;
pub const ASIC_RACK: u8 = 6;
pub const HYDRO_FARM: u8 = 7;
pub const TERRA_MACHINE: u8 = 8;
pub const QUANTUM_CLUSTER: u8 = 9;

// === Facility configurations =================================================
// format: (total_machines, power_output, cost_in_microBITS)
pub const FACILITY_CONFIGS: [(u8, u64, u64); 5] = [
    (2, 15, 80_000000),      // Cramped Bedroom   –  80  BITS
    (4, 60, 240_000000),     // Low Profile Store – 240  BITS
    (6, 200, 720_000000),    // Hidden Powerhouse – 720  BITS
    (9, 600, 1800_000000),   // Custom Garage     – 1 800 BITS
    (12, 2000, 4800_000000), // High‑rise Apt.    – 4 800 BITS
];

// === Machine configurations ====================================================
// format: (hashrate, power_consumption, cost_in_microBITS)
pub const MACHINE_CONFIGS: [(u64, u64, u64); 10] = [
    (1_500, 3, 40_000000),            // Toaster
    (6_000, 6, 120_000000),           // Raspberry Pi
    (25_000, 15, 350_000000),         // Notebook
    (60_000, 30, 700_000000),         // Gamer Rig
    (150_000, 60, 1_300_000000),      // GPU Rack
    (400_000, 120, 2_500_000000),     // ASIC Solo
    (800_000, 200, 5_000_000000),     // ASIC Rack
    (1_500_000, 400, 9_000_000000),   // Hydro Farm
    (3_500_000, 800, 18_000_000000),  // Terra Machine
    (6_000_000, 1500, 40_000_000000), // Quantum Cluster
];
