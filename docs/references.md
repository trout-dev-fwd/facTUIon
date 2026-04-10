# Cross-references

One line per directional dependency. Use this as a fast map of "who talks to whom" before diving into any companion file.

## main.rs
- → types.rs: constructs `GameState::new`, drives the game loop by calling all `update_*` / `check_*` / action methods each frame
- → render.rs: calls `render::render(f, &state)` each frame
- → config.rs: reads `MAP_*`, `KEY_*` constants for setup and input mapping

## render.rs
- → types.rs: reads `GameState` (map, capitals, npcs, player, action states) and calls query methods (`adjacent_capital_idx`, `population_of`, `capital_border_at`, `is_box_wall`, `wall_glyph_at`, `adjacent_resource`, `can_*`)
- → config.rs: reads `TERMINAL_*` colors, all `KEY_*` for HUD hints, `*_TIME_MS` / `*_SCRAP_COST` / `CARRY_CAP` / `MAX_STOCKPILE` / `EXTRACT_BAR_WIDTH` / `BASE_SELL_PRICE` / `BASE_BUY_PRICE` for the HUD
- (does not touch map.rs directly — map data is accessed through `GameState.map`)

## types.rs
- → config.rs: reads gameplay tunables everywhere (decay intervals, cooldowns, costs, thresholds, carry cap, animation tick)
- → map.rs: calls `map::generate_map` during `GameState::new` to produce the initial `Tile` grid and capital positions
- → (indirectly used by) main.rs, render.rs

## map.rs
- → config.rs: reads `MAIN_CLUSTER_SIZE`, `SCATTER_PER_RESOURCE`, `MIN_CLUSTER_DIST_DIVISOR`, `CAPITAL_RESOURCE_DIST`, `CAPITAL_DIST_VARIANCE`
- → types.rs: constructs `Tile` and `Terrain` values

## config.rs
- Pure constant module — no outgoing dependencies beyond `ratatui::style::Color`

## External
- `ratatui` 0.29 — rendering (main.rs, render.rs, config.rs for Color values)
- `crossterm` 0.28 — terminal input and raw mode (main.rs)
- `rand` 0.8 (SmallRng) — deterministic RNG for map gen and NPC movement (types.rs, map.rs)
