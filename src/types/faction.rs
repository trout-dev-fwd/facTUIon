#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactionId {
    Water, // W — Blue
    Gas,   // G — Red
    Scrap, // S — Yellow
    Cult,  // C — Purple
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
