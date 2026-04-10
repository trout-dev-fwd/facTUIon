# Cross-references

One line per directional dependency. Use this as a fast map of "who talks to whom" before diving into any companion file.

## main.rs
- → types/state.rs: constructs `GameState::new`, calls per-frame `update_*` methods
- → types/actions.rs: calls every player-initiated `check_*`, `start_*`, `sell_resource`, `buy_resource`, etc.
- → render.rs: calls `render::render(f, &state)` each frame
- → config.rs: reads `MAP_*`, `KEY_*` constants for setup and input mapping

## render.rs
- → types (via `crate::types::*`): reads `GameState` (map, capitals, npcs, player, action states) and calls query methods (`adjacent_capital_idx`, `population_of`, `capital_border_at`, `is_box_wall`, `wall_glyph_at`, `adjacent_resource`, `can_*`)
- → types::Terrain / types::CapitalKind: explicit type references for match arms
- → config.rs: reads `TERMINAL_*` colors, all `KEY_*` for HUD hints, `*_TIME_MS` / `*_SCRAP_COST` / `CARRY_CAP` / `MAX_STOCKPILE` / `EXTRACT_BAR_WIDTH` / `BASE_SELL_PRICE` / `BASE_BUY_PRICE` for HUD text

## types/ (submodule internals)

### types/mod.rs
- → each submodule: `mod` declarations and `pub use` re-exports (so `crate::types::X` resolves without callers knowing the file split)

### types/faction.rs
- → config.rs: reads `TERMINAL_PURPLE` for Cult color
- → ratatui::style::Color: return type of `FactionId::color()`

### types/terrain.rs
- → types/faction.rs (super): imports `FactionId` for `Tile.owner` and `Tile.wall` fields

### types/player.rs
- → types/faction.rs (super): imports `FactionId` for `Player.faction`
- → types/terrain.rs (super): imports `Terrain` for `ExtractionState.terrain`

### types/npc.rs
- → types/faction.rs (super): imports `FactionId` for `Npc.faction`

### types/capital.rs
- → types/faction.rs (super): imports `FactionId`
- → config.rs: reads `CITY_TOTAL_SCRAP`, `FUEL_THRESHOLDS`, `FUEL_SPEED_BONUS_PCT`, `NPC_BASE_MOVE_MS`

### types/state.rs
- → types/{capital,faction,npc,player,terrain} (super): imports every sibling type
- → map.rs: calls `generate_map` during `GameState::new`
- → config.rs: reads every gameplay tunable (starting stockpile, territory radius, NPC count, decay/dehydration intervals, movement cooldowns, carry cap, animation tick, etc.)
- → rand crate: `SmallRng`, `SeedableRng`, `Rng` for deterministic simulation RNG and non-deterministic player faction pick

### types/actions.rs
- → types/{capital,faction,player,state,terrain} (super): imports siblings
- → config.rs: reads every action timing and cost (`EXTRACT_TIME_MS`, `CLAIM_TIME_MS`, `CLAIM_CONTESTED_MULTIPLIER`, `CLAIM_SCRAP_COST`, `BASE_SELL_PRICE`, `BASE_BUY_PRICE`, `CARRY_CAP`, `MAX_STOCKPILE`, `FOUND_CITY_TIME_MS`, `FOUNDATION_SCRAP_COST`, `CITY_TOTAL_SCRAP`, `BUILD_SCRAP_TIME_MS`, `FOUND_CAMP_TIME_MS`, `CAMP_SCRAP_COST`, `WALL_SCRAP_COST`, `BUILD_WALL_TIME_MS`)
- → `GameState::is_blocked_for_npc` (super, `pub(super)`): called when evicting NPCs after city/camp founding

## map.rs
- → config.rs: reads `MAIN_CLUSTER_SIZE`, `SCATTER_PER_RESOURCE`, `MIN_CLUSTER_DIST_DIVISOR`, `CAPITAL_RESOURCE_DIST`, `CAPITAL_DIST_VARIANCE`
- → types (via `crate::types::*`): constructs `Tile` and `Terrain` values

## config.rs
- Pure constant module — no outgoing dependencies beyond `ratatui::style::Color`

## External crates
- `ratatui` 0.29 — rendering (main.rs, render.rs, config.rs for Color values)
- `crossterm` 0.28 — terminal input and raw mode (main.rs)
- `rand` 0.8 (SmallRng) — deterministic RNG for map gen and NPC movement (types/state.rs, map.rs)
