//! Which hosts the list shows. Every host is one line, so the window is counted in hosts.

/// The first host to draw, so that the cursor host is in `room` lines of the `len` matches. The
/// window moves as little as it can, and never leaves room at the end while hosts are above it.
pub(crate) fn scroll_to_show(scroll: usize, cursor: usize, len: usize, room: usize) -> usize {
    if room == 0 || cursor >= len {
        return scroll;
    }
    let mut scroll = scroll.min(cursor);
    if cursor - scroll >= room {
        scroll = cursor + 1 - room;
    }
    while scroll > 0 && len - scroll < room {
        scroll -= 1;
    }
    scroll
}

/// How many hosts from `scroll` are drawn in `room` lines.
pub(crate) fn shown(scroll: usize, len: usize, room: usize) -> usize {
    len.saturating_sub(scroll).min(room)
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    use super::*;

    //          scroll cursor len room => scroll
    #[test_case(0, 0, 20, 5, 0; "the cursor is on the first page")]
    #[test_case(0, 5, 20, 5, 1; "one past the page: the window moves one")]
    #[test_case(3, 2, 20, 5, 2; "the cursor above the window brings it up")]
    #[test_case(10, 11, 12, 5, 7; "the list ends full")]
    #[test_case(4, 4, 20, 0, 4; "no room, no move")]
    #[test_case(0, 0, 0, 5, 0; "no hosts")]
    #[test_case(5, 7, 8, 5, 3; "room at the end pulls hosts down from above")]
    #[test_case(0, 4, 20, 6, 0; "everything fits")]
    #[test_case(5, 0, 6, 8, 0; "back to the first host")]
    #[test_case(0, 19, 20, 5, 15; "the last host")]
    fn the_window_follows_the_cursor(
        scroll: usize,
        cursor: usize,
        len: usize,
        room: usize,
        want: usize,
    ) {
        assert_eq!(scroll_to_show(scroll, cursor, len, room), want);
    }

    //          scroll len room => hosts drawn
    #[test_case(0, 5, 5, 5; "all fit")]
    #[test_case(0, 5, 3, 3; "three of five")]
    #[test_case(2, 5, 5, 3; "from the scroll")]
    #[test_case(3, 5, 9, 2; "to the end")]
    #[test_case(0, 0, 5, 0; "no hosts")]
    #[test_case(9, 5, 5, 0; "past the end")]
    fn what_fits_is_drawn(scroll: usize, len: usize, room: usize, want: usize) {
        assert_eq!(shown(scroll, len, room), want);
    }

    #[test]
    fn the_cursor_host_is_drawn_whenever_it_fits() {
        for len in 0..=12 {
            for room in 1..=14 {
                for cursor in 0..len {
                    let mut scroll = 0;
                    for _ in 0..3 {
                        scroll = scroll_to_show(scroll, cursor, len, room);
                    }
                    let count = shown(scroll, len, room);
                    assert!(
                        (scroll..scroll + count).contains(&cursor),
                        "len {len}, room {room}, cursor {cursor}: {scroll}+{count}"
                    );
                    assert!(count <= room, "len {len}, room {room}: {count}");
                }
            }
        }
    }
}
