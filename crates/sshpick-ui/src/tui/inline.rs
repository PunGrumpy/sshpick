//! Every row move is relative, so the writer never needs the block's absolute row and never asks
//! the terminal anything.
//!
//! It never erases to the end of the screen: tmux copies the whole screen into its scrollback when
//! that is sent from the top row, and the writer cannot know which row it is on. Rows are erased
//! one at a time instead.
//!
//! Between two frames the cursor is on the block's top row. That is what lets a panic hook or the
//! signal thread restore the terminal with nothing but the extent.

use std::io::{self, Write};
use std::sync::atomic::{AtomicU16, Ordering};

use ratatui::backend::{Backend, ClearType, WindowSize};
use ratatui::buffer::{Cell, CellWidth};
use ratatui::layout::{Position, Size};
use ratatui::style::Modifier;

const SYNC_BEGIN: &[u8] = b"\x1b[?2026h";
const SYNC_END: &[u8] = b"\x1b[?2026l";
const RESET: &[u8] = b"\x1b[0m";
const SHOW: &[u8] = b"\x1b[?25h";
const HIDE: &[u8] = b"\x1b[?25l";
const ERASE_ROW: &[u8] = b"\x1b[2K";
const ATTRIBUTES: Modifier = Modifier::BOLD
    .union(Modifier::DIM)
    .union(Modifier::REVERSED);

pub(super) fn restore_bytes(extent: u16) -> Vec<u8> {
    let mut bytes = [SYNC_END, RESET].concat();
    erase(&mut bytes, extent);
    bytes.extend_from_slice(SHOW);
    bytes
}

/// Line feeds, not cursor-down: at the bottom of the screen a line feed scrolls instead of
/// stopping, so going back up the same count always lands on the row it started from.
fn erase(bytes: &mut Vec<u8>, rows: u16) {
    if rows == 0 {
        return;
    }
    bytes.push(b'\r');
    bytes.extend_from_slice(ERASE_ROW);
    for _ in 1..rows {
        bytes.push(b'\n');
        bytes.extend_from_slice(ERASE_ROW);
    }
    up(bytes, rows - 1);
}

fn up(bytes: &mut Vec<u8>, rows: u16) {
    if rows > 0 {
        csi(bytes, rows, b'A');
    }
}

fn csi(bytes: &mut Vec<u8>, n: u16, last: u8) {
    bytes.extend_from_slice(b"\x1b[");
    bytes.extend_from_slice(n.to_string().as_bytes());
    bytes.push(last);
}

pub(super) struct Inline<'a, W: Write> {
    out: W,
    size: Size,
    /// `x >= size.width` means a write reached the last column, so the real column is unknown
    /// (deferred wrap) and the next write must move explicitly.
    at: Position,
    pen: Modifier,
    visible: bool,
    /// The most rows ever reserved: a shrink and a restore erase this many.
    extent: u16,
    /// What a restore must erase now: `extent`, or more while an erase that reaches further is
    /// queued and not yet written.
    published: &'a AtomicU16,
    /// Per row, the column after the rightmost cell written since the row was erased.
    reach: Vec<u16>,
    clean: bool,
    buf: Vec<u8>,
}

impl<'a, W: Write> Inline<'a, W> {
    pub(super) fn open(out: W, size: Size, published: &'a AtomicU16) -> Inline<'a, W> {
        let mut inline = Inline {
            out,
            size,
            at: Position::ORIGIN,
            pen: Modifier::empty(),
            visible: true,
            extent: 0,
            published,
            reach: Vec::new(),
            clean: false,
            buf: Vec::with_capacity(4096),
        };
        inline.retarget(size);
        inline
    }

    /// The caller then calls `Terminal::resize` so that ratatui draws every cell again.
    pub(super) fn retarget(&mut self, size: Size) {
        self.extent = self.extent.max(size.height);
        let rows = self.extent.max(self.rewrapped(size.width));
        self.published.fetch_max(rows, Ordering::SeqCst);
        self.pen(Modifier::empty());
        erase(&mut self.buf, rows);
        self.size = size;
        self.at = Position::ORIGIN;
        self.reach = vec![0; usize::from(size.height)];
        self.clean = true;
    }

    /// The rows the drawn lines take once a terminal that rewraps long lines (most do) has
    /// narrowed to `width`, so that the fragments it pushed below the block are erased too.
    fn rewrapped(&self, width: u16) -> u16 {
        if width == 0 || width >= self.size.width {
            return 0;
        }
        self.reach.iter().fold(0, |rows: u16, reach| {
            rows.saturating_add(reach.div_ceil(width).max(1))
        })
    }

    fn wipe(&mut self) {
        if self.clean {
            return;
        }
        self.move_to(0, 0);
        self.pen(Modifier::empty());
        erase(&mut self.buf, self.extent);
        self.reach.fill(0);
        self.clean = true;
    }

    fn move_to(&mut self, x: u16, y: u16) {
        let x = x.min(self.size.width.saturating_sub(1));
        let y = y.min(self.size.height.saturating_sub(1));
        if self.at == (Position { x, y }) {
            return;
        }
        // A line feed for the same reason as in `erase`. It may also return the carriage, so the
        // column is set again after one.
        let down = y > self.at.y;
        if down {
            for _ in self.at.y..y {
                self.buf.push(b'\n');
            }
        } else {
            up(&mut self.buf, self.at.y - y);
        }
        if x == 0 {
            self.buf.push(b'\r');
        } else if down || x != self.at.x || self.at.x >= self.size.width {
            csi(&mut self.buf, x + 1, b'G');
        }
        self.at = Position { x, y };
    }

    fn pen(&mut self, want: Modifier) {
        let want = want & ATTRIBUTES;
        if want == self.pen {
            return;
        }
        self.buf.extend_from_slice(b"\x1b[0");
        for (flag, code) in [
            (Modifier::BOLD, &b";1"[..]),
            (Modifier::DIM, b";2"),
            (Modifier::REVERSED, b";7"),
        ] {
            if want.contains(flag) {
                self.buf.extend_from_slice(code);
            }
        }
        self.buf.push(b'm');
        self.pen = want;
    }

    fn hide(&mut self) {
        if self.visible {
            self.buf.extend_from_slice(HIDE);
            self.visible = false;
        }
    }
}

impl<W: Write> Backend for Inline<'_, W> {
    type Error = io::Error;

    fn draw<'b, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'b Cell)>,
    {
        for (x, y, cell) in content {
            self.hide();
            self.move_to(x, y);
            self.pen(cell.modifier);
            let symbol = cell.symbol();
            // The last hop before the terminal: no cell can carry an escape sequence out.
            let width = if symbol.chars().any(char::is_control) {
                self.buf.push(b' ');
                1
            } else {
                self.buf.extend_from_slice(symbol.as_bytes());
                // ratatui's own width, so this tracks the columns the diff skipped over (wide
                // glyphs, forced widths) and cannot drift from it.
                cell.cell_width()
            };
            self.at.x = self.at.x.saturating_add(width);
            if let Some(reach) = self.reach.get_mut(usize::from(self.at.y)) {
                *reach = (*reach).max(self.at.x);
            }
            self.clean = false;
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.hide();
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        if !self.visible {
            self.buf.extend_from_slice(SHOW);
            self.visible = true;
        }
        Ok(())
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        Ok(Position {
            x: self.at.x.min(self.size.width.saturating_sub(1)),
            y: self.at.y,
        })
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        let Position { x, y } = position.into();
        self.move_to(x, y);
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.clear_region(ClearType::All)
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        match clear_type {
            ClearType::All => self.wipe(),
            ClearType::AfterCursor if self.at == Position::ORIGIN => self.wipe(),
            ClearType::CurrentLine => self.buf.extend_from_slice(ERASE_ROW),
            ClearType::UntilNewLine => self.buf.extend_from_slice(b"\x1b[K"),
            ClearType::AfterCursor | ClearType::BeforeCursor => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "the block is erased whole, from its top row",
                ));
            }
        }
        Ok(())
    }

    fn size(&self) -> io::Result<Size> {
        Ok(self.size)
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        Ok(WindowSize {
            columns_rows: self.size,
            pixels: Size::new(0, 0),
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        self.move_to(self.at.x, 0);
        self.pen(Modifier::empty());
        let frame = [SYNC_BEGIN, &self.buf, SYNC_END].concat();
        self.buf.clear();
        self.out.write_all(&frame)?;
        self.out.flush()?;
        self.published.store(self.extent, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! The writer is checked against `vt100`, an emulator that did not come from this code: the
    //! screen it ends up with is the oracle.

    use std::cell::RefCell;
    use std::rc::Rc;

    use ratatui::layout::Rect;
    use ratatui::style::Style;
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Paragraph, Widget};
    use ratatui::{Terminal, TerminalOptions, Viewport};

    use super::*;

    #[derive(Clone, Default)]
    struct Wire(Rc<RefCell<Vec<u8>>>);

    impl Write for Wire {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct Emulator {
        vt: vt100::Parser,
        wire: Wire,
        seen: Vec<u8>,
    }

    impl Emulator {
        fn new(rows: u16, cols: u16) -> Emulator {
            Emulator {
                vt: vt100::Parser::new(rows, cols, 100),
                wire: Wire::default(),
                seen: Vec::new(),
            }
        }

        fn shell(&mut self, text: &str) {
            self.vt.process(text.as_bytes());
        }

        fn pump(&mut self) -> usize {
            let bytes = self.take();
            self.vt.process(&bytes);
            bytes.len()
        }

        fn take(&mut self) -> Vec<u8> {
            let bytes = std::mem::take(&mut *self.wire.0.borrow_mut());
            self.seen.extend_from_slice(&bytes);
            bytes
        }

        fn rows(&self) -> Vec<String> {
            let (_, cols) = self.vt.screen().size();
            self.vt
                .screen()
                .rows(0, cols)
                .map(|row| row.trim_end().to_owned())
                .collect()
        }

        fn cursor(&self) -> (u16, u16) {
            self.vt.screen().cursor_position()
        }

        fn scrollback(&mut self) -> Vec<String> {
            let cols = self.vt.screen().size().1;
            self.vt.screen_mut().set_scrollback(usize::MAX);
            let depth = self.vt.screen().scrollback();
            let mut lines = Vec::new();
            for offset in (1..=depth).rev() {
                self.vt.screen_mut().set_scrollback(offset);
                let first = self.vt.screen().rows(0, cols).next().unwrap_or_default();
                lines.push(first.trim_end().to_owned());
            }
            self.vt.screen_mut().set_scrollback(0);
            lines
        }
    }

    type Block<'a> = Terminal<Inline<'a, Wire>>;

    fn open<'a>(emulator: &Emulator, extent: &'a AtomicU16, width: u16, height: u16) -> Block<'a> {
        let inline = Inline::open(emulator.wire.clone(), Size::new(width, height), extent);
        Terminal::with_options(
            inline,
            TerminalOptions {
                viewport: Viewport::Fixed(Rect::new(0, 0, width, height)),
            },
        )
        .expect("a fixed viewport asks the terminal nothing")
    }

    fn paint(block: &mut Block<'_>, lines: &[Line<'static>], cursor: Position) {
        block
            .draw(|frame| {
                let area = frame.area();
                Paragraph::new(lines.to_vec()).render(area, frame.buffer_mut());
                frame.set_cursor_position(cursor);
            })
            .expect("draw");
    }

    fn plain(rows: &[&str]) -> Vec<Line<'static>> {
        rows.iter()
            .map(|row| Line::from((*row).to_owned()))
            .collect()
    }

    fn resize(block: &mut Block<'_>, width: u16, height: u16) {
        block.backend_mut().retarget(Size::new(width, height));
        block
            .resize(Rect::new(0, 0, width, height))
            .expect("resize");
    }

    fn rows_erased(bytes: &[u8]) -> usize {
        bytes
            .windows(ERASE_ROW.len())
            .filter(|window| *window == ERASE_ROW)
            .count()
    }

    const HISTORY: &str = "$ ls\r\nMakefile  src\r\n$ sshpick\r\n";

    #[test]
    fn opens_below_the_command_line_and_leaves_nothing_behind() {
        let mut emulator = Emulator::new(10, 30);
        emulator.shell(HISTORY);
        assert_eq!(emulator.cursor(), (3, 0));

        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 30, 4);
        paint(
            &mut block,
            &plain(&["> con", "  one", "  two", "  esc quit"]),
            Position::new(5, 0),
        );
        emulator.pump();
        assert_eq!(
            emulator.rows()[..8],
            [
                "$ ls",
                "Makefile  src",
                "$ sshpick",
                "> con",
                "  one",
                "  two",
                "  esc quit",
                ""
            ]
        );
        assert_eq!(
            emulator.cursor(),
            (3, 5),
            "the cursor waits in the prompt, on the top row"
        );

        emulator
            .vt
            .process(&restore_bytes(extent.load(Ordering::SeqCst)));
        assert_eq!(
            emulator.rows()[..5],
            ["$ ls", "Makefile  src", "$ sshpick", "", ""]
        );
        assert_eq!(
            emulator.cursor(),
            (3, 0),
            "the next output starts where the block started"
        );
    }

    #[test]
    fn at_the_bottom_the_screen_scrolls_and_the_context_stays_intact() {
        let mut emulator = Emulator::new(6, 30);
        emulator.shell("h0\r\nh1\r\nh2\r\nh3\r\n$ sshpick\r\n");
        assert_eq!(emulator.cursor(), (5, 0));

        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 30, 4);
        paint(
            &mut block,
            &plain(&["> ", "  a", "  b", "  c"]),
            Position::new(2, 0),
        );
        emulator.pump();
        assert_eq!(
            emulator.rows(),
            ["h3", "$ sshpick", ">", "  a", "  b", "  c"]
        );
        assert_eq!(emulator.scrollback(), ["h0", "h1", "h2"]);
        assert_eq!(emulator.cursor(), (2, 2));

        emulator
            .vt
            .process(&restore_bytes(extent.load(Ordering::SeqCst)));
        assert_eq!(emulator.rows(), ["h3", "$ sshpick", "", "", "", ""]);
        assert_eq!(emulator.cursor(), (2, 0));
    }

    #[test]
    fn a_block_as_tall_as_the_screen_still_ends_up_on_it() {
        let mut emulator = Emulator::new(5, 20);
        emulator.shell("$ sshpick\r\n");
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 20, 5);
        paint(
            &mut block,
            &plain(&["> ", "1", "2", "3", "4"]),
            Position::new(2, 0),
        );
        emulator.pump();
        assert_eq!(emulator.rows(), [">", "1", "2", "3", "4"]);
        assert_eq!(emulator.scrollback(), ["$ sshpick"]);
    }

    #[test]
    fn a_changed_cell_costs_a_few_bytes_and_an_unchanged_frame_costs_none() {
        let mut emulator = Emulator::new(10, 30);
        emulator.shell(HISTORY);
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 30, 4);
        let lines = plain(&["> ", "  one", "  two", "  esc quit"]);
        paint(&mut block, &lines, Position::new(2, 0));
        assert!(emulator.pump() > 40);

        paint(&mut block, &lines, Position::new(2, 0));
        assert_eq!(emulator.pump(), 0, "nothing changed, so nothing is written");

        paint(
            &mut block,
            &plain(&["> x", "  one", "  two", "  esc quit"]),
            Position::new(3, 0),
        );
        let typed = emulator.pump();
        assert!(typed < 50, "one typed character wrote {typed} bytes");
        assert_eq!(emulator.rows()[3], "> x");
    }

    #[test]
    fn a_taller_block_grows_from_the_same_top_row_and_shrinks_back() {
        for (rows, history) in [(14, HISTORY), (6, "a\r\nb\r\nc\r\nd\r\n$ sshpick\r\n")] {
            let mut emulator = Emulator::new(rows, 24);
            emulator.shell(history);
            let extent = AtomicU16::new(0);
            let mut block = open(&emulator, &extent, 24, 3);
            paint(
                &mut block,
                &plain(&["> ", "  a", "  hint"]),
                Position::new(2, 0),
            );
            emulator.pump();
            let top_of = |emulator: &Emulator| {
                emulator
                    .rows()
                    .iter()
                    .position(|row| row == ">")
                    .expect("prompt")
            };
            let before = top_of(&emulator);

            resize(&mut block, 24, 5);
            paint(
                &mut block,
                &plain(&["> ", "  a", "  keys 1", "  keys 2", "  hint"]),
                Position::new(2, 0),
            );
            emulator.pump();
            let top = top_of(&emulator);
            assert!(
                top <= before,
                "growing never moves the block down ({rows} rows)"
            );
            assert_eq!(
                emulator.rows()[top..top + 5],
                [">", "  a", "  keys 1", "  keys 2", "  hint"]
            );
            assert_eq!(emulator.rows().iter().filter(|row| *row == ">").count(), 1);
            assert_eq!(emulator.cursor(), (u16::try_from(top).expect("row"), 2));

            resize(&mut block, 24, 3);
            paint(
                &mut block,
                &plain(&["> ", "  a", "  hint"]),
                Position::new(2, 0),
            );
            emulator.pump();
            assert_eq!(emulator.rows()[top..top + 3], [">", "  a", "  hint"]);
            assert!(
                emulator.rows()[top + 3..].iter().all(String::is_empty),
                "{rows} rows"
            );
            assert_eq!(emulator.cursor(), (u16::try_from(top).expect("row"), 2));
        }
    }

    #[test]
    fn shrinking_and_leaving_erase_exactly_the_rows_the_block_ever_reserved() {
        let mut emulator = Emulator::new(14, 24);
        emulator.shell(HISTORY);
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 24, 3);
        paint(
            &mut block,
            &plain(&["> ", "  a", "  hint"]),
            Position::new(2, 0),
        );
        assert_eq!(
            rows_erased(&emulator.take()),
            3,
            "opening erases what it reserves"
        );
        assert_eq!(extent.load(Ordering::SeqCst), 3);

        resize(&mut block, 24, 6);
        paint(
            &mut block,
            &plain(&["> ", "  a", "  k1", "  k2", "  k3", "  hint"]),
            Position::new(2, 0),
        );
        assert_eq!(rows_erased(&emulator.take()), 6);
        assert_eq!(extent.load(Ordering::SeqCst), 6);

        resize(&mut block, 24, 3);
        paint(
            &mut block,
            &plain(&["> ", "  a", "  hint"]),
            Position::new(2, 0),
        );
        assert_eq!(
            rows_erased(&emulator.take()),
            6,
            "shrinking erases the taller block's rows, once"
        );
        assert_eq!(extent.load(Ordering::SeqCst), 6, "the extent never shrinks");

        let restore = restore_bytes(extent.load(Ordering::SeqCst));
        assert_eq!(rows_erased(&restore), 6);
    }

    #[test]
    fn the_extent_is_published_before_the_rows_it_covers_are_written() {
        let emulator = Emulator::new(10, 30);
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 30, 4);
        assert_eq!(extent.load(Ordering::SeqCst), 4);
        assert!(
            emulator.wire.0.borrow().is_empty(),
            "nothing is written yet"
        );
        resize(&mut block, 30, 7);
        assert_eq!(extent.load(Ordering::SeqCst), 7);
    }

    #[test]
    fn a_narrower_terminal_leaves_exactly_one_block() {
        let mut emulator = Emulator::new(10, 30);
        emulator.shell(HISTORY);
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 30, 3);
        paint(
            &mut block,
            &plain(&["> abc", "  one", "  hint"]),
            Position::new(5, 0),
        );
        emulator.pump();

        emulator.vt.screen_mut().set_size(10, 12);
        resize(&mut block, 12, 3);
        paint(
            &mut block,
            &plain(&["> abc", "  one", "  hint"]),
            Position::new(5, 0),
        );
        emulator.pump();
        let rows = emulator.rows();
        assert_eq!(
            rows.iter().filter(|row| row.starts_with("> ")).count(),
            1,
            "{rows:#?}"
        );
        assert_eq!(
            rows[..2],
            ["$ ls", "Makefile  sr"],
            "the context above is the terminal's, not ours"
        );
        assert_eq!(rows[3..6], ["> abc", "  one", "  hint"]);
        assert_eq!(emulator.cursor(), (3, 5));
    }

    #[test]
    fn a_narrower_terminal_erases_the_rows_its_long_lines_rewrap_into() {
        let mut emulator = Emulator::new(14, 30);
        emulator.shell(HISTORY);
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 30, 3);
        let full = "> ".to_owned() + &"x".repeat(28);
        paint(
            &mut block,
            &plain(&[&full, "  one", &"y".repeat(20)]),
            Position::new(2, 0),
        );
        emulator.pump();

        // A terminal that rewraps long lines turns the three rows into 3 + 1 + 2 at 10 columns.
        // vt100 does not, so the fragments it would leave below the block are put there by hand.
        emulator.vt.screen_mut().set_size(14, 10);
        emulator
            .vt
            .process(b"\x1b7\x1b[7;1Hxxxxxxxxxx\x1b[8;1Hxxxxxxxxxx\x1b[9;1Hyyyyyyyyyy\x1b8");
        resize(&mut block, 10, 3);
        assert_eq!(
            extent.load(Ordering::SeqCst),
            6,
            "a restore before the next frame erases them too"
        );
        paint(
            &mut block,
            &plain(&["> ", "  one", "  hint"]),
            Position::new(2, 0),
        );
        emulator.pump();
        let rows = emulator.rows();
        assert_eq!(rows[3..6], [">", "  one", "  hint"], "{rows:#?}");
        assert!(rows[6..].iter().all(String::is_empty), "{rows:#?}");
        assert_eq!(
            extent.load(Ordering::SeqCst),
            3,
            "once they are erased, a restore erases what the block reserved"
        );
    }

    #[test]
    fn a_storm_of_resizes_ends_in_the_same_place() {
        let mut emulator = Emulator::new(12, 40);
        emulator.shell(HISTORY);
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 40, 5);
        paint(
            &mut block,
            &plain(&["> ", "  a", "  b", "  c", "  hint"]),
            Position::new(2, 0),
        );
        emulator.pump();
        for (width, height) in [(30, 5), (20, 4), (40, 6), (25, 3), (40, 5)] {
            emulator.vt.screen_mut().set_size(12, width);
            resize(&mut block, width, height);
            let rows: Vec<Line<'static>> = (0..height)
                .map(|y| {
                    Line::from(if y == 0 {
                        "> ".to_owned()
                    } else {
                        format!("  r{y}")
                    })
                })
                .collect();
            paint(&mut block, &rows, Position::new(2, 0));
            emulator.pump();
            assert_eq!(
                emulator
                    .rows()
                    .iter()
                    .filter(|row| row.starts_with('>'))
                    .count(),
                1
            );
            assert_eq!(emulator.cursor(), (3, 2));
        }
        emulator
            .vt
            .process(&restore_bytes(extent.load(Ordering::SeqCst)));
        assert!(emulator.rows()[3..].iter().all(String::is_empty));
    }

    #[test]
    fn attributes_arrive_and_are_reset_at_the_end_of_the_frame() {
        let mut emulator = Emulator::new(8, 20);
        emulator.shell("$ sshpick\r\n");
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 20, 3);
        let lines = vec![
            Line::from(vec![
                Span::styled(">", Style::new().add_modifier(Modifier::BOLD)),
                Span::raw(" q"),
            ]),
            Line::styled("▌ one", Style::new().add_modifier(Modifier::REVERSED)),
            Line::styled("  two", Style::new().add_modifier(Modifier::DIM)),
        ];
        paint(&mut block, &lines, Position::new(3, 0));
        emulator.pump();
        let screen = emulator.vt.screen();
        let at = |row: u16, col: u16| screen.cell(row, col).cloned().expect("cell");
        assert!(at(1, 0).bold() && !at(1, 2).bold());
        assert!(at(2, 0).inverse() && at(2, 4).inverse());
        assert!(at(3, 2).dim() && !at(3, 2).inverse());
        assert!(!screen.bold() && !screen.inverse() && !screen.dim());
    }

    #[test]
    fn a_row_that_fills_the_width_does_not_wrap_into_the_next() {
        let mut emulator = Emulator::new(8, 10);
        emulator.shell("$ sshpick\r\n");
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 10, 3);
        paint(
            &mut block,
            &plain(&["0123456789", "abc", "ABCDEFGHIJ"]),
            Position::new(9, 0),
        );
        emulator.pump();
        assert_eq!(emulator.rows()[1..4], ["0123456789", "abc", "ABCDEFGHIJ"]);
        paint(
            &mut block,
            &plain(&["0123456789", "abx", "ABCDEFGHIJ"]),
            Position::new(9, 0),
        );
        emulator.pump();
        assert_eq!(emulator.rows()[1..4], ["0123456789", "abx", "ABCDEFGHIJ"]);
        assert_eq!(emulator.cursor(), (1, 9));
    }

    #[test]
    fn wide_and_combining_text_keeps_the_columns_in_step() {
        let mut emulator = Emulator::new(8, 20);
        emulator.shell("$ sshpick\r\n");
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 20, 2);
        paint(
            &mut block,
            &plain(&["日本語 ok", "ก็ ข้อ end"]),
            Position::new(0, 0),
        );
        emulator.pump();
        assert_eq!(emulator.rows()[1..3], ["日本語 ok", "ก็ ข้อ end"]);
        paint(
            &mut block,
            &plain(&["日本語 OK", "ก็ ข้อ END"]),
            Position::new(0, 0),
        );
        emulator.pump();
        assert_eq!(emulator.rows()[1..3], ["日本語 OK", "ก็ ข้อ END"]);
    }

    #[test]
    fn no_cell_can_carry_a_control_sequence_to_the_terminal() {
        let mut emulator = Emulator::new(8, 20);
        emulator.shell("$ sshpick\r\n");
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 20, 2);
        block
            .draw(|frame| {
                let buf = frame.buffer_mut();
                buf[(0, 0)].set_symbol("\u{9b}");
                buf[(1, 0)].set_symbol("x");
                buf[(2, 0)].set_symbol("\u{85}");
            })
            .expect("draw");
        emulator.pump();
        assert_eq!(emulator.rows()[1], " x");
        assert!(!emulator.vt.screen().alternate_screen());
    }

    /// Any other sequence is a question to the terminal, an erase that tmux turns into scrollback,
    /// or a mode the picker has no business setting.
    #[test]
    fn the_bytes_never_ask_the_terminal_anything_or_erase_past_a_row() {
        let mut emulator = Emulator::new(10, 30);
        emulator.shell(HISTORY);
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 30, 4);
        paint(
            &mut block,
            &plain(&["> a", "  one", "  two", "  hint"]),
            Position::new(3, 0),
        );
        emulator.pump();
        resize(&mut block, 20, 6);
        paint(
            &mut block,
            &plain(&["> ab", "1", "2", "3", "4", "5"]),
            Position::new(4, 0),
        );
        emulator.pump();
        resize(&mut block, 20, 2);
        paint(&mut block, &plain(&["> ab", "1"]), Position::new(4, 0));
        emulator.pump();
        emulator
            .wire
            .0
            .borrow_mut()
            .extend(restore_bytes(extent.load(Ordering::SeqCst)));
        emulator.pump();

        let bytes = &emulator.seen;
        let mut i = 0;
        let mut sequences = 0;
        while i < bytes.len() {
            if bytes[i] == 0x1b {
                assert_eq!(
                    bytes.get(i + 1),
                    Some(&b'['),
                    "only CSI, never OSC, DCS or a bare ESC at {i}"
                );
                let mut j = i + 2;
                while j < bytes.len() && !(0x40..=0x7e).contains(&bytes[j]) {
                    j += 1;
                }
                let body = &bytes[i + 2..j];
                let end = bytes[j];
                let ok = match end {
                    b'A' | b'G' => !body.is_empty() && body.iter().all(u8::is_ascii_digit),
                    b'K' => body.is_empty() || body == b"2",
                    b'm' => body.iter().all(|b| b.is_ascii_digit() || *b == b';'),
                    b'h' | b'l' => body == b"?25" || body == b"?2026",
                    _ => false,
                };
                assert!(
                    ok,
                    "unexpected sequence ESC[{}{}",
                    String::from_utf8_lossy(body),
                    end as char
                );
                sequences += 1;
                i = j + 1;
            } else {
                assert!(
                    bytes[i] == b'\r' || bytes[i] == b'\n' || bytes[i] >= 0x20,
                    "stray control byte {:#x}",
                    bytes[i]
                );
                i += 1;
            }
        }
        assert!(sequences > 10);
    }

    #[test]
    fn restore_from_the_published_extent_leaves_no_trace_and_can_run_twice() {
        let mut emulator = Emulator::new(10, 30);
        emulator.shell(HISTORY);
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 30, 4);
        paint(
            &mut block,
            &plain(&["> a", "  one", "  two", "  hint"]),
            Position::new(3, 0),
        );
        emulator.pump();
        resize(&mut block, 30, 6);
        paint(
            &mut block,
            &plain(&["> a", "  one", "  two", "  k1", "  k2", "  hint"]),
            Position::new(3, 0),
        );
        emulator.pump();
        let restore = restore_bytes(extent.load(Ordering::SeqCst));
        emulator.vt.process(&restore);
        emulator.vt.process(&restore);
        assert_eq!(
            emulator.rows(),
            [
                "$ ls",
                "Makefile  src",
                "$ sshpick",
                "",
                "",
                "",
                "",
                "",
                "",
                ""
            ]
        );
        assert_eq!(emulator.cursor(), (3, 0));
        assert!(!emulator.vt.screen().hide_cursor());
        assert!(emulator.scrollback().is_empty());
    }

    #[test]
    fn a_block_on_the_top_row_puts_nothing_in_the_scrollback() {
        let mut emulator = Emulator::new(10, 30);
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 30, 4);
        paint(
            &mut block,
            &plain(&["> a", "  one", "  two", "  hint"]),
            Position::new(3, 0),
        );
        emulator.pump();
        resize(&mut block, 30, 2);
        paint(&mut block, &plain(&["> a", "  hint"]), Position::new(3, 0));
        emulator.pump();
        emulator
            .vt
            .process(&restore_bytes(extent.load(Ordering::SeqCst)));
        assert!(emulator.rows().iter().all(String::is_empty));
        assert_eq!(emulator.cursor(), (0, 0));
        assert!(emulator.scrollback().is_empty());
    }

    #[test]
    fn the_cursor_is_back_on_the_top_row_after_every_frame() {
        let mut emulator = Emulator::new(10, 30);
        emulator.shell(HISTORY);
        let extent = AtomicU16::new(0);
        let mut block = open(&emulator, &extent, 30, 4);
        for query in ["", "c", "co", "con", "cons"] {
            let lines = plain(&[&format!("> {query}"), "  one", "  two", "  hint"]);
            paint(
                &mut block,
                &lines,
                Position::new(2 + u16::try_from(query.len()).expect("len"), 0),
            );
            emulator.pump();
            assert_eq!(emulator.cursor().0, 3);
        }
        block
            .draw(|frame| {
                Paragraph::new("> x\n  one\n  two\n  tail")
                    .render(frame.area(), frame.buffer_mut());
            })
            .expect("draw");
        emulator.pump();
        assert_eq!(emulator.cursor().0, 3);
    }
}
