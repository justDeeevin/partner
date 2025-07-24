use crossterm::event::{Event, KeyCode};
use ratatui_elm::{Task, Update};

use crate::{Action, Message, Mode, State};

pub fn update(state: &mut State, update: Update<Message>) -> (Task<Message>, bool) {
    if let Update::Message(message) = update {
        match message {
            Message::Devices(devices) => {
                state.devices = devices;
                state.table.select(Some(0));
            }
            Message::Partitions(partitions) => {
                state.partitions = partitions;
                state.table.select(Some(0));
            }
            Message::Error(_e) => {
                todo!()
            }
        }
        return (Task::None, true);
    }
    match state.mode {
        Mode::Disks => update_disks(update, state),
        Mode::Partitions(i) => update_partitions(update, state, i),
    }
}

fn update_disks(update: Update<Message>, state: &mut State) -> (Task<Message>, bool) {
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
                let selected = state.table.selected().unwrap();
                state.tx_actions.send(Action::SetDisk(selected)).unwrap();
                state.mode = Mode::Partitions(selected);
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

fn update_partitions(
    update: Update<Message>,
    state: &mut State,
    index: usize,
) -> (Task<Message>, bool) {
    let redraw = if let Update::Terminal(Event::Key(key)) = update {
        match key.code {
            KeyCode::Esc => {
                state.table.select(Some(index));
                state.mode = Mode::Disks;
                state.partitions.clear();
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
            _ => false,
        }
    } else {
        false
    };
    (Task::None, redraw)
}
