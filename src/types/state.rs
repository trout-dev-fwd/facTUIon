use rand::rngs::SmallRng;
use rand::Rng;
use rand::SeedableRng;

use super::capital::{Capital, CapitalKind};
use super::faction::FactionId;
use super::npc::Npc;
use super::player::Player;
use super::terrain::{Terrain, Tile};

pub struct GameState {
    pub map: Vec<Vec<Tile>>,
    pub capitals: Vec<Capital>,
    pub npcs: Vec<Npc>,
    pub player: Player,
    pub sim_rng: SmallRng,
    pub last_move: std::time::Instant,
    pub anim_tick: u64,
    pub last_anim: std::time::Instant,
    pub last_decay: std::time::Instant,
    pub last_dehydration: std::time::Instant,
}

impl GameState {
    pub fn new(width: u16, height: u16, seed: u64) -> Self {
        let mut rng = SmallRng::seed_from_u64(seed);
        let (mut map, capital_positions) = crate::map::generate_map(width, height, &mut rng);

        let factions = [FactionId::Water, FactionId::Gas, FactionId::Scrap];
        let capitals: Vec<Capital> = capital_positions
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| Capital {
                x: x as u16,
                y: y as u16,
                faction: factions[i],
                water: crate::config::STARTING_STOCKPILE,
                fuel: crate::config::STARTING_STOCKPILE,
                scrap: crate::config::STARTING_STOCKPILE,
                crowns: crate::config::STARTING_CROWNS,
                scrap_invested: crate::config::CITY_TOTAL_SCRAP,
                kind: CapitalKind::City,
            })
            .collect();

        // Claim starting territory around each capital (radius from border edge, not center)
        let radius = crate::config::CAPITAL_TERRITORY_RADIUS as i16 + 1; // +1 to account for border
        for cap in &capitals {
            let cx = cap.x as i16;
            let cy = cap.y as i16;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let tx = cx + dx;
                    let ty = cy + dy;
                    if tx >= 0
                        && tx < width as i16
                        && ty >= 0
                        && ty < height as i16
                        && dx.abs() + dy.abs() <= radius
                    {
                        let tile = &mut map[ty as usize][tx as usize];
                        if tile.terrain == Terrain::Wasteland {
                            tile.owner = Some(cap.faction);
                        }
                    }
                }
            }
        }

        // Assign player to a random faction (time-seeded so it varies between runs)
        let player_faction = {
            let mut player_rng = SmallRng::seed_from_u64(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64,
            );
            factions[player_rng.gen_range(0..factions.len())]
        };

        // Spawn player adjacent to their faction's capital
        let (player_cap_idx, player_cap) = capitals
            .iter()
            .enumerate()
            .find(|(_, c)| c.faction == player_faction)
            .unwrap();
        let (px, py) = find_open_adjacent(&map, &capitals, player_cap.x, player_cap.y, width, height);

        // Spawn NPCs for each faction
        let mut npcs = Vec::new();
        for (cap_idx, cap) in capitals.iter().enumerate() {
            let count = if cap.faction == player_faction {
                crate::config::NPCS_PER_FACTION - 1
            } else {
                crate::config::NPCS_PER_FACTION
            };
            for _ in 0..count {
                let (nx, ny) = find_open_adjacent_avoiding(
                    &map, &capitals, &npcs, px, py, cap.x, cap.y, width, height,
                );
                npcs.push(Npc {
                    x: nx,
                    y: ny,
                    faction: cap.faction,
                    home_capital_idx: cap_idx,
                    last_move: std::time::Instant::now(),
                });
            }
        }

        GameState {
            map,
            capitals,
            npcs,
            player: Player {
                x: px,
                y: py,
                faction: player_faction,
                home_capital_idx: player_cap_idx,
                water: 0,
                fuel: 0,
                scrap: crate::config::PLAYER_STARTING_SCRAP,
                crowns: 0,
                extracting: None,
                claiming: None,
                founding: None,
                building: None,
                founding_camp: None,
                building_wall: None,
            },
            sim_rng: SmallRng::seed_from_u64(seed ^ 0xDEAD_BEEF),
            last_move: std::time::Instant::now(),
            anim_tick: 0,
            last_anim: std::time::Instant::now(),
            last_decay: std::time::Instant::now(),
            last_dehydration: std::time::Instant::now(),
        }
    }

    // ---------- Queries ----------

    pub fn capital_at(&self, x: u16, y: u16) -> Option<&Capital> {
        self.capitals.iter().find(|c| c.x == x && c.y == y)
    }

    /// Returns the capital whose wall is at (x, y), if any. Includes both
    /// city walls (box-drawing) and camp walls (✗).
    pub fn capital_border_at(&self, x: u16, y: u16) -> Option<&Capital> {
        for cap in &self.capitals {
            let dx = x as i16 - cap.x as i16;
            let dy = y as i16 - cap.y as i16;
            match cap.kind {
                CapitalKind::City => {
                    if (dx.abs() == 1 || dy.abs() == 1)
                        && dx.abs() <= 1
                        && dy.abs() <= 1
                        && !(dx == 0 && dy == 0)
                    {
                        return Some(cap);
                    }
                }
                CapitalKind::Camp => {
                    if (dx == 0 && dy.abs() == 1) || (dy == 0 && dx.abs() == 1) {
                        return Some(cap);
                    }
                }
            }
        }
        None
    }

    /// Returns true if (x, y) participates in the box-wall network:
    /// - A city's 8 wall tiles
    /// - Any tile with a free-standing wall
    /// Camp walls (✗) are NOT part of this network.
    pub fn is_box_wall(&self, x: u16, y: u16) -> bool {
        if let Some(row) = self.map.get(y as usize) {
            if let Some(tile) = row.get(x as usize) {
                if tile.wall.is_some() {
                    return true;
                }
            }
        }
        for cap in &self.capitals {
            if cap.kind != CapitalKind::City {
                continue;
            }
            let dx = x as i16 - cap.x as i16;
            let dy = y as i16 - cap.y as i16;
            if (dx.abs() == 1 || dy.abs() == 1)
                && dx.abs() <= 1
                && dy.abs() <= 1
                && !(dx == 0 && dy == 0)
            {
                return true;
            }
        }
        false
    }

    /// Compute the correct box-drawing glyph for a wall at (x, y) based on neighbor connectivity.
    /// Only called for tiles that participate in the box-wall network.
    pub fn wall_glyph_at(&self, x: u16, y: u16) -> char {
        let n = y > 0 && self.is_box_wall(x, y - 1);
        let s = self.is_box_wall(x, y + 1);
        let w = x > 0 && self.is_box_wall(x - 1, y);
        let e = self.is_box_wall(x + 1, y);
        match (n, e, s, w) {
            (false, false, false, false) => '□',
            (false, true, false, false) => '═',
            (false, false, false, true) => '═',
            (false, true, false, true) => '═',
            (true, false, false, false) => '║',
            (false, false, true, false) => '║',
            (true, false, true, false) => '║',
            (false, true, true, false) => '╔',
            (false, false, true, true) => '╗',
            (true, true, false, false) => '╚',
            (true, false, false, true) => '╝',
            (false, true, true, true) => '╦',
            (true, true, false, true) => '╩',
            (true, true, true, false) => '╠',
            (true, false, true, true) => '╣',
            (true, true, true, true) => '╬',
        }
    }

    /// Returns true if (x, y) is inside any capital's footprint (center or walls).
    pub fn is_capital_area(&self, x: u16, y: u16) -> bool {
        self.capitals.iter().any(|c| c.is_inside(x, y))
    }

    /// Population assigned to a capital: NPCs + player whose `home_capital_idx` is this one.
    pub fn population_of(&self, cap_idx: usize) -> u32 {
        let mut count = self
            .npcs
            .iter()
            .filter(|n| n.home_capital_idx == cap_idx)
            .count() as u32;
        if self.player.home_capital_idx == cap_idx {
            count += 1;
        }
        count
    }

    /// Returns a capital the player is cardinally adjacent to (next to any footprint tile).
    pub fn adjacent_capital(&self) -> Option<&Capital> {
        self.adjacent_capital_idx().map(|i| &self.capitals[i])
    }

    /// Find the index of an adjacent capital (for mutable access from trade/actions).
    pub fn adjacent_capital_idx(&self) -> Option<usize> {
        let px = self.player.x as i16;
        let py = self.player.y as i16;
        let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        for (i, cap) in self.capitals.iter().enumerate() {
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

    pub fn npc_at(&self, x: u16, y: u16) -> Option<&Npc> {
        self.npcs.iter().find(|n| n.x == x && n.y == y)
    }

    // ---------- Per-frame updates ----------

    pub fn update_anim(&mut self) {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_anim).as_millis() >= crate::config::ANIM_TICK_MS as u128 {
            self.anim_tick += 1;
            self.last_anim = now;
        }
    }

    /// Phase 1: each NPC wanders one tile in a random direction on its cooldown.
    pub fn update_npcs(&mut self) {
        let now = std::time::Instant::now();
        let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

        for i in 0..self.npcs.len() {
            let faction = self.npcs[i].faction;
            let cooldown = self
                .capitals
                .iter()
                .find(|c| c.faction == faction)
                .map(|c| c.npc_move_cooldown())
                .unwrap_or(crate::config::NPC_BASE_MOVE_MS);

            let last = self.npcs[i].last_move;
            if now.duration_since(last).as_millis() < cooldown as u128 {
                continue;
            }

            let d = dirs[self.sim_rng.gen_range(0..4)];
            let nx = self.npcs[i].x as i16 + d.0;
            let ny = self.npcs[i].y as i16 + d.1;

            if self.is_blocked_for_npc(nx, ny, i) {
                // Still count this as a tick so they don't retry instantly on the next frame
                self.npcs[i].last_move = now;
                continue;
            }

            self.npcs[i].x = nx as u16;
            self.npcs[i].y = ny as u16;
            self.npcs[i].last_move = now;
        }
    }

    /// Decay resources: each capital loses 1 of each resource per assigned member every interval.
    pub fn update_decay(&mut self) {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_decay).as_millis() < crate::config::DECAY_INTERVAL_MS as u128 {
            return;
        }
        self.last_decay = now;

        let mut pops = vec![0u32; self.capitals.len()];
        for npc in &self.npcs {
            if npc.home_capital_idx < pops.len() {
                pops[npc.home_capital_idx] += 1;
            }
        }
        if self.player.home_capital_idx < pops.len() {
            pops[self.player.home_capital_idx] += 1;
        }

        for (i, cap) in self.capitals.iter_mut().enumerate() {
            let drain = pops[i];
            if crate::config::DECAY_WATER {
                cap.water = cap.water.saturating_sub(drain);
            }
            if crate::config::DECAY_FUEL {
                cap.fuel = cap.fuel.saturating_sub(drain);
            }
            if crate::config::DECAY_SCRAP {
                cap.scrap = cap.scrap.saturating_sub(drain);
            }
        }
    }

    /// Dehydration: if a capital has 0 water, lose one of its assigned NPCs.
    pub fn update_dehydration(&mut self) {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_dehydration).as_millis()
            < crate::config::DEHYDRATION_INTERVAL_MS as u128
        {
            return;
        }
        self.last_dehydration = now;

        let dehydrated: Vec<usize> = self
            .capitals
            .iter()
            .enumerate()
            .filter(|(_, c)| c.water == 0)
            .map(|(i, _)| i)
            .collect();

        for cap_idx in dehydrated {
            if let Some(idx) = self.npcs.iter().rposition(|n| n.home_capital_idx == cap_idx) {
                self.npcs.remove(idx);
            }
        }
    }

    // ---------- Blocking / movement ----------

    pub fn is_blocked(&self, x: i16, y: i16) -> bool {
        let w = self.map[0].len() as i16;
        let h = self.map.len() as i16;
        if x < 0 || x >= w || y < 0 || y >= h {
            return true;
        }
        let tile = &self.map[y as usize][x as usize];
        if tile.terrain != Terrain::Wasteland {
            return true;
        }
        if tile.wall.is_some() {
            return true;
        }
        if self.is_capital_area(x as u16, y as u16) {
            return true;
        }
        if self.npc_at(x as u16, y as u16).is_some() {
            return true;
        }
        false
    }

    /// Like is_blocked, but also considers the player position and excludes the NPC itself.
    /// Visible to `actions.rs` via `pub(super)`.
    pub(super) fn is_blocked_for_npc(&self, x: i16, y: i16, self_idx: usize) -> bool {
        let w = self.map[0].len() as i16;
        let h = self.map.len() as i16;
        if x < 0 || x >= w || y < 0 || y >= h {
            return true;
        }
        let tile = &self.map[y as usize][x as usize];
        if tile.terrain != Terrain::Wasteland {
            return true;
        }
        if tile.wall.is_some() {
            return true;
        }
        if self.is_capital_area(x as u16, y as u16) {
            return true;
        }
        if self.player.x == x as u16 && self.player.y == y as u16 {
            return true;
        }
        for (i, npc) in self.npcs.iter().enumerate() {
            if i == self_idx {
                continue;
            }
            if npc.x == x as u16 && npc.y == y as u16 {
                return true;
            }
        }
        false
    }

    pub fn move_player(&mut self, dx: i16, dy: i16) {
        let now = std::time::Instant::now();
        let weight = self.player.carrying() as usize;
        let cooldown = crate::config::MOVE_COOLDOWN[weight.min(crate::config::MOVE_COOLDOWN.len() - 1)];
        if now.duration_since(self.last_move).as_millis() < cooldown as u128 {
            return;
        }
        let new_x = self.player.x as i16 + dx;
        let new_y = self.player.y as i16 + dy;
        if self.is_blocked(new_x, new_y) {
            return;
        }
        self.player.x = new_x as u16;
        self.player.y = new_y as u16;
        self.last_move = now;
        // Moving cancels every in-progress action
        self.player.extracting = None;
        self.player.claiming = None;
        self.player.founding = None;
        self.player.building = None;
        self.player.founding_camp = None;
        self.player.building_wall = None;
    }
}

// ---------- Module-level spawn helpers ----------

/// Check if (x, y) is inside any capital's 3x3 area (for use during initial spawn,
/// before `GameState` exists).
fn in_capital_area(capitals: &[Capital], x: u16, y: u16) -> bool {
    capitals.iter().any(|c| {
        let dx = (x as i16 - c.x as i16).abs();
        let dy = (y as i16 - c.y as i16).abs();
        dx <= 1 && dy <= 1
    })
}

/// Find a walkable tile near (cx, cy) outside any capital's 3x3 area.
fn find_open_adjacent(
    map: &[Vec<Tile>],
    capitals: &[Capital],
    cx: u16,
    cy: u16,
    w: u16,
    h: u16,
) -> (u16, u16) {
    for r in 2..w.max(h) as i16 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() + dy.abs() > r {
                    continue;
                }
                let nx = cx as i16 + dx;
                let ny = cy as i16 + dy;
                if nx >= 0 && nx < w as i16 && ny >= 0 && ny < h as i16 {
                    let (ux, uy) = (nx as u16, ny as u16);
                    let tile = &map[ny as usize][nx as usize];
                    if tile.terrain == Terrain::Wasteland && !in_capital_area(capitals, ux, uy) {
                        return (ux, uy);
                    }
                }
            }
        }
    }
    (cx, cy)
}

/// Find an open tile near (cx, cy) that also avoids existing NPCs and the player.
fn find_open_adjacent_avoiding(
    map: &[Vec<Tile>],
    capitals: &[Capital],
    npcs: &[Npc],
    player_x: u16,
    player_y: u16,
    cx: u16,
    cy: u16,
    w: u16,
    h: u16,
) -> (u16, u16) {
    let is_occupied = |x: u16, y: u16| -> bool {
        (x == player_x && y == player_y)
            || npcs.iter().any(|n| n.x == x && n.y == y)
            || in_capital_area(capitals, x, y)
    };

    for r in 2..w.max(h) as i16 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() + dy.abs() > r {
                    continue;
                }
                let nx = cx as i16 + dx;
                let ny = cy as i16 + dy;
                if nx >= 0 && nx < w as i16 && ny >= 0 && ny < h as i16 {
                    let (ux, uy) = (nx as u16, ny as u16);
                    let tile = &map[ny as usize][nx as usize];
                    if tile.terrain == Terrain::Wasteland && !is_occupied(ux, uy) {
                        return (ux, uy);
                    }
                }
            }
        }
    }
    (cx, cy)
}
