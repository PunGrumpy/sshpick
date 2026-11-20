//! Where the picker sits in the terminal: a box nearly the whole screen, centred, with a blank
//! row above and below it and at least two blank columns at each side. A terminal too small for
//! the border gets no box: the picker is the whole terminal.

/// The widest the panel gets, however wide the terminal is.
pub const MAX_COLS: u16 = 110;
/// The fewest columns that keep a border and two blank columns at each side of the box.
const BORDER_COLS_MIN: u16 = 60;
/// The fewest rows that keep a border: the prompt, three list lines and the hints inside it —
/// the blank row under the prompt is the first a short terminal loses.
const BORDER_ROWS_MIN: u16 = 10;
/// The top edge, the rule above the key hints and the bottom edge.
const BORDER_ROWS: u16 = 3;
/// The blank columns at each side of the box, so it never touches the edge.
const SIDE: u16 = 2;

/// A rectangle of the terminal, in columns and rows from its top left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Panel {
    pub left: u16,
    pub top: u16,
    pub cols: u16,
    pub rows: u16,
    pub border: bool,
}

impl Panel {
    /// The rows of the picker's own parts: the border and the rule are not among them.
    pub fn inner_rows(self) -> u16 {
        if self.border {
            self.rows.saturating_sub(BORDER_ROWS)
        } else {
            self.rows
        }
    }
}

/// The size of the terminal alone decides the panel: a bordered box leaves a blank row above and
/// below itself and `SIDE` blank columns at each side, up to [`MAX_COLS`] wide. Without a border
/// the picker is the whole terminal.
pub fn panel(cols: u16, rows: u16) -> Panel {
    let rows = rows.max(1);
    if cols >= BORDER_COLS_MIN && rows >= BORDER_ROWS_MIN {
        let wide = cols.saturating_sub(2 * SIDE).min(MAX_COLS);
        Panel {
            left: (cols - wide) / 2,
            top: 1,
            cols: wide,
            rows: rows - 2,
            border: true,
        }
    } else {
        Panel {
            left: 0,
            top: 0,
            cols,
            rows,
            border: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    //          cols rows => left top cols rows border
    #[test_case(120, 30, (5, 1, 110, 28, true); "120x30: centred, a blank row above and below")]
    #[test_case(80, 24, (2, 1, 76, 22, true); "80x24")]
    #[test_case(200, 50, (45, 1, 110, 48, true); "200 columns: capped at 110 and centred")]
    #[test_case(60, 24, (2, 1, 56, 22, true); "60 columns keep the border and two blank columns each side")]
    #[test_case(59, 24, (0, 0, 59, 24, false); "59 columns: no border, the full width")]
    #[test_case(80, 10, (2, 1, 76, 8, true); "ten rows are the fewest with a border")]
    #[test_case(80, 9, (0, 0, 80, 9, false); "nine rows: no border, the whole terminal")]
    #[test_case(80, 1, (0, 0, 80, 1, false); "one row")]
    #[test_case(80, 0, (0, 0, 80, 1, false); "no rows are taken as one")]
    #[test_case(0, 24, (0, 0, 0, 24, false); "no columns")]
    fn the_terminal_size_places_the_panel(cols: u16, rows: u16, want: (u16, u16, u16, u16, bool)) {
        let got = panel(cols, rows);
        assert_eq!(
            (got.left, got.top, got.cols, got.rows, got.border),
            want,
            "{got:?}"
        );
    }

    #[test_case(120, 30, 25; "the border takes three rows that are not the picker's")]
    #[test_case(80, 12, 7; "a short terminal: the border takes three of the rows")]
    #[test_case(59, 24, 24; "no border takes nothing")]
    fn the_inside_is_what_the_border_leaves(cols: u16, rows: u16, inner_rows: u16) {
        assert_eq!(panel(cols, rows).inner_rows(), inner_rows);
    }

    #[test]
    fn whatever_the_size_the_panel_is_inside_the_terminal_and_centred() {
        for cols in 0..=230 {
            for rows in 0..=40 {
                let got = panel(cols, rows);
                let at = format!("{cols}x{rows}: {got:?}");
                assert!(got.top + got.rows <= rows.max(1), "{at}");
                assert!(got.left + got.cols <= cols, "{at}");
                if got.border {
                    let right = cols - got.left - got.cols;
                    assert_eq!(got.top, 1, "{at}");
                    assert_eq!(got.cols, (cols - 4).min(MAX_COLS), "{at}");
                    assert!(right >= got.left && right - got.left <= 1, "{at}");
                } else {
                    assert!(
                        got.left == 0
                            && got.top == 0
                            && got.cols == cols
                            && got.rows == rows.max(1),
                        "{at}"
                    );
                }
            }
        }
    }
}
