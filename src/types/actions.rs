use super::capital::{Capital, CapitalKind};
use super::faction::FactionId;
use super::player::{
    BuildState, BuildWallState, ClaimState, ExtractionState, FoundCampState, FoundState, Player,
};
use super::state::GameState;
use super::terrain::Terrain;

impl GameState {
    // ---------- Extraction ----------

    /// Find an adjacent resource tile the player can extract from.
    pub fn adjacent_resource(&self) -> Option<(u16, u16, Terrain)> {
        let px = self.player.x as i16;
        let py = self.player.y as i16;
        let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        for (dx, dy) in dirs {
            let nx = px + dx;
            let ny = py + dy;
            if nx >= 0 && ny >= 0 && (nx as usize) < self.map[0].len() && (ny as usize) < self.map.len() {
                let tile = &self.map[ny as usize][nx as usize];
                match tile.terrain {
                    Terrain::Water | Terrain::Rocky | Terrain::Ruins => {
                        return Some((nx as u16, ny as u16, tile.terrain));
                    }
                    _ => {}
                }
            }
        }
        None
    }

    /// Press E: start extraction if adjacent to a resource and not already extracting.
    pub fn start_extract(&mut self) {
        if self.player.extracting.is_some() {
            return;
        }
        if self.player.carrying() >= crate::config::CARRY_CAP {
            return;
        }
        if let Some((tx, ty, terrain)) = self.adjacent_resource() {
            self.player.extracting = Some(ExtractionState {
                target_x: tx,
                target_y: ty,
                terrain,
                started: std::time::Instant::now(),
            });
        }
    }

    /// Check if extraction completed (called each frame).
    pub fn check_extraction(&mut self) {
        if let Some(ref state) = self.player.extracting {
            let elapsed = std::time::Instant::now().duration_since(state.started).as_millis();
            if elapsed >= crate::config::EXTRACT_TIME_MS as u128 {
                if self.player.carrying() < crate::config::CARRY_CAP {
                    match state.terrain {
                        Terrain::Water => self.player.water += 1,
                        Terrain::Rocky => self.player.fuel += 1,
                        Terrain::Ruins => self.player.scrap += 1,
                        _ => {}
                    }
                }
                self.player.extracting = None;
            }
        }
    }

    // ---------- Claim ----------

    /// Can the player claim the tile they're standing on?
    pub fn can_claim(&self) -> bool {
        let tile = &self.map[self.player.y as usize][self.player.x as usize];
        if tile.terrain != Terrain::Wasteland {
            return false;
        }
        if tile.owner == Some(self.player.faction) {
            return false;
        }
        if self.player.scrap < crate::config::CLAIM_SCRAP_COST {
            return false;
        }
        if self.player.extracting.is_some() || self.player.claiming.is_some() {
            return false;
        }
        // Must be cardinally adjacent to a tile the player's faction already owns.
        // This forces territory to grow as a connected shape from an existing foothold.
        let px = self.player.x as i16;
        let py = self.player.y as i16;
        let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let w = self.map[0].len() as i16;
        let h = self.map.len() as i16;
        for (dx, dy) in dirs {
            let nx = px + dx;
            let ny = py + dy;
            if nx < 0 || ny < 0 || nx >= w || ny >= h {
                continue;
            }
            if self.map[ny as usize][nx as usize].owner == Some(self.player.faction) {
                return true;
            }
        }
        false
    }

    /// Press F: start claiming the tile the player is standing on.
    pub fn start_claim(&mut self) {
        if !self.can_claim() {
            return;
        }
        self.player.claiming = Some(ClaimState {
            started: std::time::Instant::now(),
        });
    }

    /// Effective claim time for the tile the player is standing on.
    pub fn claim_time_ms(&self) -> u64 {
        let tile = &self.map[self.player.y as usize][self.player.x as usize];
        let base = crate::config::CLAIM_TIME_MS;
        match tile.owner {
            Some(f) if f != self.player.faction => {
                (base as f32 * crate::config::CLAIM_CONTESTED_MULTIPLIER) as u64
            }
            _ => base,
        }
    }

    /// Check if claim completed (called each frame).
    pub fn check_claim(&mut self) {
        if let Some(ref state) = self.player.claiming {
            let elapsed = std::time::Instant::now().duration_since(state.started).as_millis();
            let required = self.claim_time_ms();
            if elapsed >= required as u128 {
                self.player.scrap -= crate::config::CLAIM_SCRAP_COST;
                let px = self.player.x as usize;
                let py = self.player.y as usize;
                self.map[py][px].owner = Some(self.player.faction);
                self.player.claiming = None;
            }
        }
    }

    // ---------- Trade ----------

    /// Sell a resource to the adjacent capital: player loses resource, gains crowns, capital gains resource.
    /// `resource`: 1 = water, 2 = fuel, 3 = scrap (matches HUD order).
    pub fn sell_resource(&mut self, resource: usize) {
        let cap_idx = match self.adjacent_capital_idx() {
            Some(i) => i,
            None => return,
        };
        let (player_has, player_field, cap_field): (
            bool,
            fn(&mut Player) -> &mut u32,
            fn(&mut Capital) -> &mut u32,
        ) = match resource {
            1 => (
                self.player.water > 0,
                |p: &mut Player| &mut p.water,
                |c: &mut Capital| &mut c.water,
            ),
            2 => (
                self.player.fuel > 0,
                |p: &mut Player| &mut p.fuel,
                |c: &mut Capital| &mut c.fuel,
            ),
            3 => (
                self.player.scrap > 0,
                |p: &mut Player| &mut p.scrap,
                |c: &mut Capital| &mut c.scrap,
            ),
            _ => return,
        };
        if !player_has {
            return;
        }

        *player_field(&mut self.player) -= 1;
        let max = self.capitals[cap_idx].resource_cap();
        let cap = &mut self.capitals[cap_idx];
        let cap_val = cap_field(cap);
        *cap_val = (*cap_val + 1).min(max);
        self.player.crowns += crate::config::BASE_SELL_PRICE;

        // Any sale may enable growth or a city upgrade.
        self.try_grow_or_upgrade(cap_idx);
    }

    /// Buy a resource from the adjacent capital: player gains resource, loses crowns, capital loses resource.
    pub fn buy_resource(&mut self, resource: usize) {
        let cap_idx = match self.adjacent_capital_idx() {
            Some(i) => i,
            None => return,
        };
        if self.player.crowns < crate::config::BASE_BUY_PRICE {
            return;
        }
        if self.player.carrying() >= crate::config::CARRY_CAP {
            return;
        }

        let (cap_has, player_field, cap_field): (
            bool,
            fn(&mut Player) -> &mut u32,
            fn(&mut Capital) -> &mut u32,
        ) = match resource {
            1 => (
                self.capitals[cap_idx].water > 0,
                |p: &mut Player| &mut p.water,
                |c: &mut Capital| &mut c.water,
            ),
            2 => (
                self.capitals[cap_idx].fuel > 0,
                |p: &mut Player| &mut p.fuel,
                |c: &mut Capital| &mut c.fuel,
            ),
            3 => (
                self.capitals[cap_idx].scrap > 0,
                |p: &mut Player| &mut p.scrap,
                |c: &mut Capital| &mut c.scrap,
            ),
            _ => return,
        };
        if !cap_has {
            return;
        }

        self.player.crowns -= crate::config::BASE_BUY_PRICE;
        *player_field(&mut self.player) += 1;
        let cap = &mut self.capitals[cap_idx];
        *cap_field(cap) -= 1;
    }

    // ---------- City founding (foundation → build) ----------

    /// Can the player lay a foundation where they stand?
    pub fn can_found_city(&self) -> bool {
        if self.player.scrap < crate::config::FOUNDATION_SCRAP_COST {
            return false;
        }
        if self.player.extracting.is_some()
            || self.player.claiming.is_some()
            || self.player.founding.is_some()
        {
            return false;
        }
        let w = self.map[0].len() as i16;
        let h = self.map.len() as i16;
        let px = self.player.x as i16;
        let py = self.player.y as i16;
        if px < 2 || px >= w - 2 || py < 2 || py >= h - 2 {
            return false;
        }

        // Center tile must be claimed by the player's faction
        let center = &self.map[py as usize][px as usize];
        if center.owner != Some(self.player.faction) {
            return false;
        }

        // 5x5 area must be clear of resources, walls, and existing capitals
        for dy in -2i16..=2 {
            for dx in -2i16..=2 {
                let tx = (px + dx) as u16;
                let ty = (py + dy) as u16;
                let tile = &self.map[ty as usize][tx as usize];
                if tile.terrain != Terrain::Wasteland {
                    return false;
                }
                if self.is_capital_area(tx, ty) || self.capital_border_at(tx, ty).is_some() {
                    return false;
                }
            }
        }
        true
    }

    /// Press C (on claimed tile): start laying the foundation for a new city.
    pub fn start_found_city(&mut self) {
        if !self.can_found_city() {
            return;
        }
        self.player.founding = Some(FoundState {
            started: std::time::Instant::now(),
        });
    }

    /// Find the index of a foundation (incomplete capital) adjacent to the player.
    pub fn adjacent_foundation_idx(&self) -> Option<usize> {
        let px = self.player.x as i16;
        let py = self.player.y as i16;
        let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        for (i, cap) in self.capitals.iter().enumerate() {
            if cap.is_complete() || cap.faction != self.player.faction {
                continue;
            }
            for (dx, dy) in dirs {
                let nx = px + dx;
                let ny = py + dy;
                if nx >= 0 && ny >= 0 && cap.is_inside(nx as u16, ny as u16) {
                    return Some(i);
                }
            }
        }
        None
    }

    pub fn can_add_to_foundation(&self) -> bool {
        if self.player.building.is_some() {
            return false;
        }
        if self.player.scrap == 0 {
            return false;
        }
        match self.adjacent_foundation_idx() {
            Some(i) => self.capitals[i].scrap_invested < crate::config::CITY_TOTAL_SCRAP,
            None => false,
        }
    }

    /// Press C (adjacent to own foundation): start a timed scrap deposit.
    pub fn start_add_to_foundation(&mut self) {
        if !self.can_add_to_foundation() {
            return;
        }
        let idx = match self.adjacent_foundation_idx() {
            Some(i) => i,
            None => return,
        };
        self.player.building = Some(BuildState {
            capital_idx: idx,
            started: std::time::Instant::now(),
        });
    }

    /// Complete the scrap deposit when the timer elapses.
    pub fn check_build(&mut self) {
        if let Some(ref state) = self.player.building {
            let elapsed = std::time::Instant::now().duration_since(state.started).as_millis();
            if elapsed >= crate::config::BUILD_SCRAP_TIME_MS as u128 {
                let idx = state.capital_idx;
                self.player.building = None;
                if idx >= self.capitals.len() {
                    return;
                }
                if self.adjacent_foundation_idx() != Some(idx) {
                    return;
                }
                if self.player.scrap == 0 {
                    return;
                }
                if self.capitals[idx].scrap_invested >= crate::config::CITY_TOTAL_SCRAP {
                    return;
                }
                self.player.scrap -= 1;
                self.capitals[idx].scrap_invested += 1;
            }
        }
    }

    /// Check if city founding completed (creates a foundation at the player's spot).
    pub fn check_found_city(&mut self) {
        if let Some(ref state) = self.player.founding {
            let elapsed = std::time::Instant::now().duration_since(state.started).as_millis();
            if elapsed >= crate::config::FOUND_CITY_TIME_MS as u128 {
                let px = self.player.x;
                let py = self.player.y;
                let faction = self.player.faction;
                self.player.scrap -= crate::config::FOUNDATION_SCRAP_COST;
                self.player.founding = None;

                self.capitals.push(Capital {
                    x: px,
                    y: py,
                    faction,
                    water: 0,
                    fuel: 0,
                    scrap: 0,
                    crowns: 0,
                    scrap_invested: crate::config::FOUNDATION_SCRAP_COST,
                    kind: CapitalKind::City,
                    tier: 1,
                });

                let cx = px as i16;
                let cy = py as i16;
                let w = self.map[0].len() as i16;
                let h = self.map.len() as i16;

                // Evict any NPCs inside the new 3x3 area to an adjacent open tile
                let mut evicted: Vec<usize> = Vec::new();
                for (i, npc) in self.npcs.iter().enumerate() {
                    let dx = (npc.x as i16 - cx).abs();
                    let dy = (npc.y as i16 - cy).abs();
                    if dx <= 1 && dy <= 1 {
                        evicted.push(i);
                    }
                }
                for i in evicted {
                    let candidates: [(i16, i16); 4] = [(0, -2), (2, 0), (0, 2), (-2, 0)];
                    for (dx, dy) in candidates {
                        let nx = cx + dx;
                        let ny = cy + dy;
                        if nx >= 0 && nx < w && ny >= 0 && ny < h {
                            if !self.is_blocked_for_npc(nx, ny, i) {
                                self.move_npc_to(i, nx as u16, ny as u16);
                                break;
                            }
                        }
                    }
                }

                // Nudge player off the 3x3 walls
                for (dx, dy) in [(0, -2i16), (2, 0), (0, 2), (-2, 0)] {
                    let nx = cx + dx;
                    let ny = cy + dy;
                    if nx >= 0 && nx < w && ny >= 0 && ny < h {
                        if !self.is_blocked(nx, ny) {
                            self.player.x = nx as u16;
                            self.player.y = ny as u16;
                            break;
                        }
                    }
                }
            }
        }
    }

    // ---------- Camp founding ----------

    /// Can the player found a camp where they stand?
    pub fn can_found_camp(&self) -> bool {
        if self.player.scrap < crate::config::CAMP_SCRAP_COST {
            return false;
        }
        if self.player.extracting.is_some()
            || self.player.claiming.is_some()
            || self.player.founding.is_some()
            || self.player.building.is_some()
            || self.player.founding_camp.is_some()
        {
            return false;
        }
        let w = self.map[0].len() as i16;
        let h = self.map.len() as i16;
        let px = self.player.x as i16;
        let py = self.player.y as i16;
        if px < 1 || px >= w - 1 || py < 1 || py >= h - 1 {
            return false;
        }

        // Center + 4 cardinal walls must all be wasteland and clear of capitals
        let offsets: [(i16, i16); 5] = [(0, 0), (0, -1), (0, 1), (-1, 0), (1, 0)];
        for (dx, dy) in offsets {
            let tx = (px + dx) as u16;
            let ty = (py + dy) as u16;
            let tile = &self.map[ty as usize][tx as usize];
            if tile.terrain != Terrain::Wasteland {
                return false;
            }
            if self.is_capital_area(tx, ty) {
                return false;
            }
        }
        true
    }

    /// Press V: start founding a camp where the player stands.
    pub fn start_found_camp(&mut self) {
        if !self.can_found_camp() {
            return;
        }
        self.player.founding_camp = Some(FoundCampState {
            started: std::time::Instant::now(),
        });
    }

    /// Check if camp founding completed.
    pub fn check_found_camp(&mut self) {
        if let Some(ref state) = self.player.founding_camp {
            let elapsed = std::time::Instant::now().duration_since(state.started).as_millis();
            if elapsed >= crate::config::FOUND_CAMP_TIME_MS as u128 {
                let px = self.player.x;
                let py = self.player.y;
                self.player.scrap -= crate::config::CAMP_SCRAP_COST;
                self.player.founding_camp = None;

                // Camps always belong to the Cult faction, regardless of who built them
                self.capitals.push(Capital {
                    x: px,
                    y: py,
                    faction: FactionId::Cult,
                    kind: CapitalKind::Camp,
                    water: 0,
                    fuel: 0,
                    scrap: 0,
                    crowns: 0,
                    scrap_invested: 0,
                    tier: 1,
                });

                let cx = px as i16;
                let cy = py as i16;
                let w = self.map[0].len() as i16;
                let h = self.map.len() as i16;

                // Evict any NPCs inside the new + footprint
                let mut evicted: Vec<usize> = Vec::new();
                for (i, npc) in self.npcs.iter().enumerate() {
                    let ddx = (npc.x as i16 - cx).abs();
                    let ddy = (npc.y as i16 - cy).abs();
                    if (ddx == 0 && ddy == 0) || (ddx + ddy == 1) {
                        evicted.push(i);
                    }
                }
                for i in evicted {
                    // Try diagonal offsets (outside the + shape)
                    let candidates: [(i16, i16); 4] = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
                    for (dx, dy) in candidates {
                        let nx = cx + dx;
                        let ny = cy + dy;
                        if nx >= 0 && nx < w && ny >= 0 && ny < h {
                            if !self.is_blocked_for_npc(nx, ny, i) {
                                self.move_npc_to(i, nx as u16, ny as u16);
                                break;
                            }
                        }
                    }
                }

                // Nudge player off the center (to a diagonal corner)
                for (dx, dy) in [(-1, -1i16), (1, -1), (-1, 1), (1, 1)] {
                    let nx = cx + dx;
                    let ny = cy + dy;
                    if nx >= 0 && nx < w && ny >= 0 && ny < h {
                        if !self.is_blocked(nx, ny) {
                            self.player.x = nx as u16;
                            self.player.y = ny as u16;
                            break;
                        }
                    }
                }
            }
        }
    }

    // ---------- Wall building ----------

    /// Can the player build a wall segment under their feet?
    pub fn can_build_wall(&self) -> bool {
        if self.player.scrap < crate::config::WALL_SCRAP_COST {
            return false;
        }
        if self.player.extracting.is_some()
            || self.player.claiming.is_some()
            || self.player.founding.is_some()
            || self.player.building.is_some()
            || self.player.founding_camp.is_some()
            || self.player.building_wall.is_some()
        {
            return false;
        }
        let px = self.player.x;
        let py = self.player.y;
        let tile = &self.map[py as usize][px as usize];
        if tile.terrain != Terrain::Wasteland {
            return false;
        }
        if tile.wall.is_some() {
            return false;
        }
        if self.is_capital_area(px, py) {
            return false;
        }
        true
    }

    pub fn start_build_wall(&mut self) {
        if !self.can_build_wall() {
            return;
        }
        self.player.building_wall = Some(BuildWallState {
            started: std::time::Instant::now(),
        });
    }

    /// Effective wall build time for the tile the player is standing on.
    /// Base time on own territory; `WALL_UNCLAIMED_MULTIPLIER`x on unclaimed or enemy tiles.
    pub fn build_wall_time_ms(&self) -> u64 {
        let tile = &self.map[self.player.y as usize][self.player.x as usize];
        let base = crate::config::BUILD_WALL_TIME_MS;
        match tile.owner {
            Some(f) if f == self.player.faction => base,
            _ => (base as f32 * crate::config::WALL_UNCLAIMED_MULTIPLIER) as u64,
        }
    }

    pub fn check_build_wall(&mut self) {
        if let Some(ref state) = self.player.building_wall {
            let elapsed = std::time::Instant::now().duration_since(state.started).as_millis();
            let required = self.build_wall_time_ms();
            if elapsed >= required as u128 {
                let px = self.player.x;
                let py = self.player.y;
                let faction = self.player.faction;
                self.player.scrap -= crate::config::WALL_SCRAP_COST;
                self.player.building_wall = None;

                // Walls no longer claim territory — only set the wall flag.
                let tile = &mut self.map[py as usize][px as usize];
                tile.wall = Some(faction);

                // Nudge player off the new wall tile
                let w = self.map[0].len() as i16;
                let h = self.map.len() as i16;
                let cx = px as i16;
                let cy = py as i16;
                let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
                for (dx, dy) in dirs {
                    let nx = cx + dx;
                    let ny = cy + dy;
                    if nx >= 0 && nx < w && ny >= 0 && ny < h {
                        if !self.is_blocked(nx, ny) {
                            self.player.x = nx as u16;
                            self.player.y = ny as u16;
                            break;
                        }
                    }
                }
            }
        }
    }
}
