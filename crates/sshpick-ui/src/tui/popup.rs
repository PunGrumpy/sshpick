use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Padding, Paragraph};
use sshpick_core::BINDINGS;

use super::style::{DIM, MATCHED};
use crate::text::{columns, fit, plural};

const TITLE: &str = " Keybinds ";
const DASH: &str = " - ";
const WARNINGS_TITLE: &str = "Warnings";
// A border and a row of padding on each side.
const FRAME_COLUMNS: u16 = 4;
const FRAME_ROWS: u16 = 4;

/// The key table over the middle of the frame, with the config warnings under it. It is cleared
/// first, so the list underneath does not show through. When the frame is too short for every
/// row the bottom edge says how many are missing.
pub(super) fn popup(frame: &mut Frame, warnings: &[String]) {
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
    let warning_width = warnings.iter().map(|text| columns(text)).max().unwrap_or(0);
    let inner = bind_width.max(warning_width).max(columns(TITLE));

    let rows = binds.len()
        + if warnings.is_empty() {
            0
        } else {
            warnings.len() + 2
        };
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
    let text_width = usize::from(wide - FRAME_COLUMNS);
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
    if !warnings.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled(WARNINGS_TITLE, MATCHED));
        for text in warnings {
            let cut: String = fit(&[(text, false)], text_width)
                .iter()
                .map(|&(part, _)| part)
                .collect();
            lines.push(Line::raw(cut));
        }
    }
    let hidden_binds = binds.len().saturating_sub(room);
    let hidden = lines.len().saturating_sub(room);
    lines.truncate(room);

    let mut block = Block::bordered().title(TITLE).padding(Padding::uniform(1));
    if hidden > 0 {
        let (count, noun) = if hidden_binds > 0 {
            (hidden_binds, "bind")
        } else {
            (hidden, "line")
        };
        block = block.title_bottom(format!(
            " {count} more {}. Make your terminal taller. ",
            plural(count, noun)
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
    use ratatui::style::Modifier;
    use sshpick_core::{Key, Slot, Warning};

    use crate::tui::testing::{DARK, cell, frame, ready, render, setup, setup_with, terminal};

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
    fn config_warnings_are_listed_under_the_keys() {
        let warnings = [
            Warning::Cycle {
                path: "/home/me/.ssh/config.d/9-late.conf".into(),
            },
            Warning::BadInclude {
                arg: "${NOPE}/x".to_owned(),
                reason: "${NOPE} is not set".to_owned(),
            },
        ];
        let (screen, mut picker) = setup_with("ui", "", DARK, &warnings);
        open(&mut picker);
        let mut terminal = terminal(100, 30);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_snapshot!("popup_warnings_100x30", frame(&lines));
        assert!(
            (0..100).any(|x| cell(&terminal, x, 19).modifier.contains(Modifier::BOLD)),
            "the Warnings heading is bold: {lines:#?}"
        );
    }

    #[test]
    fn a_long_warning_is_cut_to_the_frame_not_the_other_way_round() {
        let warnings = [Warning::Cycle {
            path: format!("/home/me/{}.conf", "very-long-name-".repeat(8)).into(),
        }];
        let (screen, mut picker) = setup_with("ui", "", DARK, &warnings);
        open(&mut picker);
        let lines = render(&mut terminal(60, 30), &screen, &mut picker, &Slot::Waiting);
        assert!(lines.iter().all(|line| line.chars().count() <= 60));
        assert!(lines.iter().any(|line| line.contains("…")), "{lines:#?}");
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
