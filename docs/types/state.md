# types/state.rs

## Purpose
Owns `GameState` — the root of the game world — plus construction, queries, per-frame updates, and blocking logic. Everything that reads or mutates *world state* but isn't a player-initiated action lives here. Player actions (extract, claim, found, build, trade) live in `actions.rs` as a separate impl block.

## The `GameState` struct
```rust
pub struct GameState {
    pub map: Vec<Vec<Tile>>,
    pub capitals: Vec<Capital>,
    pub npcs: Vec<Npc>,
    pub player: Player,
    pub sim_rng: SmallRng,           // deterministic simulation RNG
    pub last_move: Instant,          // player move cooldown
    pub anim_tick: u64,
    pub last_anim: Instant,
    pub last_decay: Instant,
    pub last_dehydration: Instant,
}
```

## Sections (as commented in the file)

### Construction — `new(width, height, seed)`
Generates the map via `map::generate_map`, builds the 3 starting Cities with full stockpiles, claims their diamond-shaped territory (`CAPITAL_TERRITORY_RADIUS + 1`), picks a time-seeded random player faction (intentionally non-deterministic so runs vary), spawns the player next to their capital, spawns `NPCS_PER_FACTION - 1` NPCs for the player's faction and `NPCS_PER_FACTION` for each other faction, assigns `home_capital_idx` for everyone.

### Queries
Read-only methods the renderer and actions use to understand world state.
- `capital_at(x, y)` — capital whose center is exactly here.
- `capital_border_at(x, y)` — capital whose wall (city or camp) touches this tile.
- `is_box_wall(x, y)` — true if this tile is a city wall or free-standing wall (NOT camp walls).
- `wall_glyph_at(x, y)` — computes the correct box-drawing char based on neighbor connectivity (16-case lookup: corners, straights, Ts, cross). Used for both city walls and free walls so they visually merge.
- `is_capital_area(x, y)` — any footprint tile of any capital.
- `population_of(cap_idx)` — NPCs + player assigned to this capital via `home_capital_idx`.
- `territory_percents()` — returns `[f32; 4]` indexed as `[Water, Gas, Scrap, Cult]`, each value in `0.0..=1.0`. Numerator is the count of wasteland tiles whose `tile.owner` is that faction; denominator is the total wasteland tile count. Used by the top-of-screen HUD territory bars. 51% wins (future win-condition check).
- `adjacent_capital()` / `adjacent_capital_idx()` — capital the player is cardinally adjacent to. Used by HUD and all trade/foundation actions.
- `npc_at(x, y)` — NPC at exactly this tile.

### Per-frame updates
Called from `main.rs` game loop each frame. Internally time-gated.
- `update_anim()` — advances `anim_tick` every `ANIM_TICK_MS`.
- `update_npcs()` — state machine (see `docs/types/npc.md` for the `NpcTask` enum). Each NPC advances its task on a per-weight, per-faction cooldown (`Capital::npc_move_cooldown(weight)`). Decision priority in the `Wandering` branch is:
  1. Pick a harvest target (`pick_harvest_target`) — highest priority, covered while any stockpile is below its threshold.
  2. Pick a claim target (`pick_claim_target`) — expand territory if harvesting is fully saturated (all stockpiles at threshold) and the home capital has ≥ `CLAIM_SCRAP_COST` scrap.
  3. Random wander — fallback when no useful task is available.

  `Extracting` and `Claiming` are time-based (not cooldown-gated). On claim completion, scrap has already been deducted (at the moment of transition into `Claiming`), so completion just writes `tile.owner = Some(faction)`.
- `pick_claim_target(npc_idx)` — nearest unclaimed wasteland tile adjacent to the NPC's faction's existing territory, skipping walled tiles, capital footprints, the NPC's blacklist, and tiles another same-faction NPC is already targeting. Returns `None` if the home capital has less than `CLAIM_SCRAP_COST` scrap.
- `claim_tile_open_for(tx, ty, self_npc_idx)` — returns false if any other same-faction NPC is currently `TargetingClaim` or `Claiming` this exact tile.
- `update_decay()` — every `DECAY_INTERVAL_MS`, each capital loses resources equal to its assigned `population_of`, respecting `DECAY_*` per-resource config toggles.
- `update_dehydration()` — every `DEHYDRATION_INTERVAL_MS`, each capital with 0 water removes one of its own assigned NPCs.
- `try_grow_or_upgrade(cap_idx)` (`pub(super)`) — AI decision called at any site where a capital's stockpile could increase: NPC deposits (`Returning` branch of `update_npcs`) and `actions.rs::sell_resource` for every resource type. The AI picks exactly one action:
  1. **Upgrade** — if `population >= npc_target()` **and** `can_upgrade()`, deduct `upgrade_cost()` from water/fuel/scrap and increment `tier`. This raises `resource_cap`, `npc_target`, and the next upgrade's cost.
  2. **Grow** — else if `population < npc_target()` **and** water is at `resource_cap()`, spawn one new NPC for `WATER_GROWTH_COST` water via `find_growth_spawn`. If no spawn tile exists, water is NOT consumed and we retry next deposit.
  3. **Nothing** — let stockpiles accumulate until one of the above conditions is met.
- `find_growth_spawn(cap_idx)` — searches outward from the capital's outer ring for a walkable tile that is: wasteland, not walled, not inside any capital's footprint (faction-aware via `Capital::is_inside`, so camp + shapes are handled correctly), not the player's tile, and not occupied by any NPC. Returns `None` if nothing qualifies within `w.max(h)` rings.

### Blocking / movement
- `is_blocked(x, y)` — the canonical "can you walk there?" check. Blocks on: edges, non-wasteland terrain, walls (`tile.wall`), capital footprints, and NPCs. Used for player movement and post-action player nudging.
- `is_blocked_for_npc(x, y, self_idx)` — like `is_blocked` but excludes the NPC at `self_idx` (so NPCs don't block themselves) and includes the player position (so NPCs can't overlap the player). Uses the **occupancy grid** for an O(1) NPC lookup instead of scanning every NPC. `pub(super)` so `actions.rs` can call it when evicting NPCs after city/camp founding.
- `move_npc_to(i, nx, ny)` (`pub(super)`) — canonical NPC position update. Clears the NPC's old occupancy slot, writes the new coordinates, and marks the new occupancy slot in one step. **Every successful NPC position change must go through this helper** so the occupancy grid doesn't drift out of sync with actual NPC positions.
- `mark_npc_occupancy(i)` (`pub(super)`) — set the grid cell for a freshly-spawned NPC. Called after pushing to `self.npcs` in `try_grow_or_upgrade`.
- `rebuild_occupancy()` (`pub(super)`) — clears the grid and repopulates it from the current NPC list. Used after bulk changes that shift indices (currently only `update_dehydration` after removals).
- `occupancy_at(x, y)` — O(1) lookup returning the NPC index at a tile, or `None`.
- `occupancy: Vec<Option<usize>>` on `GameState` — flat `width * height` grid, rebuilt once in `new()` and then maintained incrementally via the above helpers.
- `move_player(dx, dy)` — weight-scaled cooldown (`MOVE_COOLDOWN[carry_weight]`) with the home capital's fuel-tier bonus applied via `Capital::apply_fuel_bonus`, then collision check, then updates position. **Also cancels every in-progress `Player.*State` field** — moving aborts any action.

### NPC harvest helpers (private, used only by `update_npcs`)
- `pick_harvest_target(npc_idx)` — returns `(x, y, Terrain)` of the best resource tile. Two-pass: first pass skips `last_failed_target` (if any) so the NPC tries a *different* tile after a pathfinding failure; if the first pass returns `None`, a second pass includes the blacklisted tile as a last resort. Scoring (via the shared `pick_harvest_target_impl`) is `distance + effective_amount * NPC_SCARCITY_WEIGHT` (configured in `config.rs`, default 2), where `effective_amount = home_capital.stockpile + npc.already_carrying` for that resource. A lower `NPC_SCARCITY_WEIGHT` keeps NPCs harvesting nearby tiles rather than crossing the map when stockpiles get moderately full. Skips resources where effective amount has already reached `MAX_HOARD_BEFORE_USE`, and returns `None` outright if the NPC is already at `CARRY_CAP`.
- `resource_accessible(tx, ty, self_npc_idx)` — true if (a) at least one cardinal neighbor is walkable wasteland and (b) no **same-faction** NPC is already Targeting or Extracting this tile. Cross-faction NPCs are intentionally allowed to target the same tile — this is how rival factions end up contesting the same resource deposit.
- `step_npc_toward(i, target: AstarTarget) -> bool` — single-step movement toward an A* goal description. Returns `true` if an A* plan was followed, `false` if we had to fall back to random (caller should then blacklist the target). Runs up to 3 passes:
  1. **Crowd-aware A*** (`astar_next_step(i, target, true)`) — routes around other NPCs, so an NPC can automatically try different cardinal approach tiles to a resource when their preferred side is occupied by a peer.
  2. **Static-only A*** (`astar_next_step(i, target, false)`) — plans through NPCs as if they weren't there. Used as a fallback when crowd-aware fails (deep crowding near a capital).
  3. **Random cardinal step** — last resort. Returns `false` so the `TargetingResource` caller will set `last_failed_target` and drop back to `Wandering` for a re-pick.
- `astar_next_step(npc_idx, target, include_npcs)` — A* pathfinding with a goal predicate and Manhattan heuristic. `include_npcs = true` uses `is_blocked_for_npc` so peers count as obstacles; `false` uses `is_static_blocked` (terrain/walls/capitals only). Uses a **thread-local `AstarScratch`** (flat `g_score`, `parent`, and `BinaryHeap` open set) that's reused across every call — zero allocation on the hot path.
- `AstarTarget` (module-private enum) — goal description:
  - `AdjacentTo(tx, ty)` — reach any tile cardinally adjacent to the single tile `(tx, ty)`. Used for resource targets.
  - `AdjacentToBox(cx, cy)` — reach any tile cardinally adjacent to the 3×3 capital footprint centered at `(cx, cy)`. **Critical**: home-return pathfinding must use this, not `AdjacentTo(cx, cy)`, because the center's 4 cardinal neighbors are all wall tiles — targeting the center would always fail and force NPCs into random-walk fallback after every deposit.
- `is_static_blocked(x, y)` — walkability check using only static obstacles (edges, non-wasteland terrain, walls, capital footprints). No NPC/player blocking. Only used by A*.
- `npc_adjacent_to_home(i)` — true if the NPC is cardinally adjacent to any footprint tile of its home capital. Gates the deposit transition in `Returning`.

## Module-level spawn helpers
These are free functions (not methods) because they're called during `new()` before the `GameState` exists.
- `in_capital_area(capitals, x, y)` — same as the method but takes a slice.
- `find_open_adjacent(map, capitals, cx, cy, w, h)` — a walkable tile near (cx, cy) outside any capital footprint. Used for player spawn.
- `find_open_adjacent_avoiding(map, capitals, npcs, player_x, player_y, cx, cy, w, h)` — same plus avoids NPCs and player position. Used for NPC spawn.

## Notes
- **Deterministic sim RNG**: `sim_rng` is seeded from `seed ^ 0xDEAD_BEEF`. Any new simulation logic should use `self.sim_rng` (not `thread_rng()`).
- **Capital indices are stable** — only appended, never removed. `home_capital_idx` depends on this.
- **`move_player` cancels all actions** — if you add a new `Player.*State` field in `player.rs`, add a matching `= None` line here.
- **`is_blocked` vs `is_blocked_for_npc`** — always pick the right one. Player movement uses `is_blocked`; NPC wandering uses `is_blocked_for_npc` (because NPCs shouldn't block themselves).
