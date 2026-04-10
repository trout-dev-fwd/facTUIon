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
- `Extracting { started, terrain }` — stationary on the adjacent tile, running the extraction timer. Uses `EXTRACT_TIME_MS` just like the player.
- `Returning` — carrying a resource home. Depositing is instant once adjacent to the home capital.

### `Npc` struct
- `x, y` — grid position.
- `faction` — determines color and which capital's cooldown applies.
- `home_capital_idx` — the capital this NPC is assigned to for population, decay, and the harvest-target source.
- `last_move` — movement cooldown timestamp (per-NPC, respects fuel tier of home capital).
- `task: NpcTask` — current state.
- `carrying: Option<Terrain>` — at most one resource type in inventory (`Water`/`Rocky`/`Ruins`). When carrying, the NPC is in `Returning`.

## Notes
- **Single resource at a time**: unlike the player's 5-item inventory, NPCs only carry one resource per trip. Simpler logic, and the walking time is the pacing.
- **Task is `Copy`**: every field is cheap (u16s, Instant, Terrain). This lets `update_npcs` clone the task out before mutating `self.npcs[i]`.
- **Adding new tasks**: if you add claiming/wall-building/trading behaviors, extend this enum and add the corresponding match arm in `update_npcs`. The pattern is: decide in Wandering, walk in a Targeting variant, commit in a timed variant, return home if carrying, repeat.
- **The target picker lives in `state.rs`**, not here. This file only defines the enum and struct.
