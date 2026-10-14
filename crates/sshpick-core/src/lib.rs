//! sshpick's domain, with no terminal in it: which hosts `~/.ssh/config` defines, how a query
//! ranks them, what each key does, and what `ssh` says about a host.
//!
//! This crate cannot name a terminal type because it does not depend on one. `sshpick-ui` owns
//! the terminal (raw mode, drawing, reading keys).
//!
//! The whole public surface, as `sshpick-cli` and `sshpick-ui` use it:
//!
//! ```no_run
//! use sshpick_core::{Key, Outcome, Picker, Previews, Slot};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let home = std::path::PathBuf::from("/home/me");
//! let loaded = sshpick_core::load(&home)?; // ~/.ssh/config + Includes, in ssh's order
//! for warning in &loaded.warnings {
//!     eprintln!("sshpick: warning: {warning}");
//! }
//! let mut picker = Picker::new(loaded.entries, "new consul"); // argv joined
//! if let Some(host) = picker.only_match() {
//!     let err = sshpick_core::exec(host); // fzf --select-1; returns only on failure
//!     return Err(err.into());
//! }
//!
//! let mut previews = Previews::spawn()?; // one `ssh -G` worker thread
//! loop {
//!     if let Some(host) = picker.selected().cloned() {
//!         match previews.fetch(&host) {
//!             Slot::Waiting => {}
//!             Slot::Ready(preview) => preview.lines().for_each(|(k, v)| println!("{k} {v}")),
//!             Slot::Failed(error) => println!("{error}"),
//!         }
//!     }
//!     let view = picker.view(20); // the rows that fit in 20 lines, scrolled to the cursor
//!     println!("> {} ({}/{})", view.query, view.matched, view.total);
//!     match picker.handle(Key::Enter) {
//!         Some(Outcome::Pick(host)) => return Err(sshpick_core::exec(&host).into()),
//!         Some(Outcome::Cancel | Outcome::NoMatch) => return Ok(()),
//!         None => {}
//!     }
//! }
//! # }
//! ```

mod config;
mod label;
mod picker;
mod preview;
mod rank;
mod ssh;

pub use config::{Entry, Host, LoadError, Loaded, Warning, load};
pub use picker::{Key, Outcome, Picker, Row, View};
pub use preview::{Previews, Slot};
pub use ssh::{Preview, ResolveError, exec};
