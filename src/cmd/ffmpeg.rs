#[cfg(not(feature = "rsmpeg"))]
pub(crate) fn ffmpeg_run_version() {
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.stderr(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.arg("-version");
    if cmd.status().is_err() {
        crate::cmd::out::error("未找到ffmpeg, 请先安装ffmpeg.");
        std::process::exit(1);
    }
}

/// 外部 ffmpeg。进程内合并在 `bbdd::merge`，由下载流程直接调用。
/// 取消时杀掉子进程。封面写成 attached picture。
#[cfg(not(feature = "rsmpeg"))]
pub(crate) async fn merge_command(
    list: &[&str],
    output: &str,
    cover: Option<&str>,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::cmd::out::info;
    use std::process::Stdio;

    info("正在合并文件");
    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.stderr(Stdio::null());
    cmd.stdout(Stdio::null());
    for item in list {
        cmd.arg("-i");
        cmd.arg(item);
    }
    if let Some(cover) = cover {
        cmd.arg("-i");
        cmd.arg(cover);
        for index in 0..list.len() {
            cmd.arg("-map");
            cmd.arg(index.to_string());
        }
        cmd.arg("-map");
        cmd.arg(list.len().to_string());
        cmd.arg("-disposition:v:1");
        cmd.arg("attached_pic");
    }
    cmd.arg("-vcodec");
    cmd.arg("copy");
    cmd.arg("-acodec");
    cmd.arg("copy");
    cmd.arg("-y");
    cmd.arg(output);
    let mut child = cmd.spawn()?;
    tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            let _ = child.kill().await;
            Err(Box::new(bbdd::BBDDError::Cancelled))
        }
        status = child.wait() => {
            let status = status?;
            if status.success() {
                Ok(())
            } else {
                Err(Box::new(bbdd::BBDDError::StateError(format!(
                    "FFMPEG 未能成功运行 : EXIT CODE : {}",
                    status.code().unwrap_or(-1)
                ))))
            }
        }
    }
}
