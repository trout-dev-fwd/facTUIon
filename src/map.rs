use rand::Rng;
use rand::rngs::SmallRng;

use crate::config;
use crate::types::{Terrain, Tile};

/// Generate a map with one main resource body per type, capitals nearby, plus scattered tiles.
/// Returns (map, capital_positions) where capitals[i] corresponds to faction i (Water, Gas, Scrap).
pub fn generate_map(
    width: u16,
    height: u16,
    rng: &mut SmallRng,
) -> (Vec<Vec<Tile>>, [(usize, usize); 3]) {
    let w = width as usize;
    let h = height as usize;

    // Start with all wasteland
    let mut grid: Vec<Vec<Terrain>> = vec![vec![Terrain::Wasteland; w]; h];

    // Pick 3 spread-out centers for main resource clusters.
    let margin_x = w / 6;
    let margin_y = h / 6;
    let resources = [Terrain::Water, Terrain::Rocky, Terrain::Ruins];

    let centers = pick_spread_centers(w, h, margin_x, margin_y, rng);

    // Pass 1: grow a main cluster around each center
    for (i, &terrain) in resources.iter().enumerate() {
        let (cx, cy) = centers[i];
        grow_cluster(&mut grid, cx, cy, w, h, terrain, config::MAIN_CLUSTER_SIZE, rng);
    }

    // Pass 2: place capitals ~2 (+/-1) tiles from their resource cluster center
    let mut capitals = [(0usize, 0usize); 3];
    for (i, &(cx, cy)) in centers.iter().enumerate() {
        capitals[i] = place_capital_near(&grid, cx, cy, w, h, rng);
    }

    // Pass 3: scatter lone tiles of each resource away from its main cluster
    for (i, &terrain) in resources.iter().enumerate() {
        let (cx, cy) = centers[i];
        for _ in 0..config::SCATTER_PER_RESOURCE {
            for _ in 0..50 {
                let x = rng.gen_range(1..w - 1);
                let y = rng.gen_range(1..h - 1);
                let dist = ((x as i32 - cx as i32).abs() + (y as i32 - cy as i32).abs()) as usize;
                if dist > w / 4 && grid[y][x] == Terrain::Wasteland {
                    grid[y][x] = terrain;
                    break;
                }
            }
        }
    }

    let map = grid
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|terrain| {
                    let variant = rng.gen_range(0..=255);
                    Tile { terrain, owner: None, wall: None, glyph_variant: variant }
                })
                .collect()
        })
        .collect();

    (map, capitals)
}

/// Place a capital on a wasteland tile ~2 (+/-1) manhattan distance from (cx, cy).
/// Ensures at least CAPITAL_MIN_OPEN_SIDES cardinal neighbors are wasteland.
fn place_capital_near(
    grid: &[Vec<Terrain>],
    cx: usize,
    cy: usize,
    w: usize,
    h: usize,
    rng: &mut SmallRng,
) -> (usize, usize) {
    let base = config::CAPITAL_RESOURCE_DIST;
    let variance = config::CAPITAL_DIST_VARIANCE;
    let min_d = base.saturating_sub(variance);
    let max_d = base + variance;

    for _ in 0..200 {
        let dist = rng.gen_range(min_d..=max_d) as i32;
        let dx_abs = rng.gen_range(0..=dist);
        let dy_abs = dist - dx_abs;
        let dx = if rng.gen_bool(0.5) { dx_abs } else { -dx_abs };
        let dy = if rng.gen_bool(0.5) { dy_abs } else { -dy_abs };

        let nx = cx as i32 + dx;
        let ny = cy as i32 + dy;

        // Need 2 tiles margin from edges for 3x3 border + adjacency
        if nx >= 2 && nx < (w - 2) as i32 && ny >= 2 && ny < (h - 2) as i32 {
            let (nx, ny) = (nx as usize, ny as usize);
            // All 9 tiles of the 3x3 area must be wasteland
            let all_clear = (-1i32..=1).all(|bdy| {
                (-1i32..=1).all(|bdx| {
                    grid[(ny as i32 + bdy) as usize][(nx as i32 + bdx) as usize] == Terrain::Wasteland
                })
            });
            if all_clear {
                return (nx, ny);
            }
        }
    }
    (cx, cy)
}

/// Count how many cardinal neighbors of (x, y) are wasteland.
fn open_sides(grid: &[Vec<Terrain>], x: usize, y: usize, w: usize, h: usize) -> usize {
    let dirs: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    dirs.iter()
        .filter(|&&(dx, dy)| {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            nx >= 0
                && nx < w as i32
                && ny >= 0
                && ny < h as i32
                && grid[ny as usize][nx as usize] == Terrain::Wasteland
        })
        .count()
}

/// Pick 3 centers that are spread apart across the map.
fn pick_spread_centers(
    w: usize,
    h: usize,
    mx: usize,
    my: usize,
    rng: &mut SmallRng,
) -> [(usize, usize); 3] {
    let min_dist = ((w + h) / config::MIN_CLUSTER_DIST_DIVISOR) as i32;
    let mut centers = [(0usize, 0usize); 3];

    for i in 0..3 {
        for _ in 0..200 {
            let x = rng.gen_range(mx..w - mx);
            let y = rng.gen_range(my..h - my);
            let far_enough = (0..i).all(|j| {
                let (ox, oy) = centers[j];
                let d = (x as i32 - ox as i32).abs() + (y as i32 - oy as i32).abs();
                d >= min_dist
            });
            if far_enough {
                centers[i] = (x, y);
                break;
            }
        }
    }
    centers
}

/// Grow an organic cluster of `size` tiles around (cx, cy) using random walks.
fn grow_cluster(
    grid: &mut [Vec<Terrain>],
    cx: usize,
    cy: usize,
    w: usize,
    h: usize,
    terrain: Terrain,
    size: usize,
    rng: &mut SmallRng,
) {
    grid[cy][cx] = terrain;
    let mut placed = 1;
    let mut frontier = vec![(cx, cy)];

    while placed < size && !frontier.is_empty() {
        let idx = rng.gen_range(0..frontier.len());
        let (fx, fy) = frontier[idx];

        // Collect all valid wasteland neighbors, then pick one at random
        let dirs: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        let mut open: Vec<(usize, usize)> = Vec::new();
        for &(dx, dy) in &dirs {
            let nx = fx as i32 + dx;
            let ny = fy as i32 + dy;
            if nx > 0 && nx < (w - 1) as i32 && ny > 0 && ny < (h - 1) as i32 {
                let (nx, ny) = (nx as usize, ny as usize);
                if grid[ny][nx] == Terrain::Wasteland {
                    open.push((nx, ny));
                }
            }
        }

        if open.is_empty() {
            // This frontier tile is boxed in — remove it
            frontier.swap_remove(idx);
        } else {
            let &(nx, ny) = &open[rng.gen_range(0..open.len())];
            grid[ny][nx] = terrain;
            frontier.push((nx, ny));
            placed += 1;
        }
    }
}
