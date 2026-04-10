use rand::rngs::SmallRng;
use rand::Rng;
use rand::SeedableRng;

use super::capital::{Capital, CapitalKind};
use super::faction::FactionId;
use super::npc::{Npc, NpcTask};
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
                    task: NpcTask::Wandering,
                    carrying: None,
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

    /// Phase 2: NPCs harvest resources. Each NPC is a small state machine
    /// (Wandering → TargetingResource → Extracting → Returning → Wandering).
    /// Movement still respects the per-faction fuel-scaled cooldown.
    pub fn update_npcs(&mut self) {
        let now = std::time::Instant::now();
        let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

        for i in 0..self.npcs.len() {
            let task = self.npcs[i].task;

            // Extracting is time-based, not cooldown-based
            if let NpcTask::Extracting { started, terrain } = task {
                if now.duration_since(started).as_millis() >= crate::config::EXTRACT_TIME_MS as u128 {
                    self.npcs[i].carrying = Some(terrain);
                    self.npcs[i].task = NpcTask::Returning;
                }
                continue;
            }

            // All movement/decision tasks are gated by the per-NPC cooldown
            let faction = self.npcs[i].faction;
            let cooldown = self
                .capitals
                .iter()
                .find(|c| c.faction == faction)
                .map(|c| c.npc_move_cooldown())
                .unwrap_or(crate::config::NPC_BASE_MOVE_MS);
            if now.duration_since(self.npcs[i].last_move).as_millis() < cooldown as u128 {
                continue;
            }

            match task {
                NpcTask::Wandering => {
                    // Try to pick a harvest target first
                    if let Some((tx, ty, terrain)) = self.pick_harvest_target(i) {
                        self.npcs[i].task = NpcTask::TargetingResource { tx, ty, terrain };
                    } else {
                        // Nothing to harvest — take a random step
                        let d = dirs[self.sim_rng.gen_range(0..4)];
                        let nx = self.npcs[i].x as i16 + d.0;
                        let ny = self.npcs[i].y as i16 + d.1;
                        if !self.is_blocked_for_npc(nx, ny, i) {
                            self.npcs[i].x = nx as u16;
                            self.npcs[i].y = ny as u16;
                        }
                    }
                    self.npcs[i].last_move = now;
                }
                NpcTask::TargetingResource { tx, ty, terrain } => {
                    // If adjacent, start extracting; else step toward
                    let dist = (self.npcs[i].x as i16 - tx as i16).abs()
                        + (self.npcs[i].y as i16 - ty as i16).abs();
                    if dist == 1 {
                        self.npcs[i].task = NpcTask::Extracting { started: now, terrain };
                    } else {
                        // If the target has become inaccessible (e.g. another NPC claimed
                        // it), drop back to Wandering and let the next tick repick.
                        if !self.resource_accessible(tx, ty, i) {
                            self.npcs[i].task = NpcTask::Wandering;
                        } else {
                            self.step_npc_toward(i, tx, ty);
                        }
                    }
                    self.npcs[i].last_move = now;
                }
                NpcTask::Returning => {
                    if self.npc_adjacent_to_home(i) {
                        // Deposit
                        if let Some(terrain) = self.npcs[i].carrying {
                            let home_idx = self.npcs[i].home_capital_idx;
                            if home_idx < self.capitals.len() {
                                let cap = &mut self.capitals[home_idx];
                                let max = crate::config::MAX_STOCKPILE;
                                match terrain {
                                    Terrain::Water => cap.water = (cap.water + 1).min(max),
                                    Terrain::Rocky => cap.fuel = (cap.fuel + 1).min(max),
                                    Terrain::Ruins => cap.scrap = (cap.scrap + 1).min(max),
                                    _ => {}
                                }
                            }
                        }
                        self.npcs[i].carrying = None;
                        self.npcs[i].task = NpcTask::Wandering;
                    } else {
                        let home_idx = self.npcs[i].home_capital_idx;
                        if home_idx < self.capitals.len() {
                            let (cx, cy) = (self.capitals[home_idx].x, self.capitals[home_idx].y);
                            self.step_npc_toward(i, cx, cy);
                        }
                    }
                    self.npcs[i].last_move = now;
                }
                NpcTask::Extracting { .. } => unreachable!("handled above"),
            }
        }
    }

    /// Pick the nearest accessible resource tile the NPC's home capital still needs.
    /// Scores every candidate by (stockpile_amount * SCARCITY_WEIGHT + distance) and
    /// picks the lowest score — favoring needed resources while avoiding cross-map
    /// walks when a closer option is almost as needed. Skips anything at
    /// `MAX_HOARD_BEFORE_USE`.
    fn pick_harvest_target(&self, npc_idx: usize) -> Option<(u16, u16, Terrain)> {
        let home_idx = self.npcs[npc_idx].home_capital_idx;
        if home_idx >= self.capitals.len() {
            return None;
        }
        let cap = &self.capitals[home_idx];
        let threshold = crate::config::MAX_HOARD_BEFORE_USE;

        let stockpile_for = |t: Terrain| -> Option<u32> {
            match t {
                Terrain::Water => Some(cap.water),
                Terrain::Rocky => Some(cap.fuel),
                Terrain::Ruins => Some(cap.scrap),
                _ => None,
            }
        };

        let npc_x = self.npcs[npc_idx].x;
        let npc_y = self.npcs[npc_idx].y;

        // Weight: how many distance tiles a unit of stockpile is "worth".
        // Higher = prefer scarcity over distance. Lower = prefer closer tiles.
        const SCARCITY_WEIGHT: i32 = 3;

        let mut best: Option<(u16, u16, Terrain, i32)> = None;
        for (y, row) in self.map.iter().enumerate() {
            for (x, tile) in row.iter().enumerate() {
                let terrain = tile.terrain;
                let amount = match stockpile_for(terrain) {
                    Some(a) if a < threshold => a,
                    _ => continue, // not a resource, or already capped
                };
                if !self.resource_accessible(x as u16, y as u16, npc_idx) {
                    continue;
                }
                let dist = (x as i32 - npc_x as i32).abs() + (y as i32 - npc_y as i32).abs();
                let score = dist + (amount as i32) * SCARCITY_WEIGHT;
                if best.map_or(true, |(_, _, _, bs)| score < bs) {
                    best = Some((x as u16, y as u16, terrain, score));
                }
            }
        }
        best.map(|(tx, ty, terrain, _)| (tx, ty, terrain))
    }

    /// Is this resource tile reachable by `self_npc_idx`?
    /// - At least one cardinal neighbor must be walkable wasteland
    /// - No other NPC can already be Targeting or Extracting this specific tile
    fn resource_accessible(&self, tx: u16, ty: u16, self_npc_idx: usize) -> bool {
        for (i, other) in self.npcs.iter().enumerate() {
            if i == self_npc_idx {
                continue;
            }
            match other.task {
                NpcTask::TargetingResource { tx: otx, ty: oty, .. } => {
                    if otx == tx && oty == ty {
                        return false;
                    }
                }
                NpcTask::Extracting { .. } => {
                    // The other NPC is adjacent to their target resource; if that's this
                    // tile, we're blocked.
                    let dx = (other.x as i16 - tx as i16).abs();
                    let dy = (other.y as i16 - ty as i16).abs();
                    if dx + dy == 1 {
                        return false;
                    }
                }
                _ => {}
            }
        }

        // At least one cardinal neighbor must be walkable (wasteland, no wall, no capital)
        let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let w = self.map[0].len() as i16;
        let h = self.map.len() as i16;
        for (dx, dy) in dirs {
            let nx = tx as i16 + dx;
            let ny = ty as i16 + dy;
            if nx < 0 || nx >= w || ny < 0 || ny >= h {
                continue;
            }
            let ntile = &self.map[ny as usize][nx as usize];
            if ntile.terrain == Terrain::Wasteland
                && ntile.wall.is_none()
                && !self.is_capital_area(nx as u16, ny as u16)
            {
                return true;
            }
        }
        false
    }

    /// Single-step movement toward (tx, ty). Uses BFS to find the shortest path
    /// through any obstacles (walls, capitals, other NPCs) and returns the first
    /// step. If BFS can't find a path (target unreachable right now), falls back to
    /// a random cardinal step so the NPC isn't permanently frozen.
    fn step_npc_toward(&mut self, i: usize, tx: u16, ty: u16) {
        use rand::Rng;

        if let Some((sx, sy)) = self.bfs_next_step(i, tx, ty) {
            let nx = self.npcs[i].x as i16 + sx;
            let ny = self.npcs[i].y as i16 + sy;
            // BFS respects is_blocked_for_npc, so this should never be blocked,
            // but double-check to avoid panics if the simulation state shifted.
            if !self.is_blocked_for_npc(nx, ny, i) {
                self.npcs[i].x = nx as u16;
                self.npcs[i].y = ny as u16;
                return;
            }
        }

        // Fallback: random cardinal step. Used when BFS can't reach the target
        // (temporary blockage by another NPC, disconnected region, etc.) — the NPC
        // tries again on the next tick after other things may have moved.
        let npc_x = self.npcs[i].x;
        let npc_y = self.npcs[i].y;
        let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let start = self.sim_rng.gen_range(0..4);
        for offset in 0..4 {
            let (sx, sy) = dirs[(start + offset) % 4];
            let nx = npc_x as i16 + sx;
            let ny = npc_y as i16 + sy;
            if !self.is_blocked_for_npc(nx, ny, i) {
                self.npcs[i].x = nx as u16;
                self.npcs[i].y = ny as u16;
                return;
            }
        }
        // Completely boxed in — stay put
    }

    /// BFS from the NPC's current position, searching for a walkable tile
    /// cardinally adjacent to (tx, ty). Returns the first step direction
    /// `(dx, dy)` to take along the shortest path, or `None` if unreachable.
    /// The target tile itself (often a non-walkable resource or capital) is
    /// not entered — BFS only needs to *reach* a neighbor of it.
    fn bfs_next_step(&self, npc_idx: usize, tx: u16, ty: u16) -> Option<(i16, i16)> {
        use std::collections::VecDeque;

        let w = self.map[0].len();
        let h = self.map.len();
        let start = (self.npcs[npc_idx].x, self.npcs[npc_idx].y);

        let mut visited = vec![vec![false; w]; h];
        let mut parent: Vec<Vec<(u16, u16)>> = vec![vec![(u16::MAX, u16::MAX); w]; h];
        let mut queue: VecDeque<(u16, u16)> = VecDeque::new();

        visited[start.1 as usize][start.0 as usize] = true;
        queue.push_back(start);

        let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let mut goal: Option<(u16, u16)> = None;

        'bfs: while let Some((cx, cy)) = queue.pop_front() {
            for (dx, dy) in dirs {
                let nx = cx as i16 + dx;
                let ny = cy as i16 + dy;
                if nx < 0 || ny < 0 || nx >= w as i16 || ny >= h as i16 {
                    continue;
                }
                let (nux, nuy) = (nx as u16, ny as u16);
                if visited[nuy as usize][nux as usize] {
                    continue;
                }
                if self.is_blocked_for_npc(nx, ny, npc_idx) {
                    continue;
                }
                visited[nuy as usize][nux as usize] = true;
                parent[nuy as usize][nux as usize] = (cx, cy);

                // Did we reach a tile cardinally adjacent to the target?
                let adj = (nx - tx as i16).abs() + (ny - ty as i16).abs();
                if adj == 1 {
                    goal = Some((nux, nuy));
                    break 'bfs;
                }

                queue.push_back((nux, nuy));
            }
        }

        let goal = goal?;

        // Walk back via parent pointers until we find the tile whose parent is start.
        // That tile is the first step of the shortest path.
        let mut current = goal;
        loop {
            let p = parent[current.1 as usize][current.0 as usize];
            if p.0 == u16::MAX {
                return None; // broken chain (shouldn't happen)
            }
            if p == start {
                let dx = current.0 as i16 - start.0 as i16;
                let dy = current.1 as i16 - start.1 as i16;
                return Some((dx, dy));
            }
            current = p;
        }
    }

    /// True if the NPC is cardinally adjacent to any tile of its home capital's footprint.
    fn npc_adjacent_to_home(&self, i: usize) -> bool {
        let npc = &self.npcs[i];
        let home_idx = npc.home_capital_idx;
        if home_idx >= self.capitals.len() {
            return false;
        }
        let home = &self.capitals[home_idx];
        let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        for (dx, dy) in dirs {
            let nx = npc.x as i16 + dx;
            let ny = npc.y as i16 + dy;
            if nx >= 0 && ny >= 0 && home.is_inside(nx as u16, ny as u16) {
                return true;
            }
        }
        false
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
