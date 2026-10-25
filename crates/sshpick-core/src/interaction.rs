#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interaction {
    pub non_interactive: bool,
    pub stdin_is_terminal: bool,
    pub stdout_is_terminal: bool,
    /// `TERM` is exactly `dumb`.
    pub dumb: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NonInteractive,
    NoTerminal,
    /// A dumb terminal promises no cursor movement, and the picker draws by moving the cursor.
    DumbTerminal,
}

pub fn refusal(interaction: &Interaction) -> Option<Refusal> {
    if interaction.non_interactive {
        Some(Refusal::NonInteractive)
    } else if !(interaction.stdin_is_terminal && interaction.stdout_is_terminal) {
        Some(Refusal::NoTerminal)
    } else if interaction.dumb {
        Some(Refusal::DumbTerminal)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    const NON_INTERACTIVE: Option<Refusal> = Some(Refusal::NonInteractive);
    const NO_TERMINAL: Option<Refusal> = Some(Refusal::NoTerminal);
    const DUMB: Option<Refusal> = Some(Refusal::DumbTerminal);

    //          flag   stdin  stdout dumb
    #[test_case(false, true, true, false, None; "a terminal opens the picker")]
    #[test_case(false, true, true, true, DUMB; "a dumb terminal does not")]
    #[test_case(false, true, false, false, NO_TERMINAL; "stdout piped")]
    #[test_case(false, true, false, true, NO_TERMINAL; "stdout piped outranks dumb")]
    #[test_case(false, false, true, false, NO_TERMINAL; "stdin piped")]
    #[test_case(false, false, true, true, NO_TERMINAL; "stdin piped outranks dumb")]
    #[test_case(false, false, false, false, NO_TERMINAL; "both piped")]
    #[test_case(false, false, false, true, NO_TERMINAL; "both piped and dumb")]
    #[test_case(true, true, true, false, NON_INTERACTIVE; "the flag on a terminal")]
    #[test_case(true, true, true, true, NON_INTERACTIVE; "the flag outranks dumb")]
    #[test_case(true, true, false, false, NON_INTERACTIVE; "the flag outranks stdout piped")]
    #[test_case(true, true, false, true, NON_INTERACTIVE; "the flag, stdout piped and dumb")]
    #[test_case(true, false, true, false, NON_INTERACTIVE; "the flag outranks stdin piped")]
    #[test_case(true, false, true, true, NON_INTERACTIVE; "the flag, stdin piped and dumb")]
    #[test_case(true, false, false, false, NON_INTERACTIVE; "the flag outranks both piped")]
    #[test_case(true, false, false, true, NON_INTERACTIVE; "everything at once")]
    fn the_first_reason_that_applies_is_the_refusal(
        non_interactive: bool,
        stdin_is_terminal: bool,
        stdout_is_terminal: bool,
        dumb: bool,
        want: Option<Refusal>,
    ) {
        let interaction = Interaction {
            non_interactive,
            stdin_is_terminal,
            stdout_is_terminal,
            dumb,
        };
        assert_eq!(refusal(&interaction), want);
    }
}
