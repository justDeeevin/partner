use std::path::Path;

use super::State;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui_elm::{Task, Update};
use tracing::debug;

type Message = ();

pub fn update(state: &mut State, update: Update<Message>) -> (Task<Message>, bool) {
    if let Update::Terminal(Event::Key(KeyEvent { code, .. })) = &update {
        match code {
            KeyCode::Up => {
                state.table.scroll_up_by(1);
                return (Task::None, true);
            }
            KeyCode::Down => {
                state.table.scroll_down_by(1);
                return (Task::None, true);
            }
            KeyCode::Char('q') => return (Task::Quit, false),
            _ => {}
        }
    }

    if let Some(device) = state.selected_device.clone() {
        update_device(state, update, device)
    } else {
        update_devices(state, update)
    }
}

fn update_device(
    state: &mut State,
    update: Update<Message>,
    device: impl AsRef<Path>,
) -> (Task<Message>, bool) {
    if let Update::Terminal(Event::Key(KeyEvent {
        code: KeyCode::Esc, ..
    })) = update
    {
        state.table.select(
            state
                .devices
                .values()
                .position(|d| d.path() == device.as_ref()),
        );
        state.selected_device = None;
        (Task::None, true)
    } else {
        (Task::None, false)
    }
}

fn update_devices(state: &mut State, update: Update<Message>) -> (Task<Message>, bool) {
    let Update::Terminal(Event::Key(KeyEvent { code, .. })) = update else {
        return (Task::None, false);
    };

    match code {
        KeyCode::Esc => (Task::Quit, false),
        KeyCode::Enter => {
            state.selected_device = state
                .table
                .selected()
                .and_then(|i| state.devices.keys().nth(i))
                .cloned();
            state.table.select(Some(0));
            debug!(partitions = ?state.devices[state.selected_device.as_ref().unwrap()].partitions().collect::<Vec<_>>(), "selected device");
            (Task::None, true)
        }
        _ => (Task::None, false),
    }
}
