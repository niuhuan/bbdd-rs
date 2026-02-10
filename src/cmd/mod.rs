mod client;
mod download;
mod ffmpeg;
mod local;
mod login;
mod out;
mod whoami;

use std::process::exit;
use crate::cmd::out::{error, success};
use bbdd::parse::VideoType;
use clap::{Command, arg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverwriteMode {
    Ask,
    Overwrite,
    Skip,
}

pub(crate) static OVERWRITE_MODE: tokio::sync::OnceCell<OverwriteMode> =
    tokio::sync::OnceCell::const_new();
pub(crate) static CONTINUE_CACHE: tokio::sync::OnceCell<bool> = tokio::sync::OnceCell::const_new();

pub(crate) static QUALITY_PREFERENCE: tokio::sync::OnceCell<Option<i64>> =
    tokio::sync::OnceCell::const_new();

pub(crate) async fn main() {
    #[cfg(not(feature = "rsmpeg"))]
    ffmpeg::ffmpeg_api::ffmpeg_run_version();
    client::init_client(local::init_dir()).await;
    let matches = cli().get_matches();
    if matches.get_flag("debug") {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
        tracing::debug!("调试模式已启用");
    }
    if let Some(quality) = matches.get_one::<String>("quality") {
        if let Ok(quality) = quality.parse::<i64>() {
            if vec![
                127, 126, 125, 120, 116, 112, 100, 80, 74, 64, 48, 32, 16, 6, 5,
            ]
            .contains(&quality)
            {
                let _ = QUALITY_PREFERENCE.set(Some(quality));
                tracing::debug!("视频清晰度设置为 {}", quality);
            } else {
                error(
                    "参数 -q --quality 必须是以下数字之一: 127, 126, 125, 120, 116, 112, 100, 80, 74, 64, 48, 32, 16, 6, 5",
                );
                std::process::exit(1);
            }
        } else {
            error("参数 -q --quality 必须是数字");
        }
    } else {
        let _ = QUALITY_PREFERENCE.set(None);
    }
    match matches.subcommand() {
        Some(("login", _)) => login::login().await,
        Some(("whoami", _)) => whoami::whoami().await,
        _ => {
            if let Some(url) = matches.get_one::<String>("url") {
                let dir = matches.get_one::<String>("workdir").map(|s| s.as_str());
                if let Some(dir) = dir {
                    if !std::path::Path::new(dir).exists() {
                        error("工作目录不存在");
                        std::process::exit(1);
                    }
                    if let Err(e) = std::env::set_current_dir(dir) {
                        error(format!("无法切换工作目录: {}", e).as_str());
                        std::process::exit(1);
                    } else {
                        success(format!("工作目录切换到: {}", dir).as_str());
                    }
                }
                let client = client::CLIENT_CELL.get().unwrap();
                let overwrite = matches.get_flag("overwrite");
                let interactive = matches.get_flag("interactive");
                if overwrite && interactive {
                    error("参数 -o --overwrite 和 -i --interactive 不能同时使用");
                    std::process::exit(1);
                }
                let overwrite_mode = if overwrite {
                    OverwriteMode::Overwrite
                } else if interactive {
                    OverwriteMode::Ask
                } else {
                    OverwriteMode::Skip
                };
                let _ = OVERWRITE_MODE.set(overwrite_mode);
                let use_cache = matches.get_one::<String>("continue");
                let use_cache = if let Some(use_cache) = use_cache {
                    if use_cache != "true" && use_cache != "false" {
                        error("参数 -c --continue 只能是 true 或 false");
                        std::process::exit(1);
                    }
                    use_cache == "true"
                } else {
                    !overwrite_mode.eq(&OverwriteMode::Overwrite)
                };
                let _ = CONTINUE_CACHE.set(use_cache);
                let url = url.trim();
                let parse = error_exit(client.parse_input(url).await);
                match parse {
                    VideoType::AVID(avid) => {
                        exit(download::download_avid(avid).await);
                    }
                    VideoType::EPID(ep_id) => {
                        exit(download::download_ep(ep_id).await);
                    }
                    _ => {
                        error("暂时不支持的链接类型");
                        std::process::exit(1);
                    }
                }
            } else {
                print_help();
            }
        }
    }
}
fn error_exit<T>(result: bbdd::BBDDResult<T>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => {
            error(format_bbdd_error(&e).as_str());
            std::process::exit(1);
        }
    }
}

fn format_bbdd_error(e: &bbdd::BBDDError) -> String {
    match e {
        bbdd::BBDDError::HttpRequestError(e) => format!("网络请求失败: {}", e),
        bbdd::BBDDError::JsonParseError(e) => format!("数据解析失败: {}", e),
        bbdd::BBDDError::ApiError { code, message } => {
            if message.is_empty() {
                format!("接口请求失败: 错误代码 {}", code)
            } else {
                format!("接口请求失败: {} ({})", message, code)
            }
        }
        bbdd::BBDDError::ParamError(msg) => format!("{}", msg),
        bbdd::BBDDError::StateError(msg) => format!("{}", msg),
    }
}

fn print_help() {
    let _ = cli().print_help();
    println!();
}

fn cli() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("一个BILIBILI视频下载工具")
        .arg(arg!(<url>).required(false)) // 允许没有 url 参数
        .arg(
            arg!(-q --quality <QUALITY> "视频清晰度，默认为最高清晰度, 参数为数字。 超过48可能需要大会员用户。127(8K 超高清), 126(杜比视界), 125(HDR 真彩), 120(4K 超清), 116(1080P 高帧率), 112(1080P 高码率), 100(智能修复), 80(1080P 高清), 74(720P 高帧率), 64(720P 高清), 48(720P 高清), 32(480P 清晰), 16(360P 流畅), 6(240P 流畅), 5(144P 流畅)")
                .required(false),
        )
        .arg(
            arg!(-w --workdir <DIR> "工作目录，默认为当前目录，目录必须存在才能使用")
                .required(false)
        )
        .arg(
            arg!(-i --interactive "遇到已经下载的文件时，进行提问是否覆盖 （默认不提问、不覆盖，直接跳过）")
                .required(false)
                .default_value("false"),
        )
        .arg(
            arg!(-o --overwrite "遇到已经下载的文件时，直接进行覆盖 （默认不覆盖，直接跳过）")
                .required(false)
                .default_value("false"),
        )
        .arg(
            arg!(-c --continue <CACHE> "下载中断时是否保留的缓存，再次下载时是否使用缓存，-o存在时此选项默认为false，其余时为true，缓存为.video.*和.audio.*结尾的文件")
                .required(false),
        )
        .arg(arg!(
            --debug "启用调试模式，输出更多日志"
        ))
        .subcommand(login())
        .subcommand(whoami())
}

fn login() -> Command {
    Command::new("login").about("登录BILIBILI账号")
}

fn whoami() -> Command {
    Command::new("whoami")
        .about("认证并显示当前登录账号信息")
        .alias("me")
}
