# types.rs

## Purpose
The backbone of the game. Owns every core data structure (`Tile`, `Player`, `Npc`, `Capital`, `GameState`), all gameplay logic methods on `GameState`, and the helper functions for spawning. This is the largest file by far and nearly every cross-cutting feature touches it.

## Top-level types

### Enums
- `Terrain { Wasteland, Water, Rocky, Ruins }` — tile kind. `Terrain::glyph_varied(variant, x, y, tick)` returns the animated/varied display character (water ripple, wasteland dust, varied ruins glyphs). Constants `RUINS_GLYPHS`, `WATER_GLYPHS`, `DUST_GLYPH`, `WAVE_WIDTH` control animation feel.
- `FactionId { Water, Gas, Scrap, Cult }` — Cult is excluded from starting capitals/NPCs/player but can own Camp-type capitals. Methods: `glyph()` (uppercase), `npc_glyph()` (lowercase), `color()` (faction background color — Cult uses `config::TERMINAL_PURPLE`).
- `CapitalKind { City, Camp }` — City is a 3x3 box with box-drawing walls, Camp is a + shape with `✗` walls.

### Structs
- `Tile { terrain, owner: Option<FactionId>, wall: Option<FactionId>, glyph_variant: u8 }` — the grid cell. `owner` is territory (background color), `wall` is a free-standing wall segment (impassable). `glyph_variant` is seeded at generation for stable visual variety.
- `Player { x, y, faction, home_capital_idx, water, fuel, scrap, crowns, extracting, claiming, founding, building, founding_camp, building_wall }` — the six `Option<*State>` fields are action-in-progress timers. Only one should be active at a time; `can_*` methods enforce this. `home_capital_idx` is where the player "belongs" — used for decay attribution, and (future) respawn location.
- Action state structs: `ExtractionState` (target + terrain + started), `ClaimState`, `FoundState`, `BuildState` (which foundation + started), `FoundCampState`, `BuildWallState`.
- `Npc { x, y, faction, home_capital_idx, last_move }` — `home_capital_idx` mirrors player's field for decay/reassignment.
- `Capital { x, y, faction, kind, water, fuel, scrap, crowns, scrap_invested }` — cities and camps share this struct. `scrap_invested` tracks foundation progress for cities (5 = foundation, 10 = complete). Camps are created with `scrap_invested: 0` but `is_complete()` returns true for them.
- `GameState` — root game state. Holds everything: `map`, `capitals`, `npcs`, `player`, `sim_rng` (deterministic), plus last-tick `Instant` timers for animation, movement, decay, dehydration.

## `Capital` methods
- `is_complete()` — true for camps, scrap-gated for cities.
- `center_glyph()` — the letter shown at the center tile (always `faction.glyph()` now that camps are always Cult).
- `is_inside(x, y)` — footprint test. City: `dx <= 1 && dy <= 1`. Camp: center or single cardinal step.
- `fuel_tiers()` — how many `FUEL_THRESHOLDS` this capital has passed.
- `npc_move_cooldown()` — base NPC move time reduced by `FUEL_SPEED_BONUS_PCT` per tier.

## `GameState` methods (grouped by concern)

### Construction
- `new(width, height, seed)` — generates map via `map::generate_map`, builds the 3 starting Cities with `CapitalKind::City`, claims their diamond-shaped territory with `CAPITAL_TERRITORY_RADIUS + 1` (the +1 covers the wall ring), picks a random player faction (time-seeded so it varies between runs), spawns the player next to their capital, spawns `NPCS_PER_FACTION - 1` NPCs for the player's faction and `NPCS_PER_FACTION` for each other faction. Assigns `home_capital_idx` to every NPC and the player.

### Queries
- `capital_at(x, y)` / `capital_border_at(x, y)` — identify what's at a tile. `capital_border_at` now returns `Option<&Capital>` (not a glyph) so the renderer can compute wall glyphs dynamically.
- `is_box_wall(x, y)` — true if a tile participates in the box-drawing wall network (city walls **or** free-standing walls, **not** camp walls).
- `wall_glyph_at(x, y)` — computes the correct box-drawing character based on which of the 4 cardinal neighbors are also box walls. 16-case lookup handles straights, corners, T-junctions, and crosses. Called from `render.rs` for both city walls and free-standing walls.
- `is_capital_area(x, y)` — any footprint tile (city or camp) of any capital.
- `adjacent_capital()` / `adjacent_capital_idx()` — returns the capital the player is cardinally adjacent to. Used by HUD + trade + foundation building.
- `adjacent_foundation_idx()` — same but filtered to in-progress foundations of the player's faction.
- `population_of(cap_idx)` — count of NPCs + player with `home_capital_idx == cap_idx`. Used by HUD `POP:` and decay.
- `adjacent_resource()` — cardinal neighbor that's a resource tile, for extraction.
- `can_found_city` / `can_add_to_foundation` / `can_found_camp` / `can_build_wall` / `can_claim` — action preconditions, used by both the action methods and the HUD hints.
- `is_blocked(x, y)` / `is_blocked_for_npc(x, y, self_idx)` — movement gating. Checks edges, non-wasteland, walls (`tile.wall.is_some()`), capital areas, and other entities. `is_blocked_for_npc` additionally excludes the NPC itself and blocks on the player.

### Per-frame updates (called from main loop)
- `update_anim()` — advances `anim_tick` based on `ANIM_TICK_MS`.
- `update_npcs()` — Phase 1 wandering. Each NPC picks a random cardinal direction on its own cooldown (derived from their home capital's fuel tier). Deterministic via `sim_rng`.
- `update_decay()` — every `DECAY_INTERVAL_MS`, each capital loses resources equal to `population_of(cap_idx)`. Per-resource toggles via `DECAY_*` config flags.
- `update_dehydration()` — every `DEHYDRATION_INTERVAL_MS`, each capital with `water == 0` loses one of its own assigned NPCs.

### Player action lifecycle
Each action follows the same three-method pattern: `can_X()` → `start_X()` → `check_X()`.
- `start_extract` / `check_extraction` — adjacent resource → timed → adds to player inventory.
- `start_claim` / `check_claim` (+ `claim_time_ms()` for contested multiplier) — stand on tile → timed → set `tile.owner`.
- `start_found_city` / `check_found_city` — stand on claimed tile with 5 scrap → timed → create `City` capital with `scrap_invested: 5`, evict NPCs from the 3x3, nudge player out.
- `start_add_to_foundation` / `check_build` — adjacent to own foundation → timed per scrap → increments `scrap_invested` until city is complete.
- `start_found_camp` / `check_found_camp` — stand on wasteland with 5 scrap → timed → create `Camp` capital with `faction: FactionId::Cult` (always, regardless of builder).
- `start_build_wall` / `check_build_wall` — stand on wasteland with 2 scrap → timed → set `tile.wall = Some(faction)` and `tile.owner`.

All `move_player` calls cancel every in-progress action by setting the state fields to `None`.

### Trade
- `sell_resource(resource: usize)` / `buy_resource(resource: usize)` — `1` = water, `2` = fuel, `3` = scrap (matching HUD order). Moves items between player inventory and adjacent capital stockpile, trades crowns at `BASE_SELL_PRICE` / `BASE_BUY_PRICE`. Enforces carry cap, stockpile cap, and crown balance.

## Module-level helper functions (outside `impl GameState`)
- `in_capital_area(&[Capital], x, y)` — local version used during spawn setup (before `GameState` exists).
- `find_open_adjacent(map, capitals, cx, cy, w, h)` — find a walkable tile outside any capital footprint near (cx, cy). Used for initial player spawn.
- `find_open_adjacent_avoiding(map, capitals, npcs, player_x, player_y, cx, cy, w, h)` — same, but also excludes existing NPCs and player position. Used for initial NPC spawn.

## Key design decisions & gotchas
- **Deterministic sim RNG** — `sim_rng` is seeded from `seed ^ 0xDEAD_BEEF` and is used for all ongoing simulation decisions. Never use `thread_rng()` in `update_*` code. This keeps future multiplayer lockstep-ready.
- **Player faction is non-deterministic** — intentionally. Seeded from `SystemTime` inside `new()` so different runs pick different factions even with the same map seed.
- **Capital indices are stable** — `home_capital_idx` refers to positions in `self.capitals`. We never remove capitals, only append (e.g. when founding new cities/camps). If that changes, every `home_capital_idx` needs auditing.
- **Moving cancels all in-progress actions** — intentional. Every `move_player` call sets all `*State` fields on `Player` to `None`.
- **`is_blocked_for_npc` is specifically for NPC AI** and includes the player position and excludes the NPC itself. Don't reuse it for player movement checks.
- **Camp walls (`✗`) are NOT part of the box-wall network** — they don't count in `is_box_wall()` and don't connect visually to city walls or free-standing walls. This is intentional to keep camps visually distinct.
