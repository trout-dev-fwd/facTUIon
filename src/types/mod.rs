mod actions;
mod capital;
mod faction;
mod npc;
mod player;
mod state;
mod terrain;

// Re-export everything so `crate::types::X` works for all public types.
// Some (FactionId, Npc, Player, etc.) are only accessed via field access today
// so they appear "unused" from the compiler's perspective, but we keep them
// exported for API completeness.
#[allow(unused_imports)]
pub use capital::*;
#[allow(unused_imports)]
pub use faction::*;
#[allow(unused_imports)]
pub use npc::*;
#[allow(unused_imports)]
pub use player::*;
#[allow(unused_imports)]
pub use state::*;
#[allow(unused_imports)]
pub use terrain::*;
