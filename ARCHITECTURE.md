# Mad Max TUI — Architecture Document

## Overview

A two-player terminal-based strategy game set in a post-apocalyptic world. Players spawn
as champions of rival factions on a procedurally generated map, competing to dominate
territory while managing a living world simulation of autonomous factions, trade, war,
and the ever-spreading Cult.

**Stack**
```toml
ratatui    = "0.29"   # rendering
crossterm  = "0.28"   # raw input / terminal backend
tokio      = { version = "1", features = ["full"] }
serde      = { version = "1", features = ["derive"] }
bincode    = "1"      # fast binary serialization over TCP
rand       = { version = "0.8", features = ["small_rng"] }
```

---

## ASCII Visual Language

### Terrain Glyphs
```
.  = wasteland       (abundant — the default map surface)
~  = water/oasis     (sparse — Water Hoarder resource)
^  = rocky highland  (sparse — Gas Runner fuel source)
:  = ruins           (sparse — Scrap Merchant salvage)
```

Terrain distribution: ~70% wasteland, ~10% each for ~, ^, : — enough for factions
to survive early game without being handed dominance.

### Resources
```
*  = fuel
°  = scrap
≈  = water
₵  = currency (Crowns) — weightless, no carry cap
```

### Faction Territory — Color Coded, No Border Glyphs

Territory is expressed through text color. Terrain glyphs remain fully visible at all times.
Border ASCII characters are not used. Tile color is derived at render time from
`tile.owner` — it is NOT stored on the tile struct.

```
W units / owned tiles  → Blue
G units / owned tiles  → Red
S units / owned tiles  → Yellow
C units / owned tiles  → Magenta
Neutral tiles          → Default terminal color
Dead units / corpses   → Gray (lootable for N slow ticks, then removed)
```

### Factions
```
W / w  = Water Hoarders   (capital / unit) — Blue
G / g  = Gas Runners      (capital / unit) — Red
S / s  = Scrap Merchants  (capital / unit) — Yellow
C / c  = Cult             (camp / unit)    — Magenta
```

All NPC units share the same lowercase glyph regardless of role (worker/trader/soldier).
Players read intent from movement behavior only — no labels, no tooltips.
Dead NPCs display in Gray regardless of their original faction.

### Players
```
@    = local player  (empty handed)
@*   = carrying fuel (one or more)
@°   = carrying scrap
@≈   = carrying water
O    = remote player (same suffixes apply)
```

Players always render in white. Carrying suffix shows the dominant resource type
if carrying mixed resources.

---

## Data Structures

### Tile
```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct Tile {
    pub terrain: Terrain,              // Wasteland, Water, Rocky, Ruins
    pub owner: Option<FactionId>,      // None = neutral; drives render color at draw time
    pub resource_type: Option<Resource>,
    pub resource_value: u8,
    pub extractor: Option<UnitId>,     // only one extractor at a time
                                       // MUST be cleared to None when unit leaves tile
}
```

### Faction
```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct Faction {
    pub id: FactionId,
    pub capital: (u16, u16),
    pub stockpile: HashMap<Resource, u32>,  // grows only when workers physically deposit
    pub treasury: u32,                       // shared Crown pool for entire faction
    pub member_count: u32,
    pub rep: [i8; 4],
    pub at_war_with: Vec<FactionId>,
}

const UPKEEP_PER_MEMBER: u32 = 1;

fn capital_upkeep_tick(faction: &mut Faction) {
    let drain = faction.member_count * UPKEEP_PER_MEMBER;
    for resource in faction.stockpile.values_mut() {
        *resource = resource.saturating_sub(drain);
    }
}
```

### Unit
```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct Unit {
    pub id: UnitId,
    pub pos: (u16, u16),
    pub faction: FactionId,
    pub behavior: UnitBehavior,
    pub home_capital: (u16, u16),      // set at spawn, never changes — workers return here
    pub carrying: Vec<Resource>,        // physical resources — max UNIT_CARRY_CAP (5)
    pub gold: u32,                      // Crowns this unit is personally carrying
                                        // for traders: mission budget from faction treasury
                                        // lootable on death
    pub target: Option<(u16, u16)>,
    pub hostile_toward: Vec<FactionId>,
    pub alive: bool,
    pub death_tick: Option<u64>,        // set on death, corpse removed after CORPSE_LINGER_TICKS
}

const UNIT_CARRY_CAP: usize = 5;       // same cap as player — workers must return when full
const CORPSE_LINGER_TICKS: u64 = 5;   // slow ticks before corpse tile is removed

pub enum UnitBehavior {
    Worker,    // extract → fill inventory → return to home_capital → deposit → repeat
    Trader,    // take resources + gold budget from capital → path to allied/neutral capital
               // sell resources for gold, buy resources with gold → return home → deposit
    Soldier,   // patrol border, attack hostiles, claim tiles when at war
}
```

### Worker State
Workers have a two-phase cycle. Track phase via a field on the unit:

```rust
pub enum WorkerPhase {
    Extracting { progress: u8 },   // working the tile, progress toward WORK_TICKS_REQUIRED
    Returning,                      // inventory full, walking to home_capital to deposit
}
// Add WorkerPhase as an optional field on Unit, None for non-workers
pub worker_phase: Option<WorkerPhase>,
```

Faction stockpile only grows when a Worker physically arrives at the capital and deposits.
Resources lost mid-journey (worker killed) are gone permanently.

### Player
```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct Player {
    pub pos: (u16, u16),
    pub faction: FactionId,
    pub carrying: Vec<Resource>,   // shared weight pool — total capped at CARRY_CAP
    pub currency: u32,             // Crowns — weightless, no cap
    pub working_ticks: u8,
}

// Carry weight is a single shared pool across all resource types.
// 2 scrap + 3 fuel = 5/5 carry — no room for water regardless of type.
// Currency never counts against carry weight.
const CARRY_CAP: usize = 5;
```

### World
```rust
pub struct World {
    pub map: Vec<Vec<Tile>>,
    pub factions: [Faction; 4],
    pub units: Vec<Unit>,
    pub cult_camps: Vec<(u16, u16)>,   // source of truth for camp positions
                                        // map colors derive from this — never the reverse
    pub faction_rep: [[i8; 4]; 4],
    pub player_rep: [[i8; 2]; 4],
    pub tick: u64,
    pub seed: u64,
    pub rng: SmallRng,                  // seeded from seed, advanced identically on both clients
                                        // NEVER use thread_rng() anywhere in simulation code
}
```

---

## Currency — Crowns (₵)

### Faction Economy Flow
```
Worker extracts resource → walks to capital → deposits into faction stockpile
                                                        ↓
Foreign trader arrives → sells resources → Crown enters faction treasury
                                                        ↓
Own trader departs → draws resource from stockpile + Crown budget from treasury
                  → paths to allied/neutral capital → sells resources for Crowns
                  → buys resources with remaining Crowns → returns → deposits all
```

- All Crowns earned by traders flow into the **faction treasury** (shared pool)
- Traders draw a **personal gold budget** from the treasury at mission start
- If a trader is killed mid-mission, their personal gold and carried resources are
  lootable by whoever reaches the corpse. The faction treasury does not recover that gold.
- Players trade directly with capitals: sell resources for Crowns, buy resources with Crowns
- Cult camps trade at premium (buy) and below-market (sell)

### Market Pricing
```
Standard capital pricing:
  buy resource  → price scales with scarcity (low stockpile = higher cost)
  sell resource → receive Crowns at current market rate

Cult camp pricing:
  buy resource  → 1.5–2x market rate (premium — they cannot extract)
  sell resource → below market rate
```

---

## Map Generation

Four passes from a shared u64 seed. Both clients generate identical maps locally.
No map data is ever transmitted.

```rust
fn generate_map(width: u16, height: u16, seed: u64) -> Vec<Vec<Tile>> {
    // Pass 1: cellular automata → organic terrain
    //   ~70% wasteland, smoothed blobs of ~, ^, : scattered throughout
    //   Guarantee MIN_RESOURCE_SITES of each type exist

    // Pass 2: place faction capitals with terrain affinity
    //   W capital: adjacent to at least one ~ tile
    //   G capital: adjacent to at least one ^ tile
    //   S capital: adjacent to at least one : tile
    //   C first camp: anywhere, min distance from all other capitals
    //   All capitals: MIN_CAPITAL_DISTANCE apart

    // Pass 3: scatter additional resource tiles
    //   Soft clusters near matched capital region
    //   A few surprise sites in distant locations

    // Pass 4: seed starting territory
    //   Small faction-colored blob around each capital
    //   One Worker pre-placed on an adjacent resource tile per faction
}
```

---

## Screen State Machine

`main.rs` owns a single `Screen` enum and matches on it each frame.
No screen module initiates its own transitions — each returns an outcome that main.rs acts on.

```rust
pub enum Screen {
    Setup(SetupState),
    Game(GameState),
    End(EndState),
}
```

---

## Screens

### 1. Setup Screen

Controls are shown here so players enter the game fully informed.

```
╔══════════════════════════════════════════════╗
║         MAD MAX TUI  —  WASTELAND            ║
╠══════════════════════════════════════════════╣
║                                              ║
║  [H] Host a game                             ║
║  [J] Join a game   IP: 192.168.1.__          ║
║                                              ║
╠══════════════════════════════════════════════╣
║  CONTROLS                                    ║
║                                              ║
║  WASD / arrows   Move                        ║
║  Shift+WASD      Attack in direction         ║
║  E               Work extraction tile        ║
║  F               Claim current tile          ║
║  T               Trade (when at capital)     ║
║  X               Loot corpse (when adjacent) ║
║  Q               Quit                        ║
║                                              ║
╠══════════════════════════════════════════════╣
║  Carry weight: 5 total across all resources  ║
║  Currency (₵) is weightless — no cap        ║
╚══════════════════════════════════════════════╝
```

On connection, both players see faction assignment:
```
║  You are: GAS RUNNERS [G]                    ║
║  Opponent: WATER HOARDERS [W]                ║
║                                              ║
║  [ENTER] Begin                               ║
```

### 2. Game Screen

Map fills most of the terminal. HUD is pinned to the bottom 2–3 lines.

**HUD — 2 lines normally, expands to 3 when adjacent to a capital or Cult camp**

```
// Normal:
Line 1:  W ╞══▰═══════╡ 22%   G ╞════▰═════╡ 34%   S ╞═▰════════╡ 18%   C ╞▰══════════╡ 8% conv
Line 2:  [@] GAS RUNNERS  |  *fuel *fuel °scrap [3/5]  |  ₵ 340  |  Rep→W:+40 S:+10 C:-20

// Adjacent to capital or Cult camp (line 3 appears, map is NOT obscured):
Line 3:  [Water Hoarders]  water:42  scrap:8  ₵220  Rep→You:+61  [T]Trade
Line 3:  [Cult Camp #3]  water:12(premium)  fuel:6(premium)  ₵88  [T]Trade
```

**Adjacent corpse indicator on line 2:**
```
Line 2:  [@] GAS RUNNERS  |  *fuel [1/5]  |  ₵ 340  |  [X] Loot corpse
```

### 3. End Screen

**Faction victory:**
```
╔══════════════════════════════════════╗
║           WASTELAND CLAIMED          ║
╠══════════════════════════════════════╣
║                                      ║
║   GAS RUNNERS control the world.     ║
║                                      ║
║   You: GAS RUNNERS     ← WINNER      ║
║   Opponent: WATER HOARDERS           ║
║                                      ║
║   G ╞══════▰═══╡ 61%                 ║
║   W ╞══▰═══════╡ 22%                 ║
║   S ╞▰══════════╡ 11%                ║
║   C ╞▰══════════╡  6% converted      ║
║                                      ║
║   [R] Rematch    [Q] Quit            ║
╚══════════════════════════════════════╝
```

**Cult victory (both players lose):**
```
╔══════════════════════════════════════╗
║         THE WASTELAND IS THEIRS      ║
╠══════════════════════════════════════╣
║                                      ║
║   The Cult has consumed all.         ║
║   Both players failed.               ║
║                                      ║
║   C ╞═════════▰╡ 51% converted       ║
║                                      ║
║   [R] Rematch    [Q] Quit            ║
╚══════════════════════════════════════╝
```

---

## HUD Bar Renderer

Single pure function — used for territory bars, carry status, and end screen.

```rust
fn render_bar(value: f32, width: usize) -> String {
    let pct    = (value.clamp(0.0, 1.0) * 100.0) as usize;
    let filled = ((value * (width - 2) as f32) as usize).saturating_sub(1);
    let empty  = width - 2 - filled - 1;
    format!("╞{}▰{}╡ {}%", "═".repeat(filled), "═".repeat(empty), pct)
}
```

---

## Tick Architecture

### Fast Loop — ~16ms (60fps)
- Read player input
- Move player
- Resolve immediate interactions (pickup, adjacency check, combat, loot)
- Render via ratatui (full frame redraw every tick — nothing retained between frames)

### Slow Tick — ~3000ms
- Capital upkeep drain
- Worker deposit on arrival at capital
- Faction rep drift
- Unit behavior tick (workers extract/return, traders path, soldiers patrol/claim)
- Corpse expiry (remove units where `tick - death_tick > CORPSE_LINGER_TICKS`)
- Cult tick (conversion attempts, fission check, dormant respawn)
- War declarations
- Win condition check

```rust
tokio::select! {
    _ = fast_loop(&mut terminal, &mut state) => {}
    _ = slow_tick_loop(&mut world)           => {}
    _ = net_loop(&mut socket, &mut state)    => {}
}
```

Slow tick functions must complete without `.await` inside them.
Async is only for the timer and network loop.

---

## Faction AI

### Worker Cycle (slow tick)

```rust
fn worker_tick(unit: &mut Unit, world: &mut World) {
    match unit.worker_phase {
        Some(WorkerPhase::Extracting { ref mut progress }) => {
            let tile = &mut world.map[unit.pos.1 as usize][unit.pos.0 as usize];
            if tile.extractor == Some(unit.id) {
                *progress += 1;
                if *progress >= WORK_TICKS_REQUIRED {
                    if unit.carrying.len() < UNIT_CARRY_CAP {
                        unit.carrying.push(tile.resource_type.unwrap());
                    }
                    *progress = 0;
                }
                if unit.carrying.len() >= UNIT_CARRY_CAP {
                    // inventory full — head home
                    tile.extractor = None;
                    unit.target = Some(unit.home_capital);
                    unit.worker_phase = Some(WorkerPhase::Returning);
                }
            } else if tile.extractor.is_none() {
                tile.extractor = Some(unit.id);
            }
            // else: tile occupied by another extractor — wander to adjacent resource tile
        }
        Some(WorkerPhase::Returning) => {
            step_toward(unit, unit.home_capital);
            if unit.pos == unit.home_capital {
                deposit_to_capital(unit, world);  // moves resources to faction stockpile
                unit.worker_phase = Some(WorkerPhase::Extracting { progress: 0 });
                // find nearest free resource tile and walk to it
            }
        }
        None => {} // not a worker
    }
}
```

### Trader Cycle (slow tick)
```rust
// Departure: draw resource from stockpile + gold budget from treasury
// Path to allied/neutral capital (halts at war borders)
// On arrival: sell resources → gold added to unit.gold; buy resources with remaining gold
// Return to home_capital
// On deposit: unit.carrying → faction stockpile, unit.gold → faction treasury
```

If a trader is killed mid-mission:
- `unit.carrying` (purchased resources) and `unit.gold` (remaining budget) are lootable
- Faction treasury does not recover the lost gold budget

### Rep Drift (slow tick)
```rust
fn rep_drift(world: &mut World) {
    for a in 0..4 {
        for b in 0..4 {
            if a == b { continue; }
            if factions_share_peaceful_border(world, a, b) {
                world.faction_rep[a][b] = (world.faction_rep[a][b] + 1).min(100);
            }
        }
    }
}
```

### War Declaration (slow tick)
```rust
const WAR_THRESHOLD: i8 = -40;

fn check_wars(world: &mut World) {
    for a in 0..4 {
        for b in 0..4 {
            if world.faction_rep[a][b] < WAR_THRESHOLD
            && !world.factions[a].at_war_with.contains(&b.into()) {
                declare_war(world, a, b);
                // traders reroute or return home immediately
                // soldiers shift to claiming/attacking behavior
            }
        }
    }
}
```

### Soldier Behavior (slow tick)
```rust
fn soldier_tick(unit: &mut Unit, world: &mut World) {
    if world.factions[unit.faction as usize].at_war_with.is_empty() {
        patrol_border(unit, world);
    } else {
        claim_or_attack(unit, world);
        // walking over a neutral/enemy tile claims it for unit's faction (color change only)
    }
}
```

### Starvation (slow tick)
```rust
fn starvation_tick(faction: &mut Faction, units: &mut Vec<Unit>) {
    if faction.stockpile.values().all(|&v| v == 0) {
        if let Some(idx) = random_unit_of_faction(units, faction.id) {
            units[idx].faction = FactionId::Neutral;
            faction.member_count -= 1;
        }
    }
}
```

---

## Death and Looting

### On Death
```rust
fn on_unit_death(unit: &mut Unit, tick: u64) {
    unit.alive = false;
    unit.death_tick = Some(tick);
    // unit remains in world.units, rendered gray, until corpse expires
    // tile.extractor is cleared if this unit was extracting
}
```

### Corpse Rendering
Dead units render as gray regardless of original faction:
```rust
let color = if !unit.alive {
    Color::DarkGray
} else {
    faction_color(unit.faction)
};
```

### Looting (player input: X when adjacent to dead unit)
```rust
fn loot_corpse(player: &mut Player, unit: &mut Unit) {
    // Transfer as many resources as carry weight allows
    while player.carrying.len() < CARRY_CAP && !unit.carrying.is_empty() {
        player.carrying.push(unit.carrying.remove(0));
    }
    // Currency is weightless — always fully transferred
    player.currency += unit.gold;
    unit.gold = 0;
}
```

### Corpse Expiry (slow tick)
```rust
fn expire_corpses(world: &mut World) {
    world.units.retain(|u| {
        if let Some(death_tick) = u.death_tick {
            world.tick - death_tick < CORPSE_LINGER_TICKS
        } else {
            true
        }
    });
}
```

---

## The Cult

Non-player game clock. Cannot be joined. Cult victory = both players lose.

### Properties
- `world.cult_camps` is the source of truth — map colors derive from it, not the reverse
- Small magenta territory ring around each camp
- No extraction — premium buyer, below-market seller
- Rep >= 0 → passive conversion attempts on nearby units each slow tick
- Rep <  0 → attack directly; converted units are not lootable (they walk away)
- Declaring war triggers all camps to go hostile simultaneously

### Conversion (slow tick)
```rust
fn cult_unit_tick(unit: &Unit, nearby: &mut Vec<Unit>, cult_rep: i8, rng: &mut SmallRng) {
    if cult_rep >= 0 {
        for target in nearby.iter_mut().filter(|u| u.faction != FactionId::Cult) {
            let chance = 15 + (cult_rep.max(0) / 10) as u32;
            if rng.gen_range(0..100) < chance {
                target.faction = FactionId::Cult;
            }
        }
    } else {
        unit.target = Some(nearest_non_cult_pos(nearby));
    }
}
```

### Fission (slow tick)
```rust
const FISSION_MEMBERS:   u32 = 8;
const FISSION_RESOURCES: u32 = 20;

fn cult_fission_tick(world: &mut World) {
    for &camp in &world.cult_camps.clone() {
        let members   = cult_members_near(camp, &world.units);
        let resources = cult_resources_at(camp, &world.factions);
        if members >= FISSION_MEMBERS && resources >= FISSION_RESOURCES {
            let target = furthest_open_tile(&world.map);
            spawn_splinter(world, camp, target);
        }
    }
}
```

### Dormant Respawn (slow tick)
```rust
fn cult_dormant_tick(world: &mut World) {
    if world.units.iter().all(|u| u.faction != FactionId::Cult) {
        for unit in world.units.iter_mut() {
            if world.rng.gen_range(0..1000) < 1 {
                unit.faction = FactionId::Cult;
                world.cult_camps.push(unit.pos);
                break;
            }
        }
    }
}
```

---

## Player Interactions

### Input Map
```
WASD / arrows     Move
Shift+WASD        Attack in direction (deliberate — always causes rep drop)
F                 Claim current tile (aggression toward tile owner)
E                 Work extraction tile (WORK_TICKS_REQUIRED ticks)
T                 Trade (adjacent to capital/camp, not at war with them)
X                 Loot adjacent corpse (transfers resources + gold)
Q                 Quit
```

### Interaction Resolution (fast loop)
```
move into terrain tile    → move
move into resource tile   → pick up if total carry < CARRY_CAP, else "hands full"
adjacent to capital/camp  → HUD line 3 appears with that location's info
adjacent to dead unit     → HUD line 2 shows [X] Loot prompt
Shift+move into unit      → combat (attacker advantage if target unaware)
Shift+move into player    → territory check → combat or block
E on extraction tile      → lock tile if free, begin working_ticks counter
F on tile                 → claim it, rep hit toward current owner
T adjacent to capital     → trade if rep and war state allow
X adjacent to corpse      → loot resources and gold from dead unit
```

### Player Working (extraction/theft)
```rust
const WORK_TICKS_REQUIRED: u8 = 5;

fn player_work_tick(player: &mut Player, tile: &mut Tile, player_faction: FactionId) {
    if tile.resource_type.is_some() && tile.extractor.is_none() {
        tile.extractor = Some(PLAYER_UNIT_ID);
        player.working_ticks += 1;
        if player.working_ticks >= WORK_TICKS_REQUIRED {
            if player.carrying.len() < CARRY_CAP {
                player.carrying.push(tile.resource_type.unwrap());
            }
            tile.extractor = None;
            player.working_ticks = 0;
            if tile.owner != Some(player_faction) {
                // rep hit toward tile owner
            }
        }
    }
}
```

### Combat
```rust
fn combat(attacker_power: u32, defender_power: u32, aware: bool, rng: &mut SmallRng) -> CombatResult {
    let def      = if aware { defender_power } else { defender_power / 2 };
    let atk_roll = attacker_power + rng.gen_range(0..10);
    let def_roll = def + rng.gen_range(0..10);
    if atk_roll > def_roll { CombatResult::AttackerWins }
    else { CombatResult::DefenderWins }
}
```

### Rep Consequences
```
Attack enemy faction unit    → small rep drop (expected in war)
Attack neutral faction unit  → large rep drop (act of aggression)
Attack allied faction unit   → massive rep drop, likely triggers war
Attack other player          → their faction soldiers gain aggro target
```

### Avenging
```rust
fn check_avenge(event: &CombatEvent, world: &mut World) {
    for unit in world.units.iter_mut()
        .filter(|u| u.alive)
        .filter(|u| u.faction == event.victim_faction)
        .filter(|u| world.map[u.pos.1 as usize][u.pos.0 as usize].owner
                    == Some(event.victim_faction))
        .filter(|u| distance(u.pos, event.pos) < AGGRO_RANGE)
    {
        unit.target = Some(event.attacker_pos);
    }
}
```

---

## Win Conditions

```rust
const TERRITORY_WIN: f32 = 0.60;
const CULT_WIN:      f32 = 0.50;

fn check_win(world: &World) -> Option<Outcome> {
    for faction in [FactionId::W, FactionId::G, FactionId::S] {
        if territory_pct(world, faction) >= TERRITORY_WIN {
            return Some(Outcome::FactionWins(faction));
        }
    }
    if cult_conversion_pct(world) >= CULT_WIN {
        return Some(Outcome::CultWins);
    }
    None
}
```

---

## Networking

### Core Rule
World state is never serialized or transmitted. Only player actions cross the wire.
Both clients run identical deterministic simulation from the shared seed.

```rust
// World state is never serialized — seed determinism only.
// If you are tempted to send World over the network, stop and reconsider.
```

### Action Sequencing
The host is authoritative. Guest sends actions to host, host applies them and
broadcasts back. This prevents desync when both players act on the same NPC
simultaneously. Guest accepts one round-trip of input latency in exchange for
guaranteed state consistency.

### Handshake
```
host binds 0.0.0.0:7878
guest connects to host IP:7878

host → guest:  { seed: u64, host_faction: FactionId }
guest → host:  { guest_faction: FactionId }   // rerolled if matches host
```

### Message Types
```rust
#[derive(Serialize, Deserialize)]
pub enum NetMessage {
    PlayerMove   { pos: (u16, u16), carrying: Vec<Resource>, currency: u32 },
    PlayerAction { action: Action },  // Claim, Attack, Work, Trade, Loot
}
```

### Launch
```
cargo run -- host
cargo run -- join 192.168.1.x
```

---

## Project Structure

```
src/
  main.rs          entry point, arg parsing, Screen state machine owner
  screens/
    setup.rs       host/join UI, controls display, connection handshake
    game.rs        main game screen — wires fast loop + slow tick + net loop
    end.rs         victory/defeat display, rematch logic
  map.rs           Tile, terrain generation
  world.rs         World, slow tick orchestration, win condition check
  faction.rs       Faction AI — worker cycle, trader cycle, soldier, rep drift, starvation
  cult.rs          Cult logic — conversion, fission, dormant respawn
  player.rs        Player state, input handling, interaction resolution
  combat.rs        Combat rolls, rep consequences, avenging, death handling
  currency.rs      Crown economy, market pricing, trade logic
  render.rs        All ratatui draw calls — single render(f, state) entry point
                   Color from owner at draw time, gray for dead units
                   HUD bars, HUD line 3 adjacency, corpse loot prompt
  net.rs           Tokio TCP host/join, NetMessage serialization, host authority model
```

---

## Claude Code Implementation Notes

Read before generating any code. These are the non-obvious constraints most likely
to cause silent bugs or hard-to-fix structural problems.

### Ratatui is immediate mode
Every fast loop tick calls `render(f, &state)` which redraws the entire frame from
scratch. Never store ratatui widget structs between frames. Nothing is retained.
All draw logic lives in `render.rs` via a single `fn render(f: &mut Frame, state: &AppState)`.

### Tile color is derived at render time, never stored
```rust
let color = match tile.owner {
    Some(FactionId::W) => Color::Blue,
    Some(FactionId::G) => Color::Red,
    Some(FactionId::S) => Color::Yellow,
    Some(FactionId::C) => Color::Magenta,
    None               => Color::Reset,
};
// Dead units override to Color::DarkGray regardless of faction
```
Do not add a `color` field to `Tile` or `Unit`. Doing so creates a sync problem.

### RNG must be deterministic — never use thread_rng()
```rust
// World holds one seeded RNG, passed explicitly to everything that needs randomness
pub rng: SmallRng  // SmallRng::seed_from_u64(seed)

// Every call to rng advances it — both clients must call in identical order each tick
// NEVER call thread_rng() or any other RNG source in simulation code
```

### Panic handler must restore the terminal
```rust
let original_hook = std::panic::take_hook();
std::panic::set_hook(Box::new(move |info| {
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen
    );
    original_hook(info);
}));
```
Without this, any panic leaves the player's terminal broken.

### Slow tick has no .await inside it
Slow tick functions run synchronously to completion inside `tokio::select!`.
Never add `.await` points inside world simulation logic. Async is only for
the timer and the network loop.

### tile.extractor must be cleared on unit move
```rust
fn on_unit_move(unit: &Unit, old_pos: (u16, u16), world: &mut World) {
    let tile = &mut world.map[old_pos.1 as usize][old_pos.0 as usize];
    if tile.extractor == Some(unit.id) {
        tile.extractor = None;
    }
}
```
This applies to all units including the player (PLAYER_UNIT_ID).
Failing to clear this permanently locks the extraction tile.

### world.cult_camps is the source of truth for camp locations
Do not infer camp positions by scanning tiles for magenta color. The map colors
derive from `cult_camps`, never the reverse.

### Faction stockpile grows only on worker deposit, not continuously
Workers extract resources into their personal inventory (max 5), walk back to
`home_capital`, and deposit on arrival. The stockpile does not grow until the
worker physically arrives. Resources lost mid-journey are gone.

### home_capital is set at spawn and never changes
Do not re-derive a worker's home from `world.factions[faction].capital` dynamically.
If a capital is captured mid-mission, the worker still returns to the original home
position. Store `home_capital: (u16, u16)` on the unit at spawn.

### Trader gold is separate from faction treasury
At mission start, the faction treasury decreases by the trade budget and the
trader's `unit.gold` increases by the same amount. On return, `unit.gold` flows
back into the treasury. If killed, `unit.gold` stays on the corpse and is lootable.

### Screen transitions happen only in main.rs
Screen modules return an outcome or event. main.rs matches on it and transitions.
No screen module calls another screen directly.

### Terminal minimum size
Check terminal dimensions on startup and on resize events. Display a
"please resize your terminal" message rather than panicking if below minimum.

### Carry weight is a shared pool
`player.carrying.len()` is total weight. A player with 3 fuel + 2 scrap has 5/5 carry
and cannot pick up any resource regardless of type. Same applies to NPC units
(`unit.carrying.len()` vs `UNIT_CARRY_CAP`). Currency never counts toward weight.

---

## What's Intentionally Excluded (for now)

- Pathfinding beyond cardinal stepping toward a target position
- Map larger than ~60×20 tiles
- Save / persistence (sessions are ephemeral)
- Sound
- More than 2 players
- Animated tile transitions

---

## Build Lessons — Wiring Failures to Avoid

This section documents systemic issues found after the initial 6-phase build.
The code compiled and tests passed, but large portions of the game were
non-functional because functions were defined but never called from production
code paths. These are not bugs in individual functions — they are **integration
failures** where the caller was never written or was written incorrectly.

Every item below actually happened. Discuss each before starting implementation.

### 1. The renderer must use live world state, not a snapshot

`build_app_state` created a fresh `World::new(seed)` every frame instead of
referencing the actual `GameState.world`. This meant the player saw the tick-0
map permanently — all NPC movement, territory changes, wars, cult spread, and
stockpile changes were invisible.

**Rule:** Any render snapshot must derive from the live game state. If World
needs to be cloned (it now derives Clone), clone the actual state. Never
reconstruct from the seed — that produces the initial state, not the current one.

### 2. The slow tick must be called from the game loop

`world.slow_tick()` was fully implemented and tested in isolation, but the game
loop in `main.rs` never called it. The entire NPC simulation was frozen. Workers
never extracted, soldiers never moved, the cult never spread, wars never fired.

**Rule:** Every tick function defined in the architecture (slow tick, cult tick,
faction mint, etc.) must have a call site in the main game loop. If a function
exists but `grep -r "function_name" src/main.rs` returns nothing, it's dead.
Verify this for every public function in world.rs and faction.rs after
implementation.

### 3. Combat orchestration functions must be called, not re-implemented inline

`combat::player_attacks_npc()` and `combat::npc_attacks_player()` were fully
implemented but never called. Instead, `player_attack()` in player.rs
duplicated the entire combat logic inline (~60 lines). The NPC-attacks-player
path was never wired at all — soldiers could not attack the player.

**Rule:** When the architecture defines an orchestration function
(player_attacks_npc, npc_attacks_player), the caller must delegate to it.
The caller computes the target position and index; the orchestration function
handles the combat roll, death, avenge, rep, and pushback. Do not inline
the orchestration logic.

### 4. Network loop must be spawned with proper channels

`net::net_loop()` was fully implemented but never spawned. The TcpStream from
the handshake was stored on GameState and then ignored for the rest of the
session. Additionally:

- Neither player sent their position to the peer (no outgoing PlayerMove)
- Channel senders were immediately dropped (e.g., `let (_action_tx, action_rx)`)
- The remote player was permanently invisible

**Rule:** After the handshake produces a TcpStream, the transition to the Game
screen must:
1. Create channel pairs (outgoing_tx/rx, remote_tx/rx)
2. Store the senders/receivers on GameState
3. `tokio::spawn` the net_loop with the stream and channel endpoints
4. In the game loop, send PlayerMove each frame via outgoing_tx
5. In the game loop, drain remote_rx to update remote_player_pos

If any channel endpoint is prefixed with `_` (unused), that's a red flag —
it means the data path is broken.

### 5. The external IP must be fetched, not the LAN IP

`get_local_ip()` uses a UDP socket trick that returns the machine's LAN address
(e.g., 192.168.x.x). This is useless for a guest connecting over the internet.
The setup screen must show the external/public IP.

**Rule:** Use an async HTTP request to a public IP service (e.g.,
api.ipify.org) to get the external IP. Start the lookup in the background at
SetupState creation and fall back to the LAN IP if the request fails. Show the
LAN IP immediately and update to the external IP when the async lookup completes.

### 6. Trade actions must have concrete keybindings and call currency functions

The Trade action (T key) was mapped to `HudMessage::None` — a literal no-op.
The comment said "handled by currency module in game loop" but no such handling
existed. The fully-tested `currency::player_sell_to_capital()` and
`currency::player_buy_from_capital()` were never called from production code.

**Rule:** Every player action listed in the input map must have a concrete
implementation wired in `handle_player_action` or `poll_game`. If an action
depends on context (adjacent to capital), the handler must check that context
and call the appropriate function. "Will be handled elsewhere" comments are a
sign the wiring was skipped.

The trade UI uses contextual number keys when adjacent to a capital:
- 1/2/3 = sell Fuel/Scrap/Water
- 4/5/6 = buy Fuel/Scrap/Water
- The HUD line 3 shows prices and keybindings when adjacent

### 7. Player extraction must advance on slow tick, not only on keypress

`player_work_tick()` requires WORK_TICKS_REQUIRED (5) ticks to complete. It was
only called on E keypress, meaning the player had to press E five times to
extract one resource. NPC workers advance automatically each slow tick.

**Rule:** Pressing E starts extraction (sets working_ticks to 1 and locks the
tile). The slow tick in main.rs must then call `player_work_tick()` each tick
while `working_ticks > 0`. The player stays on the tile; moving away resets
working_ticks and clears the extractor.

### 8. Unused imports and dead code are integration signals

The build produced 20+ warnings for unused imports, unused variables, and dead
functions. These were not cosmetic — they were direct evidence of unwired
features:

- `unused import: interval, MissedTickBehavior` → slow tick loop never built
- `unused import: TcpStream` → networking never wired in game loop
- `unused import: SLOW_TICK_MS` → slow tick constant imported but never used
- `function player_attacks_npc is never used` → combat delegation missing
- `function generate_map is never used` → superseded by generate_map_full
- `unused variable: outcome` → return value discarded, logic dead

**Rule:** Zero warnings policy. After each phase, run `cargo check` and treat
every unused-import or dead-code warning as a potential unwired feature. Trace
each warning to determine if it indicates a missing call site.

### 9. End screen must use actual game state

The End screen received `World::new(seed)` (a fresh world) and hardcoded `0`
for the remote faction. The territory bars and faction info on the end screen
reflected the initial state, not the final game state.

**Rule:** EndState must receive the live world (clone it) and the actual
remote_faction stored on GameState. The remote_faction must be tracked from
the handshake result through to the end screen.

### 10. net_loop signature must support tokio::spawn

The original `net_loop` took `&mut TcpStream` (a borrow), which cannot be
moved into a `tokio::spawn` closure (requires `'static`). The function must
take owned `TcpStream`.

**Rule:** Any async function intended to run as a spawned task must take owned
values, not references. Design the signature for spawning from the start.

### 11. Duplicate functions create confusion about which to call

`apply_combat_rep` existed in both `combat.rs` and `faction.rs` with different
signatures. `generate_map` and `generate_map_full` coexisted in `map.rs`.
`neighbors` was duplicated in `world.rs` and `map.rs`. This created ambiguity
about which version to call, and callers sometimes picked neither.

**Rule:** Each concept has one canonical function. If a function is superseded,
remove or cfg(test) the old version immediately. If two modules need the same
logic, one calls the other — do not duplicate.
