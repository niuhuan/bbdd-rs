pub mod auth;
pub mod client;
pub mod download;
pub mod error;
pub mod fetcher;
#[cfg(feature = "rsmpeg")]
pub mod merge;
pub mod parse;
pub mod progress;
pub mod select;
pub mod util;

#[cfg(test)]
mod tests;

pub use client::*;
pub use error::{BBDDError, BBDDResult};
pub(crate) use error::{Error, Result};

#[cfg(not(any(feature = "native-tls", feature = "rustls",)))]
compile_error!("one of the features ['native-tls', 'rustls'] must be enabled");

// 手机没有系统 libav。`rsmpeg` / `ffmpeg*` 会去找本机 FFmpeg。
// 要进程内合并就开 `rsmpeg-build`。不开任何 ffmpeg feature 时库不提供合并。
#[cfg(all(
    any(target_os = "android", target_os = "ios"),
    feature = "rsmpeg",
    not(feature = "rsmpeg-build"),
))]
compile_error!(
    "Android and iOS must enable `rsmpeg-build` to link FFmpeg. `rsmpeg` and `ffmpeg*` expect a system libav."
);
