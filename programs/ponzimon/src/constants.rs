pub const GLOBAL_STATE_SEED: &[u8] = b"global_state";
pub const PLAYER_SEED: &[u8] = b"player";
pub const STAKING_VAULT_SEED: &[u8] = b"staking_vault";
pub const SOL_REWARDS_WALLET_SEED: &[u8] = b"sol_rewards_wallet";

// Fixed variables
pub const ACC_SCALE: u128 = 1_000_000_000_000; // 1e12

// Security constants
pub const MIN_RANDOMNESS_DELAY_SLOTS: u64 = 2; // Minimum slots between commit and settle
pub const MAX_CARDS_PER_PLAYER: u8 = 128; // Maximum cards a player can have
pub const MAX_STAKED_CARDS_PER_PLAYER: u8 = 25; // Maximum staked cards a player can have
pub const MAX_FARM_TYPE: u8 = 8; // Maximum valid farm type

// Fee constants - MOVED TO GLOBAL STATE FOR CONFIGURABILITY
// pub const INITIAL_FARM_PURCHASE_FEE_LAMPORTS: u64 = 300_000_000; // 0.3 SOL in lamports
// pub const BOOSTER_PACK_COST_MICROTOKENS: u64 = 100_000_000; // 10 tokens in microtokens
// pub const GAMBLE_FEE_LAMPORTS: u64 = 100_000_000; // 0.1 SOL in lamports

// Card Rarities (matching TypeScript CardRarity enum)
pub const COMMON: u8 = 0;
pub const UNCOMMON: u8 = 1;
pub const RARE: u8 = 2;
pub const DOUBLE_RARE: u8 = 3; // Mapped from DoubleRare
pub const VERY_RARE: u8 = 4; // Mapped from VeryRare
pub const SUPER_RARE: u8 = 5; // Mapped from SuperRare
pub const MEGA_RARE: u8 = 6; // Mapped from MegaRare

// Initial starter card IDs
pub const STARTER_CARD_IDS: [u16; 3] = [179, 175, 147]; // Glowhare, Flitterfrog, Sunnyotter

// === Farm configurations (matching farmList from data.ts) =================================================
// format: (total_cards, berry_capacity, cost_in_microtokens)
// Note: Converting costs from data.ts to microtokens (multiply by 1_000_000)
pub const FARM_CONFIGS: [(u8, u64, u64); 11] = [
    (0, 0, 0),                  // Level 0 - Initial state before buying first farm
    (2, 6, 0), // Level 1 - slotQuantity: 2, berryAvailable: 6, cost: 0 (first farm free)
    (4, 12, 100_000_000), // Level 2 - slotQuantity: 4, berryAvailable: 12, cost: 100 tokens
    (7, 20, 200_000_000), // Level 3 - slotQuantity: 7, berryAvailable: 20, cost: 200 tokens
    (10, 40, 400_000_000), // Level 4 - slotQuantity: 10, berryAvailable: 40, cost: 400 tokens
    (13, 70, 800_000_000), // Level 5 - slotQuantity: 13, berryAvailable: 70, cost: 800 tokens
    (16, 130, 1_600_000_000), // Level 6 - slotQuantity: 16, berryAvailable: 130, cost: 1600 tokens
    (19, 230, 3_200_000_000), // Level 7 - slotQuantity: 19, berryAvailable: 230, cost: 3200 tokens
    (22, 420, 6_400_000_000), // Level 8 - slotQuantity: 22, berryAvailable: 420, cost: 6400 tokens
    (24, 780, 12_800_000_000), // Level 9 - slotQuantity: 24, berryAvailable: 780, cost: 12800 tokens
    (25, 2000, 25_600_000_000), // Level 10 - slotQuantity: 25, berryAvailable: 2000, cost: 25600 tokens
];

// === Card data from pokemonCardList in data.ts ====================================================
// format: (id, rarity, hashpower, berry_consumption)
// This is a comprehensive list of all 191 cards from the TypeScript data
pub const CARD_DATA: [(u16, u8, u16, u8); 191] = [
    (1, MEGA_RARE, 2916, 128),  // Zephyrdrake
    (2, MEGA_RARE, 2916, 128),  // Bloomingo
    (3, MEGA_RARE, 2916, 128),  // Glaciowl
    (4, SUPER_RARE, 972, 64),   // Terraclaw
    (5, SUPER_RARE, 972, 64),   // Voltibra
    (6, SUPER_RARE, 972, 64),   // Aquarion
    (7, SUPER_RARE, 972, 64),   // Nocthorn
    (8, SUPER_RARE, 972, 64),   // Sylphox
    (9, SUPER_RARE, 972, 64),   // Pyroquill
    (10, VERY_RARE, 324, 32),   // Thornbuck
    (11, VERY_RARE, 324, 32),   // Emberox
    (12, VERY_RARE, 324, 32),   // Fungorilla
    (13, VERY_RARE, 324, 32),   // Gustling
    (14, VERY_RARE, 324, 32),   // Cobaltoad
    (15, VERY_RARE, 324, 32),   // Miragehare
    (16, VERY_RARE, 324, 32),   // Aquadrift
    (17, VERY_RARE, 324, 32),   // Photonix
    (18, VERY_RARE, 324, 32),   // Soniclaw
    (19, VERY_RARE, 324, 32),   // Luminpaca
    (20, VERY_RARE, 324, 32),   // Terrashock
    (21, VERY_RARE, 324, 32),   // Frostox
    (22, DOUBLE_RARE, 108, 16), // Hydropeck
    (23, DOUBLE_RARE, 108, 16), // Pyroclam
    (24, DOUBLE_RARE, 108, 16), // Vinemoth
    (25, DOUBLE_RARE, 108, 16), // Rockaroo
    (26, DOUBLE_RARE, 108, 16), // Aeropup
    (27, DOUBLE_RARE, 108, 16), // Chronoray
    (28, DOUBLE_RARE, 108, 16), // Floranox
    (29, DOUBLE_RARE, 108, 16), // Echowing
    (30, DOUBLE_RARE, 108, 16), // Quartzmite
    (31, DOUBLE_RARE, 108, 16), // Voltannut
    (32, DOUBLE_RARE, 108, 16), // Blizzear
    (33, DOUBLE_RARE, 108, 16), // Ravenguard
    (34, DOUBLE_RARE, 108, 16), // Glideon
    (35, DOUBLE_RARE, 108, 16), // Miretoad
    (36, DOUBLE_RARE, 108, 16), // Pyrolupus
    (37, DOUBLE_RARE, 108, 16), // Borealynx
    (38, DOUBLE_RARE, 108, 16), // Pyrokoala
    (39, DOUBLE_RARE, 108, 16), // Aquaphant
    (40, DOUBLE_RARE, 108, 16), // Chromacock
    (41, DOUBLE_RARE, 108, 16), // Terrashield
    (42, RARE, 36, 8),          // Gustgoat
    (43, RARE, 36, 8),          // Ignissquito
    (44, RARE, 36, 8),          // Fernbear
    (45, RARE, 36, 8),          // Shardster
    (46, RARE, 36, 8),          // Lumishark
    (47, RARE, 36, 8),          // Terrapotta
    (48, RARE, 36, 8),          // Cacteagle
    (49, RARE, 36, 8),          // Volticula
    (50, RARE, 36, 8),          // Shadewolf
    (51, RARE, 36, 8),          // Pyrotherium
    (52, RARE, 36, 8),          // Nimbusquid
    (53, RARE, 36, 8),          // Seraphowl
    (54, RARE, 36, 8),          // Auridillo
    (55, RARE, 36, 8),          // Verdantiger
    (56, RARE, 36, 8),          // Cryoweb
    (57, RARE, 36, 8),          // Heliofish
    (58, RARE, 36, 8),          // Ferrokit
    (59, RARE, 36, 8),          // Aetherhound
    (60, RARE, 36, 8),          // Magnetoise
    (61, RARE, 36, 8),          // Thornmunk
    (62, RARE, 36, 8),          // Prismaconda
    (63, RARE, 36, 8),          // Wyrmhawk
    (64, RARE, 36, 8),          // Stormbison
    (65, RARE, 36, 8),          // Solartaur
    (66, RARE, 36, 8),          // Aquashrew
    (67, RARE, 36, 8),          // Gustram
    (68, RARE, 36, 8),          // Chronocat
    (69, RARE, 36, 8),          // Spikoon
    (70, RARE, 36, 8),          // Prismoth
    (71, RARE, 36, 8),          // Froststag
    (72, UNCOMMON, 12, 4),      // Fluffleaf
    (73, UNCOMMON, 12, 4),      // Barkbat
    (74, UNCOMMON, 12, 4),      // Lichenmoose
    (75, UNCOMMON, 12, 4),      // Thornpup
    (76, UNCOMMON, 12, 4),      // Bloomlemur
    (77, UNCOMMON, 12, 4),      // Cryopus
    (78, UNCOMMON, 12, 4),      // Auroraccoon
    (79, UNCOMMON, 12, 4),      // Skinkflare
    (80, UNCOMMON, 12, 4),      // Buzzlebee
    (81, UNCOMMON, 12, 4),      // Camoskunk
    (82, UNCOMMON, 12, 4),      // Sparklion
    (83, UNCOMMON, 12, 4),      // Petalhog
    (84, UNCOMMON, 12, 4),      // Dewturtle
    (85, UNCOMMON, 12, 4),      // Frostbunny
    (86, UNCOMMON, 12, 4),      // Prismfly
    (87, UNCOMMON, 12, 4),      // Emberat
    (88, UNCOMMON, 12, 4),      // Mosskitty
    (89, UNCOMMON, 12, 4),      // Bloomink
    (90, UNCOMMON, 12, 4),      // Scorchpig
    (91, UNCOMMON, 12, 4),      // Sapossum
    (92, UNCOMMON, 12, 4),      // Cindercrow
    (93, UNCOMMON, 12, 4),      // Glowlure
    (94, UNCOMMON, 12, 4),      // Breezewren
    (95, UNCOMMON, 12, 4),      // Nutglow
    (96, UNCOMMON, 12, 4),      // Mistcub
    (97, UNCOMMON, 12, 4),      // Flarepup
    (98, UNCOMMON, 12, 4),      // Petalparrot
    (99, UNCOMMON, 12, 4),      // Aquarump
    (100, UNCOMMON, 12, 4),     // Lumisal
    (101, UNCOMMON, 12, 4),     // Sporestoat
    (102, UNCOMMON, 12, 4),     // Clinkfly
    (103, UNCOMMON, 12, 4),     // Dunesnail
    (104, UNCOMMON, 12, 4),     // Pearlcrab
    (105, UNCOMMON, 12, 4),     // Floracow
    (106, UNCOMMON, 12, 4),     // Emberloach
    (107, UNCOMMON, 12, 4),     // Circuitpup
    (108, UNCOMMON, 12, 4),     // Galestrich
    (109, UNCOMMON, 12, 4),     // Frostowl
    (110, UNCOMMON, 12, 4),     // Emberfin
    (111, UNCOMMON, 12, 4),     // Sparkmouse
    (112, UNCOMMON, 12, 4),     // Mossmoth
    (113, UNCOMMON, 12, 4),     // Orbitpup
    (114, UNCOMMON, 12, 4),     // Petalfawn
    (115, UNCOMMON, 12, 4),     // Stoneling
    (116, UNCOMMON, 12, 4),     // Glimmerfly
    (117, UNCOMMON, 12, 4),     // Gustbloom
    (118, UNCOMMON, 12, 4),     // Mossgator
    (119, UNCOMMON, 12, 4),     // Voltcobra
    (120, UNCOMMON, 12, 4),     // Lumiquill
    (121, UNCOMMON, 12, 4),     // CrystalFinch
    (122, UNCOMMON, 12, 4),     // Steamster
    (123, UNCOMMON, 12, 4),     // Fungipede
    (124, UNCOMMON, 12, 4),     // Petalcoat
    (125, UNCOMMON, 12, 4),     // Zephyrlark
    (126, UNCOMMON, 12, 4),     // Terrabunny
    (127, UNCOMMON, 12, 4),     // Starpup
    (128, UNCOMMON, 12, 4),     // Barkrat
    (129, UNCOMMON, 12, 4),     // Dewfawn
    (130, UNCOMMON, 12, 4),     // Suncurl
    (131, UNCOMMON, 12, 4),     // Sporehog
    (132, COMMON, 4, 2),        // Puffbird
    (133, COMMON, 4, 2),        // Pebbletoad
    (134, COMMON, 4, 2),        // Flutterfish
    (135, COMMON, 4, 2),        // Puddlehopper
    (136, COMMON, 4, 2),        // Bouncecrab
    (137, COMMON, 4, 2),        // Snugslug
    (138, COMMON, 4, 2),        // Wiggleworm
    (139, COMMON, 4, 2),        // Bubbletoad
    (140, COMMON, 4, 2),        // Nestbunny
    (141, COMMON, 4, 2),        // Dappleduck
    (142, COMMON, 4, 2),        // Pipsqueak
    (143, COMMON, 4, 2),        // Softsparrow
    (144, COMMON, 4, 2),        // Fluffcalf
    (145, COMMON, 4, 2),        // Whispermouse
    (146, COMMON, 4, 2),        // Bubblebat
    (147, COMMON, 4, 2),        // Sunnyotter
    (148, COMMON, 4, 2),        // Gustkoala
    (149, COMMON, 4, 2),        // Petalcrow
    (150, COMMON, 4, 2),        // Shimmerseal
    (151, COMMON, 4, 2),        // Sparkchick
    (152, COMMON, 4, 2),        // Fuzzfly
    (153, COMMON, 4, 2),        // Dewbeetle
    (154, COMMON, 4, 2),        // Glitterguppy
    (155, COMMON, 4, 2),        // Chirpfinch
    (156, COMMON, 4, 2),        // Toasturtle
    (157, COMMON, 4, 2),        // Pillowcub
    (158, COMMON, 4, 2),        // Leafrat
    (159, COMMON, 4, 2),        // Shiftsnake
    (160, COMMON, 4, 2),        // Puddlepig
    (161, COMMON, 4, 2),        // Coldbird
    (162, COMMON, 4, 2),        // Sunmoth
    (163, COMMON, 4, 2),        // Snugglepig
    (164, COMMON, 4, 2),        // Puddleclaw
    (165, COMMON, 4, 2),        // Berrybear
    (166, COMMON, 4, 2),        // Murmurfin
    (167, COMMON, 4, 2),        // Sproutmouse
    (168, COMMON, 4, 2),        // Softspider
    (169, COMMON, 4, 2),        // Petalpup
    (170, COMMON, 4, 2),        // Thawhare
    (171, COMMON, 4, 2),        // Dewdragonfly
    (172, COMMON, 4, 2),        // Galaxpup
    (173, COMMON, 4, 2),        // Drizzledove
    (174, COMMON, 4, 2),        // Twigrobin
    (175, COMMON, 4, 2),        // Flitterfrog
    (176, COMMON, 4, 2),        // Marshmink
    (177, COMMON, 4, 2),        // Pebblepup
    (178, COMMON, 4, 2),        // Tintaduck
    (179, COMMON, 4, 2),        // Glowhare
    (180, COMMON, 4, 2),        // Stargrass
    (181, COMMON, 4, 2),        // Gloamturtle
    (182, COMMON, 4, 2),        // Flickerfox
    (183, COMMON, 4, 2),        // Lullabear
    (184, COMMON, 4, 2),        // Sablechick
    (185, COMMON, 4, 2),        // Crispig
    (186, COMMON, 4, 2),        // Wispwren
    (187, COMMON, 4, 2),        // Murmurmink
    (188, COMMON, 4, 2),        // Velvetowl
    (189, COMMON, 4, 2),        // Dreamrat
    (190, COMMON, 4, 2),        // Cloudkit
    (191, COMMON, 4, 2),        // Pebblepup (duplicate of 177)
];

// Helper function to get card data by ID
pub fn get_card_by_id(id: u16) -> Option<(u8, u16, u8)> {
    CARD_DATA
        .iter()
        .find(|(card_id, _, _, _)| *card_id == id)
        .map(|(_, rarity, hashpower, berry_consumption)| (*rarity, *hashpower, *berry_consumption))
}
