//! sshpick's domain, with no terminal in it: which hosts `~/.ssh/config` defines, how a query
//! ranks them, and what each key does.

mod config;
mod label;
mod picker;
mod rank;

pub use config::{Entry, Host, LoadError, Loaded, Warning, load};
pub use picker::{Key, Outcome, Picker, Row, View};
