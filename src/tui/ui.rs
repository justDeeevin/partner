use super::State;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Row, Table},
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
    .block(Block::bordered().title("Devices"));

    frame.render_stateful_widget(table, top, &mut state.table);
    frame.render_widget(
        Text::raw("Esc/q: Quit | Up/Down: Change selection | Enter: Select"),
        bottom,
    );
}

fn view_device(state: &mut State, frame: &mut Frame, device: usize) {
    const COLUMNS: usize = 5;

    let dev = &state.devices[device];

    let [top, bottom] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());

    let table = Table::new(
        dev.partitions().map(|p| {
            let path_line = {
                let path_span = Span::raw(
                    p.path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "unused".to_string()),
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
    .block(Block::bordered().title(format!("Partitions of {}", dev.path().display())));

    frame.render_stateful_widget(table, top, &mut state.table);
    frame.render_widget(
        Text::raw("q: Quit | Esc: Back | Up/Down: Change selection"),
        bottom,
    );
}
