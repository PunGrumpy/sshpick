use std::path::Path;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::style::Modifier;
use sshpick_core::{Entry, Picker, Preview, Slot};

use super::plan::{Plan, Space};
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

/// The picker's block in a terminal of a given size, as `Tty` keeps it: the backend is the block,
/// and a new plan starts a new one, as `Tty::fit` makes ratatui draw every cell again.
pub(super) struct Block {
    space: Space,
    plan: Option<Plan>,
    terminal: Terminal<TestBackend>,
}

impl Block {
    pub(super) fn backend(&self) -> &TestBackend {
        self.terminal.backend()
    }

    pub(super) fn backend_mut(&mut self) -> &mut TestBackend {
        self.terminal.backend_mut()
    }
}

/// A terminal of `cols` by `rows`; what is drawn is the block that fits in it.
pub(super) fn terminal(cols: u16, rows: u16) -> Block {
    Block {
        space: Space { cols, rows },
        plan: None,
        terminal: Terminal::new(TestBackend::new(cols, 0)).unwrap(),
    }
}

pub(super) fn render(
    block: &mut Block,
    screen: &Screen,
    picker: &mut Picker,
    slot: &Slot,
) -> Vec<String> {
    let plan = screen.plan(block.space, picker);
    if block.plan != Some(plan) {
        block.terminal = Terminal::new(TestBackend::new(plan.cols, plan.height())).unwrap();
        block.plan = Some(plan);
    }
    let slot = picker.selected().map(|_| slot);
    block
        .terminal
        .draw(|frame| screen.draw(frame, picker, slot, plan))
        .unwrap();
    let buffer = block.backend().buffer();
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

pub(super) fn cell(block: &Block, x: u16, y: u16) -> Cell {
    block.backend().buffer()[(x, y)].clone()
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

pub(super) fn styles(block: &Block) -> String {
    let buffer = block.backend().buffer();
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

pub(super) fn ready() -> Slot {
    Slot::Ready(Preview::parse(
        "user ubuntu\nhostname 10.11.212.191\nport 22\nproxyjump newstg-bastion\n",
    ))
}
