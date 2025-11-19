use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Style, Stylize},
    text::Text,
    widgets::{Block, Row, Table},
};

use super::State;

pub fn view(state: &mut State, frame: &mut Frame) {
    if let Some(device) = state.selected_device {
        view_device(state, frame, device);
    } else {
        view_devices(state, frame);
    }
}

const DEVICES_COLUMNS: usize = 3;

fn view_devices(state: &mut State, frame: &mut Frame) {
    let [top, bottom] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
    let table = Table::new(
        state.devices.iter().map(|d| {
            Row::new::<[String; DEVICES_COLUMNS]>([
                d.path().display().to_string(),
                d.model().to_string(),
                format!("{:#.10}", d.size()),
            ])
        }),
        [Constraint::Ratio(1, DEVICES_COLUMNS as u32); DEVICES_COLUMNS],
    )
    .header(Row::new::<[&'static str; 3]>(["Path", "Model", "Size"]).style(Style::new().bold()))
    .row_highlight_style(Style::new().reversed())
    .block(Block::bordered().title("Devices"));

    frame.render_stateful_widget(table, top, &mut state.table);
    frame.render_widget(Text::raw("Esc/q: Quit | Up/Down: Change selection"), bottom);
}

fn view_device(state: &mut State, frame: &mut Frame, device: usize) {
    todo!()
}
