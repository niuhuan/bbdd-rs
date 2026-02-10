pub mod auth;
pub mod client;
pub mod download;
pub mod error;
pub mod fetcher;
pub mod parse;
pub mod util;

#[cfg(test)]
mod tests;

pub use client::*;
pub use error::{BBDDError, BBDDResult};
pub(crate) use error::{Error, Result};

#[cfg(not(any(feature = "native-tls", feature = "rustls",)))]
compile_error!("one of the features ['native-tls', 'rustls'] must be enabled");

#[cfg(all(not(feature = "cli"), feature = "rsmpeg"))]
compile_error!("feature 'rsmpeg' requires feature 'cli'");
