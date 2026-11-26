mod client;
mod cover;
mod download;
#[cfg(not(feature = "rsmpeg"))]
mod ffmpeg;
mod local;
mod login;
mod out;
mod whoami;

use crate::cmd::out::{error, success};
use bbdd::parse::VideoType;
use clap::{Arg, ArgAction, Command, arg};
use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio_util::sync::CancellationToken;

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
pub(crate) static SELECT_PAGE: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
pub(crate) static NO_COVER: tokio::sync::OnceCell<bool> = tokio::sync::OnceCell::const_new();
pub(crate) static KEEP_COVER: tokio::sync::OnceCell<bool> = tokio::sync::OnceCell::const_new();
pub(crate) static CANCEL: tokio::sync::OnceCell<CancellationToken> =
    tokio::sync::OnceCell::const_new();
static BUSY: AtomicBool = AtomicBool::new(false);

pub(crate) struct Busy;

impl Busy {
    pub(crate) fn enter() -> Self {
        BUSY.store(true, Ordering::Relaxed);
        Self
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Relaxed);
    }
}

pub(crate) fn cancel_token() -> CancellationToken {
    CANCEL
        .get()
        .cloned()
        .unwrap_or_else(CancellationToken::new)
}

pub(crate) fn is_cancelled() -> bool {
    CANCEL.get().is_some_and(|token| token.is_cancelled())
}

pub(crate) fn download_cover() -> bool {
    !NO_COVER.get().copied().unwrap_or(false)
}

pub(crate) fn keep_cover() -> bool {
    KEEP_COVER.get().copied().unwrap_or(false)
}

pub(crate) async fn main() {
    let cancel = CancellationToken::new();
    let _ = CANCEL.set(cancel.clone());
    tokio::spawn(async move {
        loop {
            if tokio::signal::ctrl_c().await.is_err() {
                break;
            }
            if BUSY.load(Ordering::Relaxed) && !cancel.is_cancelled() {
                cancel.cancel();
                crate::cmd::out::warn("正在停止，再按一次 Ctrl-C 立即退出");
            } else {
                std::process::exit(130);
            }
        }
    });
    #[cfg(not(feature = "rsmpeg"))]
    ffmpeg::ffmpeg_run_version();
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
    let select_page = matches
        .get_one::<String>("select-page")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "ALL".to_string());
    let _ = SELECT_PAGE.set(select_page);
    match matches.subcommand() {
        Some(("login", _)) => login::login().await,
        Some(("whoami", _)) => whoami::whoami().await,
        Some(("cover", sub)) => {
            apply_workdir(&matches);
            let url = sub
                .get_one::<String>("url")
                .map(|value| value.as_str())
                .unwrap_or("");
            exit(cover::save(url).await);
        }
        _ => {
            if let Some(url) = matches.get_one::<String>("url") {
                apply_workdir(&matches);
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
                let no_cover = matches.get_flag("no-cover");
                let keep_cover = matches.get_flag("keep-cover");
                if no_cover && keep_cover {
                    error("参数 --no-cover 和 --keep-cover 不能同时使用");
                    std::process::exit(1);
                }
                let _ = NO_COVER.set(no_cover);
                let _ = KEEP_COVER.set(keep_cover);
                let url = url.trim();
                let parse = error_exit(client.parse_input(url).await);
                match parse {
                    VideoType::AVID(avid) => {
                        exit(download::download_avid(avid).await);
                    }
                    VideoType::EPID(ep_id) => {
                        exit(download::download_ep(ep_id).await);
                    }
                    VideoType::CHEESE(ep_id) => {
                        exit(download::download_cheese(ep_id).await);
                    }
                    VideoType::LISTBIZID(biz_id) => {
                        exit(download::download_media_list(biz_id.as_str(), false).await);
                    }
                    VideoType::SERIESBIZID(biz_id) => {
                        exit(download::download_media_list(biz_id.as_str(), true).await);
                    }
                    VideoType::FAVID { fid, mid } => {
                        exit(download::download_fav(fid.as_str(), mid.as_str()).await);
                    }
                    VideoType::MID(_) => {
                        error("暂时不支持 UP 主投稿批量下载");
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
        bbdd::BBDDError::Io(err) => format!("文件读写失败: {}", err),
        bbdd::BBDDError::Cancelled => "已取消".to_string(),
    }
}

fn apply_workdir(matches: &clap::ArgMatches) {
    let dir = matches
        .get_one::<String>("workdir")
        .or_else(|| {
            matches
                .subcommand()
                .and_then(|(_, sub)| sub.get_one::<String>("workdir"))
        })
        .map(|value| value.as_str());
    let Some(dir) = dir else {
        return;
    };
    if !std::path::Path::new(dir).exists() {
        error("工作目录不存在");
        std::process::exit(1);
    }
    if let Err(e) = std::env::set_current_dir(dir) {
        error(format!("无法切换工作目录: {}", e).as_str());
        std::process::exit(1);
    }
    success(format!("工作目录切换到: {}", dir).as_str());
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
            Arg::new("select-page")
                .short('p')
                .long("select-page")
                .value_name("PAGE")
                .help("选择分P或集数，例如 8、1,2、3-5、ALL、LAST，默认 ALL"),
        )
        .arg(
            arg!(-w --workdir <DIR> "工作目录，默认为当前目录，目录必须存在才能使用")
                .required(false)
                .global(true),
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
        .arg(
            Arg::new("no-cover")
                .long("no-cover")
                .action(ArgAction::SetTrue)
                .help("不下载封面，也不写入视频"),
        )
        .arg(
            Arg::new("keep-cover")
                .long("keep-cover")
                .action(ArgAction::SetTrue)
                .help("封面写入视频后，仍把图片留在旁边"),
        )
        .arg(arg!(
            --debug "启用调试模式，输出更多日志"
        ))
        .subcommand(login())
        .subcommand(whoami())
        .subcommand(cover_command())
}

fn login() -> Command {
    Command::new("login").about("登录BILIBILI账号")
}

fn whoami() -> Command {
    Command::new("whoami")
        .about("认证并显示当前登录账号信息")
        .alias("me")
}

fn cover_command() -> Command {
    Command::new("cover")
        .about("只保存封面图片，不下载视频")
        .arg(arg!(<url> "视频、番剧、课堂、合集、系列或收藏夹链接"))
}
