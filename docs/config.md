# config.rs

## Purpose
Central home for every tunable constant in the game. Changing gameplay values, colors, and keybindings should only require editing this file. No logic lives here — it's a pure constants module.

## Sections (in file order)
1. **Map generation** — dimensions, seed, cluster size/distance/scatter, capital-to-resource distance.
2. **Territory** — `CAPITAL_TERRITORY_RADIUS` (manhattan radius of starting city's initial claimed territory).
3. **Units** — `NPCS_PER_FACTION` (starting count per faction), `CAPITAL_MIN_OPEN_SIDES` (legacy placement heuristic).
4. **Economy** — starting stockpiles, `MAX_STOCKPILE` cap, decay interval, per-resource decay toggles (`DECAY_WATER/FUEL/SCRAP`), dehydration interval.
5. **Fuel speed bonuses** — `FUEL_THRESHOLDS` array + `FUEL_SPEED_BONUS_PCT` per tier, `NPC_MOVE_COOLDOWN: [u64; 6]` (per-weight base cooldown, same structure as the player's `MOVE_COOLDOWN`). Used by `Capital::npc_move_cooldown(weight)` — heavier NPCs are slower, and fuel tiers apply a percentage reduction on top.
6. **NPC behavior** — `MAX_HOARD_BEFORE_USE` (the stockpile threshold at which NPCs stop harvesting a resource type; once every resource is capped the NPC just wanders). Future NPC expenditure behaviors (wall building, trading) will key off this same threshold.
7. **Population growth** — `WATER_GROWTH_THRESHOLD` (water stockpile required, defaults to `MAX_STOCKPILE`), `WATER_GROWTH_COST` (water spent per new NPC). Growth is **instant** — any time water increases and crosses the threshold, one NPC is immediately spawned and the cost is deducted. No timer.
6. **Display** — RGB constants for the terminal theme (`TERMINAL_FG`, `TERMINAL_BG`, `TERMINAL_GRAY`, `TERMINAL_DARK_BG`, `TERMINAL_LIGHT_BG`, `TERMINAL_PURPLE`). Entity glyphs use `TERMINAL_BG` as foreground when on colored territory for contrast.
7. **Animation** — `ANIM_TICK_MS` (water ripple / wasteland dust cycle rate).
8. **Player action times and costs** — `EXTRACT_TIME_MS`, `CLAIM_TIME_MS`, `CLAIM_CONTESTED_MULTIPLIER`, `CLAIM_SCRAP_COST`, `PLAYER_STARTING_SCRAP`.
9. **City founding** — `FOUND_CITY_TIME_MS`, `FOUNDATION_SCRAP_COST` (foundation), `CITY_TOTAL_SCRAP` (completion), `BUILD_SCRAP_TIME_MS` (derived per-scrap build time).
10. **Camp founding** — `FOUND_CAMP_TIME_MS`, `CAMP_SCRAP_COST`.
11. **Walls** — `WALL_SCRAP_COST`, `BUILD_WALL_TIME_MS`, `WALL_UNCLAIMED_MULTIPLIER` (multiplier applied when building on unclaimed tiles or another faction's territory; base time applies on your own territory).
12. **Trade pricing** — `BASE_SELL_PRICE`, `BASE_BUY_PRICE`, `CARRY_CAP`, `EXTRACT_BAR_WIDTH`.
13. **Movement speed by weight** — `MOVE_COOLDOWN: [u64; 6]` indexed by items carried (0–5).
14. **Player controls** — all `KEY_*` char constants. Changing any of these rebinds the action and updates the HUD hint automatically.

## Derived constants
- `BUILD_SCRAP_TIME_MS = FOUND_CITY_TIME_MS / (CITY_TOTAL_SCRAP - FOUNDATION_SCRAP_COST)` — each scrap added to a foundation takes an even slice of the total build time, so tweaking `FOUND_CITY_TIME_MS` automatically spreads across the remaining scrap.

## Notes
- Colors default to a Monokai Pro palette (see RGB values). Swap these to match a different terminal theme.
- `TERMINAL_FG` and `TERMINAL_BG` are used all over rendering to make entity glyphs contrast against both colored and neutral backgrounds. Don't hardcode `Color::White` / `Color::Black` in render logic — reach for these constants instead.
- Several warnings about unused constants (e.g. `CAPITAL_MIN_OPEN_SIDES`, `TERMINAL_GRAY`) are expected — they're kept as tunables for future features.
- `KEY_BUY_*` use `!`/`@`/`#` because most terminals emit those when Shift+1/2/3 is pressed.
