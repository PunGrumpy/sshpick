//! The command table. Parsing, `--help`, the root command list and the typo candidates all read
//! these constants, so a command or flag added here shows up everywhere at once.

pub(crate) const URL: &str = "https://github.com/PunGrumpy/sshpick#readme";

pub(crate) type Row = (&'static str, &'static str);

pub(crate) struct CommandSpec {
    pub(crate) name: &'static str,
    pub(crate) aliases: &'static [&'static str],
    pub(crate) default: bool,
    pub(crate) summary: &'static str,
    pub(crate) description: &'static str,
    pub(crate) arguments: &'static [Argument],
    pub(crate) options: &'static [OptionSpec],
    pub(crate) keys: &'static [Row],
    pub(crate) examples: &'static [Example],
}

pub(crate) struct Argument {
    pub(crate) name: &'static str,
    pub(crate) required: bool,
    pub(crate) multiple: bool,
}

/// An option without a `description` still parses but is left out of help.
pub(crate) struct OptionSpec {
    pub(crate) long: &'static str,
    pub(crate) short: Option<char>,
    pub(crate) aliases: &'static [&'static str],
    pub(crate) description: Option<&'static str>,
}

pub(crate) struct Example {
    pub(crate) name: &'static str,
    pub(crate) command: &'static str,
}

pub(crate) const PICK: CommandSpec = CommandSpec {
    name: "pick",
    aliases: &[],
    default: true,
    summary: "Pick a host and ssh to it",
    description: "Pick a host from ~/.ssh/config and ssh to it. Exactly one match connects at \
                  once, without opening the picker.",
    arguments: &[Argument {
        name: "query",
        required: false,
        multiple: true,
    }],
    options: &[],
    keys: &[
        ("Enter", "ssh to the selected host"),
        ("Esc, Ctrl-C", "quit (exit 130)"),
        ("Up, Down, Ctrl-P, Ctrl-N", "move"),
        ("Ctrl-U", "clear the query"),
        ("Ctrl-W", "delete the last word"),
    ],
    examples: &[
        Example {
            name: "Open the picker with every host",
            command: "sshpick",
        },
        Example {
            name: "Open it filtered to hosts matching both words",
            command: "sshpick new consul",
        },
        Example {
            name: "Connect without the picker when one host matches",
            command: "sshpick stg-bastion",
        },
        Example {
            name: "Search for a word that is also a command name",
            command: "sshpick pick list",
        },
        Example {
            name: "Search for text that starts with a dash",
            command: "sshpick -- -bastion",
        },
    ],
};

pub(crate) const LIST: CommandSpec = CommandSpec {
    name: "list",
    aliases: &["ls"],
    default: false,
    summary: "List the hosts that match a query",
    description: "List the hosts that match a query.",
    arguments: &[Argument {
        name: "query",
        required: false,
        multiple: true,
    }],
    options: &[],
    keys: &[],
    examples: &[
        Example {
            name: "List every host",
            command: "sshpick list",
        },
        Example {
            name: "List the hosts matching both words",
            command: "sshpick list new consul",
        },
        Example {
            name: "Pipe the host table to another tool",
            command: "sshpick ls | fzf",
        },
        Example {
            name: "Print only the host names",
            command: "sshpick ls | cut -f2",
        },
    ],
};

pub(crate) const HELP: CommandSpec = CommandSpec {
    name: "help",
    aliases: &[],
    default: false,
    summary: "Show help for a command",
    description: "Show help for a command.",
    arguments: &[Argument {
        name: "command",
        required: false,
        multiple: false,
    }],
    options: &[],
    keys: &[],
    examples: &[Example {
        name: "Show the usage information for the command `list`",
        command: "sshpick help list",
    }],
};

pub(crate) const COMMANDS: &[&CommandSpec] = &[&PICK, &HELP, &LIST];

pub(crate) const GLOBAL: &[OptionSpec] = &[
    OptionSpec {
        long: "help",
        short: Some('h'),
        aliases: &[],
        description: Some("Output usage information"),
    },
    OptionSpec {
        long: "list",
        short: Some('l'),
        aliases: &[],
        description: None,
    },
    OptionSpec {
        long: "no-color",
        short: None,
        aliases: &[],
        description: Some("No color mode (default off)"),
    },
    OptionSpec {
        long: "non-interactive",
        short: None,
        aliases: &[],
        description: Some("Never open the picker; connect only when exactly one host matches"),
    },
    OptionSpec {
        long: "version",
        short: Some('V'),
        aliases: &["-v"],
        description: Some("Output the version number"),
    },
];

pub(crate) const ROOT_EXAMPLES: &[Example] = &[
    Example {
        name: "Open the picker with every host",
        command: "sshpick",
    },
    Example {
        name: "Open it filtered to hosts matching both words",
        command: "sshpick new consul",
    },
    Example {
        name: "Print the matching host names for a script",
        command: "sshpick ls new consul | cut -f2",
    },
    Example {
        name: "Search for a word that is also a command name",
        command: "sshpick pick list",
    },
];

pub(crate) const ENVIRONMENT: &[Row] = &[
    (
        "NO_COLOR",
        "No color; bold, dim and reverse video stay on a terminal",
    ),
    ("CLICOLOR_FORCE", "Keep color when output is not a terminal"),
];

pub(crate) const EXIT_STATUS: &[Row] = &[
    ("0", "Success; after a pick, ssh's own status"),
    ("1", "No host matches"),
    (
        "2",
        "Usage or config error; several matches where one is needed",
    ),
    ("127", "ssh could not be run"),
    ("130", "Cancelled"),
];

impl CommandSpec {
    pub(crate) fn matches(&self, word: &str) -> bool {
        self.name == word || self.aliases.contains(&word)
    }
}

impl OptionSpec {
    pub(crate) fn matches(&self, token: &str) -> bool {
        let long = token
            .strip_prefix("--")
            .is_some_and(|name| name == self.long);
        let short = token
            .strip_prefix('-')
            .is_some_and(|name| self.short.is_some_and(|short| name.chars().eq([short])));
        long || short || self.aliases.contains(&token)
    }
}

/// A command by name or alias.
pub(crate) fn command(word: &str) -> Option<&'static CommandSpec> {
    COMMANDS.iter().copied().find(|spec| spec.matches(word))
}

pub(crate) fn option(token: &str) -> Option<&'static OptionSpec> {
    GLOBAL.iter().find(|spec| spec.matches(token))
}

/// Every spelling of every command paired with the name it stands for, for typo suggestions.
pub(crate) fn command_words() -> Vec<(&'static str, &'static str)> {
    COMMANDS
        .iter()
        .flat_map(|spec| {
            spec.aliases
                .iter()
                .copied()
                .chain([spec.name])
                .map(|word| (word, spec.name))
        })
        .collect()
}

/// The options a command accepts, its own first and then the global ones, hidden ones included.
pub(crate) fn options_of(command: Option<&str>) -> Vec<&'static OptionSpec> {
    let own = command
        .and_then(command_by_name)
        .map_or(&[][..], |spec| spec.options);
    own.iter().chain(GLOBAL).collect()
}

fn command_by_name(name: &str) -> Option<&'static CommandSpec> {
    COMMANDS.iter().copied().find(|spec| spec.name == name)
}
