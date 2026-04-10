# types/terrain.rs

## Purpose
Owns the `Terrain` enum, the animated/varied glyph logic, and the `Tile` struct. Everything related to a single map cell that isn't an entity.

## Key items
- `Terrain { Wasteland, Water, Rocky, Ruins }` — tile kind.
- `Tile { terrain, owner, wall, glyph_variant }` — `owner` is territory (background color when rendered). `wall` being `Some` makes the tile a free-standing impassable wall. `glyph_variant` is a seeded-per-tile `u8` used for stable visual variety (e.g. which ruin glyph shows).
- `Terrain::glyph()` — the plain, non-animated character for this terrain type.
- `Terrain::glyph_varied(variant, x, y, tick)` — animated/varied character:
  - `Ruins` → picks one of `RUINS_GLYPHS` based on variant (stable per tile, no animation).
  - `Water` → wave sweep using `(x + y + tick)` phase; `≈` during the wave crest, `~` otherwise. `WAVE_WIDTH` controls crest width.
  - `Wasteland` → rare dust `·` particles that sweep diagonally (1/3 of variants, long cycle).
  - `Rocky` → static `^`.

## Module-private constants
- `RUINS_GLYPHS: [char; 7]` — `[':', '∷', '┘', '┐', '□', 'Ω', '▌']`.
- `WATER_GLYPHS: [char; 2]` — `['~', '≈']`.
- `DUST_GLYPH: char` — `·`.
- `WAVE_WIDTH: u64` — how many tiles wide the active water wave band is.

## Notes
- `Tile.owner` is the only thing that colors a wasteland tile's background. Resource tiles (`~`/`^`/`:`) never have their background colored even when claimed.
- `Tile.wall` is orthogonal to city walls — city walls are derived from `Capital` positions, free-standing walls live on the tile. Both participate in the box-wall network (see `state.rs` → `wall_glyph_at`).
- `glyph_varied` takes position so animations can phase across space. Don't cache its result — always call per-frame in the renderer.
