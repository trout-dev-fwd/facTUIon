# types/capital.rs

## Purpose
Defines `CapitalKind` (City vs Camp), the `Capital` struct, and the helper methods on `Capital` for footprint checks and fuel-scaled speed bonuses. This file is about the **shape and attributes** of a capital — not about querying world state (that's in `state.rs`).

## Key items
- `CapitalKind { City, Camp }` — footprint shape selector. City = 3x3 box, Camp = + (plus) with 4 cardinal arms.
- `Capital { x, y, faction, kind, water, fuel, scrap, crowns, scrap_invested }` — all capitals share this one struct. `scrap_invested` tracks foundation progress for cities (5 = foundation, 10 = complete); camps are always considered complete.

## `Capital` methods
- `is_complete()` — true for camps unconditionally; cities need `scrap_invested >= CITY_TOTAL_SCRAP`.
- `center_glyph()` — the letter shown at the center tile (always `faction.glyph()`). Cities show W/G/S, camps show C (Cult's glyph).
- `is_inside(x, y)` — footprint test. City: `dx.abs() <= 1 && dy.abs() <= 1`. Camp: center or one cardinal step.
- `fuel_tiers()` — how many `FUEL_THRESHOLDS` this capital has passed.
- `apply_fuel_bonus(base_ms)` — apply this capital's fuel-tier percentage reduction to a base cooldown. Shared between NPC movement and the player's `move_player` so both benefit from their home capital's fuel.
- `npc_move_cooldown(weight)` — per-weight lookup in `NPC_MOVE_COOLDOWN` followed by `apply_fuel_bonus`.

## Notes
- Both kinds use the same `Capital` struct, so trade, decay, population, HUD, and all query logic in `state.rs` work on both. The only kind-dependent behavior is the footprint (`is_inside`, `capital_border_at`, etc.) and the rendered glyphs.
- Camps are always created with `faction: FactionId::Cult` regardless of who built them. This is enforced in `actions.rs::check_found_camp`.
- Adding a new `CapitalKind` variant requires updating `is_inside`, `is_complete`, `capital_border_at` in state.rs, `is_box_wall` in state.rs, and the renderer's per-tile lookup order.
