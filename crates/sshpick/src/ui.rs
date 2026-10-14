use std::ffi::OsString;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{HighlightSpacing, Paragraph, Row as TableRow, Table, TableState, Wrap};
use sshpick_core::{Entry, Host, Picker, Slot, View, Warning};

const MIN_HEIGHT_WITH_FRAME: u16 = 7;
const MIN_HEIGHT_WITH_STACKED_PREVIEW: u16 = 13;
const SIDE_PANE_MIN_WIDTH: u16 = 100;
const SIDE_PANE_WIDTH: u16 = 40;
// A space, the bar, a space.
const DIVIDER: &str = " │";
const DIVIDER_WIDTH: u16 = 3;
const STACKED_ROWS: u16 = 5;
const STACKED_INDENT: u16 = 2;

const KEY_WIDTH: usize = 14;

const TITLES: [&str; 3] = ["ENV", "HOST", "WHAT"];

const MARKER: &str = "▌ ";
const GUTTER: &str = "  ";
const COLUMN_SPACING: u16 = 1;

const PLACEHOLDER: &str = "type to filter";
const NO_MATCH: &str = "no match";
const RULE: &str = "─";
const CUT: &str = "…";
const SEPARATOR: &str = " · ";

const HINTS: [(&str, &str); 4] = [
    ("Enter", "ssh"),
    ("Esc", "quit"),
    ("Ctrl-U", "clear"),
    ("↑↓", "move"),
];
const HINT_GAP: usize = 2;
const BAR_MARGINS: usize = 2;
const ELLIPSIS: &str = "...";

const MATCHED: Style = Style::new().add_modifier(Modifier::BOLD);
const DIM: Style = Style::new().add_modifier(Modifier::DIM);

type Vars<'a> = &'a dyn Fn(&str) -> Option<OsString>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Theme {
    Dark,
    Light,
}

#[derive(Debug)]
pub struct Screen {
    env: u16,
    host: u16,
    bar: Style,
    warnings: usize,
    first_warning: Option<String>,
}

impl Screen {
    pub fn new(entries: &[Entry], warnings: &[Warning]) -> Screen {
        let bar = bar_style(&|name| std::env::var_os(name));
        Screen::styled(entries, warnings, bar)
    }

    fn styled(entries: &[Entry], warnings: &[Warning], bar: Style) -> Screen {
        let widest = |text: fn(&Entry) -> &str| {
            entries
                .iter()
                .map(|entry| width(text(entry)))
                .max()
                .unwrap_or(0)
        };
        let env = widest(|entry| &entry.env);
        Screen {
            env: if env == 0 {
                0
            } else {
                env.max(width(TITLES[0]))
            },
            host: widest(|entry| entry.host.as_str()).max(width(TITLES[1])),
            bar,
            warnings: warnings.len(),
            first_warning: warnings.first().map(ToString::to_string),
        }
    }

    pub fn draw(&self, frame: &mut Frame, picker: &mut Picker, pane: Option<(&Host, &Slot)>) {
        let area = frame.area();
        let framed = area.height >= MIN_HEIGHT_WITH_FRAME;
        let side = framed && area.width >= SIDE_PANE_MIN_WIDTH;
        let stacked = framed && !side && area.height >= MIN_HEIGHT_WITH_STACKED_PREVIEW;
        let [content, status] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        let [prompt, top_rule, header, list, bottom_rule, stacked_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(u16::from(framed)),
            Constraint::Length(u16::from(framed)),
            Constraint::Fill(1),
            Constraint::Length(u16::from(framed)),
            Constraint::Length(if stacked { STACKED_ROWS } else { 0 }),
        ])
        .areas(content);
        let split = |area| {
            let [left, right] = Layout::horizontal([
                Constraint::Fill(1),
                Constraint::Length(if side { SIDE_PANE_WIDTH } else { 0 }),
            ])
            .areas(area);
            (left, right)
        };
        let (header, header_side) = split(header);
        let (list, list_side) = split(list);

        let view = picker.view(list.height.into());
        prompt_row(frame, prompt, &view);
        self.list(frame, list, &view);
        if framed {
            self.header(frame, header);
            frame.render_widget(rule(top_rule.width, "", ""), top_rule);
            let stacked_title = pane
                .filter(|_| stacked)
                .map(|(host, slot)| format!("{RULE} {} ", title(host, slot)));
            let more = (view.below > 0).then(|| format!(" {} more below {RULE}", view.below));
            let line = rule(
                bottom_rule.width,
                stacked_title.as_deref().unwrap_or_default(),
                more.as_deref().unwrap_or_default(),
            );
            frame.render_widget(line, bottom_rule);
        }
        if side {
            side_pane(frame, header_side.union(list_side), pane);
        }
        if stacked {
            stacked_pane(frame, stacked_area, pane);
        }
        self.status(frame, status, view.total);
    }

    fn list(&self, frame: &mut Frame, area: Rect, view: &View<'_>) {
        if view.matched == 0 {
            if view.total > 0 {
                frame.render_widget(Line::styled(format!("{GUTTER}{NO_MATCH}"), DIM), area);
            }
            return;
        }
        let what_width = self.what_width(area.width);
        let table_rows = view.rows.iter().map(|row| {
            let what = fit(&row.what, what_width);
            let columns = [
                (self.env > 0).then(|| cell(&row.env, DIM)),
                Some(cell(&row.host, Style::new())),
                Some(cell(&what, DIM)),
            ];
            TableRow::new(columns.into_iter().flatten())
        });
        let table = Table::new(table_rows, self.widths())
            .column_spacing(COLUMN_SPACING)
            .highlight_symbol(MARKER)
            .highlight_spacing(HighlightSpacing::Always)
            .row_highlight_style(self.bar.remove_modifier(Modifier::DIM));
        let selected = view.rows.iter().position(|row| row.selected);
        frame.render_stateful_widget(table, area, &mut TableState::new().with_selected(selected));
    }

    fn header(&self, frame: &mut Frame, area: Rect) {
        let titles = TITLES.into_iter().skip(usize::from(self.env == 0));
        let table = Table::new(Vec::<TableRow>::new(), self.widths())
            .header(TableRow::new(titles).style(DIM))
            .column_spacing(COLUMN_SPACING)
            .highlight_symbol(GUTTER)
            .highlight_spacing(HighlightSpacing::Always);
        frame.render_widget(table, area);
    }

    fn widths(&self) -> Vec<Constraint> {
        let env = (self.env > 0).then_some(Constraint::Length(self.env));
        env.into_iter()
            .chain([Constraint::Length(self.host), Constraint::Fill(1)])
            .collect()
    }

    // The Table resolves its columns the same way; the cut must land on the column's edge.
    fn what_width(&self, total: u16) -> usize {
        let [_, columns] =
            Layout::horizontal([Constraint::Length(width(MARKER)), Constraint::Fill(0)])
                .areas(Rect::new(0, 0, total, 1));
        Layout::horizontal(self.widths())
            .spacing(COLUMN_SPACING)
            .split(columns)
            .last()
            .map_or(0, |what| usize::from(what.width))
    }

    fn status(&self, frame: &mut Frame, area: Rect, total: usize) {
        let count = format!("{total} {}", plural(total, "host"));
        let label = format!("{} {}", self.warnings, plural(self.warnings, "warning"));
        let warning = self.first_warning.as_deref().map(|first| (&*label, first));
        let line = status_line(area.width, self.bar, &count, warning);
        frame.render_widget(Paragraph::new(line).style(self.bar), area);
    }
}

fn width(text: &str) -> u16 {
    u16::try_from(columns(text)).unwrap_or(u16::MAX)
}

fn columns(text: &str) -> usize {
    Line::from(text).width()
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        noun.to_owned()
    } else {
        format!("{noun}s")
    }
}

fn prompt_row(frame: &mut Frame, area: Rect, view: &View<'_>) {
    let count = format!("{}/{}", view.matched, view.total);
    let [marker, rest] =
        Layout::horizontal([Constraint::Length(2), Constraint::Fill(1)]).areas(area);
    let [input, tally] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(width(&count) + 1)])
            .areas(rest);
    frame.render_widget(Span::styled(">", MATCHED), marker);
    frame.render_widget(Line::styled(count, DIM).right_aligned(), tally);
    let typed = width(view.query);
    let scroll = typed.saturating_sub(input.width.saturating_sub(1));
    if view.query.is_empty() {
        frame.render_widget(Span::styled(PLACEHOLDER, DIM), input);
    } else {
        frame.render_widget(Paragraph::new(view.query).scroll((0, scroll)), input);
    }
    frame.set_cursor_position((input.x.saturating_add(typed - scroll), input.y));
}

fn rule(width: u16, title: &str, more: &str) -> Line<'static> {
    let width = usize::from(width);
    let more = if columns(title) + columns(more) <= width {
        more
    } else {
        ""
    };
    let fill = width.saturating_sub(columns(title) + columns(more));
    Line::styled(format!("{title}{}{more}", RULE.repeat(fill)), DIM)
}

fn title(host: &Host, slot: &Slot) -> String {
    let failed = if matches!(slot, Slot::Failed(_)) {
        " failed"
    } else {
        ""
    };
    format!("ssh -G {host}{failed}")
}

fn stacked_pane(frame: &mut Frame, area: Rect, pane: Option<(&Host, &Slot)>) {
    let [_, body] =
        Layout::horizontal([Constraint::Length(STACKED_INDENT), Constraint::Fill(1)]).areas(area);
    if let Some((_, slot)) = pane {
        pane_body(frame, body, slot, None);
    }
}

fn side_pane(frame: &mut Frame, area: Rect, pane: Option<(&Host, &Slot)>) {
    let [divider, content] =
        Layout::horizontal([Constraint::Length(DIVIDER_WIDTH), Constraint::Fill(1)]).areas(area);
    let bars = vec![Line::styled(DIVIDER, DIM); usize::from(area.height)];
    frame.render_widget(Paragraph::new(bars), divider);
    let Some((host, slot)) = pane else {
        return;
    };
    let [heading, body] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(content);
    let heading_text = title(host, slot);
    let heading_runs = fit(&[(&heading_text, false)], usize::from(heading.width));
    frame.render_widget(cell(&heading_runs, DIM), heading);
    let room = usize::from(body.width).saturating_sub(KEY_WIDTH);
    pane_body(frame, body, slot, Some(room));
}

fn pane_body(frame: &mut Frame, area: Rect, slot: &Slot, room: Option<usize>) {
    match slot {
        Slot::Waiting => {}
        Slot::Ready(preview) => {
            frame.render_widget(Paragraph::new(key_values(preview.lines(), room)), area);
        }
        Slot::Failed(error) => {
            let text = Paragraph::new(error.to_string()).wrap(Wrap { trim: false });
            frame.render_widget(text, area);
        }
    }
}

fn key_values<'a>(
    lines: impl Iterator<Item = (&'static str, &'a str)>,
    room: Option<usize>,
) -> Text<'a> {
    lines
        .map(|(key, value)| {
            let gap = KEY_WIDTH.saturating_sub(columns(key)).max(1);
            let key = format!("{key}{}", " ".repeat(gap));
            let value = match room {
                Some(room) => fit(&[(value, false)], room),
                None => vec![(value, false)],
            };
            let mut spans = vec![Span::styled(key, DIM)];
            spans.extend(value.into_iter().map(|(text, _)| Span::raw(text)));
            Line::from(spans)
        })
        .collect()
}

fn cell<'a>(runs: &[(&'a str, bool)], base: Style) -> Line<'a> {
    runs.iter()
        .map(|&(text, matched)| Span::styled(text, if matched { MATCHED } else { base }))
        .collect()
}

fn fit<'a>(runs: &[(&'a str, bool)], limit: usize) -> Vec<(&'a str, bool)> {
    if runs.iter().map(|&(text, _)| columns(text)).sum::<usize>() <= limit {
        return runs.to_vec();
    }
    let Some(budget) = limit.checked_sub(columns(CUT)) else {
        return Vec::new();
    };
    let mut kept = Vec::new();
    let mut used = 0;
    'runs: for &(text, matched) in runs {
        for (start, c) in text.char_indices() {
            let end = start + c.len_utf8();
            let cells = columns(&text[start..end]);
            if used + cells > budget {
                if start > 0 {
                    kept.push((&text[..start], matched));
                }
                break 'runs;
            }
            used += cells;
        }
        kept.push((text, matched));
    }
    kept.push((CUT, false));
    kept
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

fn truncate_to_width(text: &str, limit: usize) -> String {
    if columns(text) <= limit {
        return text.to_owned();
    }
    let dotted = limit > ELLIPSIS.len();
    let keep = if dotted {
        limit - ELLIPSIS.len()
    } else {
        limit
    };
    let mut cut = String::new();
    let mut used = 0;
    for c in text.chars() {
        used += columns(c.encode_utf8(&mut [0; 4]));
        if used > keep {
            break;
        }
        cut.push(c);
    }
    if dotted {
        cut.push_str(ELLIPSIS);
    }
    cut
}

fn bar_style(vars: Vars) -> Style {
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
    use std::path::{Path, PathBuf};

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Cell;
    use sshpick_core::{Key, Preview, ResolveError};

    use super::*;

    const DARK: Style = Style::new().fg(Color::Black).bg(Color::White);
    const LIGHT: Style = Style::new().fg(Color::White).bg(Color::Black);

    fn entries(home: &str) -> Vec<Entry> {
        let home = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(home);
        sshpick_core::load(&home).unwrap().entries
    }

    fn setup(home: &str, query: &str) -> (Screen, Picker) {
        setup_with(home, query, DARK, &[])
    }

    fn setup_with(home: &str, query: &str, bar: Style, warnings: &[Warning]) -> (Screen, Picker) {
        let entries = entries(home);
        (
            Screen::styled(&entries, warnings, bar),
            Picker::new(entries, query),
        )
    }

    fn terminal(width: u16, height: u16) -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(width, height)).unwrap()
    }

    fn render(
        terminal: &mut Terminal<TestBackend>,
        screen: &Screen,
        picker: &mut Picker,
        slot: &Slot,
    ) -> Vec<String> {
        let selected = picker.selected().cloned();
        let pane = selected.as_ref().map(|host| (host, slot));
        terminal
            .draw(|frame| screen.draw(frame, picker, pane))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                let line: String = (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect();
                line.trim_end().to_owned()
            })
            .collect()
    }

    fn cell(terminal: &Terminal<TestBackend>, x: u16, y: u16) -> Cell {
        terminal.backend().buffer()[(x, y)].clone()
    }

    fn inverted(cell: &Cell, bar: Style) -> bool {
        Some(cell.fg) == bar.fg && Some(cell.bg) == bar.bg
    }

    fn vars<'a>(set: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            set.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    fn text(line: &Line) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    fn status_text(width: u16, count: &str, warning: Option<(&str, &str)>) -> String {
        let line = status_line(width, DARK, count, warning);
        assert_eq!(line.width(), usize::from(width), "fills the bar exactly");
        text(&line)
    }

    fn ready() -> Slot {
        Slot::Ready(Preview::parse(
            "user ubuntu\nhostname 10.11.212.191\nport 22\nproxyjump newstg-bastion\n",
        ))
    }

    fn beside(left: &str, right: &str) -> String {
        format!("{left:<60} │ {right}").trim_end().to_owned()
    }

    fn key_row(key: &str, value: &str) -> String {
        format!("{key:<14}{value}")
    }

    fn bar_line(width: usize) -> String {
        format!(
            " Enter ssh  Esc quit  Ctrl-U clear  ↑↓ move{}5 hosts",
            " ".repeat(width - 2 - 42 - 7)
        )
    }

    fn rule_line(width: usize) -> String {
        "─".repeat(width)
    }

    fn prompt_line(left: &str, count: &str, width: usize) -> String {
        format!("{left:<w$}{count}", w = width - count.len())
    }

    #[test]
    fn rows_list_top_down_under_the_header_and_the_count_sits_on_the_prompt() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            lines[..9],
            [
                prompt_line("> type to filter", "5/5", 60),
                rule_line(60),
                "  ENV         HOST           WHAT".to_owned(),
                "▌ LAB         lab-a          LAB — personal VMs".to_owned(),
                "  LAB         lab-b          build box".to_owned(),
                "  NEW STAGING newstg-consul1 consul server 1 — mgmt".to_owned(),
                "  NEW STAGING newstg-consul2".to_owned(),
                "  NEW STAGING newstg-bastion bastion".to_owned(),
                String::new(),
            ][..]
        );
        assert!(lines[9..13].iter().all(String::is_empty), "{lines:#?}");
        assert_eq!(lines[13], format!("─ ssh -G lab-a {}", "─".repeat(60 - 15)));
        assert!(lines[14..19].iter().all(String::is_empty), "{lines:#?}");
        assert_eq!(lines[19], bar_line(60));
        assert!((0..60).all(|x| inverted(&cell(&terminal, x, 3), DARK)));
        assert!((0..60).all(|x| !inverted(&cell(&terminal, x, 4), DARK)));
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    #[test]
    fn env_column_hides_when_no_host_has_a_label() {
        let (screen, mut picker) = setup("flat", "");
        let lines = render(&mut terminal(40, 14), &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            lines[..6],
            [
                prompt_line("> type to filter", "2/2", 40),
                rule_line(40),
                "  HOST   WHAT".to_owned(),
                "▌ root-a reachable from anywhere".to_owned(),
                "  root-b".to_owned(),
                String::new(),
            ][..]
        );
        assert_eq!(lines[7], format!("─ ssh -G root-a {}", "─".repeat(40 - 16)));
    }

    #[test]
    fn narrowing_the_query_leaves_no_stale_rows() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        let all = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(all[3..8].iter().filter(|line| !line.is_empty()).count(), 5);

        for c in "'lab-b".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[0], prompt_line("> 'lab-b", "1/5", 60));
        assert_eq!(lines[3], "▌ LAB         lab-b          build box");
        assert!(lines[4..13].iter().all(String::is_empty), "{lines:#?}");
        assert_eq!(lines[13], format!("─ ssh -G lab-b {}", "─".repeat(60 - 15)));
    }

    #[test]
    fn the_pane_never_shows_another_hosts_lines() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        let failed = Slot::Failed(ResolveError::Timeout);
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert_eq!(
            lines[13],
            format!("─ ssh -G lab-a failed {}", "─".repeat(60 - 22))
        );
        assert_eq!(lines[14], "  ssh -G gave no answer within 2 s");

        picker.handle(Key::Down);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[4], "▌ LAB         lab-b          build box");
        assert_eq!(lines[13], format!("─ ssh -G lab-b {}", "─".repeat(60 - 15)));
        assert!(lines[14..19].iter().all(String::is_empty), "{lines:#?}");

        for c in "zzzz".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[0], prompt_line("> zzzz", "0/5", 60));
        assert_eq!(lines[13], rule_line(60));
        assert!(lines[14..19].iter().all(String::is_empty), "{lines:#?}");
    }

    #[test]
    fn a_failed_preview_wraps_and_stops_at_five_rows() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(40, 20);
        let stderr = "word ".repeat(60);
        let failed = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr,
        });
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert!(lines[14..19].iter().all(|line| line.starts_with("  word")));
        assert!(lines[19].starts_with(" Enter ssh"), "{:?}", lines[19]);
    }

    #[test]
    fn preview_rows_pad_keys_dim_and_keep_a_gap_after_long_ones() {
        let rows = key_values(
            [
                ("user", "ubuntu"),
                ("remotecommand", "sudo ssh -t host"),
                ("proxyjump", "bastion"),
            ]
            .into_iter(),
            None,
        );
        let lines: Vec<String> = rows.lines.iter().map(text).collect();
        assert_eq!(
            lines,
            [
                "user          ubuntu",
                "remotecommand sudo ssh -t host",
                "proxyjump     bastion"
            ]
        );
        for line in &rows.lines {
            assert_eq!(line.spans[0].style, DIM);
            assert_eq!(line.spans[1].style, Style::default());
        }
    }

    #[test]
    fn preview_values_wider_than_the_room_end_in_an_ellipsis() {
        let rows = key_values(
            [
                ("user", "ubuntu"),
                ("proxyjump", "a-very-long-bastion-name"),
            ]
            .into_iter(),
            Some(10),
        );
        let lines: Vec<String> = rows.lines.iter().map(text).collect();
        assert_eq!(lines, ["user          ubuntu", "proxyjump     a-very-lo…"]);
    }

    #[test]
    fn matched_characters_are_bold_not_coloured_and_the_cursor_follows_the_query() {
        let (screen, mut picker) = setup("ui", "'lab-b");
        let mut terminal = terminal(60, 20);
        render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        let host = cell(&terminal, 14, 3);
        assert_eq!(host.symbol(), "l");
        assert!(host.modifier.contains(Modifier::BOLD));
        assert!(inverted(&host, DARK), "bold survives on the selected bar");
        let env = cell(&terminal, 2, 3);
        assert_eq!(env.symbol(), "L");
        assert!(!env.modifier.contains(Modifier::BOLD), "LAB is not matched");
        terminal.backend_mut().assert_cursor_position((2 + 6, 0));

        let buffer = terminal.backend().buffer();
        assert!(
            buffer.content().iter().all(|cell| cell.fg != Color::Green),
            "no hue is used for matches"
        );
    }

    #[test]
    fn widths_count_display_columns_not_chars() {
        let entries = entries("ui");
        let screen = Screen::styled(&entries, &[], DARK);
        assert_eq!((screen.env, screen.host), (11, 14));
        assert_eq!(
            Screen::styled(&[], &[], DARK).host,
            4,
            "never narrower than its title"
        );
    }

    #[test]
    fn dark_100x16_consul_snapshot_has_the_pane_beside_the_list() {
        let (screen, mut picker) = setup("ui", "consul");
        let mut terminal = terminal(100, 16);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        let mut want = vec![
            prompt_line("> consul", "2/5", 100),
            rule_line(100),
            beside("  ENV         HOST           WHAT", "ssh -G newstg-consul1"),
            beside(
                "▌ NEW STAGING newstg-consul1 consul server 1 — mgmt",
                &key_row("user", "ubuntu"),
            ),
            beside(
                "  NEW STAGING newstg-consul2",
                &key_row("hostname", "10.11.212.191"),
            ),
            beside("", &key_row("port", "22")),
            beside("", &key_row("proxyjump", "newstg-bastion")),
        ];
        want.extend((0..7).map(|_| beside("", "")));
        want.push(rule_line(100));
        want.push(bar_line(100));
        assert_eq!(lines, want);

        assert!((0..60).all(|x| inverted(&cell(&terminal, x, 3), DARK)));
        assert!((0..60).all(|x| !cell(&terminal, x, 3).modifier.contains(Modifier::DIM)));
        assert!((60..100).all(|x| !inverted(&cell(&terminal, x, 3), DARK)));
        assert!((0..100).all(|x| !inverted(&cell(&terminal, x, 4), DARK)));
        for y in 2..14 {
            assert_eq!(cell(&terminal, 61, y).symbol(), "│", "{y}");
            assert!(cell(&terminal, 61, y).modifier.contains(Modifier::DIM));
        }
        assert_eq!(cell(&terminal, 61, 1).symbol(), "─", "the rule is not cut");
        assert_eq!(cell(&terminal, 61, 14).symbol(), "─");

        let env = cell(&terminal, 2, 4);
        assert!(env.modifier.contains(Modifier::DIM), "ENV is context");
        let host = cell(&terminal, 14, 4);
        assert!(!host.modifier.contains(Modifier::DIM), "HOST is the thing");
        let matched = cell(&terminal, 14 + 7, 4);
        assert_eq!(matched.symbol(), "c");
        assert!(matched.modifier.contains(Modifier::BOLD));
        assert!(!cell(&terminal, 14 + 6, 4).modifier.contains(Modifier::BOLD));
        assert!(
            cell(&terminal, 2, 2).modifier.contains(Modifier::DIM),
            "header"
        );
        let title = cell(&terminal, 63, 2);
        assert_eq!(title.symbol(), "s");
        assert!(title.modifier.contains(Modifier::DIM), "pane title");
        let key = cell(&terminal, 63, 3);
        assert_eq!(key.symbol(), "u");
        assert!(key.modifier.contains(Modifier::DIM), "pane key");
        let value = cell(&terminal, 63 + 14, 3);
        assert_eq!(value.symbol(), "u");
        assert!(!value.modifier.contains(Modifier::DIM), "pane value");

        assert!(
            cell(&terminal, 0, 0).modifier.contains(Modifier::BOLD),
            "prompt"
        );
        assert!(!cell(&terminal, 2, 0).modifier.contains(Modifier::BOLD));
        let count = cell(&terminal, 97, 0);
        assert_eq!(count.symbol(), "2");
        assert!(count.modifier.contains(Modifier::DIM), "count");
        assert!(
            cell(&terminal, 50, 1).modifier.contains(Modifier::DIM),
            "top rule"
        );
        assert!(
            cell(&terminal, 50, 14).modifier.contains(Modifier::DIM),
            "bottom rule"
        );
        terminal.backend_mut().assert_cursor_position((2 + 6, 0));

        assert!((0..100).all(|x| inverted(&cell(&terminal, x, 15), DARK)));
        assert!(
            cell(&terminal, 1, 15).modifier.contains(Modifier::BOLD),
            "Enter"
        );
        assert!(
            !cell(&terminal, 7, 15).modifier.contains(Modifier::BOLD),
            "ssh"
        );
        assert!(!cell(&terminal, 99, 15).modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn dark_80x24_consul_snapshot_stacks_the_preview_under_the_list() {
        let (screen, mut picker) = setup("ui", "consul");
        let mut terminal = terminal(80, 24);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        let mut want = vec![
            prompt_line("> consul", "2/5", 80),
            rule_line(80),
            "  ENV         HOST           WHAT".to_owned(),
            "▌ NEW STAGING newstg-consul1 consul server 1 — mgmt".to_owned(),
            "  NEW STAGING newstg-consul2".to_owned(),
        ];
        want.extend((0..12).map(|_| String::new()));
        want.push(format!("─ ssh -G newstg-consul1 {}", "─".repeat(80 - 24)));
        want.push(format!("  {}", key_row("user", "ubuntu")));
        want.push(format!("  {}", key_row("hostname", "10.11.212.191")));
        want.push(format!("  {}", key_row("port", "22")));
        want.push(format!("  {}", key_row("proxyjump", "newstg-bastion")));
        want.push(String::new());
        want.push(bar_line(80));
        assert_eq!(lines, want);

        assert!((0..80).all(|x| inverted(&cell(&terminal, x, 3), DARK)));
        assert!((0..80).all(|x| !inverted(&cell(&terminal, x, 4), DARK)));
        assert!(
            cell(&terminal, 5, 17).modifier.contains(Modifier::DIM),
            "title"
        );
        assert!(
            cell(&terminal, 70, 17).modifier.contains(Modifier::DIM),
            "rule"
        );
        assert!(
            cell(&terminal, 2, 18).modifier.contains(Modifier::DIM),
            "key"
        );
        assert!(
            !cell(&terminal, 16, 18).modifier.contains(Modifier::DIM),
            "value"
        );
        terminal.backend_mut().assert_cursor_position((2 + 6, 0));
    }

    #[test]
    fn small_60x14_terminal_draws_what_fits() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 14);
        let failed = Slot::Failed(ResolveError::Timeout);
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert_eq!(
            lines,
            [
                prompt_line("> type to filter", "5/5", 60),
                rule_line(60),
                "  ENV         HOST           WHAT".to_owned(),
                "▌ LAB         lab-a          LAB — personal VMs".to_owned(),
                "  LAB         lab-b          build box".to_owned(),
                "  NEW STAGING newstg-consul1 consul server 1 — mgmt".to_owned(),
                "  NEW STAGING newstg-consul2".to_owned(),
                format!(
                    "─ ssh -G lab-a failed {} 1 more below ─",
                    "─".repeat(60 - 22 - 15)
                ),
                "  ssh -G gave no answer within 2 s".to_owned(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                bar_line(60),
            ]
        );
    }

    #[test]
    fn no_color_draws_with_modifiers_only() {
        let bar = bar_style(&vars(&[("NO_COLOR", "1"), ("SSHPICK_THEME", "light")]));
        let (screen, mut picker) = setup_with("ui", "consul", bar, &[]);
        let mut terminal = terminal(120, 40);
        render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        let buffer = terminal.backend().buffer();
        assert!(
            buffer
                .content()
                .iter()
                .all(|cell| cell.fg == Color::Reset && cell.bg == Color::Reset),
            "no colour of any kind"
        );
        assert!((0..80).all(|x| cell(&terminal, x, 3).modifier.contains(Modifier::REVERSED)));
        assert!((0..120).all(|x| cell(&terminal, x, 39).modifier.contains(Modifier::REVERSED)));
        assert!((0..120).all(|x| !cell(&terminal, x, 2).modifier.contains(Modifier::REVERSED)));
        assert!((0..120).all(|x| !cell(&terminal, x, 4).modifier.contains(Modifier::REVERSED)));
        assert!(cell(&terminal, 1, 39).modifier.contains(Modifier::BOLD));
        assert!(!cell(&terminal, 7, 39).modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn light_theme_swaps_the_inverted_colours() {
        let bar = bar_style(&vars(&[("SSHPICK_THEME", "light")]));
        assert_eq!(bar, LIGHT);
        let (screen, mut picker) = setup_with("ui", "consul", bar, &[]);
        let mut terminal = terminal(120, 40);
        render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert!((0..80).all(|x| inverted(&cell(&terminal, x, 3), LIGHT)));
        assert!((0..120).all(|x| inverted(&cell(&terminal, x, 39), LIGHT)));
        assert!(!inverted(&cell(&terminal, 12, 4), LIGHT));
    }

    #[test]
    fn content_spans_the_whole_terminal_at_any_width() {
        let (screen, mut picker) = setup("ui", "");
        for width in [60, 99, 100, 101, 160, 200] {
            let wide = width >= 100;
            let list_width = if wide { width - 40 } else { width };
            let mut terminal = terminal(width, 20);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            assert!(lines[0].starts_with("> type to filter"), "{width}");
            assert!(lines[0].ends_with("5/5"), "{width}");
            assert_eq!(lines[0].chars().count(), usize::from(width), "{width}");
            assert_eq!(lines[1], rule_line(width.into()), "{width}");
            assert!(lines[3].starts_with("▌ LAB         lab-a"), "{width}");
            assert!((0..list_width).all(|x| inverted(&cell(&terminal, x, 3), DARK)));
            assert!((list_width..width).all(|x| !inverted(&cell(&terminal, x, 3), DARK)));
            assert!((0..width).all(|x| inverted(&cell(&terminal, x, 19), DARK)));
            let bottom = if wide { 18 } else { 13 };
            assert!(
                lines[bottom].chars().all(|c| c != ' ') || lines[bottom].starts_with("─ ssh"),
                "{width}: {:?}",
                lines[bottom]
            );
            assert_eq!(lines[bottom].chars().count(), usize::from(width), "{width}");
            assert_eq!(lines[2].contains('│'), wide, "{width}");
            terminal.backend_mut().assert_cursor_position((2, 0));
        }
    }

    #[test]
    fn a_long_what_ends_in_an_ellipsis_on_a_character_boundary() {
        let (screen, mut picker) = setup("ui", "consul");
        let row = "▌ NEW STAGING newstg-consul1 ";
        for (width, what) in [
            (46, "consul server 1 …"),
            (47, "consul server 1 —…"),
            (48, "consul server 1 — …"),
            (49, "consul server 1 — m…"),
            (50, "consul server 1 — mg…"),
            (51, "consul server 1 — mgmt"),
            (80, "consul server 1 — mgmt"),
        ] {
            let mut terminal = terminal(width, 20);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            assert_eq!(lines[3], format!("{row}{what}"), "{width}");
            assert_eq!(lines[4], "  NEW STAGING newstg-consul2", "{width}");
        }
    }

    #[test]
    fn bold_survives_inside_the_kept_part_and_the_ellipsis_is_plain() {
        let (screen, mut picker) = setup("ui", "'server 1");
        let mut terminal = terminal(41, 20);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[3], "▌ NEW STAGING newstg-consul1 consul serv…");
        let bold = |x| cell(&terminal, x, 3).modifier.contains(Modifier::BOLD);
        assert!(!(29..36).any(bold), "consul ");
        assert!((36..40).all(bold), "serv");
        assert_eq!(cell(&terminal, 40, 3).symbol(), "…");
        assert!(!bold(40), "the ellipsis is not a match");
        assert!(inverted(&cell(&terminal, 40, 3), DARK));
    }

    #[test]
    fn the_prompt_shows_a_dim_placeholder_until_the_first_key() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(60, 20);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[0], prompt_line("> type to filter", "5/5", 60));
        assert!(cell(&terminal, 0, 0).modifier.contains(Modifier::BOLD));
        assert!((2..16).all(|x| cell(&terminal, x, 0).modifier.contains(Modifier::DIM)));
        assert!((2..16).all(|x| !cell(&terminal, x, 0).modifier.contains(Modifier::BOLD)));
        terminal.backend_mut().assert_cursor_position((2, 0));

        picker.handle(Key::Char('l'));
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[0], prompt_line("> l", "4/5", 60));
        assert!(!cell(&terminal, 2, 0).modifier.contains(Modifier::DIM));
        terminal.backend_mut().assert_cursor_position((3, 0));

        picker.handle(Key::Ctrl('u'));
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            lines[0],
            prompt_line("> type to filter", "5/5", 60),
            "clearing brings it back"
        );
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    #[test]
    fn the_count_stays_on_the_right_when_the_query_outgrows_the_room() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(30, 20);
        for c in "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        let room = 30 - 2 - 4;
        assert_eq!(
            lines[0],
            format!("> {}  0/5", "z".repeat(room - 1)),
            "the query scrolls, the cursor cell, a space, the count"
        );
        terminal
            .backend_mut()
            .assert_cursor_position((2 + room as u16 - 1, 0));
        assert!(cell(&terminal, 27, 0).modifier.contains(Modifier::DIM));
        assert!(!cell(&terminal, 24, 0).modifier.contains(Modifier::DIM));
        assert_eq!(cell(&terminal, 25, 0).symbol(), " ", "the cursor cell");
    }

    #[test]
    fn zero_matches_say_no_match_on_the_first_row_and_the_bar_keeps_the_count() {
        let warnings = [Warning::Cycle {
            path: PathBuf::from("/x"),
        }];
        let (screen, mut picker) = setup_with("ui", "zzzz", DARK, &warnings);
        let mut wide = terminal(120, 20);
        let lines = render(&mut wide, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[0], prompt_line("> zzzz", "0/5", 120));
        assert_eq!(
            lines[2],
            beside_at(120, "  ENV         HOST           WHAT", "")
        );
        assert_eq!(lines[3], beside_at(120, "  no match", ""));
        assert!(lines[4..18].iter().all(|line| line.ends_with('│')));
        assert!(
            lines[4..18]
                .iter()
                .all(|line| line.trim_end_matches('│').trim().is_empty())
        );
        assert!(
            (2..10).all(|x| cell(&wide, x, 3).modifier.contains(Modifier::DIM)),
            "no match is dim"
        );
        assert!((0..120).all(|x| !inverted(&cell(&wide, x, 3), DARK)));
        assert_eq!(lines[18], rule_line(120), "a bare rule");
        assert!(
            lines[19].ends_with("5 hosts · 1 warning · /x: skipped, include cycle"),
            "{:?}",
            lines[19]
        );
        assert!((0..120).all(|x| inverted(&cell(&wide, x, 19), DARK)));
        assert!(
            (0..120).all(|x| !cell(&wide, x, 19).modifier.contains(Modifier::DIM)),
            "the bar has no dim"
        );

        let (screen, mut picker) = setup("ui", "zzzz");
        let mut small = terminal(60, 20);
        let lines = render(&mut small, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[3], "  no match");
        assert_eq!(
            lines[19],
            bar_line(60),
            "the bar says 5 hosts, not no match"
        );
    }

    fn beside_at(width: usize, left: &str, right: &str) -> String {
        format!("{left:<w$} │ {right}", w = width - 40)
            .trim_end()
            .to_owned()
    }

    #[test]
    fn hidden_rows_are_counted_on_the_bottom_rule() {
        let (screen, mut picker) = setup("ui", "");
        let mut count = |height: u16| {
            let mut terminal = terminal(60, height);
            let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
            let preview = if height >= 13 { 5 } else { 0 };
            let list = height - 5 - preview;
            lines[usize::from(3 + list)].clone()
        };
        let more = |line: &str| line.ends_with("more below ─");
        assert!(!more(&count(16)), "all five rows fit");
        assert!(!more(&count(15)), "exactly fit");
        assert!(count(14).ends_with(" 1 more below ─"));
        assert!(count(13).ends_with(" 2 more below ─"));
        assert!(!more(&count(12)), "the list takes the preview's room");
        assert!(!more(&count(10)), "exactly fit without a preview");
        assert!(count(9).ends_with(" 1 more below ─"));
        assert!(count(8).ends_with(" 2 more below ─"));
        assert!(count(7).ends_with(" 3 more below ─"));
        assert_eq!(count(9), format!("{} 1 more below ─", "─".repeat(60 - 15)));

        let mut scrolled = terminal(60, 13);
        for _ in 0..4 {
            picker.handle(Key::Down);
        }
        let lines = render(&mut scrolled, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(
            lines[3],
            "  NEW STAGING newstg-consul1 consul server 1 — mgmt"
        );
        assert_eq!(lines[5], "▌ NEW STAGING newstg-bastion bastion");
        assert!(
            lines[6].starts_with("─ ssh -G newstg-bastion ─") && !more(&lines[6]),
            "scrolled to the end, nothing below: {:?}",
            lines[6]
        );

        let (screen, mut picker) = setup("ui", "'lab");
        let mut small = terminal(60, 13);
        let lines = render(&mut small, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[3], "▌ LAB         lab-b          build box");
        assert_eq!(lines[4], "  LAB         lab-a          LAB — personal VMs");
        assert_eq!(lines[0], prompt_line("> 'lab", "2/5", 60));
        assert!(!more(&lines[6]), "two matches, three rows, none hidden");
    }

    #[test]
    fn the_wide_pane_shows_one_more_below_on_a_short_list() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(100, 9);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(
            lines,
            [
                prompt_line("> type to filter", "5/5", 100),
                rule_line(100),
                beside("  ENV         HOST           WHAT", "ssh -G lab-a"),
                beside(
                    "▌ LAB         lab-a          LAB — personal VMs",
                    "user          ubuntu"
                ),
                beside(
                    "  LAB         lab-b          build box",
                    "hostname      10.11.212.191"
                ),
                beside(
                    "  NEW STAGING newstg-consul1 consul server 1 — mgmt",
                    "port          22"
                ),
                beside(
                    "  NEW STAGING newstg-consul2",
                    "proxyjump     newstg-bastion"
                ),
                format!("{} 1 more below ─", "─".repeat(100 - 15)),
                bar_line(100),
            ]
        );
        assert!(cell(&terminal, 90, 7).modifier.contains(Modifier::DIM));
        assert!(cell(&terminal, 80, 7).modifier.contains(Modifier::DIM));
    }

    #[test]
    fn the_wide_pane_cuts_long_values_and_wraps_errors_inside_its_37_columns() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(100, 16);
        let long = Slot::Ready(Preview::parse(
            "user ubuntu\nproxyjump a-very-long-bastion-host-name\n",
        ));
        let lines = render(&mut terminal, &screen, &mut picker, &long);
        assert!(
            lines[4].ends_with("proxyjump     a-very-long-bastion-ho…"),
            "{:?}",
            lines[4]
        );
        assert_eq!(lines[4].chars().count(), 100);
        assert_eq!(cell(&terminal, 99, 4).symbol(), "…");

        let failed = Slot::Failed(ResolveError::Ssh {
            code: Some(255),
            stderr: "word ".repeat(30),
        });
        let lines = render(&mut terminal, &screen, &mut picker, &failed);
        assert!(
            lines[2].ends_with("│ ssh -G lab-a failed"),
            "{:?}",
            lines[2]
        );
        for line in &lines[3..8] {
            let pane: String = line.chars().skip(63).collect();
            assert!(
                pane.starts_with("word") && pane.chars().count() <= 37,
                "{pane:?}"
            );
        }
        assert!(lines[8].ends_with("│"), "{:?}", lines[8]);
    }

    #[test]
    fn a_waiting_pane_shows_only_its_title_and_no_selection_shows_only_the_divider() {
        let (screen, mut picker) = setup("ui", "consul");
        let mut terminal = terminal(100, 16);
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert!(lines[2].ends_with("│ ssh -G newstg-consul1"));
        assert!(lines[3..14].iter().all(|line| line.ends_with('│')));

        for c in "zzzz".chars() {
            picker.handle(Key::Char(c));
        }
        let lines = render(&mut terminal, &screen, &mut picker, &Slot::Waiting);
        assert_eq!(lines[2], beside("  ENV         HOST           WHAT", ""));
        assert!(lines[3..14].iter().all(|line| line.ends_with('│')));
        assert!(
            !lines.iter().any(|line| line.contains("ssh -G")),
            "{lines:#?}"
        );
    }

    #[test]
    fn a_100x12_terminal_still_shows_the_side_pane_and_the_list_takes_the_rest() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(100, 12);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(
            lines,
            [
                prompt_line("> type to filter", "5/5", 100),
                rule_line(100),
                beside("  ENV         HOST           WHAT", "ssh -G lab-a"),
                beside(
                    "▌ LAB         lab-a          LAB — personal VMs",
                    "user          ubuntu"
                ),
                beside(
                    "  LAB         lab-b          build box",
                    "hostname      10.11.212.191"
                ),
                beside(
                    "  NEW STAGING newstg-consul1 consul server 1 — mgmt",
                    "port          22"
                ),
                beside(
                    "  NEW STAGING newstg-consul2",
                    "proxyjump     newstg-bastion"
                ),
                beside("  NEW STAGING newstg-bastion bastion", ""),
                beside("", ""),
                beside("", ""),
                rule_line(100),
                bar_line(100),
            ]
        );
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    #[test]
    fn an_80x12_terminal_has_no_preview_and_the_list_takes_its_room() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(80, 12);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(
            lines,
            [
                prompt_line("> type to filter", "5/5", 80),
                rule_line(80),
                "  ENV         HOST           WHAT".to_owned(),
                "▌ LAB         lab-a          LAB — personal VMs".to_owned(),
                "  LAB         lab-b          build box".to_owned(),
                "  NEW STAGING newstg-consul1 consul server 1 — mgmt".to_owned(),
                "  NEW STAGING newstg-consul2".to_owned(),
                "  NEW STAGING newstg-bastion bastion".to_owned(),
                String::new(),
                String::new(),
                rule_line(80),
                bar_line(80),
            ]
        );
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    #[test]
    fn a_100x6_terminal_keeps_only_the_prompt_the_list_and_the_status_bar() {
        let (screen, mut picker) = setup("ui", "");
        let mut terminal = terminal(100, 6);
        let lines = render(&mut terminal, &screen, &mut picker, &ready());
        assert_eq!(
            lines,
            [
                prompt_line("> type to filter", "5/5", 100),
                "▌ LAB         lab-a          LAB — personal VMs".to_owned(),
                "  LAB         lab-b          build box".to_owned(),
                "  NEW STAGING newstg-consul1 consul server 1 — mgmt".to_owned(),
                "  NEW STAGING newstg-consul2".to_owned(),
                bar_line(100),
            ]
        );
        assert!((0..100).all(|x| inverted(&cell(&terminal, x, 1), DARK)));
        terminal.backend_mut().assert_cursor_position((2, 0));
    }

    #[test]
    fn the_stacked_preview_needs_13_rows_and_the_frame_needs_7() {
        let (screen, mut picker) = setup("ui", "");
        let mut lines_at = |width, height| {
            let mut terminal = terminal(width, height);
            render(&mut terminal, &screen, &mut picker, &Slot::Waiting)
        };
        let has =
            |lines: &[String], wanted: &dyn Fn(&str) -> bool| lines.iter().any(|line| wanted(line));
        let title = |line: &str| line.starts_with("─ ssh -G");
        let header = |line: &str| line.starts_with("  ENV         HOST           WHAT");
        let rule = |line: &str| line.starts_with('─');

        assert!(has(&lines_at(80, 13), &title));
        assert!(!has(&lines_at(80, 12), &title));
        assert!(
            !has(&lines_at(100, 40), &title),
            "wide has no stacked preview"
        );
        for width in [80, 100] {
            for (height, shown) in [(7, true), (6, false)] {
                let lines = lines_at(width, height);
                assert_eq!(has(&lines, &header), shown, "{width}x{height}");
                assert_eq!(
                    lines.iter().filter(|line| rule(line)).count(),
                    if shown { 2 } else { 0 },
                    "{width}x{height}: {lines:#?}"
                );
            }
        }
        let lines = lines_at(80, 7);
        assert!(lines[0].starts_with("> "));
        assert!(lines[3].starts_with("▌ LAB"));
        assert!(lines[4].starts_with("  LAB         lab-b"));
        assert_eq!(lines[5], format!("{} 3 more below ─", "─".repeat(80 - 15)));
        assert_eq!(lines[6], bar_line(80));
    }

    #[test]
    fn tiny_terminals_never_panic() {
        let (screen, mut picker) = setup("ui", "consul");
        let sizes = (0..=45)
            .map(|width| (width, 14))
            .chain([(99, 20), (100, 20), (101, 20), (140, 20)])
            .flat_map(|(width, most)| (0..=most).map(move |height| (width, height)));
        for (width, height) in sizes {
            let mut terminal = terminal(width, height);
            let failed = Slot::Failed(ResolveError::Timeout);
            render(&mut terminal, &screen, &mut picker, &failed);
            let lines = render(&mut terminal, &screen, &mut picker, &ready());
            if height >= 2 && width >= 1 {
                let prompt = &lines[0];
                assert!(prompt.starts_with('>'), "{width}x{height}: {lines:#?}");
            }
        }
        let lines = render(&mut terminal(30, 1), &screen, &mut picker, &Slot::Waiting);
        assert!(lines[0].starts_with(" Enter ssh"), "the bar still draws");
    }

    #[test]
    fn fit_cuts_on_display_width_and_keeps_runs_whole_or_cut_cleanly() {
        let plain = |text| [(text, false)];
        assert_eq!(fit(&plain("abcdef"), 6), plain("abcdef"));
        assert_eq!(fit(&plain("abcdef"), 5), [("abcd", false), (CUT, false)]);
        assert_eq!(fit(&plain("abcdef"), 1), [(CUT, false)]);
        assert!(fit(&plain("abcdef"), 0).is_empty());

        let runs = [("ab", false), ("cdef", true), ("gh", false)];
        assert_eq!(
            fit(&runs, 5),
            [("ab", false), ("cd", true), (CUT, false)],
            "a cut run keeps its match"
        );
        assert_eq!(fit(&runs, 3), [("ab", false), (CUT, false)]);
        assert_eq!(fit(&runs, 8), runs);
        assert_eq!(fit(&runs, 7), [("ab", false), ("cdef", true), (CUT, false)]);

        assert_eq!(
            fit(&plain("a — b"), 4),
            [("a —", false), (CUT, false)],
            "an em dash is one column and three bytes"
        );
        assert_eq!(
            fit(&plain("日本語です"), 6),
            [("日本", false), (CUT, false)],
            "a wide char that does not fit is dropped, not split"
        );
        assert_eq!(
            fit(&plain("日本語です"), 7),
            [("日本語", false), (CUT, false)]
        );
        assert_eq!(fit(&plain("日本語です"), 10), plain("日本語です"));
        assert_eq!(
            fit(&plain("สวัสดี"), 3),
            [("สวั", false), (CUT, false)],
            "a combining mark stays with its base"
        );
        assert_eq!(fit(&plain("สวัสดี"), 4), plain("สวัสดี"));
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

    #[test]
    fn the_status_bar_drops_hints_from_the_right() {
        let full = " Enter ssh  Esc quit  Ctrl-U clear  ↑↓ move";
        assert_eq!(
            status_text(53, "5 hosts", None),
            format!("{full}  5 hosts ")
        );
        assert_eq!(
            status_text(52, "5 hosts", None),
            " Enter ssh  Esc quit  Ctrl-U clear          5 hosts "
        );
        assert_eq!(
            status_text(40, "5 hosts", None),
            " Enter ssh  Esc quit            5 hosts "
        );
        assert_eq!(
            status_text(28, "5 hosts", None),
            " Enter ssh          5 hosts "
        );
        assert_eq!(status_text(12, "5 hosts", None), "    5 hosts ");
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

    #[test]
    fn the_status_counts_one_host_and_one_warning_in_the_singular() {
        let warnings = [Warning::Cycle {
            path: PathBuf::from("/x"),
        }];
        let (screen, mut picker) = setup_with("home", "", DARK, &warnings);
        let lines = render(&mut terminal(100, 12), &screen, &mut picker, &Slot::Waiting);
        assert!(
            lines[11].ends_with("1 host · 1 warning · /x: skipped, include cycle"),
            "{:?}",
            lines[11]
        );
    }

    #[test]
    fn truncation_counts_display_columns() {
        assert_eq!(truncate_to_width("abcdef", 6), "abcdef");
        assert_eq!(truncate_to_width("abcdef", 5), "ab...");
        assert_eq!(truncate_to_width("abcdef", 3), "abc");
        assert_eq!(truncate_to_width("abcdef", 0), "");
        assert_eq!(truncate_to_width("日本語です", 7), "日本...");
        assert_eq!(truncate_to_width("日本語です", 10), "日本語です");
        assert_eq!(truncate_to_width("日本語です", 8), "日本...");
        assert_eq!(truncate_to_width("日本語です", 9), "日本語...");
    }
}
