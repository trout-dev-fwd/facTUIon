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
    /// extraction timer. `tx, ty` name the specific resource tile being
    /// worked so other same-faction NPCs don't claim it simultaneously.
    Extracting {
        tx: u16,
        ty: u16,
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
    /// Per-resource carry inventory — NPCs can hold multiple items of mixed
    /// types up to `config::CARRY_CAP` total, same as the player.
    pub carrying_water: u32,
    pub carrying_fuel: u32,
    pub carrying_scrap: u32,
    /// A recently-abandoned target tile that pathfinding couldn't reach.
    /// `pick_harvest_target` skips this tile in its first pass, forcing the
    /// NPC to try a *different* resource tile rather than getting stuck
    /// re-picking the same unreachable one. Cleared when the NPC successfully
    /// starts extracting (transitions to `Extracting`).
    pub last_failed_target: Option<(u16, u16)>,
}

impl Npc {
    /// Total number of items currently carried across all resource types.
    pub fn carrying_total(&self) -> u32 {
        self.carrying_water + self.carrying_fuel + self.carrying_scrap
    }
}
