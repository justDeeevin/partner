use crossterm::event::{Event, KeyCode};
use ratatui_elm::{Task, Update};

use crate::{Mode, State};

pub fn update(state: &mut State, update: Update<()>) -> (Task<()>, bool) {
    match state.mode {
        Mode::Disks => update_disks(update, state),
        Mode::Partitions(i) => update_partitions(update, state, i),
    }
}

fn update_disks(update: Update<()>, state: &mut State) -> (Task<()>, bool) {
    let redraw = if let Update::Terminal(Event::Key(key)) = update {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return (Task::Quit, false),
            KeyCode::Up => {
                state.table.scroll_up_by(1);
                true
            }
            KeyCode::Down => {
                state.table.scroll_down_by(1);
                true
            }
            KeyCode::Enter => {
                state.mode = Mode::Partitions(state.table.selected().unwrap());
                state.table.select(Some(0));
                true
            }
            _ => false,
        }
    } else {
        false
    };

    (Task::None, redraw)
}

fn update_partitions(update: Update<()>, state: &mut State, index: usize) -> (Task<()>, bool) {
    let redraw = if let Update::Terminal(Event::Key(key)) = update {
        match key.code {
            KeyCode::Esc => {
                state.table.select(Some(index));
                state.mode = Mode::Disks;
                true
            }
            KeyCode::Char('q') => return (Task::Quit, false),
            KeyCode::Up => {
                state.table.scroll_up_by(1);
                true
            }
            KeyCode::Down => {
                state.table.scroll_down_by(1);
                true
            }
            KeyCode::Enter => {
                state.table.select(Some(0));
                state.mode = Mode::Disks;
                true
            }
            _ => false,
        }
    } else {
        false
    };
    (Task::None, redraw)
}
