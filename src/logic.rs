use super::State;
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui_elm::{Task, Update};

type Message = ();

pub fn update(state: &mut State, update: Update<Message>) -> (Task<Message>, bool) {
    if let Some(device) = state.selected_device {
        update_device(state, update, device)
    } else {
        update_devices(state, update)
    }
}

fn update_device(
    state: &mut State,
    update: Update<Message>,
    device: usize,
) -> (Task<Message>, bool) {
    todo!()
}

fn update_devices(state: &mut State, update: Update<Message>) -> (Task<Message>, bool) {
    let Update::Terminal(Event::Key(KeyEvent { code, .. })) = update else {
        return (Task::None, false);
    };

    match code {
        KeyCode::Char('q') | KeyCode::Esc => (Task::Quit, false),
        KeyCode::Down => {
            state.table.scroll_down_by(1);
            (Task::None, true)
        }
        KeyCode::Up => {
            state.table.scroll_up_by(1);
            (Task::None, true)
        }
        _ => (Task::None, false),
    }
}
