use super::faction::FactionId;

pub struct Npc {
    pub x: u16,
    pub y: u16,
    pub faction: FactionId,
    pub home_capital_idx: usize,
    pub last_move: std::time::Instant,
}
