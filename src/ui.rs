use crate::{Mode, State};
use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Style, Stylize},
    text::Text,
    widgets::{Block, Borders, Row, Table},
};

pub fn view(state: &mut State, frame: &mut ratatui::Frame) {
    match &state.mode {
        Mode::Disks => view_disks(frame, state),
        Mode::Partitions { index, temp_name } => {
            view_partitions(frame, state, *index, temp_name.clone())
        }
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

fn view_partitions(
    frame: &mut ratatui::Frame,
    state: &mut State,
    index: usize,
    temp_name: Option<String>,
) {
    const N_COLS: usize = 5;

    let [top, bottom] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(frame.area());

    let header = Row::new::<[&str; N_COLS]>([
        "Path",
        "Filesystem",
        "Size",
        if state.mode.is_editing_name() {
            "Name (editing)"
        } else {
            "Name"
        },
        "Mount point",
    ])
    .style(Style::default().bold());

    let rows = state.partitions.iter().enumerate().map(|(i, p)| {
        let mount = p.path.as_ref().and_then(|p| state.mounts.get(p.as_ref()));
        let name = if i == state.table.selected().unwrap() {
            temp_name
                .clone()
                .unwrap_or_else(|| p.name.as_ref().map(|p| p.to_string()).unwrap_or_default())
        } else {
            p.name.as_ref().map(|p| p.to_string()).unwrap_or_default()
        };
        Row::new::<[String; N_COLS]>([
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
            name,
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

    let table = Table::new(rows, [Constraint::Ratio(1, N_COLS as u32); N_COLS])
        .block(block)
        .header(header)
        .row_highlight_style(Style::default().reversed());

    frame.render_stateful_widget(table, top, &mut state.table);

    let [bottom_left, bottom_right] =
        Layout::horizontal([Constraint::Ratio(1, 2); 2]).areas(bottom);

    let legend = if state.mode.is_editing_name() {
        "Type to edit name | Esc: Abandon changes".to_string()
    } else {
        let mut out =
            "q: Quit | Esc: Back | Up/Down: Change Selection | Ctrl+z: Undo change".to_string();
        if let Some(selected) = state.table.selected()
            && state.partitions[selected].path.is_some()
        {
            out += " | n: Edit name | N: Replace name";
        }
        out
    };
    frame.render_widget(Text::raw(legend), bottom_left);
    frame.render_widget(
        Text::raw(format!("{} pending changes", state.n_changes)).alignment(Alignment::Right),
        bottom_right,
    );
}
