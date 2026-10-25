use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph};
use sshpick_core::BINDINGS;

use super::style::DIM;
use crate::text::{columns, plural};

const TITLE: &str = " Keybinds ";
const DASH: &str = " - ";
const BORDER: u16 = 1;
const PADDING: u16 = 1;
const FRAME_COLUMNS: u16 = 2 * (BORDER + PADDING);
const FRAME_ROWS: u16 = 2 * (BORDER + PADDING);

pub(super) fn popup(frame: &mut Frame) {
    let area = frame.area();
    let keys = BINDINGS
        .iter()
        .map(|binding| columns(binding.keys))
        .max()
        .unwrap_or(0);
    let binds: Vec<(String, &str)> = BINDINGS
        .iter()
        .map(|binding| {
            let pad = " ".repeat(keys - columns(binding.keys));
            (format!("{}{pad}", binding.keys), binding.does)
        })
        .collect();
    let bind_width = binds
        .iter()
        .map(|(keys, does)| columns(keys) + columns(DASH) + columns(does))
        .max()
        .unwrap_or(0);
    let inner = bind_width.max(columns(TITLE));

    let rows = binds.len();
    let wide = width_of(inner)
        .saturating_add(FRAME_COLUMNS)
        .min(area.width);
    let high = u16::try_from(rows)
        .unwrap_or(u16::MAX)
        .saturating_add(FRAME_ROWS)
        .min(area.height);
    if wide < FRAME_COLUMNS || high < FRAME_ROWS {
        return;
    }
    let rect = Rect::new(
        area.x + (area.width - wide) / 2,
        area.y + (area.height - high) / 2,
        wide,
        high,
    );

    let room = usize::from(high - FRAME_ROWS);
    let mut lines: Vec<Line> = binds
        .iter()
        .map(|(keys, does)| {
            Line::from(vec![
                Span::raw(keys.clone()),
                Span::styled(DASH, DIM),
                Span::raw(*does),
            ])
        })
        .collect();
    let hidden = lines.len().saturating_sub(room);
    lines.truncate(room);

    let mut block = Block::bordered()
        .title(TITLE)
        .padding(Padding::uniform(PADDING));
    if hidden > 0 {
        block = block.title_bottom(format!(
            " {hidden} more {}. Make your terminal taller. ",
            plural(hidden, "bind")
        ));
    }
    frame.render_widget(Clear, rect);
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}

fn width_of(columns: usize) -> u16 {
    u16::try_from(columns).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use sshpick_core::Key;

    use crate::tui::testing::{frame, ready, render, setup, terminal};

    fn open(picker: &mut sshpick_core::Picker) {
        assert_eq!(picker.handle(Key::F(1)), None);
        assert!(picker.help_open());
    }

    #[test]
    fn the_popup_sits_in_the_middle_of_the_frame_over_the_list() {
        let (screen, mut picker) = setup("ui", "");
        open(&mut picker);
        let lines = render(&mut terminal(80, 24), &screen, &mut picker, &ready());
        assert_snapshot!("popup_80x24", frame(&lines));
    }

    #[test]
    fn the_popup_crosses_the_side_pane_in_a_wide_frame() {
        let (screen, mut picker) = setup("ui", "consul");
        open(&mut picker);
        let lines = render(&mut terminal(120, 30), &screen, &mut picker, &ready());
        assert_snapshot!("popup_120x30", frame(&lines));
    }

    #[test]
    fn a_short_frame_cuts_the_rows_and_says_how_many_are_missing() {
        let (screen, mut picker) = setup("ui", "");
        open(&mut picker);
        let lines = render(&mut terminal(80, 12), &screen, &mut picker, &ready());
        assert_snapshot!("popup_80x12", frame(&lines));
        let (screen, mut picker) = setup("ui", "");
        open(&mut picker);
        let lines = render(&mut terminal(80, 13), &screen, &mut picker, &ready());
        assert!(
            lines[12].contains("1 more bind. Make your terminal taller."),
            "{lines:#?}"
        );
        let lines = render(&mut terminal(80, 14), &screen, &mut picker, &ready());
        assert!(
            !lines.iter().any(|line| line.contains("more bind")),
            "all ten fit"
        );
    }

    #[test]
    fn closing_the_help_brings_the_list_back() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 24);
        let plain = render(&mut terminal, &screen, &mut picker, &ready());
        open(&mut picker);
        let with_popup = render(&mut terminal, &screen, &mut picker, &ready());
        assert_ne!(plain, with_popup);
        assert_eq!(picker.handle(Key::Esc), None);
        assert_eq!(render(&mut terminal, &screen, &mut picker, &ready()), plain);
    }

    #[test]
    fn the_popup_is_drawn_over_the_whole_frame_whatever_its_size() {
        let (screen, mut picker) = setup("ui", "consul");
        open(&mut picker);
        let sizes = (0..=45)
            .map(|width| (width, 14))
            .chain([(99, 24), (100, 24), (140, 40)])
            .flat_map(|(width, most)| (0..=most).map(move |height| (width, height)));
        for (width, height) in sizes {
            let mut terminal = terminal(width, height);
            let lines = render(&mut terminal, &screen, &mut picker, &ready());
            assert!(
                lines
                    .iter()
                    .all(|line| line.chars().count() <= usize::from(width)),
                "{width}x{height}"
            );
        }
    }
}
