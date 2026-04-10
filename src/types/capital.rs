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
    /// City upgrade tier (1-5). Higher tiers increase resource cap, NPC target
    /// population, and the upgrade cost for the next step. Camps stay at tier 1
    /// and currently don't upgrade.
    pub tier: u32,
    /// Pre-computed fortress wall tiles. Each entry is a tile that should be
    /// turned into a wall once the faction owns it. The list is produced at
    /// map generation time by `compute_fortress_walls` (in state.rs) based on
    /// a bounding box around the capital and its primary resource cluster,
    /// with a 2-tile buffer. Gate tiles (the gap in the wall) are already
    /// excluded from this list. Only starting cities get a populated list;
    /// founded cities and camps currently leave this empty.
    pub fortress_walls: Vec<(u16, u16)>,
}

impl Capital {
    pub fn is_complete(&self) -> bool {
        match self.kind {
            CapitalKind::City => self.scrap_invested >= crate::config::CITY_TOTAL_SCRAP,
            CapitalKind::Camp => true,
        }
    }

    /// 2-character label for the center tile of the map render. Cities show
    /// the faction letter + tier digit ("W1", "G3", etc.); camps show the
    /// faction letter + space ("C ").
    pub fn center_label(&self) -> String {
        match self.kind {
            CapitalKind::City => format!("{}{}", self.faction.glyph(), self.tier),
            CapitalKind::Camp => format!("{} ", self.faction.glyph()),
        }
    }

    /// Maximum stockpile this capital can hold, scaling with tier. Tier 1 = 20,
    /// tier 2 = 40, ..., tier 5 = 100.
    pub fn resource_cap(&self) -> u32 {
        self.tier * crate::config::MAX_STOCKPILE
    }

    /// NPC harvest stops once a resource reaches this amount (a small buffer
    /// below the effective cap). NPCs resume once decay drops the stockpile
    /// back below this point.
    pub fn harvest_threshold(&self) -> u32 {
        self.resource_cap().saturating_sub(crate::config::HOARD_BUFFER)
    }

    /// Target population for this capital — NPCs grow via the water-spend
    /// mechanic until the population reaches this count, then the AI shifts
    /// toward upgrading instead.
    pub fn npc_target(&self) -> u32 {
        self.resource_cap() / 4
    }

    /// Per-resource cost to upgrade from the current tier to the next. Paid
    /// simultaneously in water, fuel, and scrap.
    pub fn upgrade_cost(&self) -> u32 {
        self.tier * crate::config::BASE_UPGRADE_COST
    }

    /// True if this city could be upgraded right now: not at max tier, and
    /// all three stockpiles cover the cost.
    pub fn can_upgrade(&self) -> bool {
        if self.kind != CapitalKind::City {
            return false;
        }
        if self.tier >= crate::config::MAX_CITY_TIER {
            return false;
        }
        let cost = self.upgrade_cost();
        self.water >= cost && self.fuel >= cost && self.scrap >= cost
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

    /// Apply this capital's fuel-tier percentage reduction to a base cooldown.
    /// Shared between NPC movement and the player's movement so both benefit
    /// from their home capital's fuel stockpile in the same way.
    pub fn apply_fuel_bonus(&self, base_ms: u64) -> u64 {
        let bonus_pct = self.fuel_tiers() * crate::config::FUEL_SPEED_BONUS_PCT;
        let reduction = (base_ms * bonus_pct as u64) / 100;
        base_ms.saturating_sub(reduction)
    }

    /// NPC move cooldown in ms for a given carry weight, reduced by fuel bonuses.
    /// Looks up the per-weight base from `NPC_MOVE_COOLDOWN` and then applies
    /// `apply_fuel_bonus`.
    pub fn npc_move_cooldown(&self, weight: u32) -> u64 {
        let weight_idx = (weight as usize).min(crate::config::NPC_MOVE_COOLDOWN.len() - 1);
        self.apply_fuel_bonus(crate::config::NPC_MOVE_COOLDOWN[weight_idx])
    }
}
