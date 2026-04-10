use super::faction::FactionId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Terrain {
    Wasteland, // .
    Water,     // ~
    Rocky,     // ^
    Ruins,     // :
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
            Terrain::Rocky => self.glyph(),
        }
    }
}

#[derive(Clone)]
pub struct Tile {
    pub terrain: Terrain,
    pub owner: Option<FactionId>,
    pub wall: Option<FactionId>, // free-standing wall segment
    pub glyph_variant: u8,       // seeded per-tile for stable visual variety
}
