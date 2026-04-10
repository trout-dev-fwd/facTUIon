use super::faction::FactionId;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CapitalKind {
    City,
    Camp,
}

pub struct Capital {
    pub x: u16,
    pub y: u16,
    pub faction: FactionId,
    pub kind: CapitalKind,
    pub water: u32,
    pub fuel: u32,
    pub scrap: u32,
    pub crowns: u32,
    pub scrap_invested: u32, // cities: 5 = foundation, 10 = complete. camps: always complete.
}

impl Capital {
    pub fn is_complete(&self) -> bool {
        match self.kind {
            CapitalKind::City => self.scrap_invested >= crate::config::CITY_TOTAL_SCRAP,
            CapitalKind::Camp => true,
        }
    }

    /// The letter glyph shown at the center tile.
    pub fn center_glyph(&self) -> char {
        self.faction.glyph()
    }

    /// Returns true if (x, y) is one of this capital's footprint tiles
    /// (including center and walls).
    pub fn is_inside(&self, x: u16, y: u16) -> bool {
        let dx = (x as i16 - self.x as i16).abs();
        let dy = (y as i16 - self.y as i16).abs();
        match self.kind {
            CapitalKind::City => dx <= 1 && dy <= 1,
            CapitalKind::Camp => (dx == 0 && dy == 0) || (dx + dy == 1),
        }
    }

    /// How many fuel speed thresholds this capital has reached.
    pub fn fuel_tiers(&self) -> u32 {
        crate::config::FUEL_THRESHOLDS
            .iter()
            .filter(|&&t| self.fuel >= t)
            .count() as u32
    }

    /// NPC move cooldown in ms for a given carry weight, reduced by fuel bonuses.
    /// Uses the per-weight `NPC_MOVE_COOLDOWN` table (heavier NPCs are slower)
    /// and then applies a percentage reduction for each fuel threshold the
    /// capital has hit.
    pub fn npc_move_cooldown(&self, weight: u32) -> u64 {
        let weight_idx = (weight as usize).min(crate::config::NPC_MOVE_COOLDOWN.len() - 1);
        let base = crate::config::NPC_MOVE_COOLDOWN[weight_idx];
        let bonus_pct = self.fuel_tiers() * crate::config::FUEL_SPEED_BONUS_PCT;
        let reduction = (base * bonus_pct as u64) / 100;
        base.saturating_sub(reduction)
    }
}
