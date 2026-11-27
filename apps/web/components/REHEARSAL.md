# picker-demo rehearsal

What was filmed for `picker-demo.tsx`, and what was not. Film it again with the Stageshow skill's
`scripts/film.mjs` against a built site (`bunx next build`, then serve `out/`), selecting
`.kw-stage`.

## Where it came from

Copied from the Stageshow gold example `hero-demo` (an animated hero beside copy) for its beat
structure, and framed as a whole window rather than a corner bleed: the picker takes the entire
terminal, so the only filler a crop could cut is the blank columns beside the box, and cutting a
window on its left is the one bleed the framing rules forbid. The window therefore fills the
stage with an even 16px margin, which is the rules' other option for a hero.

## What was captured

| Pass | Command | Result |
|---|---|---|
| Wide loop, 14s | `--select .kw-stage --every 700 --count 20 --gate` | `rehearsal passed` |
| 390 loop | `--width 390 --every 900 --count 8 --delay 1200 --gate` | `rehearsal passed` |
| Every state | `--states --gate` | `rehearsal passed` |
| Reduced motion | `--still --reduced` | the poster: the narrowed list, the second host selected |
| The page | 1440 and 390 screenshots, light and dark | read in full |

## Lines left standing, and why

- `appears with no entrance` / `vanishes with no exit` on the rows, the matched runs and the
  panel. A terminal redraws cells; nothing in a TUI fades. The query itself carries `data-typing`.
- `jump:` on the rows at the loop seam. The picker reopens over the shell, so the whole screen
  changes at once; the measured jump is React keeping the row elements across that redraw.
- `steps in its box: kbd "F1"`. The footer's help key is `?` while the filter is empty and `F1`
  once it is not, because `?` is a filter character then. The caps shift by one column when it
  changes, as they do in the terminal.
- `near-empty frames` around the connected beat. The payoff is a shell prompt on a host; the lines
  sit at the foot of the terminal, where a shell leaves them.

## Not checked

- Any browser but headless Chromium 130 on Linux. No Safari, no real device.
- Hover and press states: the demo has none, and no cursor.
- A real screen reader. The stage is decorative; the page says what the picker does in prose.
