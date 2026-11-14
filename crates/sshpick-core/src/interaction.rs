/// What a pick does with the host it lands on, chosen by argv.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    /// Exec `ssh -- <host>`.
    Connect,
    /// Print the host on stdout and exit 0; ssh never runs.
    Print,
}

/// Where the picker draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Stdout,
    /// The controlling terminal, `/dev/tty`, so stdout carries only the result (fzf's way).
    Tty,
}

impl Finish {
    /// `Print` draws on the tty even when stdout is a terminal, so `$(sshpick --print)` and a
    /// person typing `sshpick --print` see the same picker.
    pub fn surface(self) -> Surface {
        match self {
            Finish::Connect => Surface::Stdout,
            Finish::Print => Surface::Tty,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interaction {
    pub non_interactive: bool,
    pub stdin_is_terminal: bool,
    /// The picker's [`Surface`] is a terminal: stdout for `Connect`, `/dev/tty` opened for
    /// `Print`.
    pub surface_is_terminal: bool,
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

impl Refusal {
    /// A clause that follows "and" or ";".
    pub fn why(self) -> &'static str {
        match self {
            Refusal::NonInteractive => "--non-interactive never opens the picker",
            Refusal::NoTerminal => "the picker needs an interactive terminal",
            Refusal::DumbTerminal => {
                "the picker needs a terminal that moves the cursor (TERM=dumb)"
            }
        }
    }
}

pub fn refusal(interaction: &Interaction) -> Option<Refusal> {
    if interaction.non_interactive {
        Some(Refusal::NonInteractive)
    } else if !(interaction.stdin_is_terminal && interaction.surface_is_terminal) {
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

    //          flag   stdin  surface dumb
    #[test_case(false, true, true, false, None; "a terminal opens the picker")]
    #[test_case(false, true, true, true, DUMB; "a dumb terminal does not")]
    #[test_case(false, true, false, false, NO_TERMINAL; "the surface is not a terminal")]
    #[test_case(false, true, false, true, NO_TERMINAL; "no surface outranks dumb")]
    #[test_case(false, false, true, false, NO_TERMINAL; "stdin piped")]
    #[test_case(false, false, true, true, NO_TERMINAL; "stdin piped outranks dumb")]
    #[test_case(false, false, false, false, NO_TERMINAL; "both piped")]
    #[test_case(false, false, false, true, NO_TERMINAL; "both piped and dumb")]
    #[test_case(true, true, true, false, NON_INTERACTIVE; "the flag on a terminal")]
    #[test_case(true, true, true, true, NON_INTERACTIVE; "the flag outranks dumb")]
    #[test_case(true, true, false, false, NON_INTERACTIVE; "the flag outranks no surface")]
    #[test_case(true, true, false, true, NON_INTERACTIVE; "the flag, no surface and dumb")]
    #[test_case(true, false, true, false, NON_INTERACTIVE; "the flag outranks stdin piped")]
    #[test_case(true, false, true, true, NON_INTERACTIVE; "the flag, stdin piped and dumb")]
    #[test_case(true, false, false, false, NON_INTERACTIVE; "the flag outranks both piped")]
    #[test_case(true, false, false, true, NON_INTERACTIVE; "everything at once")]
    fn the_first_reason_that_applies_is_the_refusal(
        non_interactive: bool,
        stdin_is_terminal: bool,
        surface_is_terminal: bool,
        dumb: bool,
        want: Option<Refusal>,
    ) {
        let interaction = Interaction {
            non_interactive,
            stdin_is_terminal,
            surface_is_terminal,
            dumb,
        };
        assert_eq!(refusal(&interaction), want);
    }

    #[test_case(Finish::Connect, Surface::Stdout; "connect draws on stdout")]
    #[test_case(Finish::Print, Surface::Tty; "print draws on the tty, so stdout holds only the host")]
    fn each_finish_draws_on_one_surface(finish: Finish, want: Surface) {
        assert_eq!(finish.surface(), want);
    }

    #[test_case(Refusal::NonInteractive, "--non-interactive never opens the picker"; "the flag")]
    #[test_case(Refusal::NoTerminal, "the picker needs an interactive terminal"; "no terminal")]
    #[test_case(Refusal::DumbTerminal, "the picker needs a terminal that moves the cursor (TERM=dumb)"; "dumb")]
    fn each_refusal_says_why(refusal: Refusal, want: &str) {
        assert_eq!(refusal.why(), want);
    }
}
