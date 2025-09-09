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
