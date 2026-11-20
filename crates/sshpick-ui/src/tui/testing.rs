use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Color, Modifier};
use sshpick_core::{Entry, Picker, Preview, Recents, Slot};

use super::plan::{Plan, Space};
use super::screen::Screen;
use crate::ColorConfig;

pub(super) fn entries(home: &str) -> Vec<Entry> {
    let home = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(home);
    sshpick_core::load(&home).unwrap().entries
}

/// The picker with no colour: what `NO_COLOR` or a terminal without it gets.
pub(super) fn setup(home: &str, query: &str) -> (Screen, Picker) {
    setup_with(home, query, &Recents::none())
}

pub(super) fn setup_with(home: &str, query: &str, recents: &Recents) -> (Screen, Picker) {
    setup_in(home, query, recents, ColorConfig::none())
}

/// The picker on a terminal that takes colour.
pub(super) fn setup_colored(home: &str, query: &str) -> (Screen, Picker) {
    let colors = ColorConfig::new(true, false, &|_| None);
    setup_in(home, query, &Recents::none(), colors)
}

pub(super) fn setup_in(
    home: &str,
    query: &str,
    recents: &Recents,
    colors: ColorConfig,
) -> (Screen, Picker) {
    let entries = entries(home);
    (
        Screen::new(&entries, colors),
        Picker::new(entries, query, recents),
    )
}

/// The `what` fixture's hosts a minute ago, as of a fixed instant.
pub(super) fn recently_what() -> Recents {
    let now = UNIX_EPOCH + Duration::from_secs(1_794_556_800);
    Recents::of(
        [
            ("aaa", now - Duration::from_secs(60)),
            ("bbb", now - Duration::from_secs(60)),
        ],
        now,
    )
}

/// Two hours ago `newstg-consul1`, three days ago `lab-b`, two weeks ago `newstg-consul2`, as of a
/// fixed instant. The first two have a WHAT, the last has not.
pub(super) fn recently_used() -> Recents {
    let now = UNIX_EPOCH + Duration::from_secs(1_794_556_800);
    Recents::of(
        [
            ("newstg-consul1", now - Duration::from_secs(2 * 3600)),
            ("lab-b", now - Duration::from_secs(3 * 86_400)),
            ("newstg-consul2", now - Duration::from_secs(14 * 86_400)),
        ],
        now,
    )
}

/// The picker in a terminal of a given size. The backend keeps what was drawn, as a terminal does,
/// so a cell left over from an earlier frame shows.
pub(super) struct Block {
    space: Space,
    terminal: Terminal<TestBackend>,
    inside: Rect,
    outer: Rect,
}

impl Block {
    pub(super) fn backend(&self) -> &TestBackend {
        self.terminal.backend()
    }

    pub(super) fn plan(&self, screen: &Screen, picker: &Picker) -> Plan {
        screen.plan(self.space, picker)
    }

    /// The columns of [`render`]'s lines.
    pub(super) fn width(&self) -> u16 {
        self.inside.width
    }

    /// The cursor, in columns and rows of [`render`]'s lines.
    pub(super) fn assert_cursor(&mut self, x: u16, y: u16) {
        let at = (self.inside.x + x, self.inside.y + y);
        self.terminal.backend_mut().assert_cursor_position(at);
    }

    fn rows(&self, rect: Rect) -> Vec<String> {
        let buffer = self.backend().buffer();
        (rect.y..rect.bottom())
            .map(|y| {
                let line: String = (rect.x..rect.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect();
                line.trim_end().to_owned()
            })
            .collect()
    }
}

/// A terminal of `cols` by `rows`: the whole screen, of which the picker draws a panel.
pub(super) fn terminal(cols: u16, rows: u16) -> Block {
    let rows = rows.max(1);
    Block {
        space: Space { cols, rows },
        terminal: Terminal::new(TestBackend::new(cols, rows)).unwrap(),
        inside: Rect::default(),
        outer: Rect::default(),
    }
}

fn draw(block: &mut Block, screen: &Screen, picker: &mut Picker, slot: &Slot) -> Plan {
    let plan = block.plan(screen, picker);
    let slot = picker.selected().map(|_| slot);
    block
        .terminal
        .draw(|frame| screen.draw(frame, picker, slot, plan))
        .unwrap();
    let panel = plan.panel;
    let outer = Rect::new(panel.left, panel.top, panel.cols, panel.rows);
    block.outer = outer;
    block.inside = if panel.border {
        outer.inner(Margin::new(1, 1))
    } else {
        outer
    };
    plan
}

/// The picker's rows: what is inside the border, or the panel when it has none. With a border the
/// rule above the key hints is a row of it.
pub(super) fn render(
    block: &mut Block,
    screen: &Screen,
    picker: &mut Picker,
    slot: &Slot,
) -> Vec<String> {
    draw(block, screen, picker, slot);
    block.rows(block.inside)
}

/// The panel with its border, as it sits in the terminal.
pub(super) fn render_panel(
    block: &mut Block,
    screen: &Screen,
    picker: &mut Picker,
    slot: &Slot,
) -> Vec<String> {
    draw(block, screen, picker, slot);
    block.rows(block.outer)
}

/// Every row of the terminal as it was drawn.
pub(super) fn screen_lines(block: &Block) -> Vec<String> {
    block.rows(block.backend().buffer().area)
}

/// The terminal as it was drawn, rows numbered, down to the last that is not blank.
pub(super) fn shown(block: &Block) -> String {
    let lines = screen_lines(block);
    let used = lines
        .iter()
        .rposition(|line| !line.is_empty())
        .map_or(0, |at| at + 1);
    lines[..used]
        .iter()
        .enumerate()
        .map(|(y, line)| format!("{y:>2}|{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The cell at a column and row of [`render`]'s lines.
pub(super) fn cell(block: &Block, x: u16, y: u16) -> Cell {
    block.backend().buffer()[(block.inside.x + x, block.inside.y + y)].clone()
}

/// The cell at a column and row of the terminal.
pub(super) fn screen_cell(block: &Block, x: u16, y: u16) -> Cell {
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

/// The role's cells, with colour (`color`) or without. Each is spelled out here, so a change to a
/// role in `style.rs` fails these before it reaches a terminal.
pub(super) fn is_bar(cell: &Cell, color: bool) -> bool {
    if color {
        cell.fg == Color::Blue && cell.bg == Color::Reset && reversed(cell) && !dim(cell)
    } else {
        reversed(cell) && !dim(cell) && plain_hue(cell)
    }
}

pub(super) fn is_muted(cell: &Cell, color: bool) -> bool {
    if color {
        cell.fg == Color::Indexed(245) && cell.bg == Color::Reset && !dim(cell) && !bold(cell)
    } else {
        dim(cell) && !bold(cell) && !reversed(cell) && plain_hue(cell)
    }
}

/// The border and the rule: a dimmer grey than `Muted` under colour.
pub(super) fn is_stroke(cell: &Cell, color: bool) -> bool {
    if color {
        cell.fg == Color::Indexed(245) && cell.bg == Color::Reset && dim(cell) && !bold(cell)
    } else {
        dim(cell) && !bold(cell) && !reversed(cell) && plain_hue(cell)
    }
}

pub(super) fn is_keycap(cell: &Cell, color: bool) -> bool {
    if color {
        cell.bg == Color::Indexed(236) && cell.fg == Color::Indexed(250) && !bold(cell)
    } else {
        bold(cell) && !dim(cell) && !reversed(cell) && plain_hue(cell)
    }
}

pub(super) fn is_field(cell: &Cell, color: bool) -> bool {
    if color {
        cell.bg == Color::Indexed(236) && cell.fg == Color::Reset && cell.modifier.is_empty()
    } else {
        is_plain(cell)
    }
}

/// Text typed on the field: the ramp's brightest grey on the field's background.
pub(super) fn is_typed(cell: &Cell, color: bool) -> bool {
    if color {
        cell.bg == Color::Indexed(236) && cell.fg == Color::Indexed(255) && cell.modifier.is_empty()
    } else {
        is_plain(cell)
    }
}

pub(super) fn is_placeholder(cell: &Cell, color: bool) -> bool {
    if color {
        cell.bg == Color::Indexed(236) && cell.fg == Color::Indexed(245) && cell.modifier.is_empty()
    } else {
        is_muted(cell, false)
    }
}

/// A match is the look of the text it sits in, bold and never dim.
pub(super) fn is_match(cell: &Cell) -> bool {
    bold(cell) && !dim(cell) && !reversed(cell) && plain_hue(cell)
}

pub(super) fn is_muted_match(cell: &Cell, color: bool) -> bool {
    if color {
        cell.fg == Color::Indexed(245) && cell.bg == Color::Reset && bold(cell) && !dim(cell)
    } else {
        is_match(cell)
    }
}

pub(super) fn is_bar_match(cell: &Cell, color: bool) -> bool {
    bold(cell)
        && !dim(cell)
        && if color {
            cell.fg == Color::Blue && cell.bg == Color::Reset && reversed(cell)
        } else {
            reversed(cell) && plain_hue(cell)
        }
}

pub(super) fn is_fail(cell: &Cell, color: bool) -> bool {
    if color {
        cell.fg == Color::Red && cell.bg == Color::Reset && cell.modifier.is_empty()
    } else {
        is_plain(cell)
    }
}

pub(super) fn is_plain(cell: &Cell) -> bool {
    plain_hue(cell) && cell.modifier.is_empty()
}

fn plain_hue(cell: &Cell) -> bool {
    cell.fg == Color::Reset && cell.bg == Color::Reset
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
