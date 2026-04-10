# types/npc.rs

## Purpose
Minimal — just the `Npc` struct. NPCs don't have action states yet (Phase 1 wandering is the only behavior), so this file stays small. Phase 2 worker/trader/soldier behaviors will add fields here.

## Key items
- `Npc { x, y, faction, home_capital_idx, last_move }` — position, faction, which capital they belong to for population/decay, and their last movement timestamp for cooldown.

## Notes
- `home_capital_idx` is set at spawn and never automatically changes. Future reassignment mechanics would mutate this field.
- `last_move` is an `Instant` used by `GameState::update_npcs()` to enforce per-NPC movement cooldown (scaled by home capital's fuel tier).
- If you add NPC roles (Worker/Trader/Soldier from the architecture doc), extend this struct with a `role` or `behavior` enum and add state fields as needed. Do NOT put the role logic here — put it in `state.rs` (or a new `updates.rs`) as methods on `GameState`.
