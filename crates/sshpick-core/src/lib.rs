//! sshpick's domain, with no terminal in it: which hosts `~/.ssh/config` defines.

mod config;
mod label;

pub use config::{Entry, Host, LoadError, Loaded, Warning, load};
