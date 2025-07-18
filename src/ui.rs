use crate::{Mode, State, ext::*};
use byte_unit::Byte;
use libparted::Disk;
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
    let header = Row::new(["Model", "Path", "Length"]).style(Style::default().bold());
    let rows = state.devices.iter().map(|d| {
        Row::new([
            d.model().into(),
            d.path().display().to_string(),
            format!("{:#}", d.size()),
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
    let device = &mut state.devices[index];
    let path = device.path().to_owned();
    let sector_size = device.sector_size();
    let disk = Disk::new(device).expect("Failed to open disk");

    let header = Row::new(["Label", "Path", "Type", "Length"]).style(Style::default().bold());

    let rows = disk.parts().map(|p| {
        let mut label = p.name().unwrap_or_default();
        if label.is_empty() {
            label = "N/A".into();
        }
        let size = Byte::from_u64(p.geom_length() as u64 * sector_size);
        Row::new([
            label,
            p.get_path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "N/A".into()),
            p.fs_type_name().unwrap_or("N/A").into(),
            format!("{size:#}"),
        ])
    });

    let block = Block::default()
        .title(format!("Partitions for {}", path.display()))
        .borders(Borders::ALL);

    let table = Table::new(rows, [Constraint::Ratio(1, 4); 4])
        .block(block)
        .header(header)
        .row_highlight_style(Style::default().reversed());

    frame.render_stateful_widget(table, top, &mut state.table);
    frame.render_widget(Text::raw("Esc: Back | Up/Down: Change Selection"), bottom);
}
