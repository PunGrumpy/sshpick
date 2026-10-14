use std::ffi::OsString;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use sshpick_core::Warning;

use super::style::DIM;
use crate::text::{ELLIPSIS, columns, plural, truncate_to_width};

pub(super) const RULE: &str = "─";
const SEPARATOR: &str = " · ";

const HINTS: [(&str, &str); 4] = [
    ("Enter", "ssh"),
    ("Esc", "quit"),
    ("Ctrl-U", "clear"),
    ("↑↓", "move"),
];
const HINT_GAP: usize = 2;
const BAR_MARGINS: usize = 2;

type Vars<'a> = &'a dyn Fn(&str) -> Option<OsString>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Theme {
    Dark,
    Light,
}

#[derive(Debug)]
pub(super) struct Bar {
    style: Style,
    warnings: usize,
    first_warning: Option<String>,
}

impl Bar {
    pub(super) fn styled(warnings: &[Warning], style: Style) -> Bar {
        Bar {
            style,
            warnings: warnings.len(),
            first_warning: warnings.first().map(ToString::to_string),
        }
    }

    pub(super) fn style(&self) -> Style {
        self.style
    }

    pub(super) fn status(&self, frame: &mut Frame, area: Rect, total: usize) {
        let count = format!("{total} {}", plural(total, "host"));
        let label = format!("{} {}", self.warnings, plural(self.warnings, "warning"));
        let warning = self.first_warning.as_deref().map(|first| (&*label, first));
        let line = status_line(area.width, self.style, &count, warning);
        frame.render_widget(Paragraph::new(line).style(self.style), area);
    }
}

pub(super) fn rule(width: u16, title: &str, more: &str) -> Line<'static> {
    let width = usize::from(width);
    let more = if columns(title) + columns(more) <= width {
        more
    } else {
        ""
    };
    let fill = width.saturating_sub(columns(title) + columns(more));
    Line::styled(format!("{title}{}{more}", RULE.repeat(fill)), DIM)
}

fn status_line(
    width: u16,
    bar: Style,
    count: &str,
    warning: Option<(&str, &str)>,
) -> Line<'static> {
    let width = usize::from(width);
    let key = bar.add_modifier(Modifier::BOLD);
    let pair = |(name, verb): (&str, &str)| columns(name) + 1 + columns(verb);
    let hints_width = |shown: usize| {
        HINTS[..shown].iter().copied().map(pair).sum::<usize>() + HINT_GAP * shown.saturating_sub(1)
    };
    let shown = (0..=HINTS.len())
        .rev()
        .find(|&shown| BAR_MARGINS + hints_width(shown) + HINT_GAP + columns(count) <= width)
        .unwrap_or(0);

    let room = width.saturating_sub(BAR_MARGINS + hints_width(shown) + HINT_GAP);
    let status = match warning {
        Some((label, first))
            if room >= columns(count) + columns(SEPARATOR) + columns(label) + ELLIPSIS.len() =>
        {
            truncate_to_width(
                &format!("{count}{SEPARATOR}{label}{SEPARATOR}{first}"),
                room,
            )
        }
        _ => truncate_to_width(count, room),
    };

    let mut spans = vec![Span::styled(" ", bar)];
    for (index, &(name, verb)) in HINTS[..shown].iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" ".repeat(HINT_GAP), bar));
        }
        spans.push(Span::styled(name, key));
        spans.push(Span::styled(format!(" {verb}"), bar));
    }
    let gap = width.saturating_sub(BAR_MARGINS + hints_width(shown) + columns(&status));
    spans.push(Span::styled(" ".repeat(gap), bar));
    spans.push(Span::styled(status, bar));
    spans.push(Span::styled(" ", bar));
    Line::from(spans)
}

pub(super) fn bar_style(vars: Vars) -> Style {
    let colors_disabled = vars("NO_COLOR").is_some() && vars("CLICOLOR_FORCE").is_none();
    bar_for(theme(vars), colors_disabled)
}

fn bar_for(theme: Theme, colors_disabled: bool) -> Style {
    if colors_disabled {
        return Style::new().add_modifier(Modifier::REVERSED);
    }
    match theme {
        Theme::Dark => Style::new().fg(Color::Black).bg(Color::White),
        Theme::Light => Style::new().fg(Color::White).bg(Color::Black),
    }
}

fn theme(vars: Vars) -> Theme {
    let text = |name| vars(name).and_then(|value| value.into_string().ok());
    text("SSHPICK_THEME")
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "dark" => Some(Theme::Dark),
            "light" => Some(Theme::Light),
            _ => None,
        })
        .or_else(|| text("COLORFGBG").and_then(|value| theme_of_background(&value)))
        .unwrap_or(Theme::Dark)
}

fn theme_of_background(colorfgbg: &str) -> Option<Theme> {
    match colorfgbg.rsplit(';').next()?.parse::<u8>().ok()? {
        0..=6 | 8 => Some(Theme::Dark),
        7 | 9..=15 => Some(Theme::Light),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;
    use crate::tui::testing::{DARK, text, vars};

    fn status_text(width: u16, count: &str, warning: Option<(&str, &str)>) -> String {
        let line = status_line(width, DARK, count, warning);
        assert_eq!(line.width(), usize::from(width), "fills the bar exactly");
        text(&line)
    }

    #[test]
    fn the_theme_follows_the_override_then_colorfgbg_then_dark() {
        let theme = |set: &[(&str, &str)]| theme(&vars(set));
        assert_eq!(theme(&[]), Theme::Dark);
        assert_eq!(theme(&[("SSHPICK_THEME", "light")]), Theme::Light);
        assert_eq!(theme(&[("SSHPICK_THEME", " Dark ")]), Theme::Dark);
        assert_eq!(
            theme(&[("SSHPICK_THEME", "dark"), ("COLORFGBG", "0;15")]),
            Theme::Dark,
            "the override wins"
        );
        assert_eq!(
            theme(&[("SSHPICK_THEME", "solarized"), ("COLORFGBG", "0;15")]),
            Theme::Light,
            "an unknown override falls through"
        );
        assert_eq!(theme(&[("COLORFGBG", "0;15")]), Theme::Light);
        assert_eq!(theme(&[("COLORFGBG", "15;0")]), Theme::Dark);
        assert_eq!(theme(&[("COLORFGBG", "nonsense")]), Theme::Dark);
    }

    #[test]
    fn colorfgbg_background_picks_dark_or_light() {
        for background in (0..=6).chain([8]) {
            assert_eq!(
                theme_of_background(&format!("15;{background}")),
                Some(Theme::Dark),
                "{background}"
            );
        }
        for background in [7].into_iter().chain(9..=15) {
            assert_eq!(
                theme_of_background(&format!("0;{background}")),
                Some(Theme::Light),
                "{background}"
            );
        }
        assert_eq!(theme_of_background("0;default;15"), Some(Theme::Light));
        assert_eq!(theme_of_background("15;16"), None);
        assert_eq!(theme_of_background("15;default"), None);
        assert_eq!(theme_of_background(""), None);
    }

    #[test]
    fn colours_stay_off_under_no_color_unless_forced() {
        assert_eq!(bar_style(&vars(&[])), DARK);
        let plain = Style::new().add_modifier(Modifier::REVERSED);
        assert_eq!(bar_style(&vars(&[("NO_COLOR", "1")])), plain);
        assert_eq!(bar_style(&vars(&[("NO_COLOR", "")])), plain);
        assert_eq!(
            bar_style(&vars(&[("NO_COLOR", "1"), ("CLICOLOR_FORCE", "1")])),
            DARK
        );
    }

    #[test_case(53, " Enter ssh  Esc quit  Ctrl-U clear  ↑↓ move  5 hosts "; "all four hints")]
    #[test_case(52, " Enter ssh  Esc quit  Ctrl-U clear          5 hosts "; "the last hint goes first")]
    #[test_case(40, " Enter ssh  Esc quit            5 hosts "; "two hints")]
    #[test_case(28, " Enter ssh          5 hosts "; "one hint")]
    #[test_case(12, "    5 hosts "; "no hints")]
    fn the_status_bar_drops_hints_from_the_right(width: u16, want: &str) {
        assert_eq!(status_text(width, "5 hosts", None), want);
    }

    #[test]
    fn the_status_bar_truncates_the_first_warning_and_keeps_the_count() {
        let warning = Some(("2 warnings", "/home/me/.ssh/conf: skipped, include cycle"));
        let wide = status_text(120, "29 hosts", warning);
        assert!(
            wide.ends_with("29 hosts · 2 warnings · /home/me/.ssh/conf: skipped, include cycle "),
            "{wide:?}"
        );
        let cut = status_text(90, "29 hosts", warning);
        assert!(
            cut.ends_with("  29 hosts · 2 warnings · /home/me/.ssh/con... "),
            "{cut:?}"
        );
        assert!(cut.starts_with(" Enter ssh  Esc quit  Ctrl-U clear  ↑↓ move"));
        let narrow = status_text(69, "29 hosts", warning);
        assert!(
            narrow.ends_with("  29 hosts "),
            "no half a label: {narrow:?}"
        );
    }
}
