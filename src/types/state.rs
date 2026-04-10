use rand::rngs::SmallRng;
use rand::Rng;
use rand::SeedableRng;

use super::capital::{Capital, CapitalKind};
use super::faction::FactionId;
use super::npc::{Npc, NpcTask};
use super::player::Player;
use super::terrain::{Terrain, Tile};

/// Goal description for `astar_next_step`. Parameterizes the goal predicate
/// and the heuristic so the same A* implementation works for both walking to
/// a resource tile and walking back to a 3×3 capital footprint.
#[derive(Clone, Copy)]
enum AstarTarget {
    /// Reach any tile cardinally adjacent to the single tile (tx, ty).
    /// Used for resource targets — the tile itself is non-walkable, but its
    /// 4 cardinal neighbors are walkable wasteland.
    AdjacentTo(u16, u16),
    /// Reach any tile cardinally adjacent to the 3×3 capital footprint
    /// centered at (cx, cy). Used for home returning — the capital's cardinal
    /// neighbors from the center are all walls, so we need to pathfind to the
    /// outer ring instead.
    AdjacentToBox(u16, u16),
}

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
                    carrying_water: 0,
                    carrying_fuel: 0,
                    carrying_scrap: 0,
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
            if let NpcTask::Extracting { tx, ty, started, terrain } = task {
                if now.duration_since(started).as_millis() >= crate::config::EXTRACT_TIME_MS as u128 {
                    // Add to the appropriate carry slot
                    match terrain {
                        Terrain::Water => self.npcs[i].carrying_water += 1,
                        Terrain::Rocky => self.npcs[i].carrying_fuel += 1,
                        Terrain::Ruins => self.npcs[i].carrying_scrap += 1,
                        _ => {}
                    }

                    // Chain-harvest decision: full -> return, still needed -> keep
                    // extracting from the same tile, otherwise drop back to Wandering
                    // so the NPC can re-pick based on current stockpile state.
                    let total = self.npcs[i].carrying_total();
                    if total >= crate::config::CARRY_CAP {
                        self.npcs[i].task = NpcTask::Returning;
                    } else {
                        let home_idx = self.npcs[i].home_capital_idx;
                        let stock_still_needs = if home_idx < self.capitals.len() {
                            let cap = &self.capitals[home_idx];
                            let stock = match terrain {
                                Terrain::Water => cap.water,
                                Terrain::Rocky => cap.fuel,
                                Terrain::Ruins => cap.scrap,
                                _ => u32::MAX,
                            };
                            stock < crate::config::MAX_HOARD_BEFORE_USE
                        } else {
                            false
                        };
                        if stock_still_needs {
                            // Keep extracting this same tile with a fresh timer
                            self.npcs[i].task = NpcTask::Extracting {
                                tx, ty,
                                started: now,
                                terrain,
                            };
                        } else {
                            self.npcs[i].task = NpcTask::Wandering;
                        }
                    }
                }
                continue;
            }

            // All movement/decision tasks are gated by the per-NPC cooldown,
            // which scales with carry weight and home capital's fuel tier.
            let weight = self.npcs[i].carrying_total();
            let faction = self.npcs[i].faction;
            let cooldown = self
                .capitals
                .iter()
                .find(|c| c.faction == faction)
                .map(|c| c.npc_move_cooldown(weight))
                .unwrap_or_else(|| {
                    let idx = (weight as usize).min(crate::config::NPC_MOVE_COOLDOWN.len() - 1);
                    crate::config::NPC_MOVE_COOLDOWN[idx]
                });
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
                        self.npcs[i].task = NpcTask::Extracting { tx, ty, started: now, terrain };
                    } else {
                        // If the target has become inaccessible (e.g. another NPC claimed
                        // it), drop back to Wandering and let the next tick repick.
                        if !self.resource_accessible(tx, ty, i) {
                            self.npcs[i].task = NpcTask::Wandering;
                        } else {
                            self.step_npc_toward(i, AstarTarget::AdjacentTo(tx, ty));
                        }
                    }
                    self.npcs[i].last_move = now;
                }
                NpcTask::Returning => {
                    if self.npc_adjacent_to_home(i) {
                        // Deposit all carried resources
                        let home_idx = self.npcs[i].home_capital_idx;
                        let deposited_water = self.npcs[i].carrying_water > 0;
                        if home_idx < self.capitals.len() {
                            let cap = &mut self.capitals[home_idx];
                            let max = crate::config::MAX_STOCKPILE;
                            cap.water = (cap.water + self.npcs[i].carrying_water).min(max);
                            cap.fuel = (cap.fuel + self.npcs[i].carrying_fuel).min(max);
                            cap.scrap = (cap.scrap + self.npcs[i].carrying_scrap).min(max);
                        }
                        self.npcs[i].carrying_water = 0;
                        self.npcs[i].carrying_fuel = 0;
                        self.npcs[i].carrying_scrap = 0;
                        self.npcs[i].task = NpcTask::Wandering;
                        // Growth check if water was deposited
                        if deposited_water {
                            self.try_grow_capital(home_idx);
                        }
                    } else {
                        let home_idx = self.npcs[i].home_capital_idx;
                        if home_idx < self.capitals.len() {
                            let (cx, cy) = (self.capitals[home_idx].x, self.capitals[home_idx].y);
                            // Target the 3×3 footprint, not the unreachable center tile.
                            self.step_npc_toward(i, AstarTarget::AdjacentToBox(cx, cy));
                        }
                    }
                    self.npcs[i].last_move = now;
                }
                NpcTask::Extracting { .. } => unreachable!("handled above"),
            }
        }
    }

    /// Pick the nearest accessible resource tile the NPC's home capital still needs.
    /// Scores every candidate by `(stockpile + already_carrying) * SCARCITY_WEIGHT + distance`
    /// — "effective amount" accounts for what this NPC will deposit on return so
    /// they don't over-fetch a single type. Skips resources where the effective
    /// amount has already reached `MAX_HOARD_BEFORE_USE`. Also skips if the NPC
    /// is already at their carry cap.
    fn pick_harvest_target(&self, npc_idx: usize) -> Option<(u16, u16, Terrain)> {
        let npc = &self.npcs[npc_idx];
        if npc.carrying_total() >= crate::config::CARRY_CAP {
            return None;
        }
        let home_idx = npc.home_capital_idx;
        if home_idx >= self.capitals.len() {
            return None;
        }
        let cap = &self.capitals[home_idx];
        let threshold = crate::config::MAX_HOARD_BEFORE_USE;

        let stockpile_and_carried = |t: Terrain| -> Option<u32> {
            match t {
                Terrain::Water => Some(cap.water + npc.carrying_water),
                Terrain::Rocky => Some(cap.fuel + npc.carrying_fuel),
                Terrain::Ruins => Some(cap.scrap + npc.carrying_scrap),
                _ => None,
            }
        };

        let npc_x = npc.x;
        let npc_y = npc.y;

        // Weight: how many distance tiles a unit of "effective stockpile" is worth.
        // Higher = prefer scarcity over distance. Lower = prefer closer tiles.
        const SCARCITY_WEIGHT: i32 = 3;

        let mut best: Option<(u16, u16, Terrain, i32)> = None;
        for (y, row) in self.map.iter().enumerate() {
            for (x, tile) in row.iter().enumerate() {
                let terrain = tile.terrain;
                let effective = match stockpile_and_carried(terrain) {
                    Some(a) if a < threshold => a,
                    _ => continue, // not a resource, or already enough combined
                };
                if !self.resource_accessible(x as u16, y as u16, npc_idx) {
                    continue;
                }
                let dist = (x as i32 - npc_x as i32).abs() + (y as i32 - npc_y as i32).abs();
                let score = dist + (effective as i32) * SCARCITY_WEIGHT;
                if best.map_or(true, |(_, _, _, bs)| score < bs) {
                    best = Some((x as u16, y as u16, terrain, score));
                }
            }
        }
        best.map(|(tx, ty, terrain, _)| (tx, ty, terrain))
    }

    /// Is this resource tile reachable by `self_npc_idx`?
    /// - At least one cardinal neighbor must be walkable wasteland
    /// - No **same-faction** NPC can already be Targeting or Extracting this tile
    ///
    /// Cross-faction NPCs can intentionally target the same tile — that's how
    /// rival factions end up contesting the same resource deposit.
    fn resource_accessible(&self, tx: u16, ty: u16, self_npc_idx: usize) -> bool {
        let self_faction = self.npcs[self_npc_idx].faction;
        for (i, other) in self.npcs.iter().enumerate() {
            if i == self_npc_idx {
                continue;
            }
            if other.faction != self_faction {
                continue; // rivals from other factions don't block us
            }
            match other.task {
                NpcTask::TargetingResource { tx: otx, ty: oty, .. } => {
                    if otx == tx && oty == ty {
                        return false;
                    }
                }
                NpcTask::Extracting { tx: otx, ty: oty, .. } => {
                    if otx == tx && oty == ty {
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

    /// Single-step movement toward a target. Uses A* pathfinding with a goal
    /// predicate so the same function works whether the NPC is walking toward
    /// a single resource tile or back to the 3×3 capital footprint. The path
    /// is planned through static obstacles only; transient NPC collisions are
    /// resolved at the move-attempt stage with a random-step fallback.
    fn step_npc_toward(&mut self, i: usize, target: AstarTarget) {
        use rand::Rng;

        if let Some((sx, sy)) = self.astar_next_step(i, target) {
            let nx = self.npcs[i].x as i16 + sx;
            let ny = self.npcs[i].y as i16 + sy;
            if !self.is_blocked_for_npc(nx, ny, i) {
                self.npcs[i].x = nx as u16;
                self.npcs[i].y = ny as u16;
                return;
            }
        }

        // Fallback: random cardinal step. Used when A* can't reach the target
        // (temporary blockage by another NPC, disconnected region, etc.) — the
        // NPC tries again on the next tick after other things may have moved.
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

    /// A* pathfinding from the NPC's position to the first tile satisfying the
    /// goal predicate (see `AstarTarget`). Plans through static obstacles only
    /// (`is_static_blocked`) so crowded NPCs don't deadlock each other. Uses
    /// Manhattan distance to the target anchor as the heuristic.
    fn astar_next_step(&self, npc_idx: usize, target: AstarTarget) -> Option<(i16, i16)> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        let w = self.map[0].len();
        let h = self.map.len();
        let start = (self.npcs[npc_idx].x, self.npcs[npc_idx].y);

        // Goal anchor for the heuristic. For AdjacentTo this is the target tile
        // itself; for AdjacentToBox it's the center of the 3×3.
        let (anchor_x, anchor_y) = match target {
            AstarTarget::AdjacentTo(tx, ty) => (tx, ty),
            AstarTarget::AdjacentToBox(cx, cy) => (cx, cy),
        };

        let is_goal = |x: u16, y: u16| -> bool {
            match target {
                AstarTarget::AdjacentTo(tx, ty) => {
                    (x as i16 - tx as i16).abs() + (y as i16 - ty as i16).abs() == 1
                }
                AstarTarget::AdjacentToBox(cx, cy) => {
                    let dx = (x as i16 - cx as i16).abs();
                    let dy = (y as i16 - cy as i16).abs();
                    // Cardinally adjacent to the 3×3 = 2 out on one axis, ≤1 on the other
                    (dx == 2 && dy <= 1) || (dy == 2 && dx <= 1)
                }
            }
        };

        let heuristic = |x: u16, y: u16| -> u32 {
            let dx = (x as i32 - anchor_x as i32).abs();
            let dy = (y as i32 - anchor_y as i32).abs();
            (dx + dy) as u32
        };

        let mut g_score = vec![vec![u32::MAX; w]; h];
        let mut parent: Vec<Vec<(u16, u16)>> = vec![vec![(u16::MAX, u16::MAX); w]; h];
        // Min-heap on f_score = g + h
        let mut open: BinaryHeap<(Reverse<u32>, u16, u16)> = BinaryHeap::new();

        g_score[start.1 as usize][start.0 as usize] = 0;
        open.push((Reverse(heuristic(start.0, start.1)), start.0, start.1));

        let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let mut goal_tile: Option<(u16, u16)> = None;

        while let Some((_, cx, cy)) = open.pop() {
            if is_goal(cx, cy) {
                goal_tile = Some((cx, cy));
                break;
            }

            let cg = g_score[cy as usize][cx as usize];
            for (dx, dy) in dirs {
                let nx = cx as i16 + dx;
                let ny = cy as i16 + dy;
                if nx < 0 || ny < 0 || nx >= w as i16 || ny >= h as i16 {
                    continue;
                }
                if self.is_static_blocked(nx, ny) {
                    continue;
                }
                let (nux, nuy) = (nx as u16, ny as u16);
                let tentative_g = cg + 1;
                if tentative_g < g_score[nuy as usize][nux as usize] {
                    g_score[nuy as usize][nux as usize] = tentative_g;
                    parent[nuy as usize][nux as usize] = (cx, cy);
                    let f = tentative_g + heuristic(nux, nuy);
                    open.push((Reverse(f), nux, nuy));
                }
            }
        }

        let goal_tile = goal_tile?;
        if goal_tile == start {
            return None; // caller should have transitioned before this point
        }

        // Walk back via parent pointers to the first step after start
        let mut current = goal_tile;
        loop {
            let p = parent[current.1 as usize][current.0 as usize];
            if p.0 == u16::MAX {
                return None; // broken chain
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

    /// If `capitals[cap_idx].water >= WATER_GROWTH_THRESHOLD` **and** a valid
    /// spawn tile exists near the capital, spend `WATER_GROWTH_COST` water and
    /// spawn one new NPC assigned to this capital. Called immediately after any
    /// water increase (NPC deposit, player sale) so growth is instant.
    ///
    /// If no valid spawn tile is available (the capital is completely surrounded
    /// by walls, NPCs, other capitals, or the player), growth pauses and water
    /// is NOT consumed — the next water deposit will try again.
    ///
    /// `pub(super)` so `actions.rs::sell_resource` can trigger it when the player
    /// sells water to a capital.
    pub(super) fn try_grow_capital(&mut self, cap_idx: usize) {
        if cap_idx >= self.capitals.len() {
            return;
        }
        if self.capitals[cap_idx].water < crate::config::WATER_GROWTH_THRESHOLD {
            return;
        }

        // Look for a spawn tile BEFORE spending water so a failed spawn doesn't
        // waste the resource.
        let spawn = match self.find_growth_spawn(cap_idx) {
            Some(pos) => pos,
            None => return,
        };

        self.capitals[cap_idx].water = self.capitals[cap_idx]
            .water
            .saturating_sub(crate::config::WATER_GROWTH_COST);

        let faction = self.capitals[cap_idx].faction;
        self.npcs.push(Npc {
            x: spawn.0,
            y: spawn.1,
            faction,
            home_capital_idx: cap_idx,
            last_move: std::time::Instant::now(),
            task: NpcTask::Wandering,
            carrying_water: 0,
            carrying_fuel: 0,
            carrying_scrap: 0,
        });
    }

    /// Find an empty tile near the given capital where a new NPC can be placed.
    /// Properly handles camp footprints (`is_inside`), free-standing walls
    /// (`tile.wall`), the player position, and existing NPCs. Returns `None` if
    /// no valid spot exists within the search radius.
    fn find_growth_spawn(&self, cap_idx: usize) -> Option<(u16, u16)> {
        let cap = &self.capitals[cap_idx];
        let cx = cap.x;
        let cy = cap.y;
        let w = self.map[0].len() as i16;
        let h = self.map.len() as i16;

        // Expand outward from the capital's outer ring. Stop once we've scanned
        // up to the width+height of the map (guaranteed full coverage).
        for r in 2..=(w.max(h)) {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() + dy.abs() != r {
                        continue; // only scan the current ring, not filled diamond
                    }
                    let nx = cx as i16 + dx;
                    let ny = cy as i16 + dy;
                    if nx < 0 || nx >= w || ny < 0 || ny >= h {
                        continue;
                    }
                    let (ux, uy) = (nx as u16, ny as u16);
                    let tile = &self.map[ny as usize][nx as usize];
                    if tile.terrain != Terrain::Wasteland {
                        continue;
                    }
                    if tile.wall.is_some() {
                        continue;
                    }
                    if self.is_capital_area(ux, uy) {
                        continue;
                    }
                    if self.player.x == ux && self.player.y == uy {
                        continue;
                    }
                    if self.npcs.iter().any(|n| n.x == ux && n.y == uy) {
                        continue;
                    }
                    return Some((ux, uy));
                }
            }
        }
        None
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

    /// Static obstacles only: edges, non-wasteland terrain, walls, capital footprints.
    /// Does NOT consider other NPCs or the player. Used by BFS pathfinding so NPCs
    /// can plan routes through space temporarily occupied by moving peers.
    fn is_static_blocked(&self, x: i16, y: i16) -> bool {
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
