use super::State;
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui_elm::{Task, Update};
use tui_input::{Input, backend::crossterm::EventHandler};

type Message = ();

pub fn update(state: &mut State, update: Update<Message>) -> (Task<Message>, bool) {
    if let Update::Terminal(Event::Key(KeyEvent {
        code, modifiers, ..
    })) = &update
    {
        match code {
            KeyCode::Up if state.selected_partition.is_none() => {
                state.table.scroll_up_by(1);
                return (Task::None, true);
            }
            KeyCode::Down if state.selected_partition.is_none() => {
                state.table.scroll_down_by(1);
                return (Task::None, true);
            }
            KeyCode::Char('q') => return (Task::Quit, false),
            KeyCode::Char('z') if modifiers.contains(KeyModifiers::CONTROL) => {
                if state.input.is_none()
                    && let Some(device) = state.selected_device
                {
                    state.devices[device].undo_change();
                }
                return (Task::None, true);
            }
            _ => {}
        }
    }

    if let Some(partition) = state.selected_partition {
        update_partition(state, update, partition)
    } else if let Some(device) = state.selected_device {
        update_device(state, update, device)
    } else {
        update_devices(state, update)
    }
}

fn update_partition(
    state: &mut State,
    update: Update<Message>,
    partition: usize,
) -> (Task<Message>, bool) {
    let Update::Terminal(event) = update else {
        return (Task::None, false);
    };
    let Event::Key(KeyEvent { code, .. }) = event else {
        return (Task::None, false);
    };

    match code {
        KeyCode::Esc => {
            if state.input.is_some() {
                state.input = None;
                return (Task::None, true);
            }
            state.table.select(Some(partition));

            state.selected_partition = None;
            (Task::None, true)
        }
        KeyCode::Enter => {
            if let Some(input) = &state.input {
                state.devices[state.selected_device.unwrap()]
                    .change_partition_name(partition, input.value().into());
                state.input = None;
            } else {
                state.input = Some(Input::new(
                    state.devices[state.selected_device.unwrap()]
                        .partitions()
                        .nth(partition)
                        .unwrap()
                        .name()
                        .to_string(),
                ));
            }
            (Task::None, true)
        }
        _ => {
            if let Some(input) = &mut state.input {
                (Task::None, input.handle_event(&event).is_some())
            } else {
                (Task::None, false)
            }
        }
    }
}

fn update_device(
    state: &mut State,
    update: Update<Message>,
    device: usize,
) -> (Task<Message>, bool) {
    let Update::Terminal(Event::Key(KeyEvent { code, .. })) = update else {
        return (Task::None, false);
    };

    let selected_partition_index = state.table.selected().unwrap();
    let selected_partition = state.devices[device]
        .partitions()
        .nth(selected_partition_index)
        .unwrap();

    match code {
        KeyCode::Esc => {
            state.table.select(Some(device));

            state.selected_device = None;
            (Task::None, true)
        }
        KeyCode::Enter if !selected_partition.mounted() && selected_partition.used => {
            state.selected_partition = state.table.selected();
            (Task::None, true)
        }
        _ => (Task::None, false),
    }
}

fn update_devices(state: &mut State, update: Update<Message>) -> (Task<Message>, bool) {
    let Update::Terminal(Event::Key(KeyEvent { code, .. })) = update else {
        return (Task::None, false);
    };

    match code {
        KeyCode::Esc => (Task::Quit, false),
        KeyCode::Enter => {
            state.selected_device = state.table.selected();
            state.table.select(Some(0));
            (Task::None, true)
        }
        _ => (Task::None, false),
    }
}
