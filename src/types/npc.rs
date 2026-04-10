use super::faction::FactionId;
use super::terrain::Terrain;

/// What an NPC is currently trying to do. The state machine advances in
/// `GameState::update_npcs`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NpcTask {
    /// Idle — no active goal. On the next tick the NPC will try to pick a
    /// harvest target; if none is available it takes a random wander step.
    Wandering,
    /// Walking toward a specific resource tile to extract from.
    TargetingResource {
        tx: u16,
        ty: u16,
        terrain: Terrain,
    },
    /// Stationary on the tile adjacent to the resource, running the
    /// extraction timer. `terrain` is the resource type being produced.
    Extracting {
        started: std::time::Instant,
        terrain: Terrain,
    },
    /// Carrying a resource back to the home capital to deposit.
    Returning,
}

pub struct Npc {
    pub x: u16,
    pub y: u16,
    pub faction: FactionId,
    pub home_capital_idx: usize,
    pub last_move: std::time::Instant,
    pub task: NpcTask,
    /// The single resource type this NPC is currently carrying (at most 1 item
    /// at a time for Phase 2 harvesters). `None` means hands are free.
    pub carrying: Option<Terrain>,
}
