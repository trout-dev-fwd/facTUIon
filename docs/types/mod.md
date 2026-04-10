# types/mod.rs

## Purpose
Module root for the `types` namespace. Declares all submodules and re-exports their public types so external code can continue to use `crate::types::GameState`, `crate::types::Terrain`, etc., without knowing about the internal file split.

## Contents
- `mod actions; mod capital; mod faction; mod npc; mod player; mod state; mod terrain;` — declares each submodule.
- `pub use <module>::*;` for each submodule — re-exports every public item so callers don't have to care about which file a type lives in.
- `#[allow(unused_imports)]` on each re-export line — a few types (FactionId, Npc, Player, etc.) are only accessed via field access from outside the module (e.g. `state.player.faction`), so the compiler flags the re-export as unused. They're kept exported for API completeness.

## Notes
- If you add a new submodule, declare it here and add a matching `pub use` line.
- There are no impl blocks or logic in this file — it's purely module glue.
