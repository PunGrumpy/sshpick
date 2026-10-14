use std::ffi::OsString;

/// What one output stream may carry: colours, and the bold, dim and reverse attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorConfig {
    color: bool,
    attributes: bool,
}

impl ColorConfig {
    /// Decide for one stream, in order:
    ///
    /// 1. `--no-color` or a non-empty `NO_COLOR`: no hue.
    /// 2. A `CLICOLOR_FORCE` that is set, non-empty and not `0`: colour, even when piped.
    /// 3. `TERM=dumb`: no colour.
    /// 4. Colour exactly when the stream is a terminal.
    ///
    /// Bold, dim and reverse stay on a terminal even with no hue. A pipe gets no escape bytes at
    /// all unless `CLICOLOR_FORCE` asked for them.
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
        ColorConfig {
            color,
            attributes: color || on_terminal,
        }
    }

    pub fn color(self) -> bool {
        self.color
    }

    pub fn attributes(self) -> bool {
        self.attributes
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

    #[test_case(true, false, &[], true, true; "a terminal gets colour")]
    #[test_case(false, false, &[], false, false; "a pipe gets nothing")]
    #[test_case(true, true, &[], false, true; "--no-color keeps the attributes on a terminal")]
    #[test_case(false, true, &[], false, false; "--no-color on a pipe")]
    #[test_case(true, false, &[("NO_COLOR", "1")], false, true; "NO_COLOR on a terminal")]
    #[test_case(true, false, &[("NO_COLOR", "")], true, true; "an empty NO_COLOR is ignored")]
    #[test_case(false, false, &[("CLICOLOR_FORCE", "1")], true, true; "CLICOLOR_FORCE colours a pipe")]
    #[test_case(false, false, &[("CLICOLOR_FORCE", "0")], false, false; "CLICOLOR_FORCE=0 is off")]
    #[test_case(false, false, &[("CLICOLOR_FORCE", "")], false, false; "an empty CLICOLOR_FORCE is off")]
    #[test_case(false, false, &[("CLICOLOR_FORCE", "1"), ("NO_COLOR", "1")], false, false; "NO_COLOR beats CLICOLOR_FORCE")]
    #[test_case(true, false, &[("TERM", "dumb")], false, false; "a dumb terminal gets nothing")]
    #[test_case(false, false, &[("TERM", "dumb"), ("CLICOLOR_FORCE", "1")], true, true; "forcing beats a dumb terminal")]
    fn each_stream_decides_for_itself(
        is_terminal: bool,
        no_color_flag: bool,
        set: &[(&str, &str)],
        color: bool,
        attributes: bool,
    ) {
        let config = ColorConfig::new(is_terminal, no_color_flag, &vars(set));
        assert_eq!((config.color(), config.attributes()), (color, attributes));
    }
}
