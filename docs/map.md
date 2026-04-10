# map.rs

## Purpose
Procedural map generation. Produces a `Vec<Vec<Tile>>` and 3 capital positions from a seeded RNG. Called once during `GameState::new`.

## Key function
- `generate_map(width, height, rng) -> (Vec<Vec<Tile>>, [(usize, usize); 3])`

The return tuple is the tile grid and three `(x, y)` capital positions in faction order (`[Water, Gas, Scrap]`).

## Algorithm (passes)
1. **Initialize** — allocate a `Vec<Vec<Terrain::Wasteland>>` grid of the requested size.
2. **Pick centers** — `pick_spread_centers` chooses 3 positions spread across the map with min manhattan distance `(w + h) / MIN_CLUSTER_DIST_DIVISOR`. Uses rejection sampling (up to 200 tries per center) within a margin.
3. **Grow clusters** — `grow_cluster` places a connected blob of the target terrain type around each center via random-walk frontier expansion. Tries all 4 neighbors of a frontier tile before giving up, which guarantees the cluster grows to the requested size as long as space is available. Size is `MAIN_CLUSTER_SIZE` (default 12).
4. **Place capitals** — `place_capital_near` finds a wasteland tile `CAPITAL_RESOURCE_DIST ± CAPITAL_DIST_VARIANCE` manhattan tiles from each cluster center. Requires all 9 tiles of the future 3x3 city area to be clear wasteland. Up to 200 attempts per capital.
5. **Scatter resources** — `SCATTER_PER_RESOURCE` lone tiles of each resource type are sprinkled in, constrained to be far enough from their main cluster (dist > w/4) so other factions have small scattered deposits near them.
6. **Convert to Tiles** — each `Terrain` cell becomes a `Tile { terrain, owner: None, wall: None, glyph_variant: rng.gen_range(0..=255) }`. The `glyph_variant` is used later by `Terrain::glyph_varied` for stable visual variety (e.g. which ruin glyph a tile shows).

## Helpers
- `pick_spread_centers(w, h, mx, my, rng)` — returns 3 spread-out `(x, y)` centers.
- `grow_cluster(grid, cx, cy, w, h, terrain, size, rng)` — random-walk growth.
- `place_capital_near(grid, cx, cy, w, h, rng)` — finds a valid capital center.
- `open_sides(grid, x, y, w, h)` — legacy helper counting cardinal wasteland neighbors. Currently unused (was for the old `CAPITAL_MIN_OPEN_SIDES` check) but left in place for potential reuse.

## Notes
- The starting territory (the diamond around each capital) is **not** claimed here — that happens in `GameState::new` after map generation, because `Tile.owner` lives on the tile but requires the capital positions to be known first.
- All randomness comes from the seeded `SmallRng` passed in, so maps are reproducible from the seed.
- The returned 3 capital positions correspond to the resource order `[Water, Rocky, Ruins]` → `[FactionId::Water, FactionId::Gas, FactionId::Scrap]` (Water Hoarders get water, Gas Runners get rocky, Scrap Merchants get ruins).
- Capital placement requires a 2-tile margin from edges so the 3x3 city area fits. Resource cluster distance of 4 ± 1 is large enough that the city walls don't touch resource tiles in normal cases.
