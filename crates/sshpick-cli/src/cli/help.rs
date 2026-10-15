use sshpick_ui::ColorConfig;

use super::spec::{
    COMMANDS, CommandSpec, ENVIRONMENT, EXIT_STATUS, Example, GLOBAL, OptionSpec, ROOT_EXAMPLES,
    Row, URL,
};

const INDENT: &str = "  ";
const LOGO: &str = "▌";
const NAME: &str = "sshpick";
pub(crate) const MIN_WIDTH: usize = 40;
pub(crate) const MAX_WIDTH: usize = 100;
const MIN_TEXT: usize = 10;
const MIN_SUMMARY: usize = 30;
// "  -h," : the shorthand column is this wide even when no row has a shorthand.
const SHORT_COLUMN: usize = 5;
const COMMAND_INDENT: &str = "    ";
const MAX_LISTED_ALIAS: usize = 3;

/// A help page as text: a pure function of the table, the width and the colour setting.
/// `topic` is a command, or the root page when `None`. Lines never end in a space.
pub(crate) fn render(topic: Option<&CommandSpec>, width: usize, colors: ColorConfig) -> String {
    let page = Page {
        width: width.clamp(MIN_WIDTH, MAX_WIDTH),
        colors,
    };
    let mut lines = match topic {
        Some(spec) => page.command(spec),
        None => page.root(),
    };
    lines.push(String::new());
    lines.join("\n")
}

struct Page {
    width: usize,
    colors: ColorConfig,
}

impl Page {
    fn root(&self) -> Vec<String> {
        let colors = self.colors;
        let mut lines = vec![
            concat!("sshpick ", env!("CARGO_PKG_VERSION")).to_owned(),
            String::new(),
            format!(
                "{INDENT}{LOGO} {} [options] [command | query ...]",
                colors.bold(NAME)
            ),
            String::new(),
            format!("{INDENT}For picker help, run `sshpick pick --help`"),
            String::new(),
        ];
        lines.extend(self.commands());
        lines.extend(self.options("Global Options", GLOBAL));
        lines.extend(self.rows("Environment", ENVIRONMENT));
        lines.extend(self.rows("Exit Status", EXIT_STATUS));
        lines.extend(self.examples(ROOT_EXAMPLES));
        lines.push(String::new());
        lines.push(self.heading("Documentation"));
        lines.push(String::new());
        lines.push(format!("{INDENT}{INDENT}{URL}"));
        lines
    }

    fn command(&self, spec: &CommandSpec) -> Vec<String> {
        let mut lines = vec![String::new(), self.synopsis(spec), String::new()];
        lines.extend(
            wrap(spec.description, self.width - INDENT.len())
                .into_iter()
                .map(|line| format!("{INDENT}{line}")),
        );
        lines.push(String::new());
        if !spec.options.is_empty() {
            lines.extend(self.options("Options", spec.options));
        }
        if !spec.keys.is_empty() {
            let keys: Vec<Row> = spec.keys.iter().map(|b| (b.keys, b.does)).collect();
            lines.extend(self.rows("Keys", &keys));
        }
        lines.extend(self.options("Global Options", GLOBAL));
        lines.extend(self.examples(spec.examples));
        lines
    }

    fn synopsis(&self, spec: &CommandSpec) -> String {
        let mut line = format!(
            "{INDENT}{LOGO} {} {}",
            self.colors.bold(NAME),
            self.colors.bold(spec.name)
        );
        for argument in spec.arguments {
            let dots = if argument.multiple { " ..." } else { "" };
            if argument.required {
                line.push_str(&format!(" {}{dots}", argument.name));
            } else {
                line.push_str(&format!(" [{}{dots}]", argument.name));
            }
        }
        if !spec.options.is_empty() {
            line.push_str(" [options]");
        }
        line
    }

    fn heading(&self, title: &str) -> String {
        format!("{INDENT}{}:", self.colors.dim(title))
    }

    /// Two blank lines follow every table.
    fn table(&self, title: &str, rows: Vec<String>) -> Vec<String> {
        let mut lines = vec![self.heading(title), String::new()];
        lines.extend(rows);
        lines.push(String::new());
        lines.push(String::new());
        lines
    }

    fn commands(&self) -> Vec<String> {
        let labels: Vec<String> = COMMANDS.iter().map(|spec| label(spec)).collect();
        let widest = labels.iter().map(String::len).max().unwrap_or(0);
        let text_indent = COMMAND_INDENT.len() + widest + 2;
        let room = self.width.saturating_sub(text_indent).max(MIN_SUMMARY);
        let mut rows = Vec::new();
        for (spec, label) in COMMANDS.iter().zip(&labels) {
            let summary = if spec.default {
                format!("{} (default)", spec.summary)
            } else {
                spec.summary.to_owned()
            };
            for (index, text) in wrap(&summary, room).into_iter().enumerate() {
                let text = text.replace("(default)", &self.colors.bold("(default)"));
                rows.push(if index == 0 {
                    format!("{COMMAND_INDENT}{label:<widest$}  {text}")
                } else {
                    format!("{}{text}", " ".repeat(text_indent))
                });
            }
        }
        self.table("Commands", rows)
    }

    fn options(&self, title: &str, options: &[OptionSpec]) -> Vec<String> {
        let mut shown: Vec<(&OptionSpec, &str)> = options
            .iter()
            .filter_map(|option| Some((option, option.description?)))
            .collect();
        shown.sort_by_key(|(option, _)| option.long);
        let cells: Vec<(String, String)> = shown
            .iter()
            .map(|(option, _)| {
                let short = option
                    .short
                    .map_or_else(String::new, |short| format!("{INDENT}-{short},"));
                (short, format!("{INDENT}--{}{INDENT}", option.long))
            })
            .collect();
        let long_column = cells.iter().map(|(_, long)| long.len()).max().unwrap_or(0);
        let room = self
            .width
            .saturating_sub(SHORT_COLUMN + long_column)
            .max(MIN_TEXT);
        let mut rows = Vec::new();
        for ((short, long), (_, description)) in cells.iter().zip(&shown) {
            for (index, text) in wrap(description, room).into_iter().enumerate() {
                let row = if index == 0 {
                    format!("{short:<SHORT_COLUMN$}{long:<long_column$}{text}")
                } else {
                    format!("{}{text}", " ".repeat(SHORT_COLUMN + long_column))
                };
                rows.push(row.trim_end().to_owned());
            }
        }
        self.table(title, rows)
    }

    fn rows(&self, title: &str, rows: &[Row]) -> Vec<String> {
        let label = rows
            .iter()
            .map(|(label, _)| INDENT.len() + label.len())
            .max()
            .unwrap_or(0)
            + 2;
        let room = self.width.saturating_sub(label).max(MIN_TEXT);
        let mut lines = Vec::new();
        for (name, description) in rows {
            for (index, text) in wrap(description, room).into_iter().enumerate() {
                lines.push(if index == 0 {
                    format!("{:<label$}{text}", format!("{INDENT}{name}"))
                } else {
                    format!("{}{text}", " ".repeat(label))
                });
            }
        }
        self.table(title, lines)
    }

    fn examples(&self, examples: &[Example]) -> Vec<String> {
        let mut lines = vec![self.heading("Examples"), String::new()];
        for (index, example) in examples.iter().enumerate() {
            if index > 0 {
                lines.push(String::new());
            }
            lines.push(format!(
                "{INDENT}{} {}",
                self.colors.gray("-"),
                example.name
            ));
            lines.push(String::new());
            lines.push(format!(
                "{INDENT}{INDENT}{}",
                self.colors.cyan(&format!("$ {}", example.command))
            ));
        }
        lines
    }
}

fn label(spec: &CommandSpec) -> String {
    let aliases = spec
        .aliases
        .iter()
        .filter(|alias| alias.len() <= MAX_LISTED_ALIAS);
    aliases
        .chain([&spec.name])
        .copied()
        .collect::<Vec<_>>()
        .join(" | ")
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(MIN_TEXT);
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(line) if line.chars().count() + 1 + word.chars().count() <= width => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}
