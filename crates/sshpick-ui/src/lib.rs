//! sshpick's terminal: everything that names a ratatui or crossterm type.
//!
//! `sshpick-core` decides what a key does and which rows match; this crate draws the picker,
//! owns raw mode and the alternate screen, and maps crossterm key events to `sshpick_core::Key`
//! one to one.

mod text;
mod tui;

pub use tui::{Screen, run};
