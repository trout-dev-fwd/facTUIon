mod config;
mod map;
mod render;
mod types;

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use types::GameState;

fn main() -> io::Result<()> {
    // Panic handler: restore terminal on crash
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Fixed map size — both players must use the same dimensions
    let mut state = GameState::new(config::MAP_WIDTH, config::MAP_HEIGHT, config::MAP_SEED);

    // Game loop
    loop {
        state.update_anim();
        state.check_extraction();
        state.check_claim();
        state.check_found_city();
        state.check_found_camp();
        state.check_build();
        state.check_build_wall();
        state.update_decay();
        state.update_dehydration();
        state.update_npcs();
        terminal.draw(|f| render::render(f, &state))?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let shifted = key.modifiers.contains(KeyModifiers::SHIFT);
                // Arrow keys (non-configurable aliases for movement)
                match key.code {
                    KeyCode::Up    => { state.move_player(0, -1); continue; }
                    KeyCode::Down  => { state.move_player(0, 1);  continue; }
                    KeyCode::Left  => { state.move_player(-1, 0); continue; }
                    KeyCode::Right => { state.move_player(1, 0);  continue; }
                    KeyCode::Esc   => break,
                    _ => {}
                }
                if let KeyCode::Char(ch) = key.code {
                    use config::*;
                    match ch {
                        c if c == KEY_QUIT       => break,
                        c if c == KEY_MOVE_UP    => state.move_player(0, -1),
                        c if c == KEY_MOVE_DOWN  => state.move_player(0, 1),
                        c if c == KEY_MOVE_LEFT  => state.move_player(-1, 0),
                        c if c == KEY_MOVE_RIGHT => state.move_player(1, 0),
                        c if c == KEY_EXTRACT    => state.start_extract(),
                        c if c == KEY_CLAIM      => state.start_claim(),
                        c if c == KEY_CITY       => {
                            if state.adjacent_foundation_idx().is_some() {
                                state.start_add_to_foundation();
                            } else {
                                state.start_found_city();
                            }
                        }
                        c if c == KEY_CAMP       => state.start_found_camp(),
                        c if c == KEY_WALL       => state.start_build_wall(),
                        c if c == KEY_SELL_WATER && !shifted => state.sell_resource(1),
                        c if c == KEY_SELL_FUEL  && !shifted => state.sell_resource(2),
                        c if c == KEY_SELL_SCRAP && !shifted => state.sell_resource(3),
                        c if c == KEY_BUY_WATER  => state.buy_resource(1),
                        c if c == KEY_BUY_FUEL   => state.buy_resource(2),
                        c if c == KEY_BUY_SCRAP  => state.buy_resource(3),
                        _ => {}
                    }
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
