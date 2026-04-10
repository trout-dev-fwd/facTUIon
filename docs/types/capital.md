# types/capital.rs

## Purpose
Defines `CapitalKind` (City vs Camp), the `Capital` struct, and the helper methods for footprint checks, fuel-scaled speed bonuses, and the tier-based upgrade system. This file is about the **shape and attributes** of a capital — not about querying world state (that's in `state.rs`).

## Key items
- `CapitalKind { City, Camp }` — footprint shape selector. City = 3x3 box, Camp = + (plus) with 4 cardinal arms.
- `Capital { x, y, faction, kind, water, fuel, scrap, crowns, scrap_invested, tier }` — all capitals share this one struct. `scrap_invested` tracks foundation progress for cities (5 = foundation, 10 = complete); camps are always considered complete. `tier` ranges 1-5 (cities) and scales `resource_cap`, `npc_target`, and `upgrade_cost`.

## `Capital` methods
- `is_complete()` — true for camps unconditionally; cities need `scrap_invested >= CITY_TOTAL_SCRAP`.
- `center_label()` — 2-character label for the map render. Cities show `W1`/`G2`/`S3` etc. (faction letter + tier); camps show `"C "` (letter + space).
- `is_inside(x, y)` — footprint test. City: `dx.abs() <= 1 && dy.abs() <= 1`. Camp: center or one cardinal step.
- `resource_cap()` — effective stockpile cap = `tier * MAX_STOCKPILE`. Tier 1 = 20, tier 5 = 100.
- `harvest_threshold()` — `resource_cap - HOARD_BUFFER`. NPCs stop harvesting a resource once it reaches this (so there's always a small buffer below the cap).
- `npc_target()` — AI-driven target population = `resource_cap / 4`. When population reaches this, the AI stops growing new NPCs and starts saving for an upgrade.
- `upgrade_cost()` — per-resource cost to upgrade to the next tier = `tier * BASE_UPGRADE_COST`. Tier 1→2 = 10, tier 2→3 = 20, etc.
- `can_upgrade()` — true if this is a city, not at max tier, and all three stockpiles cover `upgrade_cost()`. Camps never upgrade.
- `fuel_tiers()` — how many `FUEL_THRESHOLDS` this capital has passed.
- `apply_fuel_bonus(base_ms)` — apply this capital's fuel-tier percentage reduction to a base cooldown. Shared between NPC movement and the player's `move_player`.
- `npc_move_cooldown(weight)` — per-weight lookup in `NPC_MOVE_COOLDOWN` followed by `apply_fuel_bonus`.

## Notes
- Both kinds use the same `Capital` struct, so trade, decay, population, HUD, and all query logic in `state.rs` work on both. The only kind-dependent behavior is the footprint (`is_inside`, `capital_border_at`, etc.) and the rendered glyphs.
- Camps are always created with `faction: FactionId::Cult` regardless of who built them. This is enforced in `actions.rs::check_found_camp`.
- Adding a new `CapitalKind` variant requires updating `is_inside`, `is_complete`, `capital_border_at` in state.rs, `is_box_wall` in state.rs, and the renderer's per-tile lookup order.
