# types/npc.rs

## Purpose
Defines the `Npc` struct and the `NpcTask` state machine that drives NPC behavior. The state transitions themselves are implemented on `GameState` in `state.rs::update_npcs`.

## Key items

### `NpcTask` enum
Each NPC carries a `task` field describing its current goal. The state machine:

```
Wandering
  ↓ (idle → pick a resource)
TargetingResource { tx, ty, terrain }
  ↓ (arrived adjacent)
Extracting { started, terrain }
  ↓ (timer elapsed)
Returning
  ↓ (arrived adjacent to home capital)
Wandering
```

Variants:
- `Wandering` — idle; next tick the NPC tries to pick a harvest target via `pick_harvest_target`. If no target is available (all resources capped or unreachable), the NPC takes one random step.
- `TargetingResource { tx, ty, terrain }` — walking toward a specific resource tile. If the target becomes inaccessible mid-walk (another NPC claimed it), the NPC drops back to `Wandering`.
- `Extracting { tx, ty, started, terrain }` — stationary on the adjacent tile, running the extraction timer. `tx, ty` name the specific resource tile being worked so the same-faction accessibility check is exact. Uses `EXTRACT_TIME_MS` just like the player.
- `Returning` — carrying a resource home. Depositing is instant once adjacent to the home capital.
- `TargetingClaim { tx, ty }` — walking *onto* a specific tile to claim it. Unlike `TargetingResource` the NPC stands on the tile itself (not a cardinal neighbor), because the claim timer runs while standing on the target.
- `Claiming { tx, ty, started }` — stationary on the claim target, running the claim timer. Scrap was deducted from the home capital at the moment of transition into this state, so completion is guaranteed and just sets `tile.owner`.

### `Npc` struct
- `x, y` — grid position.
- `faction` — determines color and which capital's cooldown applies.
- `home_capital_idx` — the capital this NPC is assigned to for population, decay, and the harvest-target source.
- `last_move` — movement cooldown timestamp (per-NPC, scales with carry weight + home capital's fuel tier).
- `task: NpcTask` — current state.
- `carrying_water`, `carrying_fuel`, `carrying_scrap` — per-resource inventory. Total is capped at `config::CARRY_CAP` (same as the player).
- `carrying_total()` — helper returning the sum across all three slots. Used for weight-based cooldown and carry-cap checks.
- `last_failed_target: Option<(u16, u16)>` — a tile that pathfinding recently couldn't reach. `pick_harvest_target` skips it in the first pass so the NPC picks a different resource tile instead of looping back to the same unreachable one. Cleared when the NPC successfully reaches an adjacent tile and transitions to `Extracting`.

## Notes
- **Carry cap matches the player**: NPCs can hold up to `CARRY_CAP` (5) items mixed across water/fuel/scrap, and their movement cooldown scales with total weight via `NPC_MOVE_COOLDOWN[weight]`.
- **Chain extraction**: when an `Extracting` tick completes with room to spare AND the home capital still needs that resource type, the NPC stays in `Extracting` with a fresh timer instead of transitioning out. This avoids 2 wasted cooldown ticks per item when harvesting from a consistent source.
- **Task is `Copy`**: every field is cheap (u16s, Instant, Terrain). This lets `update_npcs` clone the task out before mutating `self.npcs[i]`.
- **Adding new tasks**: if you add claiming/wall-building/trading behaviors, extend this enum and add the corresponding match arm in `update_npcs`. The pattern is: decide in Wandering, walk in a Targeting variant, commit in a timed variant, return home if carrying, repeat.
- **The target picker lives in `state.rs`**, not here. This file only defines the enum and struct.
