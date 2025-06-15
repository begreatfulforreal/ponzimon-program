pub const GLOBAL_STATE_SEED: &[u8] = b"global_state";
pub const PLAYER_SEED: &[u8] = b"player";

// Fixed variables
pub const ACC_SCALE: u128 = 1_000_000_000_000; // 1e12

// Security constants
pub const MIN_RANDOMNESS_DELAY_SLOTS: u64 = 2; // Minimum slots between commit and settle
pub const MAX_CARDS_PER_PLAYER: u8 = 200; // Maximum cards a player can have
pub const MAX_STAKED_CARDS_PER_PLAYER: u8 = 25; // Maximum staked cards a player can have
pub const MAX_FARM_TYPE: u8 = 10; // Maximum valid farm type

// Card Rarities (matching TypeScript CardRarity enum)
pub const COMMON: u8 = 0;
pub const UNCOMMON: u8 = 1;
pub const RARE: u8 = 2;
pub const DOUBLE_RARE: u8 = 3; // Mapped from DoubleRare
pub const VERY_RARE: u8 = 4; // Mapped from VeryRare
pub const SUPER_RARE: u8 = 5; // Mapped from SuperRare
pub const MEGA_RARE: u8 = 6; // Mapped from MegaRare

// Initial starter card IDs from data.ts
pub const STARTER_CARD_IDS: [u16; 3] = [179, 175, 147]; // Glowhare, Flitterfrog, Sunnyotter

// === Farm configurations (matching farmList from data.ts) =================================================
// format: (total_cards, berry_capacity, cost_in_microtokens)
// Note: Converting costs from data.ts to microtokens (multiply by 1_000_000)
pub const FARM_CONFIGS: [(u8, u64, u64); 11] = [
    (0, 0, 0),                    // Level 0 - Initial state before buying first farm
    (2, 15, 0), // Level 1 - slotQuantity: 2, berryAvailable: 15, cost: 0 (first farm free)
    (4, 30, 100_000_000), // Level 2 - slotQuantity: 4, berryAvailable: 30, cost: 100 tokens
    (7, 60, 200_000_000), // Level 3 - slotQuantity: 7, berryAvailable: 60, cost: 200 tokens
    (10, 120, 400_000_000), // Level 4 - slotQuantity: 10, berryAvailable: 120, cost: 400 tokens
    (13, 250, 800_000_000), // Level 5 - slotQuantity: 13, berryAvailable: 250, cost: 800 tokens
    (16, 500, 1_600_000_000), // Level 6 - slotQuantity: 16, berryAvailable: 500, cost: 1600 tokens
    (19, 1_000, 3_200_000_000), // Level 7 - slotQuantity: 19, berryAvailable: 1000, cost: 3200 tokens
    (22, 2_000, 6_400_000_000), // Level 8 - slotQuantity: 22, berryAvailable: 2000, cost: 6400 tokens
    (24, 4_000, 12_800_000_000), // Level 9 - slotQuantity: 24, berryAvailable: 4000, cost: 12800 tokens
    (25, 30_000, 30_000_000_000), // Level 10 - slotQuantity: 25, berryAvailable: 30000, cost: 30000 tokens
];

// === Card data from pokemonCardList in data.ts ====================================================
// format: (id, rarity, power, berry_consumption)
// This is a comprehensive list of all 191 cards from the TypeScript data
pub const CARD_DATA: [(u16, u8, u16, u8); 191] = [
    (1, MEGA_RARE, 98, 22),   // Zephyrdrake
    (2, MEGA_RARE, 105, 24),  // Bloomingo
    (3, MEGA_RARE, 110, 25),  // Glaciowl
    (4, SUPER_RARE, 55, 13),  // Terraclaw
    (5, SUPER_RARE, 62, 15),  // Voltibra
    (6, SUPER_RARE, 48, 11),  // Aquarion
    (7, SUPER_RARE, 68, 16),  // Nocthorn
    (8, SUPER_RARE, 51, 12),  // Sylphox
    (9, SUPER_RARE, 70, 16),  // Pyroquill
    (10, VERY_RARE, 33, 8),   // Thornbuck
    (11, VERY_RARE, 38, 9),   // Emberox
    (12, VERY_RARE, 29, 7),   // Fungorilla
    (13, VERY_RARE, 40, 9),   // Gustling
    (14, VERY_RARE, 26, 6),   // Cobaltoad
    (15, VERY_RARE, 35, 8),   // Miragehare
    (16, VERY_RARE, 31, 7),   // Aquadrift
    (17, VERY_RARE, 28, 6),   // Photonix
    (18, VERY_RARE, 37, 8),   // Soniclaw
    (19, VERY_RARE, 30, 7),   // Luminpaca
    (20, VERY_RARE, 27, 6),   // Terrashock
    (21, VERY_RARE, 39, 9),   // Frostox
    (22, DOUBLE_RARE, 20, 5), // Hydropeck
    (23, DOUBLE_RARE, 24, 6), // Pyroclam
    (24, DOUBLE_RARE, 17, 4), // Vinemoth
    (25, DOUBLE_RARE, 22, 5), // Rockaroo
    (26, DOUBLE_RARE, 19, 4), // Aeropup
    (27, DOUBLE_RARE, 25, 6), // Chronoray
    (28, DOUBLE_RARE, 16, 4), // Floranox
    (29, DOUBLE_RARE, 23, 5), // Echowing
    (30, DOUBLE_RARE, 18, 4), // Quartzmite
    (31, DOUBLE_RARE, 21, 5), // Voltannut
    (32, DOUBLE_RARE, 20, 5), // Blizzear
    (33, DOUBLE_RARE, 24, 6), // Ravenguard
    (34, DOUBLE_RARE, 17, 4), // Glideon
    (35, DOUBLE_RARE, 22, 5), // Miretoad
    (36, DOUBLE_RARE, 19, 4), // Pyrolupus
    (37, DOUBLE_RARE, 25, 6), // Borealynx
    (38, DOUBLE_RARE, 16, 4), // Pyrokoala
    (39, DOUBLE_RARE, 23, 5), // Aquaphant
    (40, DOUBLE_RARE, 18, 4), // Chromacock
    (41, DOUBLE_RARE, 21, 5), // Terrashield
    (42, RARE, 13, 3),        // Gustgoat
    (43, RARE, 15, 3),        // Ignissquito
    (44, RARE, 11, 2),        // Fernbear
    (45, RARE, 14, 3),        // Shardster
    (46, RARE, 12, 2),        // Lumishark
    (47, RARE, 15, 3),        // Terrapotta
    (48, RARE, 11, 2),        // Cacteagle
    (49, RARE, 14, 3),        // Volticula
    (50, RARE, 12, 2),        // Shadewolf
    (51, RARE, 15, 3),        // Pyrotherium
    (52, RARE, 11, 2),        // Nimbusquid
    (53, RARE, 14, 3),        // Seraphowl
    (54, RARE, 12, 2),        // Auridillo
    (55, RARE, 15, 3),        // Verdantiger
    (56, RARE, 11, 2),        // Cryoweb
    (57, RARE, 14, 3),        // Heliofish
    (58, RARE, 12, 2),        // Ferrokit
    (59, RARE, 15, 3),        // Aetherhound
    (60, RARE, 11, 2),        // Magnetoise
    (61, RARE, 14, 3),        // Thornmunk
    (62, RARE, 12, 2),        // Prismaconda
    (63, RARE, 15, 3),        // Wyrmhawk
    (64, RARE, 11, 2),        // Stormbison
    (65, RARE, 14, 3),        // Solartaur
    (66, RARE, 12, 2),        // Aquashrew
    (67, RARE, 15, 3),        // Gustram
    (68, RARE, 11, 2),        // Chronocat
    (69, RARE, 14, 3),        // Spikoon
    (70, RARE, 12, 2),        // Prismoth
    (71, RARE, 15, 3),        // Froststag
    (72, UNCOMMON, 8, 2),     // Fluffleaf
    (73, UNCOMMON, 10, 2),    // Barkbat
    (74, UNCOMMON, 6, 1),     // Lichenmoose
    (75, UNCOMMON, 9, 2),     // Thornpup
    (76, UNCOMMON, 7, 1),     // Bloomlemur
    (77, UNCOMMON, 10, 2),    // Cryopus
    (78, UNCOMMON, 6, 1),     // Auroraccoon
    (79, UNCOMMON, 9, 2),     // Skinkflare
    (80, UNCOMMON, 7, 1),     // Buzzlebee
    (81, UNCOMMON, 10, 2),    // Camoskunk
    (82, UNCOMMON, 6, 1),     // Sparklion
    (83, UNCOMMON, 9, 2),     // Petalhog
    (84, UNCOMMON, 7, 1),     // Dewturtle
    (85, UNCOMMON, 10, 2),    // Frostbunny
    (86, UNCOMMON, 6, 1),     // Prismfly
    (87, UNCOMMON, 9, 2),     // Emberat
    (88, UNCOMMON, 7, 1),     // Mosskitty
    (89, UNCOMMON, 10, 2),    // Bloomink
    (90, UNCOMMON, 6, 1),     // Scorchpig
    (91, UNCOMMON, 9, 2),     // Sapossum
    (92, UNCOMMON, 7, 1),     // Cindercrow
    (93, UNCOMMON, 10, 2),    // Glowlure
    (94, UNCOMMON, 6, 1),     // Breezewren
    (95, UNCOMMON, 9, 2),     // Nutglow
    (96, UNCOMMON, 7, 1),     // Mistcub
    (97, UNCOMMON, 10, 2),    // Flarepup
    (98, UNCOMMON, 6, 1),     // Petalparrot
    (99, UNCOMMON, 9, 2),     // Aquarump
    (100, UNCOMMON, 7, 1),    // Lumisal
    (101, UNCOMMON, 10, 2),   // Sporestoat
    (102, UNCOMMON, 6, 1),    // Clinkfly
    (103, UNCOMMON, 9, 2),    // Dunesnail
    (104, UNCOMMON, 7, 1),    // Pearlcrab
    (105, UNCOMMON, 10, 2),   // Floracow
    (106, UNCOMMON, 6, 1),    // Emberloach
    (107, UNCOMMON, 9, 2),    // Circuitpup
    (108, UNCOMMON, 7, 1),    // Galestrich
    (109, UNCOMMON, 10, 2),   // Frostowl
    (110, UNCOMMON, 6, 1),    // Emberfin
    (111, UNCOMMON, 9, 2),    // Sparkmouse
    (112, UNCOMMON, 7, 1),    // Mossmoth
    (113, UNCOMMON, 10, 2),   // Orbitpup
    (114, UNCOMMON, 6, 1),    // Petalfawn
    (115, UNCOMMON, 9, 2),    // Stoneling
    (116, UNCOMMON, 7, 1),    // Glimmerfly
    (117, UNCOMMON, 10, 2),   // Gustbloom
    (118, UNCOMMON, 6, 1),    // Mossgator
    (119, UNCOMMON, 9, 2),    // Voltcobra
    (120, UNCOMMON, 7, 1),    // Lumiquill
    (121, UNCOMMON, 10, 2),   // CrystalFinch
    (122, UNCOMMON, 6, 1),    // Steamster
    (123, UNCOMMON, 9, 2),    // Fungipede
    (124, UNCOMMON, 7, 1),    // Petalcoat
    (125, UNCOMMON, 10, 2),   // Zephyrlark
    (126, UNCOMMON, 6, 1),    // Terrabunny
    (127, UNCOMMON, 9, 2),    // Starpup
    (128, UNCOMMON, 7, 1),    // Barkrat
    (129, UNCOMMON, 10, 2),   // Dewfawn
    (130, UNCOMMON, 6, 1),    // Suncurl
    (131, UNCOMMON, 9, 2),    // Sporehog
    (132, COMMON, 3, 1),      // Puffbird
    (133, COMMON, 5, 1),      // Pebbletoad
    (134, COMMON, 1, 1),      // Flutterfish
    (135, COMMON, 4, 1),      // Puddlehopper
    (136, COMMON, 2, 1),      // Bouncecrab
    (137, COMMON, 5, 1),      // Snugslug
    (138, COMMON, 1, 1),      // Wiggleworm
    (139, COMMON, 4, 1),      // Bubbletoad
    (140, COMMON, 2, 1),      // Nestbunny
    (141, COMMON, 5, 1),      // Dappleduck
    (142, COMMON, 1, 1),      // Pipsqueak
    (143, COMMON, 4, 1),      // Softsparrow
    (144, COMMON, 2, 1),      // Fluffcalf
    (145, COMMON, 5, 1),      // Whispermouse
    (146, COMMON, 1, 1),      // Bubblebat
    (147, COMMON, 4, 1),      // Sunnyotter
    (148, COMMON, 2, 1),      // Gustkoala
    (149, COMMON, 5, 1),      // Petalcrow
    (150, COMMON, 1, 1),      // Shimmerseal
    (151, COMMON, 4, 1),      // Sparkchick
    (152, COMMON, 2, 1),      // Fuzzfly
    (153, COMMON, 5, 1),      // Dewbeetle
    (154, COMMON, 1, 1),      // Glitterguppy
    (155, COMMON, 4, 1),      // Chirpfinch
    (156, COMMON, 2, 1),      // Toasturtle
    (157, COMMON, 5, 1),      // Pillowcub
    (158, COMMON, 1, 1),      // Leafrat
    (159, COMMON, 4, 1),      // Shiftsnake
    (160, COMMON, 2, 1),      // Puddlepig
    (161, COMMON, 5, 1),      // Coldbird
    (162, COMMON, 1, 1),      // Sunmoth
    (163, COMMON, 4, 1),      // Snugglepig
    (164, COMMON, 2, 1),      // Puddleclaw
    (165, COMMON, 5, 1),      // Berrybear
    (166, COMMON, 1, 1),      // Murmurfin
    (167, COMMON, 4, 1),      // Sproutmouse
    (168, COMMON, 2, 1),      // Softspider
    (169, COMMON, 5, 1),      // Petalpup
    (170, COMMON, 1, 1),      // Thawhare
    (171, COMMON, 4, 1),      // Dewdragonfly
    (172, COMMON, 2, 1),      // Galaxpup
    (173, COMMON, 5, 1),      // Drizzledove
    (174, COMMON, 1, 1),      // Twigrobin
    (175, COMMON, 4, 1),      // Flitterfrog
    (176, COMMON, 2, 1),      // Marshmink
    (177, COMMON, 5, 1),      // Pebblepup
    (178, COMMON, 1, 1),      // Tintaduck
    (179, COMMON, 4, 1),      // Glowhare
    (180, COMMON, 2, 1),      // Stargrass
    (181, COMMON, 5, 1),      // Gloamturtle
    (182, COMMON, 1, 1),      // Flickerfox
    (183, COMMON, 4, 1),      // Lullabear
    (184, COMMON, 2, 1),      // Sablechick
    (185, COMMON, 5, 1),      // Crispig
    (186, COMMON, 1, 1),      // Wispwren
    (187, COMMON, 4, 1),      // Murmurmink
    (188, COMMON, 2, 1),      // Velvetowl
    (189, COMMON, 5, 1),      // Dreamrat
    (190, COMMON, 1, 1),      // Cloudkit
    (191, COMMON, 4, 1),      // Pebblepup (duplicate of 177)
];

// Helper function to get card data by ID
pub fn get_card_by_id(id: u16) -> Option<(u8, u16, u8)> {
    CARD_DATA
        .iter()
        .find(|(card_id, _, _, _)| *card_id == id)
        .map(|(_, rarity, power, berry_consumption)| (*rarity, *power, *berry_consumption))
}
