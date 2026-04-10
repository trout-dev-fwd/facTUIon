# render.rs

## Purpose
All ratatui drawing. Exposes a single `render(f, &state)` function called once per frame from `main.rs`. Contains the full map-to-spans conversion and the multi-line HUD.

## Entry point
- `render(f: &mut Frame, state: &GameState)` — draws the entire frame.

## Layout
- **Map area** — fills the top of the terminal. Height = `area.height - hud_lines`. Width is `area.width / 2` tiles wide because each tile is rendered as 2 characters (e.g. `"@ "`) to compensate for terminal cells being ~2x taller than wide. Nothing is retained between frames — the full map is rebuilt every draw.
- **HUD** — pinned to the bottom. 2 lines normally, 4 when adjacent to a capital (player line + capital info + trade instructions + controls).

## Tile render priority (per grid cell)
For each `(col, row)` the renderer picks ONE of these (first match wins):
1. **Player** — `@` if `(col, row) == (player.x, player.y)`.
2. **NPC** — faction `npc_glyph` if an NPC is there.
3. **Capital center** — `cap.center_glyph()` for complete capitals, or empty background for in-progress city foundations.
4. **Capital wall (border)** — for cities, uses `state.wall_glyph_at(col, row)` to compute the auto-connected box-drawing character. For camps, always `✗`.
5. **Free-standing wall** — `tile.wall.is_some()` → `state.wall_glyph_at(col, row)` (same connection logic as city walls).
6. **Terrain** — `tile.terrain.glyph_varied(variant, col, row, anim_tick)` for animation (water ripple, wasteland dust, varied ruins).

### Foreground / background contrast
There's a key pattern used for player/NPC/wall glyphs: `tile_bg` is the faction color if the tile has an `owner`. When an entity sits on a colored tile, its foreground color becomes `TERMINAL_BG` (dark) for contrast against the colored background. When off colored territory, entities use `TERMINAL_FG` (light). This is done so glyphs are always readable regardless of theme.

## HUD structure

### Line 1 — player info
- `[@]` badge with faction-colored background
- Resources: `≈{water}  *{fuel}  °{scrap}  ₵{crowns}  [{carry}/{cap}]`
- **OR** an action progress bar if the player is mid-action: extracting, claiming, founding city, building foundation, founding camp, or building wall. Each uses `render_bar(progress, EXTRACT_BAR_WIDTH)`.
- **OR** available action hints if idle: `[E] extract`, `[F] claim`, `[C] found foundation`, `[C] build (N/10)`, `[V] found camp`, `[R] build wall`. All hint letters come from `config::KEY_*` via `.to_ascii_uppercase()`.

### Line 2 — capital info (only when `adjacent_capital_idx().is_some()`)
- `[W/G/S/C]` faction badge
- Stockpile: `≈{water}/{max}  *{fuel}/{max}  °{scrap}/{max}  ₵{crowns}  POP:{pop}` in faction color

### Line 3 — trade instructions (only when adjacent to a capital)
- `sell:1≈ 2* 3°(₵5)  buy:!≈ @* #°(₵8)` — values in faction color, separators in dark gray. The `!@#` are the Shift+1/2/3 characters most terminals emit.

### Bottom line — controls
- `WASD: move  E: extract  F: claim  C: city  V: camp  R: wall  Q: quit` — all letters pulled from `config::KEY_*` so rebinding auto-updates the hint text.

## Helpers
- `render_bar(value: f32, width: usize) -> String` — single pure function used for all progress bars. Produces `╞═══▰═════╡ 67%` style output. Width includes the brackets and the percentage text.

## Notes
- Ratatui is immediate mode — never store widgets between frames. Every frame rebuilds everything from `state`.
- Tile color is **derived at render time** from `tile.owner` and entity kind. Never store a "color" field on `Tile` or anywhere else — that creates sync bugs.
- `state.wall_glyph_at(x, y)` is the single source of truth for wall glyphs. Both city walls and free walls route through it so they connect visually.
- When adding a new action with a progress bar, add a new `else if let Some(ref x) = p.action_name { ... }` branch in the line 1 section, matching the existing pattern.
- When adding a new HUD hint, add a matching `if state.can_X() { ... }` block in the idle action hint section.
