// Map generation
pub const MAP_WIDTH: u16 = 80;
pub const MAP_HEIGHT: u16 = 24;
pub const MAP_SEED: u64 = 42;
pub const MAIN_CLUSTER_SIZE: usize = 12;
pub const SCATTER_PER_RESOURCE: usize = 4;
pub const MIN_CLUSTER_DIST_DIVISOR: usize = 4; // min distance = (w + h) / this
pub const CAPITAL_RESOURCE_DIST: usize = 4; // base distance from capital center to its resource cluster
pub const CAPITAL_DIST_VARIANCE: usize = 1; // +/- random variance on that distance

// Territory
pub const CAPITAL_TERRITORY_RADIUS: usize = 2;

// Units
pub const NPCS_PER_FACTION: usize = 3;
pub const CAPITAL_MIN_OPEN_SIDES: usize = 3;

// Economy
pub const STARTING_STOCKPILE: u32 = 10;
pub const STARTING_CROWNS: u32 = 100;
pub const MAX_STOCKPILE: u32 = 20;
pub const DECAY_INTERVAL_MS: u64 = 30_000; // resource decay every 30 seconds
pub const DECAY_WATER: bool = true;
pub const DECAY_FUEL: bool = true;
pub const DECAY_SCRAP: bool = false; // scrap used for claiming tiles, not decayed
pub const DEHYDRATION_INTERVAL_MS: u64 = 30_000; // lose NPC every 30s when water is 0

// Fuel speed bonuses: NPC cooldown reduced by this % at each fuel threshold
// [fuel >= 5, >= 10, >= 15, >= 20] — each tier stacks
pub const FUEL_THRESHOLDS: [u32; 4] = [5, 10, 15, 20];
pub const FUEL_SPEED_BONUS_PCT: u32 = 10; // % faster per threshold reached
// NPC movement cooldown by carry weight (items carried). Indexed like the
// player's MOVE_COOLDOWN — heavier NPCs move slower. Fuel bonuses reduce
// whichever entry applies by a percentage.
pub const NPC_MOVE_COOLDOWN: [u64; 6] = [400, 475, 550, 625, 700, 775];

// NPC behavior
/// Once a home capital's stockpile of a resource reaches this amount,
/// NPCs stop harvesting it (future: start spending it on walls, trade, etc.).
pub const MAX_HOARD_BEFORE_USE: u32 = 15;

// Display
// Terminal theme — set these to match your terminal's color scheme
// Entity glyphs use FG when off territory, BG when on colored territory
pub const TERMINAL_FG: ratatui::style::Color = ratatui::style::Color::Rgb(0xfc, 0xfc, 0xfa);     // #fcfcfa
pub const TERMINAL_BG: ratatui::style::Color = ratatui::style::Color::Rgb(0x40, 0x3e, 0x41);     // #403e41
pub const TERMINAL_GRAY: ratatui::style::Color = ratatui::style::Color::Rgb(0xc1, 0xc0, 0xc0);   // #c1c0c0
pub const TERMINAL_DARK_BG: ratatui::style::Color = ratatui::style::Color::Rgb(0x2d, 0x2a, 0x2e); // #2d2a2e
pub const TERMINAL_LIGHT_BG: ratatui::style::Color = ratatui::style::Color::Rgb(0x5b, 0x59, 0x5c); // #5b595c
pub const TERMINAL_PURPLE: ratatui::style::Color = ratatui::style::Color::Rgb(0xab, 0x9d, 0xf2); // #ab9df2 (cult)

// Animation
pub const ANIM_TICK_MS: u64 = 800; // how often water ripples cycle

// Player
pub const EXTRACT_TIME_MS: u64 = 3000;
pub const CLAIM_TIME_MS: u64 = 3000;
pub const CLAIM_CONTESTED_MULTIPLIER: f32 = 1.5; // claiming another faction's tile takes this much longer
pub const CLAIM_SCRAP_COST: u32 = 1;
pub const PLAYER_STARTING_SCRAP: u32 = 3;

// City founding
pub const FOUND_CITY_TIME_MS: u64 = 8000;
pub const FOUNDATION_SCRAP_COST: u32 = 5;  // scrap to lay the foundation
pub const CITY_TOTAL_SCRAP: u32 = 10;      // total scrap needed for a complete city
// Time per scrap added after foundation: spreads the remaining build time evenly
pub const BUILD_SCRAP_TIME_MS: u64 =
    FOUND_CITY_TIME_MS / ((CITY_TOTAL_SCRAP - FOUNDATION_SCRAP_COST) as u64);

// Camp founding (+ shape, cheap, fast)
pub const FOUND_CAMP_TIME_MS: u64 = 3000;
pub const CAMP_SCRAP_COST: u32 = 5;

// Free-standing walls
pub const WALL_SCRAP_COST: u32 = 2;
pub const BUILD_WALL_TIME_MS: u64 = 2000;
// Multiplier applied when building on unclaimed tiles or another faction's territory
pub const WALL_UNCLAIMED_MULTIPLIER: f32 = 3.0;

// Trade pricing
pub const BASE_SELL_PRICE: u32 = 5;  // crowns per resource sold to capital
pub const BASE_BUY_PRICE: u32 = 8;  // crowns per resource bought from capital
pub const CARRY_CAP: u32 = 5;
pub const EXTRACT_BAR_WIDTH: usize = 24;

// Movement speed (ms) by carry weight: index = items carried
pub const MOVE_COOLDOWN: [u64; 6] = [100, 150, 200, 250, 300, 350];

// Player controls — change these to rebind any action
pub const KEY_QUIT:         char = 'q';
pub const KEY_MOVE_UP:      char = 'w';
pub const KEY_MOVE_DOWN:    char = 's';
pub const KEY_MOVE_LEFT:    char = 'a';
pub const KEY_MOVE_RIGHT:   char = 'd';
pub const KEY_EXTRACT:      char = 'e';
pub const KEY_CLAIM:        char = 'f';
pub const KEY_CITY:         char = 'c';
pub const KEY_CAMP:         char = 'v';
pub const KEY_WALL:         char = 'r';
// Trade: 1/2/3 sell water/fuel/scrap; Shift+1/2/3 buy (terminals send !/@/#)
pub const KEY_SELL_WATER:   char = '1';
pub const KEY_SELL_FUEL:    char = '2';
pub const KEY_SELL_SCRAP:   char = '3';
pub const KEY_BUY_WATER:    char = '!';
pub const KEY_BUY_FUEL:     char = '@';
pub const KEY_BUY_SCRAP:    char = '#';
