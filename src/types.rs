use rand::rngs::SmallRng;
use rand::SeedableRng;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Terrain {
    Wasteland, // .
    Water,     // ~
    Rocky,     // ^
    Ruins,     // :
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactionId {
    Water,  // W — Blue
    Gas,    // G — Red
    Scrap,  // S — Yellow
    Cult,   // C — Purple
}

impl FactionId {
    pub fn glyph(self) -> char {
        match self {
            FactionId::Water => 'W',
            FactionId::Gas => 'G',
            FactionId::Scrap => 'S',
            FactionId::Cult => 'C',
        }
    }

    pub fn npc_glyph(self) -> char {
        match self {
            FactionId::Water => 'w',
            FactionId::Gas => 'g',
            FactionId::Scrap => 's',
            FactionId::Cult => 'c',
        }
    }

    pub fn color(self) -> ratatui::style::Color {
        match self {
            FactionId::Water => ratatui::style::Color::Blue,
            FactionId::Gas => ratatui::style::Color::Red,
            FactionId::Scrap => ratatui::style::Color::Yellow,
            FactionId::Cult => crate::config::TERMINAL_PURPLE,
        }
    }
}

const RUINS_GLYPHS: [char; 7] = [':', '∷', '┘', '┐', '□', 'Ω', '▌'];
const WATER_GLYPHS: [char; 2] = ['~', '≈'];
const DUST_GLYPH: char = '·';
const WAVE_WIDTH: u64 = 4; // how many tiles wide the "active" wave band is

impl Terrain {
    pub fn glyph(self) -> char {
        match self {
            Terrain::Wasteland => '.',
            Terrain::Water => '~',
            Terrain::Rocky => '^',
            Terrain::Ruins => ':',
        }
    }

    /// Animated/varied glyph based on tile position, variant, and frame tick.
    /// Position-based phase creates a diagonal wave sweep effect.
    pub fn glyph_varied(self, variant: u8, x: usize, y: usize, tick: u64) -> char {
        match self {
            Terrain::Ruins => RUINS_GLYPHS[variant as usize % RUINS_GLYPHS.len()],
            Terrain::Water => {
                // Wave sweeps diagonally: phase based on (x + y) position
                let wave_pos = (x + y) as u64;
                let cycle_len = 20; // total wave cycle length in tiles
                let phase = (tick * 2 + wave_pos) % cycle_len;
                if phase < WAVE_WIDTH {
                    WATER_GLYPHS[1] // ≈ during wave crest
                } else {
                    WATER_GLYPHS[0] // ~ normally
                }
            }
            Terrain::Wasteland => {
                // Subtle dust: a thin diagonal line of · sweeps across
                let wave_pos = (x + y) as u64;
                let cycle_len = 40; // longer cycle = rarer dust
                let phase = (tick + wave_pos) % cycle_len;
                if phase == 0 && variant % 3 == 0 {
                    DUST_GLYPH
                } else {
                    '.'
                }
            }
            _ => self.glyph(),
        }
    }
}

#[derive(Clone)]
pub struct Tile {
    pub terrain: Terrain,
    pub owner: Option<FactionId>,
    pub wall: Option<FactionId>, // free-standing wall segment
    pub glyph_variant: u8, // seeded per-tile for stable visual variety
}

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

pub struct FoundCampState {
    pub started: std::time::Instant,
}

pub struct BuildWallState {
    pub started: std::time::Instant,
}

pub struct FoundState {
    pub started: std::time::Instant,
}

pub struct BuildState {
    pub capital_idx: usize,
    pub started: std::time::Instant,
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

impl Player {
    pub fn carrying(&self) -> u32 {
        self.water + self.fuel + self.scrap
    }
}

pub struct Npc {
    pub x: u16,
    pub y: u16,
    pub faction: FactionId,
    pub home_capital_idx: usize,
    pub last_move: std::time::Instant,
}

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

    /// NPC move cooldown in ms, reduced by fuel bonuses.
    pub fn npc_move_cooldown(&self) -> u64 {
        let base = crate::config::NPC_BASE_MOVE_MS;
        let bonus_pct = self.fuel_tiers() * crate::config::FUEL_SPEED_BONUS_PCT;
        let reduction = (base * bonus_pct as u64) / 100;
        base.saturating_sub(reduction)
    }
}

pub struct GameState {
    pub map: Vec<Vec<Tile>>,
    pub capitals: Vec<Capital>,
    pub npcs: Vec<Npc>,
    pub player: Player,
    pub sim_rng: rand::rngs::SmallRng,
    pub last_move: std::time::Instant,
    pub anim_tick: u64,
    pub last_anim: std::time::Instant,
    pub last_decay: std::time::Instant,
    pub last_dehydration: std::time::Instant,
}

impl GameState {
    pub fn new(width: u16, height: u16, seed: u64) -> Self {
        use rand::Rng;

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
                    &map, &capitals, &npcs, px, py,
                    cap.x, cap.y, width, height,
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
                x: px, y: py, faction: player_faction,
                home_capital_idx: player_cap_idx,
                water: 0, fuel: 0, scrap: crate::config::PLAYER_STARTING_SCRAP, crowns: 0,
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
                        && dx.abs() <= 1 && dy.abs() <= 1
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
        // Free-standing wall?
        if let Some(row) = self.map.get(y as usize) {
            if let Some(tile) = row.get(x as usize) {
                if tile.wall.is_some() {
                    return true;
                }
            }
        }
        // City wall?
        for cap in &self.capitals {
            if cap.kind != CapitalKind::City { continue; }
            let dx = x as i16 - cap.x as i16;
            let dy = y as i16 - cap.y as i16;
            if (dx.abs() == 1 || dy.abs() == 1)
                && dx.abs() <= 1 && dy.abs() <= 1
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
            (false, true,  false, false) => '═',
            (false, false, false, true)  => '═',
            (false, true,  false, true)  => '═',
            (true,  false, false, false) => '║',
            (false, false, true,  false) => '║',
            (true,  false, true,  false) => '║',
            (false, true,  true,  false) => '╔',
            (false, false, true,  true)  => '╗',
            (true,  true,  false, false) => '╚',
            (true,  false, false, true)  => '╝',
            (false, true,  true,  true)  => '╦',
            (true,  true,  false, true)  => '╩',
            (true,  true,  true,  false) => '╠',
            (true,  false, true,  true)  => '╣',
            (true,  true,  true,  true)  => '╬',
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

    pub fn npc_at(&self, x: u16, y: u16) -> Option<&Npc> {
        self.npcs.iter().find(|n| n.x == x && n.y == y)
    }

    pub fn update_anim(&mut self) {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_anim).as_millis() >= crate::config::ANIM_TICK_MS as u128 {
            self.anim_tick += 1;
            self.last_anim = now;
        }
    }

    /// Phase 1: each NPC wanders one tile in a random direction on its cooldown.
    pub fn update_npcs(&mut self) {
        use rand::Rng;
        let now = std::time::Instant::now();
        let dirs: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

        for i in 0..self.npcs.len() {
            // Determine this NPC's faction cooldown
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

            // Pick a random direction and check if the destination is free
            let d = dirs[self.sim_rng.gen_range(0..4)];
            let nx = self.npcs[i].x as i16 + d.0;
            let ny = self.npcs[i].y as i16 + d.1;

            // Check for blocking: terrain, edges, capitals, other NPCs, and player
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

    /// Like is_blocked, but also considers the player position and excludes the NPC itself.
    fn is_blocked_for_npc(&self, x: i16, y: i16, self_idx: usize) -> bool {
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
            if i == self_idx { continue; }
            if npc.x == x as u16 && npc.y == y as u16 {
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

        // Pre-compute population per capital (NPCs + player assigned to this capital)
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
            if crate::config::DECAY_WATER { cap.water = cap.water.saturating_sub(drain); }
            if crate::config::DECAY_FUEL { cap.fuel = cap.fuel.saturating_sub(drain); }
            if crate::config::DECAY_SCRAP { cap.scrap = cap.scrap.saturating_sub(drain); }
        }
    }

    /// Dehydration: if a capital has 0 water, lose one NPC from that faction.
    pub fn update_dehydration(&mut self) {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_dehydration).as_millis()
            < crate::config::DEHYDRATION_INTERVAL_MS as u128
        {
            return;
        }
        self.last_dehydration = now;

        // For each capital with 0 water, remove one NPC assigned to it
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
        self.player.extracting = None; // moving cancels extraction
        self.player.claiming = None; // moving cancels claiming
        self.player.founding = None; // moving cancels founding
        self.player.building = None; // moving cancels building
        self.player.founding_camp = None; // moving cancels camp founding
        self.player.building_wall = None; // moving cancels wall building
    }

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
            return; // already in progress
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

    /// Can the player claim the tile they're standing on?
    pub fn can_claim(&self) -> bool {
        let tile = &self.map[self.player.y as usize][self.player.x as usize];
        // Must be wasteland, not already owned by player's faction, and player has scrap
        tile.terrain == Terrain::Wasteland
            && tile.owner != Some(self.player.faction)
            && self.player.scrap >= crate::config::CLAIM_SCRAP_COST
            && self.player.extracting.is_none()
            && self.player.claiming.is_none()
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

    /// Find the index of an adjacent capital (for mutable access).
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

    /// Sell a resource to the adjacent capital: player loses resource, gains crowns, capital gains resource.
    pub fn sell_resource(&mut self, resource: usize) {
        let cap_idx = match self.adjacent_capital_idx() {
            Some(i) => i,
            None => return,
        };
        let (player_has, player_field, cap_field): (bool, fn(&mut Player) -> &mut u32, fn(&mut Capital) -> &mut u32) = match resource {
            1 => (self.player.water > 0,
                  |p: &mut Player| &mut p.water,
                  |c: &mut Capital| &mut c.water),
            2 => (self.player.fuel > 0,
                  |p: &mut Player| &mut p.fuel,
                  |c: &mut Capital| &mut c.fuel),
            3 => (self.player.scrap > 0,
                  |p: &mut Player| &mut p.scrap,
                  |c: &mut Capital| &mut c.scrap),
            _ => return,
        };
        if !player_has { return; }

        *player_field(&mut self.player) -= 1;
        let cap = &mut self.capitals[cap_idx];
        let cap_val = cap_field(cap);
        *cap_val = (*cap_val + 1).min(crate::config::MAX_STOCKPILE);
        self.player.crowns += crate::config::BASE_SELL_PRICE;
    }

    /// Buy a resource from the adjacent capital: player gains resource, loses crowns, capital loses resource.
    pub fn buy_resource(&mut self, resource: usize) {
        let cap_idx = match self.adjacent_capital_idx() {
            Some(i) => i,
            None => return,
        };
        if self.player.crowns < crate::config::BASE_BUY_PRICE { return; }
        if self.player.carrying() >= crate::config::CARRY_CAP { return; }

        let (cap_has, player_field, cap_field): (bool, fn(&mut Player) -> &mut u32, fn(&mut Capital) -> &mut u32) = match resource {
            1 => (self.capitals[cap_idx].water > 0,
                  |p: &mut Player| &mut p.water,
                  |c: &mut Capital| &mut c.water),
            2 => (self.capitals[cap_idx].fuel > 0,
                  |p: &mut Player| &mut p.fuel,
                  |c: &mut Capital| &mut c.fuel),
            3 => (self.capitals[cap_idx].scrap > 0,
                  |p: &mut Player| &mut p.scrap,
                  |c: &mut Capital| &mut c.scrap),
            _ => return,
        };
        if !cap_has { return; }

        self.player.crowns -= crate::config::BASE_BUY_PRICE;
        *player_field(&mut self.player) += 1;
        let cap = &mut self.capitals[cap_idx];
        *cap_field(cap) -= 1;
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

    /// Can the player lay a foundation where they stand?
    /// Requirements:
    /// - Player has 5 scrap (foundation cost)
    /// - Player stands on a tile claimed by their faction (wasteland)
    /// - The 5x5 area around the player is clear wasteland with no resources/walls/existing capitals
    /// - Not already doing another action
    pub fn can_found_city(&self) -> bool {
        if self.player.scrap < crate::config::FOUNDATION_SCRAP_COST { return false; }
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
        if px < 2 || px >= w - 2 || py < 2 || py >= h - 2 { return false; }

        // Center tile must be claimed by the player's faction
        let center = &self.map[py as usize][px as usize];
        if center.owner != Some(self.player.faction) { return false; }

        // 5x5 area must be clear of resources, walls, and existing capitals
        for dy in -2i16..=2 {
            for dx in -2i16..=2 {
                let tx = (px + dx) as u16;
                let ty = (py + dy) as u16;
                let tile = &self.map[ty as usize][tx as usize];
                if tile.terrain != Terrain::Wasteland { return false; }
                if self.is_capital_area(tx, ty) || self.capital_border_at(tx, ty).is_some() {
                    return false;
                }
            }
        }
        true
    }

    /// Press C (on claimed tile): start laying the foundation for a new city.
    pub fn start_found_city(&mut self) {
        if !self.can_found_city() { return; }
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
            if cap.is_complete() || cap.faction != self.player.faction { continue; }
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
        if self.player.building.is_some() { return false; }
        if self.player.scrap == 0 { return false; }
        match self.adjacent_foundation_idx() {
            Some(i) => self.capitals[i].scrap_invested < crate::config::CITY_TOTAL_SCRAP,
            None => false,
        }
    }

    /// Press C (adjacent to own foundation): start a timed scrap deposit.
    pub fn start_add_to_foundation(&mut self) {
        if !self.can_add_to_foundation() { return; }
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
                // Re-verify state is still valid (still adjacent, still has scrap, not full)
                if idx >= self.capitals.len() { return; }
                if self.adjacent_foundation_idx() != Some(idx) { return; }
                if self.player.scrap == 0 { return; }
                if self.capitals[idx].scrap_invested >= crate::config::CITY_TOTAL_SCRAP { return; }
                self.player.scrap -= 1;
                self.capitals[idx].scrap_invested += 1;
            }
        }
    }

    /// Check if city founding completed (called each frame). Creates a foundation at the player's spot.
    pub fn check_found_city(&mut self) {
        if let Some(ref state) = self.player.founding {
            let elapsed = std::time::Instant::now().duration_since(state.started).as_millis();
            if elapsed >= crate::config::FOUND_CITY_TIME_MS as u128 {
                let px = self.player.x;
                let py = self.player.y;
                let faction = self.player.faction;
                self.player.scrap -= crate::config::FOUNDATION_SCRAP_COST;
                self.player.founding = None;

                // Create a foundation (incomplete capital) — empty stockpile, 5 scrap invested
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
                    // Try cardinal offsets at distance 2 (just outside the walls)
                    let candidates: [(i16, i16); 4] = [(0, -2), (2, 0), (0, 2), (-2, 0)];
                    for (dx, dy) in candidates {
                        let nx = cx + dx;
                        let ny = cy + dy;
                        if nx >= 0 && nx < w && ny >= 0 && ny < h {
                            if !self.is_blocked_for_npc(nx, ny, i) {
                                self.npcs[i].x = nx as u16;
                                self.npcs[i].y = ny as u16;
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

    /// Can the player found a camp where they stand?
    /// Requirements:
    /// - Has 1 scrap (camp cost)
    /// - All 5 + tiles are wasteland (no resources)
    /// - No existing capital overlap
    /// - Not in the middle of another action
    pub fn can_found_camp(&self) -> bool {
        if self.player.scrap < crate::config::CAMP_SCRAP_COST { return false; }
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
        if px < 1 || px >= w - 1 || py < 1 || py >= h - 1 { return false; }

        // Center + 4 cardinal walls
        let offsets: [(i16, i16); 5] = [(0, 0), (0, -1), (0, 1), (-1, 0), (1, 0)];
        for (dx, dy) in offsets {
            let tx = (px + dx) as u16;
            let ty = (py + dy) as u16;
            let tile = &self.map[ty as usize][tx as usize];
            if tile.terrain != Terrain::Wasteland { return false; }
            if self.is_capital_area(tx, ty) { return false; }
        }
        true
    }

    /// Press V: start founding a camp where the player stands.
    pub fn start_found_camp(&mut self) {
        if !self.can_found_camp() { return; }
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
                    // Try diagonal offsets (which are outside the + shape)
                    let candidates: [(i16, i16); 4] = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
                    for (dx, dy) in candidates {
                        let nx = cx + dx;
                        let ny = cy + dy;
                        if nx >= 0 && nx < w && ny >= 0 && ny < h {
                            if !self.is_blocked_for_npc(nx, ny, i) {
                                self.npcs[i].x = nx as u16;
                                self.npcs[i].y = ny as u16;
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

    /// Can the player build a wall segment under their feet?
    pub fn can_build_wall(&self) -> bool {
        if self.player.scrap < crate::config::WALL_SCRAP_COST { return false; }
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
        if tile.terrain != Terrain::Wasteland { return false; }
        if tile.wall.is_some() { return false; }
        if self.is_capital_area(px, py) { return false; }
        true
    }

    pub fn start_build_wall(&mut self) {
        if !self.can_build_wall() { return; }
        self.player.building_wall = Some(BuildWallState {
            started: std::time::Instant::now(),
        });
    }

    pub fn check_build_wall(&mut self) {
        if let Some(ref state) = self.player.building_wall {
            let elapsed = std::time::Instant::now().duration_since(state.started).as_millis();
            if elapsed >= crate::config::BUILD_WALL_TIME_MS as u128 {
                let px = self.player.x;
                let py = self.player.y;
                let faction = self.player.faction;
                self.player.scrap -= crate::config::WALL_SCRAP_COST;
                self.player.building_wall = None;

                let tile = &mut self.map[py as usize][px as usize];
                tile.wall = Some(faction);
                tile.owner = Some(faction);

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

/// Check if (x, y) is inside any capital's 3x3 area.
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
    // Start search from radius 2 (just outside the 3x3 border)
    for r in 2..w.max(h) as i16 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() + dy.abs() > r { continue; }
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
                if dx.abs() + dy.abs() > r { continue; }
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
