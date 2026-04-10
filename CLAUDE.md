# CLAUDE.md

Guidance for future Claude sessions working on this codebase.

## Start here

Before exploring source files, read these in order:

1. **`docs/references.md`** — a fast cross-file dependency map. One line per directional dependency between files. This tells you who calls whom without reading any source.
2. **The companion file in `docs/`** for whatever source file is relevant to your task. Each `.rs` in `src/` has a matching `.md` in `docs/` (e.g. `src/types.rs` → `docs/types.md`). The companion describes the file's purpose, key types/functions, and design decisions.
3. **Only then** dive into the source file itself.

If the user asks about the project roadmap, current features, or what's built vs. planned, read `README.md` (the "Current state" and "Roadmap" sections).

If the user asks about the original design intent or references something from the architecture doc, read `ARCHITECTURE.md`. **But note**: ARCHITECTURE.md is the aspirational original design. Large parts of it (networking, combat, cult behavior, slow-tick, etc.) are not yet implemented. The README is the source of truth for what actually exists.

## File map

```
src/
  main.rs      ↔  docs/main.md
  config.rs    ↔  docs/config.md
  types.rs     ↔  docs/types.md
  map.rs       ↔  docs/map.md
  render.rs    ↔  docs/render.md

docs/
  references.md      - cross-file dependency map
  {filename}.md      - companion doc for each source file

README.md            - project overview, controls, roadmap
ARCHITECTURE.md      - original aspirational design (not current state)
CLAUDE.md            - this file
```

## Updating documentation

After making changes, ask yourself this question:

> **Could a fresh Claude instance quickly understand what I just did and have ease of finding it for future modifications?**

If the answer is "no" without a doc update, update the relevant docs.

### When to update a companion file (`docs/{filename}.md`)
- You added, removed, or meaningfully changed a public type/function/method
- You added or removed a field on a struct that other code depends on
- You introduced a new pattern or design decision that isn't obvious from the code
- You changed the file's high-level responsibility or added a new concern

**Do not** update the companion for cosmetic changes (formatting, renames that don't affect meaning, comment tweaks) or for small internal refactors that don't change the public surface.

### When to update `docs/references.md`
- You added a new cross-file call or import
- You removed a cross-file dependency
- You added a new source file (add its section)

### When to update `README.md`
- You completed a roadmap item (move it from "Roadmap" to "Built")
- You added a new keybinding (update the Controls table)
- You changed the project structure (src/ layout)

### When to update `CLAUDE.md`
- You added a new top-level file or directory that future Claude should know about
- You changed the "start here" reading order
- You introduced a new pattern or workflow that should be enforced across sessions

### When you don't need to update anything
- Bug fix that doesn't change behavior
- Tuning a config value
- Internal helper refactor
- Formatting / lint cleanup

## Working principles

- **Config-first**: every tunable belongs in `config.rs`. New timing, cost, color, or keybinding should be added there, not hardcoded. See `docs/config.md`.
- **Rendering is immediate mode**: ratatui rebuilds the entire frame every tick. Don't cache widgets. Don't store colors on structs — derive them at render time. See `docs/render.md`.
- **Deterministic RNG for simulation**: `GameState.sim_rng` is seeded from the map seed. Never call `thread_rng()` in `update_*` code. Player faction pick is an intentional exception — it's time-seeded for variety.
- **Capital indices are stable**: `home_capital_idx` on NPCs and the Player indexes into `GameState.capitals`. Capitals are only ever appended, never removed. If that ever changes, audit every use of `home_capital_idx`.
- **Moving cancels all actions**: in `move_player`, every `Option<*State>` on `Player` gets set to `None`. If you add a new action with a state, add it to that list.
- **Action lifecycle pattern**: every timed player action uses `can_X()` → `start_X()` → `check_X()`. The `can_X` result also powers the HUD hint. Follow this pattern for new actions.
- **Don't hardcode colors**: use `config::TERMINAL_FG` / `TERMINAL_BG` / faction colors. This makes the game retheme-able.
- **No unused imports**: build warnings for unused imports are flagged by the architecture doc as "a sign the wiring was skipped". Take them seriously.

## Build

```
cargo build        # compile
cargo run          # run the game (attaches to your terminal)
```

There are no tests yet. `cargo check` is the fastest way to verify edits compile.
