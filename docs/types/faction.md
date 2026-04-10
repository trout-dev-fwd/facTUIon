# types/faction.rs

## Purpose
Defines the `FactionId` enum and its display properties (letter, NPC letter, color). Small, leaf-level file with no dependencies on other types module files.

## Key items
- `FactionId { Water, Gas, Scrap, Cult }` — the 4 factions in the game. Water/Gas/Scrap each have a starting capital and NPCs. Cult is excluded from starting capitals/NPCs/player selection but owns all Camp-type capitals regardless of who builds them.
- `FactionId::glyph()` — uppercase letter (`W`/`G`/`S`/`C`).
- `FactionId::npc_glyph()` — lowercase letter (`w`/`g`/`s`/`c`).
- `FactionId::color()` — faction background color. Cult uses `config::TERMINAL_PURPLE` (RGB); the others use ratatui's base colors (Blue/Red/Yellow).

## Notes
- Adding a new faction requires updating all three methods plus any match-on-FactionId sites (grep for `FactionId::`).
- The enum derives `Debug` so NPC/Player faction can be used in eprintln debug scaffolding if needed.
