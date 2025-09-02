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
            temp_label: Some(temp_label),
            ..
        } = &mut state.mode
            && let KeyCode::Char(c) = key.code
        {
            temp_label.push(c);
            return (Task::None, true);
        }
        match key.code {
            KeyCode::Esc => {
                if let Mode::Partitions { temp_label, .. } = &mut state.mode
                    && temp_label.is_some()
                {
                    *temp_label = None;
                } else {
                    state.table.select(Some(index));
                    state.mode = Mode::Disks;
                    state.partitions.clear();
                }
                true
            }
            KeyCode::Char('q') => return (Task::Quit, false),
            KeyCode::Up if !state.mode.is_editing_label() => {
                state.table.scroll_up_by(1);
                true
            }
            KeyCode::Down if !state.mode.is_editing_label() => {
                state.table.scroll_down_by(1);
                true
            }
            KeyCode::Backspace => {
                if let Mode::Partitions {
                    temp_label: Some(temp_label),
                    ..
                } = &mut state.mode
                {
                    temp_label.pop();
                    true
                } else {
                    false
                }
            }
            KeyCode::Char('l') | KeyCode::Char('L') => {
                if let Mode::Partitions { temp_label, .. } = &mut state.mode
                    && temp_label.is_none()
                    && state.partitions[state.table.selected().unwrap()]
                        .path
                        .is_some()
                {
                    *temp_label = if let KeyCode::Char('l') = key.code {
                        state.partitions[state.table.selected().unwrap()]
                            .label
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
                if let Mode::Partitions { temp_label, .. } = &mut state.mode
                    && temp_label.is_some()
                {
                    let selected = state.table.selected().unwrap();
                    let new_label = temp_label.clone().unwrap();
                    *temp_label = None;
                    state.action(Action::ChangeLabel {
                        partition: selected,
                        new_label: new_label.clone().into(),
                        previous_label: state.partitions[selected].label.clone(),
                    });
                    state.partitions[selected].label = Some(new_label.into());
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
