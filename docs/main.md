# main.rs

## Purpose
Entry point. Owns the terminal lifecycle (raw mode, alternate screen, panic handler), constructs the `GameState`, and drives the single game loop that pumps state updates, rendering, and input each frame.

## Structure
- **Panic handler** — installed before anything so a crash restores the terminal cleanly (disable raw mode + leave alt screen) before the default panic hook runs. Without it, a panic leaves the user's terminal broken.
- **Terminal setup** — enables raw mode, enters alternate screen, wraps stdout in a ratatui `Terminal<CrosstermBackend>`.
- **State construction** — `GameState::new(MAP_WIDTH, MAP_HEIGHT, MAP_SEED)` from config.
- **Game loop**:
  1. Call every `update_*` / `check_*` method on `state` (animation, extraction, claim, city founding, camp founding, build, wall building, decay, dehydration, npc movement). Order matters for some (e.g. check completions before deciding what the player can do visually).
  2. Call `render::render(f, &state)` via `terminal.draw`.
  3. `event::poll(16ms)` → read keys → route to `state` actions.
- **Input routing** — arrow keys are hardcoded for movement. Everything else matches against `config::KEY_*` char constants via `if let KeyCode::Char(ch)` + guarded `match ch { c if c == KEY_X => ... }` pattern (needed because const pattern matching doesn't work with non-literal constants). Shift+1/2/3 arrive as `!`/`@`/`#`.
- **Teardown** — disable raw mode + leave alternate screen before returning from main.

## Key pattern: matching config key constants
Rust's `match` can't match on non-literal constants directly. The main loop uses:
```rust
if let KeyCode::Char(ch) = key.code {
    match ch {
        c if c == KEY_QUIT => break,
        ...
    }
}
```
If you add a new keybinding, add a `KEY_*` to config and a matching `c if c == KEY_* => ...` arm here.

## Notes
- The `check_*` methods (check_extraction, check_claim, check_found_city, check_build, check_found_camp, check_build_wall) run *before* render each frame so completion flashes are visible on the frame they happen.
- `state.update_npcs()` uses a time-based cooldown per NPC (`NPC_BASE_MOVE_MS` minus fuel bonus). It's called every frame but most iterations are no-ops because the cooldown hasn't elapsed.
- `event::poll` uses a 16ms timeout which effectively caps the loop at ~60fps when idle. This is the only place `.await`-like waiting happens.
