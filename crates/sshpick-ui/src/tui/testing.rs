use std::ffi::OsString;
use std::path::Path;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use sshpick_core::{Entry, Picker, Preview, Slot, Warning};

use super::screen::Screen;

pub(super) const DARK: Style = Style::new().fg(Color::Black).bg(Color::White);
pub(super) const LIGHT: Style = Style::new().fg(Color::White).bg(Color::Black);

pub(super) fn entries(home: &str) -> Vec<Entry> {
    let home = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(home);
    sshpick_core::load(&home).unwrap().entries
}

pub(super) fn setup(home: &str, query: &str) -> (Screen, Picker) {
    setup_with(home, query, DARK, &[])
}

pub(super) fn setup_with(
    home: &str,
    query: &str,
    bar: Style,
    warnings: &[Warning],
) -> (Screen, Picker) {
    let entries = entries(home);
    (
        Screen::styled(&entries, warnings, bar),
        Picker::new(entries, query),
    )
}

pub(super) fn terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(width, height)).unwrap()
}

pub(super) fn render(
    terminal: &mut Terminal<TestBackend>,
    screen: &Screen,
    picker: &mut Picker,
    slot: &Slot,
) -> Vec<String> {
    let selected = picker.selected().cloned();
    let pane = selected.as_ref().map(|host| (host, slot));
    terminal
        .draw(|frame| screen.draw(frame, picker, pane))
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .map(|y| {
            let line: String = (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            line.trim_end().to_owned()
        })
        .collect()
}

/// One row per line, numbered, so a snapshot shows blank rows and where the frame ends.
pub(super) fn frame(lines: &[String]) -> String {
    lines
        .iter()
        .enumerate()
        .map(|(y, line)| format!("{y:>2}|{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn cell(terminal: &Terminal<TestBackend>, x: u16, y: u16) -> Cell {
    terminal.backend().buffer()[(x, y)].clone()
}

pub(super) fn inverted(cell: &Cell, bar: Style) -> bool {
    Some(cell.fg) == bar.fg && Some(cell.bg) == bar.bg
}

pub(super) fn vars<'a>(set: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |name| {
        set.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    }
}

pub(super) fn text(line: &Line) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

pub(super) fn ready() -> Slot {
    Slot::Ready(Preview::parse(
        "user ubuntu\nhostname 10.11.212.191\nport 22\nproxyjump newstg-bastion\n",
    ))
}
