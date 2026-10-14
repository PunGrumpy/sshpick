//! sshpick's terminal and everything it prints: the colour decision, console output, and the
//! picker, the one place that names a ratatui or crossterm type.
//!
//! `sshpick-core` decides what a key does and which rows match; this crate draws the picker,
//! owns raw mode and the alternate screen, and maps crossterm key events to `sshpick_core::Key`
//! one to one. `sshpick-cli` decides what to say and calls [`Output`] to say it.

mod color;
pub mod listing;
mod output;
mod text;
mod tui;

pub use color::ColorConfig;
pub use output::{Output, Stdout};
pub use tui::{Screen, run};
