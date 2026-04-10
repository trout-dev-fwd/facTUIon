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
- `adjacent_capital()` / `adjacent_capital_idx()` — capital the player is cardinally adjacent to. Used by HUD and all trade/foundation actions.
- `npc_at(x, y)` — NPC at exactly this tile.

### Per-frame updates
Called from `main.rs` game loop each frame. Internally time-gated.
- `update_anim()` — advances `anim_tick` every `ANIM_TICK_MS`.
- `update_npcs()` — Phase 2 state machine (see `docs/types/npc.md` for the `NpcTask` enum). Each NPC advances its task on its per-faction cooldown: idle NPCs pick a harvest target via `pick_harvest_target`, walk toward it via `step_npc_toward`, extract on `EXTRACT_TIME_MS`, then carry the resource home and deposit it. `Extracting` is time-based (not cooldown-gated). Random wander is the fallback when no target is available. Deterministic via `sim_rng`.
- `update_decay()` — every `DECAY_INTERVAL_MS`, each capital loses resources equal to its assigned `population_of`, respecting `DECAY_*` per-resource config toggles.
- `update_dehydration()` — every `DEHYDRATION_INTERVAL_MS`, each capital with 0 water removes one of its own assigned NPCs.

### Blocking / movement
- `is_blocked(x, y)` — the canonical "can you walk there?" check. Blocks on: edges, non-wasteland terrain, walls (`tile.wall`), capital footprints, and NPCs. Used for player movement and post-action player nudging.
- `is_blocked_for_npc(x, y, self_idx)` — like `is_blocked` but excludes the NPC at `self_idx` (so NPCs don't block themselves) and includes the player position (so NPCs can't overlap the player). `pub(super)` so `actions.rs` can call it when evicting NPCs after city/camp founding.
- `move_player(dx, dy)` — weight-scaled cooldown, then collision check, then updates position. **Also cancels every in-progress `Player.*State` field** — moving aborts any action.

### NPC harvest helpers (private, used only by `update_npcs`)
- `pick_harvest_target(npc_idx)` — returns `(x, y, Terrain)` of the best resource tile. Scores each candidate as `distance + stockpile_amount * SCARCITY_WEIGHT` (currently 3) and returns the lowest score — this blends neediness and proximity so NPCs don't walk cross-map for a slightly scarcer resource when a closer one is almost as needed. Skips anything at or above `MAX_HOARD_BEFORE_USE`.
- `resource_accessible(tx, ty, self_npc_idx)` — true if (a) at least one cardinal neighbor is walkable wasteland and (b) no **same-faction** NPC is already Targeting or Extracting this tile. Cross-faction NPCs are intentionally allowed to target the same tile — this is how rival factions end up contesting the same resource deposit.
- `step_npc_toward(i, target: AstarTarget)` — single-step movement toward an A* goal description. Calls `astar_next_step`, takes the first step if not blocked by another NPC, otherwise falls back to a random cardinal step.
- `astar_next_step(npc_idx, target)` — A* pathfinding with a goal predicate and Manhattan heuristic. Plans through static obstacles only (`is_static_blocked`) so crowded NPCs don't deadlock each other; NPC collisions are resolved at move time.
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
