use super::{NewPartition, OneOf, State, get_preceding};
use byte_unit::Byte;
use itertools::intersperse_with;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Row, Table, TableState},
};

pub fn view(state: &mut State, frame: &mut Frame) {
    if let Some(device) = state.selected_device {
        view_device(state, frame, device);
    } else {
        view_devices(state, frame);
    }
}

fn view_devices(state: &mut State, frame: &mut Frame) {
    const COLUMNS: usize = 3;

    let [top, bottom] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());

    let table = Table::new(
        state.devices.iter().map(|d| {
            Row::new::<[String; COLUMNS]>([
                d.path().display().to_string(),
                d.model().to_string(),
                format!("{:#.10}", d.size()),
            ])
        }),
        [Constraint::Ratio(1, COLUMNS as u32); COLUMNS],
    )
    .header(
        Row::new::<[&'static str; COLUMNS]>(["Path", "Model", "Size"]).style(Style::new().bold()),
    )
    .row_highlight_style(Style::new().reversed())
    .block(
        Block::bordered()
            .title("Devices")
            .title_style(Style::new().bold()),
    );

    frame.render_stateful_widget(table, top, &mut state.table);
    frame.render_widget(
        legend(["Esc/q: Quit", "Up/Down: Change selection", "Enter: Select"]),
        bottom,
    );
}

fn view_device(state: &mut State, frame: &mut Frame, device: usize) {
    const COLUMNS: usize = 5;

    let dev = &state.devices[device];

    let mut constraints = if state.selected_partition.is_some() {
        vec![Constraint::Ratio(1, 2); 2]
    } else {
        vec![Constraint::Min(0)]
    };
    constraints.push(Constraint::Length(1));
    let layout = Layout::vertical(constraints).split(frame.area());

    let n_changes_contents = format!(
        "{} pending change{}",
        dev.n_changes(),
        if dev.n_changes() > 1 { "s" } else { "" }
    );

    let top = layout[0];
    let [legend_area, n_changes] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(n_changes_contents.chars().count() as u16),
    ])
    .areas(*layout.last().unwrap());

    let block = Block::bordered().title(format!("Partitions of {}", dev.path().display()));

    let block = if state.selected_partition.is_none() {
        block.title_style(Style::new().bold())
    } else {
        block
    };

    let table = Table::new(
        dev.partitions().map(|p| {
            let path_line = {
                let path_span = Span::raw(
                    p.path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| if p.fs().is_some() { "N/A" } else { "unused" }.into()),
                );
                if p.mounted() {
                    Line::from_iter([path_span, Span::styled(" (mounted)", Style::new().bold())])
                } else {
                    Line::from(path_span)
                }
            };
            Row::new::<[Line; COLUMNS]>([
                path_line,
                Line::raw(p.fs().map(|f| f.to_string()).unwrap_or_default()),
                Line::raw(format!("{:#.10}", p.size())),
                Line::raw(p.name()),
                Line::raw(
                    p.mount_point
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                ),
            ])
        }),
        [Constraint::Ratio(1, COLUMNS as u32); COLUMNS],
    )
    .header(
        Row::new::<[&'static str; COLUMNS]>(["Path", "File System", "Size", "Name", "Mount"])
            .style(Style::new().bold()),
    )
    .row_highlight_style(Style::new().reversed())
    .block(block);

    // the table has to be rendered first so out-of-bounds selections get corrected
    frame.render_stateful_widget(table, top, &mut state.table);

    let mut actions = if state.input.is_none() {
        vec!["q: Quit"]
    } else {
        Vec::new()
    };
    if matches!(state.selected_partition, Some((OneOf::Right(_), _))) {
        actions.push("Esc: Abort");
    } else {
        actions.push("Esc: Back");
    }
    let partition = dev
        .partitions()
        .nth(state.table.selected().unwrap())
        .unwrap();
    if state.selected_partition.is_none() {
        actions.push("Up/Down: Change selection");
    }
    if state.input.is_none() && dev.n_changes() > 0 {
        actions.push("Ctrl+z: Undo");
    }
    if state.selected_partition.is_none() && !partition.used() {
        actions.push("Enter: Create");
    }
    if state.selected_partition.is_none() && !partition.mounted() && partition.used() {
        actions.push("Enter: Edit");
    }
    if state.selected_partition.is_some() && state.input.is_none() {
        actions.push("Enter: Select");
    }
    if state.selected_partition.is_none() && !partition.mounted() && partition.used() {
        actions.push("Delete: Remove");
    }
    if state.input.is_some() {
        actions.extend(["Esc: Abort", "Enter: Apply"]);
    }

    frame.render_widget(legend(actions), legend_area);
    if dev.n_changes() > 0 {
        frame.render_widget(
            Text::raw(n_changes_contents).alignment(ratatui::layout::Alignment::Right),
            n_changes,
        );
    }

    if let Some(partition) = state.selected_partition.take() {
        view_partition(state, frame, layout[1], device, partition);
    }
}

fn legend<'a>(spans: impl IntoIterator<Item = impl Into<Span<'a>>>) -> Text<'a> {
    Line::from_iter(intersperse_with(spans.into_iter().map(Into::into), || {
        Span::raw(" | ")
    }))
    .into()
}

fn view_partition(
    state: &mut State,
    frame: &mut Frame,
    area: Rect,
    device: usize,
    (partition, mut table_state): (OneOf<usize, NewPartition>, TableState),
) {
    let dev = &state.devices[device];
    let title = if let OneOf::Left(partition) = &partition {
        format!(
            "Partition {}",
            dev.partitions()
                .nth(*partition)
                .as_ref()
                .unwrap()
                .path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| { partition.to_string() })
        )
    } else {
        "New Partition".to_string()
    };
    let block = Block::bordered()
        .title(title)
        .title_style(Style::new().bold());
    let name = state
        .input
        .as_ref()
        .map(|i| i.value())
        .unwrap_or(match &partition {
            OneOf::Left(partition) => dev.partitions().nth(*partition).unwrap().name(),
            OneOf::Right(partition) => &partition.name,
        });
    let bounds = match &partition {
        OneOf::Left(partition) => dev.partitions().nth(*partition).unwrap().bounds(),
        OneOf::Right(partition) => &partition.bounds,
    };
    let size = match &partition {
        OneOf::Left(partition) => dev.partitions().nth(*partition).unwrap().size(),
        OneOf::Right(partition) => Byte::from_u64(
            (partition.bounds.end() - partition.bounds.start()) as u64 * dev.sector_size(),
        ),
    };

    let preceding = get_preceding(dev, bounds);

    let mut rows = vec![
        Row::from_iter([format!("Name: {name}")]),
        Row::from_iter([format!("Preceding: {preceding:#.10}")]),
        Row::from_iter([format!("Size: {size:#.10}")]),
    ];
    if matches!(partition, OneOf::Right(_)) {
        rows.push(Row::from_iter(["Submit"]));
    }
    let mut table = Table::new(rows, [Constraint::Min(0)]).block(block);
    if state.input.is_none() {
        table = table.cell_highlight_style(Style::new().reversed());
    }

    frame.render_stateful_widget(table, area, &mut table_state);

    if let Some(input) = &state.input {
        let x_offset = match table_state.selected_cell().unwrap() {
            (0, 0) => "Name: ".len(),
            (1, 0) => "Preceding: ".len(),
            (3, 0) => 0,
            _ => unreachable!(),
        } as u16
            + 1;
        let x = input.visual_cursor();
        frame.set_cursor_position((area.x + x as u16 + x_offset, area.y + 1));
    }

    state.selected_partition = Some((partition, table_state));
}
