# facTUIon

A two-player terminal-based strategy game set in a post-apocalyptic world. Players compete to dominate territory while managing resources, building cities, and navigating a four-faction economy. Built with [ratatui](https://github.com/ratatui-org/ratatui) and [crossterm](https://github.com/crossterm-rs/crossterm).

See [ARCHITECTURE.md](ARCHITECTURE.md) for the original design document.

## Run

```
cargo run
```

## Controls

All keybindings live in `src/config.rs` under `KEY_*` and can be rebound there.

| Key     | Action                                     |
|---------|--------------------------------------------|
| W A S D | Move (arrow keys also work)                |
| E       | Extract resource from adjacent tile        |
| F       | Claim the tile you're standing on (1 scrap) |
| C       | Found city foundation (5 scrap) / add scrap to adjacent foundation |
| V       | Found a cult camp (5 scrap)                |
| R       | Build a wall segment under you (2 scrap)  |
| 1 / 2 / 3 | Sell water / fuel / scrap to adjacent capital |
| Shift + 1/2/3 | Buy water / fuel / scrap from adjacent capital |
| Q / Esc | Quit                                       |

## Project layout

```
src/
  main.rs      - entry point, panic handler, game loop, input routing
  config.rs    - all tunables and keybindings
  types.rs     - core types, GameState, all gameplay logic
  map.rs       - procedural map generation
  render.rs    - ratatui draw calls and HUD

docs/          - companion documentation for each source file
  main.md, config.md, types.md, map.md, render.md
  references.md - cross-file dependency map

CLAUDE.md      - guidance for future Claude sessions
ARCHITECTURE.md - original design doc (aspirational)
```

## Current state

### Built
- **Map generation**: clustered resource deposits (water/rocky/ruins), 3 spread-out faction capitals with diamond-shaped territory
- **Rendering**: 2-char-wide tiles, colored territory backgrounds, varied ruin glyphs, water ripple and wasteland dust animations
- **Player movement**: weight-based cooldown (empty = 100ms, full = 350ms)
- **Resource extraction** (E): stand adjacent, 3s timer, 1 resource per action
- **Tile claiming** (F): 1 scrap, 3s timer, 1.5x on contested tiles
- **City founding** (C): 2-phase (5 scrap foundation + 5 scrap to complete), telegraphed to opponents
- **Cult camps** (V): 5 scrap, + shape with `✗` walls, always Cult faction regardless of builder
- **Wall segments** (R): 2 scrap, auto-connecting box-drawing glyphs that merge with city walls
- **Trade**: buy/sell water/fuel/scrap with adjacent capitals (1-3 sell, !@# buy)
- **Faction decay**: each capital loses resources per assigned population every 30s
- **Dehydration**: capitals with 0 water lose an assigned NPC every 30s
- **Fuel speed bonuses**: NPCs move faster at fuel thresholds 5/10/15/20
- **NPC wandering** (Phase 1): random movement with collision avoidance, deterministic RNG
- **Per-capital population tracking**: via `home_capital_idx` on NPCs/player
- **HUD**: player info, capital info when adjacent, trade hints, controls, action progress bars

### Roadmap (not yet built)

Items below are from `ARCHITECTURE.md` but haven't been implemented yet.

- **Slow-tick architecture** — currently we use per-system time-based timers. The architecture doc proposes a unified 3000ms slow tick for all simulation.
- **NPC worker cycle** — extract → return home → deposit → repeat. Phase 2 of NPC behavior.
- **NPC trader cycle** — pull from treasury, path to allied capital, buy/sell, return and deposit.
- **NPC soldier behavior** — patrol, claim tiles, attack hostiles.
- **Combat** — Shift+WASD attack, combat rolls, death, corpse looting (`X` key), avenging, reputation drops.
- **Reputation** — per-faction rep tracking, rep drift, war declarations at threshold.
- **Cult behavior** — conversion attempts, fission (spawning new camps), dormant respawn, war triggering.
- **Starvation** — NPCs defecting to neutral when stockpile is empty.
- **Networking** — TCP host/join, handshake, host-authoritative action sync over `NetMessage`.
- **Setup screen** — host/join UI, IP entry, faction assignment.
- **End screen** — win conditions (60% territory or 50% cult conversion), rematch.
- **External IP lookup** — fetch public IP for setup screen instead of LAN IP.
- **NPC reassignment UI** — transfer NPCs between capitals (`home_capital_idx` is already in place, just needs UI).
- **Player death / respawn** — respawn at home capital.
