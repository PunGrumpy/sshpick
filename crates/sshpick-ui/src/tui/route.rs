//! Values come from ssh and may hold control characters: they reach the buffer through `Span`,
//! which drops them.

use sshpick_core::Slot;

use super::style::Role;

const RESOLVING: &str = "resolving…";

/// The left of the footer row: where the selected host goes, `resolving…`, or why `ssh -G`
/// failed. A host with nothing to say leaves the row's left end blank.
pub(super) fn text(slot: &Slot) -> Option<(String, Role)> {
    match slot {
        Slot::Waiting => Some((RESOLVING.to_owned(), Role::Muted)),
        Slot::Failed(error) => Some((error.headline(), Role::Fail)),
        Slot::Ready(preview) => {
            let route = preview.route()?;
            let mut text = route.target();
            for (joiner, word) in [("via", route.via), ("runs", route.runs)] {
                if let Some(word) = word {
                    text.push_str(&format!(" {joiner} {word}"));
                }
            }
            Some((text, Role::Muted))
        }
    }
}
