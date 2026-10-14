use ratatui::layout::Rect;

const MIN_HEIGHT_WITH_FRAME: u16 = 7;
const MIN_HEIGHT_WITH_STACKED_PREVIEW: u16 = 13;
const SIDE_PANE_MIN_WIDTH: u16 = 100;
pub(super) const SIDE_PANE_WIDTH: u16 = 40;
pub(super) const STACKED_ROWS: u16 = 5;

/// How much of the screen the terminal's size allows, from the preview beside the list down to
/// the prompt and the list alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tier {
    /// 100 columns and 7 rows or more: the preview sits in a pane beside the list.
    Side,
    /// Under 100 columns and 13 rows or more: five preview rows under the list.
    Stacked,
    /// 7 rows or more, no room for a preview.
    ListOnly,
    /// Under 7 rows: the prompt, the list and the status bar, no header or rules.
    Bare,
}

pub(super) fn tier(area: Rect) -> Tier {
    if area.height < MIN_HEIGHT_WITH_FRAME {
        Tier::Bare
    } else if area.width >= SIDE_PANE_MIN_WIDTH {
        Tier::Side
    } else if area.height >= MIN_HEIGHT_WITH_STACKED_PREVIEW {
        Tier::Stacked
    } else {
        Tier::ListOnly
    }
}

impl Tier {
    pub(super) fn framed(self) -> bool {
        self != Tier::Bare
    }
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    #[test_case(100, 40, Tier::Side; "wide and tall")]
    #[test_case(100, 7, Tier::Side; "wide, the shortest frame")]
    #[test_case(100, 6, Tier::Bare; "wide but under 7 rows")]
    #[test_case(120, 30, Tier::Side; "the 120x30 terminal")]
    #[test_case(99, 24, Tier::Stacked; "one column short of the side pane")]
    #[test_case(80, 24, Tier::Stacked; "the default terminal")]
    #[test_case(60, 13, Tier::Stacked; "the shortest stacked preview")]
    #[test_case(60, 12, Tier::ListOnly; "one row short of the stacked preview")]
    #[test_case(80, 7, Tier::ListOnly; "the shortest frame")]
    #[test_case(80, 6, Tier::Bare; "under 7 rows")]
    #[test_case(0, 0, Tier::Bare; "no screen at all")]
    fn the_size_picks_the_tier(width: u16, height: u16, want: Tier) {
        assert_eq!(tier(Rect::new(0, 0, width, height)), want);
        assert_eq!(want.framed(), want != Tier::Bare);
    }
}
