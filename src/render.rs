use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::types::GameState;

fn render_bar(value: f32, width: usize) -> String {
    let pct = (value.clamp(0.0, 1.0) * 100.0) as usize;
    let inner = width - 2;
    let filled = ((value * inner as f32) as usize).min(inner.saturating_sub(1));
    let empty = inner.saturating_sub(filled).saturating_sub(1);
    format!("╞{}▰{}╡ {}%", "═".repeat(filled), "═".repeat(empty), pct)
}

pub fn render(f: &mut Frame, state: &GameState) {
    let area = f.area();
    let map = &state.map;
    let px = state.player.x;
    let py = state.player.y;

    // Reserve 2 lines for HUD normally, 3 when adjacent to a capital
    let adjacent_cap = state.adjacent_capital();
    // Player line + controls line, plus capital info + trade line (2 extra) when adjacent
    let hud_lines = if adjacent_cap.is_some() { 4 } else { 2 };
    let map_height = (area.height as usize).saturating_sub(hud_lines).min(map.len());
    // Each tile is 2 chars wide to compensate for terminal cells being ~2x taller than wide
    let map_width = (area.width as usize) / 2;

    let mut lines: Vec<Line> = Vec::with_capacity(map_height);

    for row in 0..map_height {
        let mut spans: Vec<Span> = Vec::new();
        let row_width = map[row].len().min(map_width);
        for col in 0..row_width {
            let tile = &map[row][col];
            let tile_bg = if tile.terrain == crate::types::Terrain::Wasteland {
                tile.owner.map(|o| o.color())
            } else {
                None
            };

            // On colored bg: use terminal bg color as fg for contrast
            // Off colored bg: use terminal fg color
            let on_territory = tile_bg.is_some();
            let entity_fg = if on_territory {
                crate::config::TERMINAL_BG
            } else {
                crate::config::TERMINAL_FG
            };

            if col as u16 == px && row as u16 == py {
                let mut style = Style::default().fg(entity_fg);
                if let Some(bg) = tile_bg {
                    style = style.bg(bg);
                }
                spans.push(Span::styled("@ ", style));
            } else if let Some(npc) = state.npc_at(col as u16, row as u16) {
                // NPCs use tile bg (same logic as player): dark fg on colored tile, otherwise faction-colored fg
                let (fg, bg) = if let Some(bg) = tile_bg {
                    (crate::config::TERMINAL_BG, Some(bg))
                } else {
                    (npc.faction.color(), None)
                };
                let mut style = Style::default().fg(fg);
                if let Some(bg) = bg {
                    style = style.bg(bg);
                }
                spans.push(Span::styled(
                    format!("{} ", npc.faction.npc_glyph()),
                    style,
                ));
            } else if let Some(cap) = state.capital_at(col as u16, row as u16) {
                if cap.is_complete() {
                    spans.push(Span::styled(
                        format!("{} ", cap.center_glyph()),
                        Style::default().fg(crate::config::TERMINAL_BG).bg(cap.faction.color()),
                    ));
                } else {
                    // Foundation: empty center with faction background
                    spans.push(Span::styled(
                        "  ".to_string(),
                        Style::default().bg(cap.faction.color()),
                    ));
                }
            } else if let Some(cap) = state.capital_border_at(col as u16, row as u16) {
                let ch = match cap.kind {
                    crate::types::CapitalKind::City => state.wall_glyph_at(col as u16, row as u16),
                    crate::types::CapitalKind::Camp => '✗',
                };
                spans.push(Span::styled(
                    format!("{} ", ch),
                    Style::default().fg(crate::config::TERMINAL_BG).bg(cap.faction.color()),
                ));
            } else if let Some(wall_faction) = tile.wall {
                let ch = state.wall_glyph_at(col as u16, row as u16);
                // Follow the same pattern as player/NPCs: use the tile's owner color
                // as background if claimed, otherwise no background with the wall's
                // faction as foreground. Walls don't claim territory, so a wall on
                // unclaimed wasteland shouldn't look like claimed territory.
                let (fg, bg) = if let Some(bg) = tile_bg {
                    (crate::config::TERMINAL_BG, Some(bg))
                } else {
                    (wall_faction.color(), None)
                };
                let mut style = Style::default().fg(fg);
                if let Some(bg) = bg {
                    style = style.bg(bg);
                }
                spans.push(Span::styled(format!("{} ", ch), style));
            } else {
                let glyph = tile.terrain.glyph_varied(tile.glyph_variant, col, row, state.anim_tick);
                let fg = match tile.terrain {
                    crate::types::Terrain::Wasteland => Color::DarkGray,
                    crate::types::Terrain::Water => Color::Blue,
                    crate::types::Terrain::Rocky => Color::Red,
                    crate::types::Terrain::Ruins => Color::Yellow,
                };
                let mut style = Style::default().fg(fg);
                if let Some(bg) = tile_bg {
                    style = style.bg(bg);
                }
                spans.push(Span::styled(format!("{} ", glyph), style));
            }
        }
        lines.push(Line::from(spans));
    }

    let map_widget = Paragraph::new(lines);
    let map_area = Rect::new(0, 0, area.width, map_height as u16);
    f.render_widget(map_widget, map_area);

    // HUD Line 1: Player faction + resources + extraction status
    let p = &state.player;
    let carry = p.carrying();
    let cap_max = crate::config::CARRY_CAP;

    let mut player_spans = vec![
        Span::styled(
            " [@] ".to_string(),
            Style::default().fg(crate::config::TERMINAL_BG).bg(p.faction.color()),
        ),
        Span::styled(
            format!(
                " ≈{}  *{}  °{}  ₵{}  [{}/{}]",
                p.water, p.fuel, p.scrap, p.crowns, carry, cap_max
            ),
            Style::default().fg(crate::config::TERMINAL_FG),
        ),
    ];

    // Extraction progress bar
    if let Some(ref ext) = p.extracting {
        let elapsed = std::time::Instant::now().duration_since(ext.started).as_millis() as f32;
        let progress = (elapsed / crate::config::EXTRACT_TIME_MS as f32).clamp(0.0, 1.0);
        let resource_name = match ext.terrain {
            crate::types::Terrain::Water => "≈",
            crate::types::Terrain::Rocky => "*",
            crate::types::Terrain::Ruins => "°",
            _ => "?",
        };
        let bar = render_bar(progress, crate::config::EXTRACT_BAR_WIDTH);
        player_spans.push(Span::styled(
            format!("  {} {}", resource_name, bar),
            Style::default().fg(Color::Green),
        ));
    } else if let Some(ref claim) = p.claiming {
        let elapsed = std::time::Instant::now().duration_since(claim.started).as_millis() as f32;
        let progress = (elapsed / state.claim_time_ms() as f32).clamp(0.0, 1.0);
        let bar = render_bar(progress, crate::config::EXTRACT_BAR_WIDTH);
        player_spans.push(Span::styled(
            format!("  claiming {}", bar),
            Style::default().fg(Color::Cyan),
        ));
    } else if let Some(ref found) = p.founding {
        let elapsed = std::time::Instant::now().duration_since(found.started).as_millis() as f32;
        let progress = (elapsed / crate::config::FOUND_CITY_TIME_MS as f32).clamp(0.0, 1.0);
        let bar = render_bar(progress, crate::config::EXTRACT_BAR_WIDTH);
        player_spans.push(Span::styled(
            format!("  founding city {}", bar),
            Style::default().fg(Color::Magenta),
        ));
    } else if let Some(ref build) = p.building {
        let elapsed = std::time::Instant::now().duration_since(build.started).as_millis() as f32;
        let progress = (elapsed / crate::config::BUILD_SCRAP_TIME_MS as f32).clamp(0.0, 1.0);
        let bar = render_bar(progress, crate::config::EXTRACT_BAR_WIDTH);
        player_spans.push(Span::styled(
            format!("  building {}", bar),
            Style::default().fg(Color::Magenta),
        ));
    } else if let Some(ref camp) = p.founding_camp {
        let elapsed = std::time::Instant::now().duration_since(camp.started).as_millis() as f32;
        let progress = (elapsed / crate::config::FOUND_CAMP_TIME_MS as f32).clamp(0.0, 1.0);
        let bar = render_bar(progress, crate::config::EXTRACT_BAR_WIDTH);
        player_spans.push(Span::styled(
            format!("  founding camp {}", bar),
            Style::default().fg(Color::Magenta),
        ));
    } else if let Some(ref wall) = p.building_wall {
        let elapsed = std::time::Instant::now().duration_since(wall.started).as_millis() as f32;
        let progress = (elapsed / state.build_wall_time_ms() as f32).clamp(0.0, 1.0);
        let bar = render_bar(progress, crate::config::EXTRACT_BAR_WIDTH);
        player_spans.push(Span::styled(
            format!("  building wall {}", bar),
            Style::default().fg(Color::Cyan),
        ));
    } else {
        // Show available actions (key letters come from config)
        let upper = |c: char| c.to_ascii_uppercase();
        if state.adjacent_resource().is_some() && carry < cap_max {
            player_spans.push(Span::styled(
                format!("  [{}] extract", upper(crate::config::KEY_EXTRACT)),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if state.can_claim() {
            player_spans.push(Span::styled(
                format!("  [{}] claim", upper(crate::config::KEY_CLAIM)),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if state.can_found_city() {
            player_spans.push(Span::styled(
                format!(
                    "  [{}] found foundation ({}°)",
                    upper(crate::config::KEY_CITY),
                    crate::config::FOUNDATION_SCRAP_COST
                ),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if state.can_add_to_foundation() {
            if let Some(idx) = state.adjacent_foundation_idx() {
                let cap = &state.capitals[idx];
                player_spans.push(Span::styled(
                    format!(
                        "  [{}] build ({}/{})",
                        upper(crate::config::KEY_CITY),
                        cap.scrap_invested,
                        crate::config::CITY_TOTAL_SCRAP
                    ),
                    Style::default().fg(Color::DarkGray),
                ));
            }
        }
        if state.can_found_camp() {
            player_spans.push(Span::styled(
                format!(
                    "  [{}] found camp ({}°)",
                    upper(crate::config::KEY_CAMP),
                    crate::config::CAMP_SCRAP_COST
                ),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if state.can_build_wall() {
            player_spans.push(Span::styled(
                format!(
                    "  [{}] build wall ({}°)",
                    upper(crate::config::KEY_WALL),
                    crate::config::WALL_SCRAP_COST
                ),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    let hud_player = Line::from(player_spans);
    let mut hud_content = vec![hud_player];

    // Capital info line (when adjacent)
    if let Some(cap_idx) = state.adjacent_capital_idx() {
        let cap = &state.capitals[cap_idx];
        let pop = state.population_of(cap_idx);

        let hud_cap = Line::from(vec![
            Span::styled(
                format!(" [{}] ", cap.faction.glyph()),
                Style::default().fg(crate::config::TERMINAL_BG).bg(cap.faction.color()),
            ),
            Span::styled(
                format!(
                    " ≈{}/{}  *{}/{}  °{}/{}  ₵{}  POP:{}",
                    cap.water, crate::config::MAX_STOCKPILE,
                    cap.fuel, crate::config::MAX_STOCKPILE,
                    cap.scrap, crate::config::MAX_STOCKPILE,
                    cap.crowns,
                    pop,
                ),
                Style::default().fg(cap.faction.color()),
            ),
        ]);
        hud_content.push(hud_cap);

        // Trade instructions line
        let hud_trade = Line::from(vec![
            Span::styled(
                "     sell:1≈ 2* 3°(₵".to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{}", crate::config::BASE_SELL_PRICE),
                Style::default().fg(cap.faction.color()),
            ),
            Span::styled(
                ")  buy:!≈ @* #°(₵".to_string(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{}", crate::config::BASE_BUY_PRICE),
                Style::default().fg(cap.faction.color()),
            ),
            Span::styled(
                ")".to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        hud_content.push(hud_trade);
    }

    // Bottom line: controls
    let hud_controls = Line::from(vec![
        Span::styled(
            {
                let up = |c: char| c.to_ascii_uppercase();
                format!(
                    " {}{}{}{}: move  {}: extract  {}: claim  {}: city  {}: camp  {}: wall  {}: quit",
                    up(crate::config::KEY_MOVE_UP),
                    up(crate::config::KEY_MOVE_LEFT),
                    up(crate::config::KEY_MOVE_DOWN),
                    up(crate::config::KEY_MOVE_RIGHT),
                    up(crate::config::KEY_EXTRACT),
                    up(crate::config::KEY_CLAIM),
                    up(crate::config::KEY_CITY),
                    up(crate::config::KEY_CAMP),
                    up(crate::config::KEY_WALL),
                    up(crate::config::KEY_QUIT),
                )
            },
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    hud_content.push(hud_controls);

    let hud_y = map_height as u16;
    let hud = Paragraph::new(hud_content);
    f.render_widget(hud, Rect::new(0, hud_y, area.width, hud_lines as u16));
}
