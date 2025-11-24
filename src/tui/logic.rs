use crate::get_preceding;

use super::{NewPartition, OneOf, State};
use byte_unit::Byte;
use partner::FileSystem;
use ratatui::{
    crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers},
    widgets::TableState,
};
use ratatui_elm::{Task, Update};
use tui_input::{Input, backend::crossterm::EventHandler};

type Message = ();

pub fn update(state: &mut State, update: Update<Message>) -> (Task<Message>, bool) {
    if let Update::Terminal(Event::Key(KeyEvent {
        code, modifiers, ..
    })) = &update
    {
        match code {
            KeyCode::Up => {
                if let Some((_, table)) = &mut state.selected_partition {
                    table.scroll_up_by(1);
                } else {
                    state.table.scroll_up_by(1);
                }
                return (Task::None, true);
            }
            KeyCode::Down => {
                if let Some((_, table)) = &mut state.selected_partition {
                    table.scroll_down_by(1);
                } else {
                    state.table.scroll_down_by(1);
                }
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

    if let Some(partition) = state.selected_partition.take() {
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
    (mut partition, table): (OneOf<usize, NewPartition>, TableState),
) -> (Task<Message>, bool) {
    let Update::Terminal(event) = update else {
        return (Task::None, false);
    };
    let Event::Key(KeyEvent { code, .. }) = event else {
        return (Task::None, false);
    };

    let out = match code {
        KeyCode::Esc => {
            if state.input.is_some() {
                state.input = None;
                state.selected_partition = Some((partition, table));
                return (Task::None, true);
            }

            if let OneOf::Left(partition) = partition {
                state.table.select(Some(partition));
            }

            state.selected_partition = None;
            return (Task::None, true);
        }
        KeyCode::Enter => {
            if let Some(input) = &state.input {
                match table.selected_cell() {
                    Some((0, 0)) => match &mut partition {
                        OneOf::Left(partition) => {
                            state.devices[state.selected_device.unwrap()]
                                .change_partition_name(*partition, input.value().into());
                        }
                        OneOf::Right(partition) => {
                            partition.name = input.value().into();
                        }
                    },
                    _ => {}
                }
                state.input = None;
            } else {
                match table.selected_cell() {
                    Some((0, 0)) => {
                        let starting_name = match &partition {
                            OneOf::Left(partition) => state.devices[state.selected_device.unwrap()]
                                .partitions()
                                .nth(*partition)
                                .unwrap()
                                .name()
                                .to_string(),
                            OneOf::Right(partition) => partition.name.clone(),
                        };
                        state.input = Some(Input::new(starting_name));
                    }
                    Some((1, 0)) => {
                        let dev = &state.devices[state.selected_device.unwrap()];
                        let starting_preceding = match &partition {
                            OneOf::Left(partition) => get_preceding(
                                dev,
                                dev.partitions().nth(*partition).unwrap().bounds(),
                            ),
                            OneOf::Right(partition) => get_preceding(dev, &partition.bounds),
                        };
                        state.input = Some(Input::new(format!("{starting_preceding:#.10}")));
                    }
                    Some((2, 0)) => {
                        let dev = &state.devices[state.selected_device.unwrap()];
                        let starting_size = match &partition {
                            OneOf::Left(partition) => {
                                dev.partitions().nth(*partition).unwrap().size()
                            }
                            OneOf::Right(partition) => Byte::from_u64(
                                (partition.bounds.end() - partition.bounds.start()) as u64
                                    * dev.sector_size(),
                            ),
                        };
                        state.input = Some(Input::new(format!("{starting_size:#.10}")));
                    }
                    Some((3, 0)) => {
                        if let OneOf::Right(partition) = partition {
                            state.devices[state.selected_device.unwrap()]
                                .new_partition(
                                    partition.name.into(),
                                    Some(partition.fs),
                                    partition.bounds,
                                )
                                .unwrap();
                            return (Task::None, true);
                        }
                    }
                    _ => {}
                }
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
    };
    state.selected_partition = Some((partition, table));
    out
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
        KeyCode::Enter if selected_partition.used() && !selected_partition.mounted() => {
            state.selected_partition = state.table.selected().map(|s| {
                (
                    OneOf::Left(s),
                    TableState::new().with_selected_cell(Some((0, 0))),
                )
            });
            (Task::None, true)
        }
        KeyCode::Enter if !selected_partition.used() => {
            state.selected_partition = Some((
                OneOf::Right(NewPartition {
                    name: "".into(),
                    fs: FileSystem::Ext4,
                    bounds: selected_partition.bounds().clone(),
                }),
                TableState::new().with_selected_cell(Some((0, 0))),
            ));
            (Task::None, true)
        }
        KeyCode::Delete if selected_partition.used() && !selected_partition.mounted() => {
            state.devices[device].remove_partition(selected_partition_index);
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
