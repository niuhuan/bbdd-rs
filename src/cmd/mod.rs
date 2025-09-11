mod client;
mod download;
mod ffmpeg;
mod local;
mod login;
mod out;

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
    match matches.subcommand() {
        Some(("login", _)) => login::login().await,
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
                        download::download_avid(avid).await;
                    }
                    VideoType::EPID(epid) => {
                        download::download_ep(epid).await;
                    }
                    _ => {
                        error("不支持的链接类型");
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
}

fn login() -> Command {
    Command::new("login").about("登录BILIBILI账号")
}
