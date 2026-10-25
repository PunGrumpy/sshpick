mod color;
pub mod listing;
mod output;
mod text;
mod tui;

pub use color::{ColorConfig, Reason};
pub use output::{Output, Stdout, error_text};
pub use tui::{Screen, run};
