//! Every look the picker draws is a [`Role`]. The rest of `tui` asks for a role and never builds a
//! `Style` or names a modifier. The neutrals come from the fixed xterm grey ramp (232–255) and the
//! accent is ANSI 4 with the terminal's own background as its text colour, so no look depends on
//! what a theme calls black or white.

use keyway_core::Run;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::ColorConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Role {
    /// The query and host names, and the keys of the help.
    Fg,
    /// WHAT, LAST and ENV of unselected rows, the route, the help's descriptions and the footer
    /// labels.
    Muted,
    /// The border and the rule above the footer.
    Stroke,
    /// Every cell of the selected row.
    Bar,
    /// The key of a footer hint.
    Keycap,
    /// The strip the whole prompt row is drawn on.
    Field,
    /// The query, and anything else the prompt row says, on the strip.
    Typed,
    /// What the strip says while the query is empty, and the host count at its right.
    Placeholder,
    /// Why `ssh -G` failed.
    Fail,
}

const BOLD: Modifier = Modifier::BOLD;

/// What a role looks like with colour and without it.
const fn looks(role: Role) -> (Style, Style) {
    match role {
        Role::Fg => (Style::new(), Style::new()),
        Role::Muted => (
            Style::new().fg(Color::Indexed(245)),
            Style::new().add_modifier(Modifier::DIM),
        ),
        Role::Stroke => (
            Style::new()
                .fg(Color::Indexed(245))
                .add_modifier(Modifier::DIM),
            Style::new().add_modifier(Modifier::DIM),
        ),
        Role::Bar => (
            Style::new()
                .fg(Color::Blue)
                .add_modifier(Modifier::REVERSED),
            Style::new().add_modifier(Modifier::REVERSED),
        ),
        Role::Keycap => (
            Style::new().bg(Color::Indexed(236)).fg(Color::Indexed(250)),
            Style::new().add_modifier(BOLD),
        ),
        Role::Field => (Style::new().bg(Color::Indexed(236)), Style::new()),
        Role::Typed => (
            Style::new().bg(Color::Indexed(236)).fg(Color::Indexed(255)),
            Style::new(),
        ),
        Role::Placeholder => (
            Style::new().bg(Color::Indexed(236)).fg(Color::Indexed(245)),
            Style::new().add_modifier(Modifier::DIM),
        ),
        Role::Fail => (Style::new().fg(Color::Red), Style::new()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Theme {
    color: bool,
}

impl Theme {
    /// `colors` is the decision for the stream the picker draws on.
    pub(super) fn new(colors: ColorConfig) -> Theme {
        Theme {
            color: colors.color(),
        }
    }

    pub(super) fn style(self, role: Role) -> Style {
        let (colour, plain) = looks(role);
        if self.color { colour } else { plain }
    }

    pub(super) fn span(self, text: impl Into<String>, role: Role) -> Span<'static> {
        Span::styled(text.into(), self.style(role))
    }

    /// `runs` in `base`. A matched run keeps the look of its text and is bold, never dim.
    pub(super) fn cell<'a>(self, runs: &[Run<'a>], base: Role) -> Line<'a> {
        runs.iter()
            .map(|run| {
                let style = self.style(base);
                let style = if run.matched {
                    style.add_modifier(BOLD).remove_modifier(Modifier::DIM)
                } else {
                    style
                };
                Span::styled(run.text, style)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    fn style(fg: Option<Color>, bg: Option<Color>, modifier: Modifier) -> Style {
        Style {
            fg,
            bg,
            add_modifier: modifier,
            ..Style::default()
        }
    }

    /// The grey ramp and ANSI 4, so the looks never ask what a theme calls black or white.
    #[test_case(Role::Fg, style(None, None, Modifier::empty()), Style::new() ; "fg")]
    #[test_case(
        Role::Muted,
        style(Some(Color::Indexed(245)), None, Modifier::empty()),
        style(None, None, Modifier::DIM) ; "muted"
    )]
    #[test_case(
        Role::Stroke,
        style(Some(Color::Indexed(245)), None, Modifier::DIM),
        style(None, None, Modifier::DIM) ; "stroke"
    )]
    #[test_case(
        Role::Bar,
        style(Some(Color::Blue), None, Modifier::REVERSED),
        style(None, None, Modifier::REVERSED) ; "bar"
    )]
    #[test_case(
        Role::Keycap,
        style(Some(Color::Indexed(250)), Some(Color::Indexed(236)), Modifier::empty()),
        style(None, None, Modifier::BOLD) ; "keycap"
    )]
    #[test_case(
        Role::Field,
        style(None, Some(Color::Indexed(236)), Modifier::empty()),
        Style::new() ; "field"
    )]
    #[test_case(
        Role::Typed,
        style(Some(Color::Indexed(255)), Some(Color::Indexed(236)), Modifier::empty()),
        Style::new() ; "typed"
    )]
    #[test_case(
        Role::Placeholder,
        style(Some(Color::Indexed(245)), Some(Color::Indexed(236)), Modifier::empty()),
        style(None, None, Modifier::DIM) ; "placeholder"
    )]
    #[test_case(
        Role::Fail,
        style(Some(Color::Red), None, Modifier::empty()),
        Style::new() ; "fail"
    )]
    fn each_role_has_a_colour_look_and_a_no_colour_look(role: Role, colour: Style, plain: Style) {
        assert_eq!(looks(role), (colour, plain));
    }
}
