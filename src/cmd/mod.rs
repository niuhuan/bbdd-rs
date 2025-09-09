mod client;
mod local;
mod login;
mod out;
mod download;
mod ffmpeg;

use crate::cmd::out::error;
use clap::{Command, arg};
use bbdd::parse::VideoType;

pub(crate) async fn main() {
    #[cfg(not(feature = "rsmpeg"))]
    ffmpeg::ffmpeg_api::ffmpeg_run_version();
    client::init_client(local::init_dir()).await;
    let matches = cli().get_matches();
    match matches.subcommand() {
        Some(("login", _)) => login::login().await,
        _ => {
            if let Some(url) = matches.get_one::<String>("url") {
                let client = client::CLIENT_CELL.get().unwrap();
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
        .subcommand(login())
}

fn login() -> Command {
    Command::new("login").about("登录BILIBILI账号")
}
