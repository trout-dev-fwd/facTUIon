# types/actions.rs

## Purpose
A second `impl GameState` block containing every **player-initiated action**. Split out from `state.rs` so the "what actions the player can take" surface is in one place without drowning in world-query methods.

Each action follows a consistent three-method lifecycle: `can_X()` (precondition for HUD + guard) → `start_X()` (begins the timer) → `check_X()` (called every frame; completes when elapsed).

## Action groups (as commented in the file)

### Extraction
- `adjacent_resource()` — returns `(x, y, terrain)` of a cardinal resource tile next to the player.
- `start_extract()` — if adjacent to a resource and not over carry cap, sets `Player.extracting`.
- `check_extraction()` — after `EXTRACT_TIME_MS`, adds 1 of the right resource to player inventory and clears the state.

### Claim
- `can_claim()` — player stands on wasteland they don't own, has scrap, no other action in progress.
- `start_claim()` — sets `Player.claiming`.
- `claim_time_ms()` — base time, multiplied by `CLAIM_CONTESTED_MULTIPLIER` if the tile is owned by another faction.
- `check_claim()` — deducts scrap, sets `tile.owner = Some(player.faction)`.

### Trade
- `sell_resource(resource: usize)` — sells 1 water/fuel/scrap (1/2/3) to the adjacent capital. Caps capital stockpile at `MAX_STOCKPILE`, credits player with `BASE_SELL_PRICE` crowns.
- `buy_resource(resource: usize)` — inverse of sell; enforces player crown balance and carry cap.

Both use function pointers (`fn(&mut Player) -> &mut u32`) to avoid duplicating the three-resource match logic. The `resource` arg maps: `1 = water (≈)`, `2 = fuel (*)`, `3 = scrap (°)` — matching the HUD display order.

### City founding (foundation → build)
- `can_found_city()` — 5 scrap, center tile claimed by player's faction, 5x5 area clear of resources/walls/capitals. Called by the HUD hint.
- `start_found_city()` — sets `Player.founding`.
- `check_found_city()` — after `FOUND_CITY_TIME_MS`: creates a `Capital` with `kind: City, scrap_invested: FOUNDATION_SCRAP_COST` (so it's not yet complete), evicts NPCs from the 3x3, nudges the player out.
- `adjacent_foundation_idx()` — finds an in-progress foundation of the player's faction adjacent to the player.
- `can_add_to_foundation()` — must be adjacent to own foundation, have scrap, not already building.
- `start_add_to_foundation()` — sets `Player.building` with the target capital index.
- `check_build()` — after `BUILD_SCRAP_TIME_MS`, deducts 1 scrap and adds 1 to `scrap_invested`. Re-verifies state in case the foundation was completed or the player moved.

### Camp founding
- `can_found_camp()` — 5 scrap, 5-tile + footprint all wasteland, no capital overlap.
- `start_found_camp()` — sets `Player.founding_camp`.
- `check_found_camp()` — after `FOUND_CAMP_TIME_MS`: creates a `Capital { kind: Camp, faction: FactionId::Cult }` (always Cult, regardless of builder), evicts NPCs, nudges player to a diagonal corner (outside the + shape).

### Wall building
- `can_build_wall()` — 2 scrap, standing on clear wasteland, no capital overlap.
- `start_build_wall()` — sets `Player.building_wall`.
- `build_wall_time_ms()` — effective build time for the current tile: base `BUILD_WALL_TIME_MS` on own territory, `BUILD_WALL_TIME_MS * WALL_UNCLAIMED_MULTIPLIER` on unclaimed or enemy-owned tiles. Used by both the check and the HUD progress bar.
- `check_build_wall()` — after `build_wall_time_ms()`: sets `tile.wall = Some(faction)` and nudges player off. **Does not claim territory** (`tile.owner` is left alone — walls are impassable strategic pieces, but ownership still has to be earned with `F`).

## Notes
- **Visibility trick**: `actions.rs` calls `self.is_blocked_for_npc()` (private in state.rs) via `pub(super)`. Everything else is reachable because both files live in the `types` module and share the same `impl GameState`.
- **Camp faction override**: `check_found_camp` hardcodes `FactionId::Cult` as the new capital's faction. Don't use `self.player.faction` there.
- **`check_build` re-verification**: after the timer elapses, it checks that the foundation still exists, is still adjacent, still needs scrap, and the player still has scrap. This is because the state might have changed during the build time (player moved, scrap was spent, foundation was finished).
- **Adding a new action**: follow the `can_X` / `start_X` / `check_X` pattern. Add the state struct in `player.rs`, add the `Option<*State>` field to `Player`, extend `move_player` in `state.rs` to cancel it, bind a key in `config.rs` + `main.rs`, and add a HUD hint in `render.rs`.
