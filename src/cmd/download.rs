use crate::cmd::out::{error, info, success, warn};
use bbdd::{BBDDError, BBDDResult};
use dialoguer::Confirm;
use futures::future;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::Path;
use tokio::fs;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

pub(crate) async fn download_avid(avid: i64) -> i32 {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let quality = *super::QUALITY_PREFERENCE.get().unwrap();
    let video_info = match client.fetch_video_info(avid).await {
        Ok(video_info) => video_info,
        Err(err) => {
            error(format!("无法获取视频信息: {:?}", err).as_str());
            return 1;
        }
    };
    let page = video_info.pages.first().expect("视频分P信息为空");
    info(format!("匹配到视频 : {}", video_info.title,).as_str());
    let file_title = file_title(&video_info.title);
    let merge_file = format!("{}.mp4", file_title);
    if !continue_download(merge_file.as_str()) {
        return 0;
    }
    let play_url = match client.play_url(avid, page.cid).await {
        Ok(play_url) => play_url,
        Err(err) => {
            error(format!("无法获取视频播放地址: {:?}", err).as_str());
            return 1;
        }
    };
    let video = if let Some(quality) = quality
        && let Some(v) = play_url
            .dash
            .video
            .iter()
            .filter(|v| v.id <= quality)
            .collect::<Vec<_>>()
            .first()
    {
        (*v).clone()
    } else {
        match play_url
            .dash
            .video
            .first()
            .ok_or(BBDDError::StateError("视频下载地址列表为空".to_string()))
        {
            Ok(first) => first,
            Err(err) => {
                error(format!("无法获取视频下载地址: {:?}", err).as_str());
                return 1;
            }
        }
        .clone()
    };
    let audio = match play_url.dash.audio.first() {
        None => {
            error("无法获取音频下载地址: 音频下载地址列表为空");
            return 1;
        }
        Some(audio) => audio,
    };
    let video_url = video.base_url.as_str();
    let audio_url = audio.base_url.as_str();
    let video_id = video.id;
    let audio_id = audio.id;
    let video_file = format!("{}.video.{}", file_title, video_id);
    let audio_file = format!("{}.audio.{}", file_title, audio_id);

    info(format!("开始下载: “{}”", video_info.title).as_str());

    let result = download_and_cache_files(vec![
        (audio_file.as_str(), audio_url, "音频"),
        (video_file.as_str(), video_url, "视频"),
    ])
    .await;

    if let Err(_e) = result {
        return 1;
    }
    match merge_files(
        vec![video_file.as_str(), audio_file.as_str()],
        merge_file.as_str(),
    )
    .await
    {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

pub(crate) async fn download_ep(ep_id: i64) -> i32 {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let ep_info = match client.fetch_ep_info(ep_id).await {
        Ok(ep_info) => ep_info,
        Err(err) => {
            error(format!("无法获取EP信息: {:?}", err).as_str());
            return 1;
        }
    };
    info(
        format!(
            "匹配到EP: {} (共{}个视频)",
            ep_info.season_title,
            ep_info.episodes.len()
        )
        .as_str(),
    );
    let folder_name = file_title(&ep_info.season_title);
    let folder_path = Path::new(folder_name.as_str());
    if !folder_path.exists() {
        if let Err(e) = fs::create_dir(folder_path).await {
            error(format!("无法创建目录 {}: {}", folder_name, e).as_str());
            return 1;
        }
    }
    if let Err(e) = std::env::set_current_dir(folder_path) {
        error(format!("无法切换工作目录: {}", e).as_str());
        return 1;
    } else {
        success(format!("工作目录切换到: {}", folder_name).as_str());
    }
    let quality = *super::QUALITY_PREFERENCE.get().unwrap();
    let mut failed_episodes = Vec::new();
    let mut success_episodes = Vec::new();
    for x in &ep_info.episodes {
        let file_title = file_title(&x.show_title);
        let merge_file_name = format!("{}.mp4", file_title);
        if !continue_download(merge_file_name.as_str()) {
            continue;
        }
        let play_url = match client
            .play_url_ep(
                x.aid,
                x.cid,
                ep_id,
                if let Some(quality) = quality {
                    quality
                } else {
                    127
                },
            )
            .await
        {
            Ok(url) => url,
            Err(e) => {
                error(format!("无法获取视频 {} 的播放地址: {:?}", x.title, e).as_str());
                failed_episodes.push(x);
                continue;
            }
        };
        let video = play_url.dash.video.first().expect("无法获取视频下载地址");
        let audio = play_url.dash.audio.first().expect("无法获取音频下载地址");
        let video_url = video.base_url.as_str();
        let audio_url = audio.base_url.as_str();
        let video_id = video.id;
        let audio_id = audio.id;
        let video_file = format!("{}.video.{}", file_title, video_id);
        let audio_file = format!("{}.audio.{}", file_title, audio_id);
        info(format!("开始下载: “{}”", file_title).as_str());
        let result = download_and_cache_files(vec![
            (audio_file.as_str(), audio_url, "音频"),
            (video_file.as_str(), video_url, "视频"),
        ])
        .await;
        if let Err(_) = result {
            failed_episodes.push(x);
            continue;
        }
        if let Err(_) = merge_files(
            vec![video_file.as_str(), audio_file.as_str()],
            merge_file_name.as_str(),
        )
        .await
        {
            failed_episodes.push(x);
            continue;
        }
        success_episodes.push(x);
    }
    // todo: max failed for interrupt
    // todo: max retry
    if failed_episodes.len() == 0 {
        0
    } else {
        if success_episodes.len() == 0 { 1 } else { 2 }
    }
}

async fn merge_files(input_files: Vec<&str>, output_file: &str) -> BBDDResult<()> {
    info(format!("开始合并文件到: {}", output_file).as_str());
    let result = super::ffmpeg::ffmpeg_api::ffmpeg_merge_files(input_files.clone(), output_file);
    match result {
        Ok(_) => {
            for file in &input_files {
                let _ = fs::remove_file(file).await;
            }
            success(format!("合并完成: {}", output_file).as_str());
            Ok(())
        }
        Err(err) => {
            let _ = fs::remove_file(output_file).await;
            cleanup_temp_files_on_fail(input_files).await;
            error(format!("合并失败: {:?}", err).as_str());
            Err(BBDDError::StateError(format!("合并失败: {:?}", err)))
        }
    }
}

async fn cleanup_temp_files_on_fail(files: Vec<&str>) {
    let continue_cache = *super::CONTINUE_CACHE.get().unwrap();
    if !continue_cache {
        for file in files {
            let _ = fs::remove_file(file).await;
        }
    }
}

fn continue_download(file_name: &str) -> bool {
    let overwrite_mode = super::OVERWRITE_MODE.get().unwrap();
    let path = Path::new(file_name);
    if path.exists() {
        match overwrite_mode {
            super::OverwriteMode::Skip => {
                info(format!("文件 “{}” 已存在，跳过下载", file_name).as_str());
                false
            }
            super::OverwriteMode::Overwrite => {
                info(format!("文件 “{}” 已存在，强制重新下载", file_name).as_str());
                true
            }
            super::OverwriteMode::Ask => {
                let confirm = Confirm::new()
                    .with_prompt(format!("文件 “{}” 已存在，是否重新下载？", file_name,))
                    .default(false)
                    .interact()
                    .unwrap_or(false);
                if !confirm {
                    warn("取消下载");
                    false
                } else {
                    true
                }
            }
        }
    } else {
        true
    }
}

async fn download_and_cache_files(
    files: Vec<(
        &str, // file_name
        &str, // url
        &str, // label
    )>,
) -> Result<(), Box<dyn std::error::Error>> {
    let result = download_files(files.clone()).await;
    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            error(format!("下载失败: {:?}", e).as_str());
            cleanup_temp_files_on_fail(files.iter().map(|(file_name, _, _)| *file_name).collect())
                .await;
            Err(e)
        }
    }
}

async fn download_files(
    files: Vec<(
        &str, // file_name
        &str, // url
        &str, // label
    )>,
) -> Result<(), Box<dyn std::error::Error>> {
    let continue_cache = *super::CONTINUE_CACHE.get().unwrap();
    let client = super::client::CLIENT_CELL.get().unwrap();
    // 获取每个文件的长度并创建进度条
    let mut bars = Vec::new();
    let m = MultiProgress::new();
    for (file_name, url, label) in files {
        let path = Path::new(file_name);
        let (file, resp, file_len, len) = if continue_cache && path.exists() {
            let md = std::fs::metadata(path)?;
            let file_len = md.len();
            let resp = client.download_resource_head(&url).await?;
            let len_total = resp
                .error_for_status()?
                .headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or("无法获取文件大小，无法继续下载")?;
            if file_len >= len_total {
                info(
                    format!(
                        "文件 “{}” 已下载, 文件大小 {}(SERVER) {}(LOCAL)",
                        file_name, len_total, file_len
                    )
                    .as_str(),
                );
                continue;
            }
            info(format!("文件 {} 续传", file_name).as_str());
            let mut file = tokio::fs::OpenOptions::new()
                .append(true)
                .open(path)
                .await?;
            file.seek(std::io::SeekFrom::Start(file_len)).await?;
            let resp = client
                .download_resource_with_range(&url, file_len, None)
                .await?
                .error_for_status()?;
            (file, resp, file_len, len_total)
        } else {
            let file = fs::File::create(path).await?;
            let resp = client.download_resource(url).await?.error_for_status()?;
            let file_len = 0;
            let resp_len = resp
                .content_length()
                .ok_or("无法获取文件长度，无法继续下载")?;
            (file, resp, file_len, resp_len)
        };
        let pb = m.add(ProgressBar::new(len));
        pb.set_style(
            ProgressStyle::default_bar()
                .template(&format!(
                    "{{msg}} [{{bar:40.cyan/blue}}] {{bytes}}/{{total_bytes}} ({{eta}})"
                ))?
                .progress_chars("##-"),
        );
        pb.set_message(label.to_string());
        bars.push((file, pb, resp, len, file_len));
    }
    // 下载任务
    let tasks: Vec<_> = bars
        .into_iter()
        .map(|(mut file, pb, mut resp, len, file_len)| async move {
            if file_len > 0 {
                pb.set_position(file_len);
            }
            if file_len >= len {
                pb.finish();
                return Ok::<_, Box<dyn std::error::Error>>(());
            }
            let mut cum: usize = 0;
            while let Some(chunk) = resp.chunk().await? {
                file.write_all(&chunk).await?;
                cum += chunk.len();
                if cum > 1 << 20 {
                    pb.inc(cum as u64);
                    cum = 0;
                }
            }
            pb.inc(cum as u64);
            pb.finish();
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .collect();
    let _ = future::try_join_all(tasks).await?;
    Ok(())
}

fn file_title(title: &str) -> String {
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut file_title = title.to_string();
    for ch in invalid_chars.iter() {
        file_title = file_title.replace(*ch, "_");
    }
    file_title
}
