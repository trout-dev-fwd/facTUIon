# types/player.rs

## Purpose
Defines the `Player` struct and every "action in progress" state struct. The player's inventory, position, faction, and all the `Option<*State>` fields that track timed actions (extract, claim, found city, add to foundation, found camp, build wall).

## Key items
- `Player { x, y, faction, home_capital_idx, water, fuel, scrap, crowns, extracting, claiming, founding, building, founding_camp, building_wall }` — root player state.
- `Player::carrying()` — sum of `water + fuel + scrap`. Used for weight-based movement cooldown and carry cap enforcement.

## Action state structs
Each timed player action stores its start `Instant` (and sometimes extra context) in its own struct, referenced via an `Option<>` field on `Player`. All have `started: std::time::Instant` at minimum.

- `ExtractionState { target_x, target_y, terrain, started }` — which resource tile is being worked and what it produces.
- `ClaimState { started }` — minimal, uses current player tile at check time.
- `FoundState { started }` — city foundation placement.
- `BuildState { capital_idx, started }` — adds one scrap to a specific foundation. Stores the target index so it survives adjacency changes mid-action.
- `FoundCampState { started }` — camp founding.
- `BuildWallState { started }` — wall segment under the player's current tile.

## Notes
- Only one `*State` field should be `Some` at a time. The `can_*` methods in `actions.rs` enforce this by returning `false` if any other state is already active.
- Moving (`GameState::move_player`) sets every `*State` to `None` — cancelling any in-progress action. If you add a new state, extend `move_player` too.
- `home_capital_idx` points into `GameState.capitals`. It's stable because capitals are never removed. Used by decay (`update_decay`), dehydration (`update_dehydration`), and future respawn logic.
- `ExtractionState.target_x/target_y` are stored but not currently read by any logic (triggers an unused-field warning) — they're kept for future "still adjacent?" validation.
