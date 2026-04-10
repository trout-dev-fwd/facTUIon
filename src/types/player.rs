use super::faction::FactionId;
use super::terrain::Terrain;

pub struct Player {
    pub x: u16,
    pub y: u16,
    pub faction: FactionId,
    pub home_capital_idx: usize,
    pub water: u32,
    pub fuel: u32,
    pub scrap: u32,
    pub crowns: u32,
    pub extracting: Option<ExtractionState>,
    pub claiming: Option<ClaimState>,
    pub founding: Option<FoundState>,
    pub building: Option<BuildState>,
    pub founding_camp: Option<FoundCampState>,
    pub building_wall: Option<BuildWallState>,
}

pub struct ExtractionState {
    pub target_x: u16,
    pub target_y: u16,
    pub terrain: Terrain,
    pub started: std::time::Instant,
}

pub struct ClaimState {
    pub started: std::time::Instant,
}

pub struct FoundState {
    pub started: std::time::Instant,
}

pub struct BuildState {
    pub capital_idx: usize,
    pub started: std::time::Instant,
}

pub struct FoundCampState {
    pub started: std::time::Instant,
}

pub struct BuildWallState {
    pub started: std::time::Instant,
}

impl Player {
    pub fn carrying(&self) -> u32 {
        self.water + self.fuel + self.scrap
    }
}
