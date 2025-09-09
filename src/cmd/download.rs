use crate::cmd::error_exit;
use dialoguer::Confirm;
use std::path::Path;
use tokio::io::AsyncWriteExt;
use futures::future;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::fs;
use crate::cmd::out::{error, info, success};

pub(crate) async fn download_avid(avid: i64) {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let video_info = error_exit(client.fetch_video_info(avid).await);
    let page = video_info.pages.first().expect(
        "视频分P信息为空，可能是番剧、电影等非普通视频，请使用对应的链接进行下载，如番剧请使用ep链接",
    );
    let file_title = file_title(&video_info.title);
    let merge_file = format!("{}.mp4", file_title);
    let video_file = format!("{}.video", file_title);
    let audio_file = format!("{}.audio", file_title);
    let merge_file_path = Path::new(merge_file.as_str());
    if merge_file_path.exists() {
        let confirm = Confirm::new()
            .with_prompt(format!("文件 {} 已存在，是否重新下载？", merge_file))
            .default(false)
            .interact()
            .unwrap_or(false);
        if !confirm {
            println!("取消下载");
            return;
        }
    }
    let play_url = error_exit(client.play_url(avid, page.cid).await);
    let video_url = play_url.dash.video.first().unwrap().base_url.as_str();
    let audio_url = play_url.dash.audio.first().unwrap().base_url.as_str();

    info(format!("开始下载: {}", video_info.title).as_str());

    let result = download_files(vec![
        (audio_file.as_str(), audio_url, "音频"),
        (video_file.as_str(), video_url, "视频"),
    ]).await;

    if let Err(e) = result {
        error(format!("下载失败: {:?}", e).as_str());
        let _ = fs::remove_file(&video_file).await;
        let _ = fs::remove_file(&audio_file).await;
        return;
    }
    info("下载完成，开始合并音视频");
    let result = super::ffmpeg::ffmpeg_api::ffmpeg_merge_files(
        vec![video_file.as_str(), audio_file.as_str()],
        merge_file.as_str(),
    );
    match result {
        Ok(_) => {
            let _ = fs::remove_file(&video_file).await;
            let _ = fs::remove_file(&audio_file).await;
            success("所有操作完成");
        }
        Err(err) => {
            let _ = fs::remove_file(&merge_file).await;
            error(format!("合并失败: {:?}", err).as_str());
            error("临时文件未删除");
        }
    }
}

async fn download_files(
    files: Vec<(
        &str, // file_name
        &str, // url
        &str, // label
    )>
) -> Result<(), Box<dyn std::error::Error>> {
    let client = super::client::CLIENT_CELL.get().unwrap();
    // 获取每个文件的长度并创建进度条
    let mut bars = Vec::new();
    let m = MultiProgress::new();
    for (file_name, url, label) in files {
        let resp = client.download_resource(&url).await?;
        let len = resp.content_length().unwrap_or(0);
        let pb = m.add(ProgressBar::new(len));
        pb.set_style(ProgressStyle::default_bar()
            .template(&format!("{{msg}} [{{bar:40.cyan/blue}}] {{bytes}}/{{total_bytes}} ({{eta}})"))?
            .progress_chars("##-"));
        pb.set_message(label.to_string());
        bars.push((file_name, pb, resp, len));
    }
    // 下载任务
    let tasks: Vec<_> = bars.into_iter().map(|(file_name, pb,mut resp, _len)| {
        async move {
            let mut file = fs::File::create(&file_name).await?;
            while let Some(chunk) = resp.chunk().await? {
                file.write_all(&chunk).await?;
                pb.inc(chunk.len() as u64);
            }
            pb.finish();
            Ok::<_, Box<dyn std::error::Error>>(())
        }
    }).collect();
    let _ = future::try_join_all(tasks).await?;
    Ok(())
}

pub(crate) async fn download_ep(_epid: i64) {}

fn file_title(title: &str) -> String {
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut file_title = title.to_string();
    for ch in invalid_chars.iter() {
        file_title = file_title.replace(*ch, "_");
    }
    file_title
}
