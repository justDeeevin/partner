use crossterm::event::{Event, KeyCode, KeyModifiers};
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
        Mode::Partitions { index, .. } => update_partitions(update, state, index),
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
                state.action(Action::SetDisk(selected));
                state.mode = Mode::partitions(selected);
                false
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
        if let Mode::Partitions {
            temp_name: Some(temp_name),
            ..
        } = &mut state.mode
            && let KeyCode::Char(c) = key.code
        {
            temp_name.push(c);
            return (Task::None, true);
        }
        match key.code {
            KeyCode::Esc => {
                if let Mode::Partitions { temp_name, .. } = &mut state.mode
                    && temp_name.is_some()
                {
                    *temp_name = None;
                } else {
                    state.table.select(Some(index));
                    state.mode = Mode::Disks;
                    state.partitions.clear();
                }
                true
            }
            KeyCode::Char('q') => return (Task::Quit, false),
            KeyCode::Up if !state.mode.is_editing_name() => {
                state.table.scroll_up_by(1);
                true
            }
            KeyCode::Down if !state.mode.is_editing_name() => {
                state.table.scroll_down_by(1);
                true
            }
            KeyCode::Backspace => {
                if let Mode::Partitions {
                    temp_name: Some(temp_name),
                    ..
                } = &mut state.mode
                {
                    temp_name.pop();
                    true
                } else {
                    false
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                if let Mode::Partitions { temp_name, .. } = &mut state.mode
                    && temp_name.is_none()
                    && state.partitions[state.table.selected().unwrap()]
                        .path
                        .is_some()
                {
                    *temp_name = if let KeyCode::Char('n') = key.code {
                        state.partitions[state.table.selected().unwrap()]
                            .name
                            .as_ref()
                            .map(|s| s.to_string())
                    } else {
                        Some(String::new())
                    };
                    true
                } else {
                    false
                }
            }
            KeyCode::Enter => {
                if let Mode::Partitions { temp_name, .. } = &mut state.mode
                    && temp_name.is_some()
                {
                    let selected = state.table.selected().unwrap();
                    let new_name = temp_name.clone().unwrap();
                    *temp_name = None;
                    state.action(Action::ChangeName {
                        partition: selected,
                        new_name: new_name.clone().into(),
                        previous_name: state.partitions[selected].name.clone(),
                    });
                    state.partitions[selected].name = Some(new_name.into());
                    true
                } else {
                    false
                }
            }
            KeyCode::Char('z')
                if key.modifiers.contains(KeyModifiers::CONTROL) && state.n_changes > 0 =>
            {
                state.action(Action::Undo);
                true
            }
            KeyCode::Char('s')
                if key.modifiers.contains(KeyModifiers::CONTROL) && state.n_changes > 0 =>
            {
                state.action(Action::Commit);
                true
            }
            _ => false,
        }
    } else {
        false
    };
    (Task::None, redraw)
}
