use crate::{Mode, State};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    text::Text,
    widgets::{Block, Borders, Row, Table},
};

pub fn view(state: &mut State, frame: &mut ratatui::Frame) {
    match state.mode {
        Mode::Disks => view_disks(frame, state),
        Mode::Partitions(i) => view_partitions(frame, state, i),
    }
}

fn view_disks(frame: &mut ratatui::Frame, state: &mut State) {
    let [top, bottom] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

    let header = Row::new(["Model", "Path", "Size"]).style(Style::default().bold());

    let rows = state.devices.iter().map(|d| {
        Row::new([
            d.model.to_string(),
            d.path.display().to_string(),
            format!("{:#}", d.length),
        ])
    });

    let block = Block::default().title("Disks").borders(Borders::ALL);

    let table = Table::new(rows, [Constraint::Ratio(1, 3); 3])
        .block(block)
        .header(header)
        .row_highlight_style(Style::default().reversed());

    frame.render_stateful_widget(table, top, &mut state.table);
    frame.render_widget(
        Text::raw("q/Esc: Quit | Up/Down: Change Selection | Enter: Select drive"),
        bottom,
    );
}

fn view_partitions(frame: &mut ratatui::Frame, state: &mut State, index: usize) {
    let [top, bottom] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

    let header =
        Row::new(["Path", "Filesystem", "Size", "Mount point"]).style(Style::default().bold());

    let rows = state.partitions.iter().map(|p| {
        let mount = p.path.as_ref().and_then(|p| state.mounts.get(p.as_ref()));
        Row::new([
            p.path
                .as_ref()
                .map(|p| {
                    let out = p.display().to_string();
                    if mount.is_some() {
                        out + " (Mounted)"
                    } else {
                        out
                    }
                })
                .unwrap_or_else(|| "Unallocated".into()),
            p.fs_type
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| if p.path.is_some() { "Unknown" } else { "" }.into()),
            format!("{:#}", p.length),
            mount
                .map(|m| m.dest.display().to_string())
                .unwrap_or_default(),
        ])
    });

    let block = Block::default()
        .title(format!(
            "Partitions for {}",
            state
                .devices
                .get(index)
                .map(|d| d.path.display().to_string())
                .unwrap_or_else(|| "LOADING".into())
        ))
        .borders(Borders::ALL);

    let table = Table::new(rows, [Constraint::Ratio(1, 4); 4])
        .block(block)
        .header(header)
        .row_highlight_style(Style::default().reversed());

    frame.render_stateful_widget(table, top, &mut state.table);
    frame.render_widget(Text::raw("Esc: Back | Up/Down: Change Selection"), bottom);
}
