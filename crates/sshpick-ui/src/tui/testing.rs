use std::path::Path;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::style::Modifier;
use ratatui::text::Line;
use sshpick_core::{Entry, Picker, Preview, Slot};

use super::screen::Screen;

pub(super) fn entries(home: &str) -> Vec<Entry> {
    let home = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(home);
    sshpick_core::load(&home).unwrap().entries
}

pub(super) fn setup(home: &str, query: &str) -> (Screen, Picker) {
    let entries = entries(home);
    (Screen::new(&entries), Picker::new(entries, query))
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

pub(super) fn reversed(cell: &Cell) -> bool {
    cell.modifier.contains(Modifier::REVERSED)
}

pub(super) fn dim(cell: &Cell) -> bool {
    cell.modifier.contains(Modifier::DIM)
}

pub(super) fn bold(cell: &Cell) -> bool {
    cell.modifier.contains(Modifier::BOLD)
}

pub(super) fn styles(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let (width, height) = (buffer.area.width, buffer.area.height);
    let mut rows = Vec::new();
    for y in 0..height {
        let mut runs: Vec<(u16, u16, &str)> = Vec::new();
        for (name, modifier) in [
            ("REV", Modifier::REVERSED),
            ("BOLD", Modifier::BOLD),
            ("DIM", Modifier::DIM),
        ] {
            let mut start = None;
            for x in 0..=width {
                let on = x < width && buffer[(x, y)].modifier.contains(modifier);
                match (start, on) {
                    (None, true) => start = Some(x),
                    (Some(from), false) => {
                        runs.push((from, x - 1, name));
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        runs.sort_unstable();
        if !runs.is_empty() {
            let runs: Vec<String> = runs
                .iter()
                .map(|(from, to, name)| format!("{name}[{from}..{to}]"))
                .collect();
            rows.push(format!("y{y:<2} {}", runs.join(" ")));
        }
    }
    rows.join("\n")
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
