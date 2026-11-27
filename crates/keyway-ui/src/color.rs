use std::ffi::OsString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorConfig {
    color: bool,
    attributes: bool,
    reason: Reason,
}

/// The first of [`ColorConfig::new`]'s rules that applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    NoColorFlag,
    NoColorEnv,
    Forced,
    Dumb,
    Terminal,
    Pipe,
}

impl Reason {
    /// What `doctor` prints in parentheses. `doctor` reads stdout's decision, and the words say so.
    pub fn words(self) -> &'static str {
        match self {
            Reason::NoColorFlag => "--no-color",
            Reason::NoColorEnv => "NO_COLOR is set",
            Reason::Forced => "CLICOLOR_FORCE is set",
            Reason::Dumb => "TERM=dumb",
            Reason::Terminal => "stdout is a terminal",
            Reason::Pipe => "stdout is not a terminal",
        }
    }
}

impl ColorConfig {
    /// Decide for one stream, in order:
    ///
    /// 1. `--no-color` or a non-empty `NO_COLOR`: no hue.
    /// 2. A `CLICOLOR_FORCE` that is set, non-empty and not `0`: colour, even when piped.
    /// 3. `TERM=dumb`: no colour.
    /// 4. Colour exactly when the stream is a terminal.
    ///
    /// Bold, dim and reverse stay on a terminal with no hue, except under `TERM=dumb`. A pipe gets
    /// no escape bytes unless `CLICOLOR_FORCE` asked for them.
    pub fn new(
        is_terminal: bool,
        no_color_flag: bool,
        var: &dyn Fn(&str) -> Option<OsString>,
    ) -> ColorConfig {
        let set = |name: &str| var(name).is_some_and(|value| !value.is_empty());
        let no_hue = no_color_flag || set("NO_COLOR");
        let forced = var("CLICOLOR_FORCE").is_some_and(|value| !value.is_empty() && value != "0");
        let dumb = var("TERM").is_some_and(|value| value == "dumb");
        let on_terminal = is_terminal && !dumb;
        let color = !no_hue && (forced || on_terminal);
        let reason = if no_color_flag {
            Reason::NoColorFlag
        } else if set("NO_COLOR") {
            Reason::NoColorEnv
        } else if forced {
            Reason::Forced
        } else if dumb {
            Reason::Dumb
        } else if is_terminal {
            Reason::Terminal
        } else {
            Reason::Pipe
        };
        ColorConfig {
            color,
            attributes: color || on_terminal,
            reason,
        }
    }

    pub const fn none() -> ColorConfig {
        ColorConfig {
            color: false,
            attributes: false,
            reason: Reason::Pipe,
        }
    }

    pub fn color(self) -> bool {
        self.color
    }

    pub fn attributes(self) -> bool {
        self.attributes
    }

    pub fn reason(self) -> Reason {
        self.reason
    }

    pub fn bold(self, text: &str) -> String {
        self.paint(self.attributes, "1", "22", text)
    }

    pub fn dim(self, text: &str) -> String {
        self.paint(self.attributes, "2", "22", text)
    }

    pub fn cyan(self, text: &str) -> String {
        self.paint(self.color, "36", "39", text)
    }

    pub fn red(self, text: &str) -> String {
        self.paint(self.color, "31", "39", text)
    }

    pub fn yellow(self, text: &str) -> String {
        self.paint(self.color, "33", "39", text)
    }

    pub fn gray(self, text: &str) -> String {
        self.paint(self.color, "90", "39", text)
    }

    fn paint(self, enabled: bool, on: &str, off: &str, text: &str) -> String {
        if enabled && !text.is_empty() {
            format!("\x1b[{on}m{text}\x1b[{off}m")
        } else {
            text.to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    fn vars<'a>(set: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            set.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    #[test_case(true, false, &[], true, true, Reason::Terminal; "a terminal gets colour")]
    #[test_case(false, false, &[], false, false, Reason::Pipe; "a pipe gets nothing")]
    #[test_case(true, true, &[], false, true, Reason::NoColorFlag; "--no-color keeps the attributes on a terminal")]
    #[test_case(false, true, &[], false, false, Reason::NoColorFlag; "--no-color on a pipe")]
    #[test_case(true, false, &[("NO_COLOR", "1")], false, true, Reason::NoColorEnv; "NO_COLOR on a terminal")]
    #[test_case(true, false, &[("NO_COLOR", "")], true, true, Reason::Terminal; "an empty NO_COLOR is ignored")]
    #[test_case(false, false, &[("CLICOLOR_FORCE", "1")], true, true, Reason::Forced; "CLICOLOR_FORCE colours a pipe")]
    #[test_case(false, false, &[("CLICOLOR_FORCE", "0")], false, false, Reason::Pipe; "CLICOLOR_FORCE=0 is off")]
    #[test_case(false, false, &[("CLICOLOR_FORCE", "")], false, false, Reason::Pipe; "an empty CLICOLOR_FORCE is off")]
    #[test_case(false, false, &[("CLICOLOR_FORCE", "1"), ("NO_COLOR", "1")], false, false, Reason::NoColorEnv; "NO_COLOR beats CLICOLOR_FORCE")]
    #[test_case(true, true, &[("NO_COLOR", "1")], false, true, Reason::NoColorFlag; "the flag is named before NO_COLOR")]
    #[test_case(true, false, &[("TERM", "dumb")], false, false, Reason::Dumb; "a dumb terminal gets nothing")]
    #[test_case(false, false, &[("TERM", "dumb")], false, false, Reason::Dumb; "a dumb pipe is named for TERM")]
    #[test_case(false, false, &[("TERM", "dumb"), ("CLICOLOR_FORCE", "1")], true, true, Reason::Forced; "forcing beats a dumb terminal")]
    fn each_stream_decides_for_itself(
        is_terminal: bool,
        no_color_flag: bool,
        set: &[(&str, &str)],
        color: bool,
        attributes: bool,
        reason: Reason,
    ) {
        let config = ColorConfig::new(is_terminal, no_color_flag, &vars(set));
        assert_eq!(
            (config.color(), config.attributes(), config.reason()),
            (color, attributes, reason)
        );
    }

    #[test_case(Reason::NoColorFlag, "--no-color")]
    #[test_case(Reason::NoColorEnv, "NO_COLOR is set")]
    #[test_case(Reason::Forced, "CLICOLOR_FORCE is set")]
    #[test_case(Reason::Dumb, "TERM=dumb")]
    #[test_case(Reason::Terminal, "stdout is a terminal")]
    #[test_case(Reason::Pipe, "stdout is not a terminal")]
    fn a_reason_has_the_words_doctor_prints(reason: Reason, words: &str) {
        assert_eq!(reason.words(), words);
    }

    #[test]
    fn styles_wrap_the_text_only_where_the_stream_allows_them() {
        let all = ColorConfig::new(true, false, &vars(&[]));
        assert_eq!(all.bold("a"), "\x1b[1ma\x1b[22m");
        assert_eq!(all.dim("a"), "\x1b[2ma\x1b[22m");
        assert_eq!(all.cyan("a"), "\x1b[36ma\x1b[39m");
        assert_eq!(all.gray("a"), "\x1b[90ma\x1b[39m");
        assert_eq!(all.red("a"), "\x1b[31ma\x1b[39m");
        assert_eq!(all.yellow("a"), "\x1b[33ma\x1b[39m");
        assert_eq!(all.dim(""), "", "nothing to style, nothing to wrap");

        let no_hue = ColorConfig::new(true, true, &vars(&[]));
        assert_eq!(no_hue.bold("a"), "\x1b[1ma\x1b[22m");
        assert_eq!(no_hue.cyan("a"), "a");
        assert_eq!(no_hue.gray("a"), "a");
        assert_eq!(no_hue.red("a"), "a");

        let pipe = ColorConfig::new(false, false, &vars(&[]));
        for text in [
            pipe.bold("a"),
            pipe.dim("a"),
            pipe.cyan("a"),
            pipe.gray("a"),
        ] {
            assert_eq!(text, "a");
        }
    }
}
