use crate::cmd::out::{error, info, success, warn};
use bbdd::parse::select_page_indexes;
use bbdd::progress::{Control, Stage, Update};
use bbdd::{BBDDError, BBDDResult};
use dialoguer::Confirm;
use futures::future;
use std::io::{IsTerminal, Write};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::sync::mpsc;

const MAX_ATTEMPTS: u32 = 3;

struct PageJob {
    aid: i64,
    cid: i64,
    ep_id: Option<i64>,
    cheese: bool,
    index: i64,
    stem: String,
    cover_url: String,
    label: String,
}

pub(crate) async fn download_avid(avid: i64) -> i32 {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let video_info = match client.fetch_video_info(avid).await {
        Ok(video_info) => video_info,
        Err(err) => {
            error(format!("无法获取视频信息: {:?}", err).as_str());
            return 1;
        }
    };
    if video_info.pages.is_empty() {
        error("视频分P信息为空");
        return 1;
    }
    let total = video_info.pages.len();
    let multi = total > 1;
    info(format!("匹配到视频 : {} (共{}个分P)", video_info.title, total).as_str());
    let jobs = video_info
        .pages
        .iter()
        .enumerate()
        .map(|(offset, page)| {
            let index = offset as i64 + 1;
            let raw = if page.part.is_empty() {
                video_info.title.clone()
            } else {
                page.part.clone()
            };
            let stem = if multi {
                format!("[P{:02}]{}", index, file_title(&raw))
            } else {
                file_title(&video_info.title)
            };
            PageJob {
                aid: avid,
                cid: page.cid,
                ep_id: None,
                cheese: false,
                index,
                stem,
                cover_url: video_info.pic.clone(),
                label: format!("P{index} {raw}"),
            }
        })
        .collect();
    let folder = if multi {
        Some(video_info.title.clone())
    } else {
        None
    };
    download_jobs(&video_info.title, folder, jobs).await
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
    let jobs = ep_info
        .episodes
        .iter()
        .enumerate()
        .map(|(offset, episode)| {
            let title = if !episode.show_title.is_empty() {
                episode.show_title.clone()
            } else if !episode.title.is_empty() {
                episode.title.clone()
            } else {
                episode.long_title.clone()
            };
            let episode_id = if episode.ep_id != 0 {
                episode.ep_id
            } else {
                episode.id
            };
            PageJob {
                aid: episode.aid,
                cid: episode.cid,
                ep_id: Some(episode_id),
                cheese: false,
                index: offset as i64 + 1,
                stem: file_title(&title),
                cover_url: if episode.cover.is_empty() {
                    ep_info.cover.clone()
                } else {
                    episode.cover.clone()
                },
                label: title,
            }
        })
        .collect();
    download_jobs(
        &ep_info.season_title,
        Some(ep_info.season_title.clone()),
        jobs,
    )
    .await
}

pub(crate) async fn download_cheese(ep_id: i64) -> i32 {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let cheese = match client.fetch_cheese_info(ep_id).await {
        Ok(cheese) => cheese,
        Err(err) => {
            error(format!("无法获取课堂信息: {:?}", err).as_str());
            return 1;
        }
    };
    info(
        format!(
            "匹配到课堂: {} (共{}节)",
            cheese.title,
            cheese.episodes.len()
        )
        .as_str(),
    );
    let jobs = cheese
        .episodes
        .iter()
        .enumerate()
        .map(|(offset, episode)| PageJob {
            aid: episode.aid,
            cid: episode.cid,
            ep_id: Some(episode.ep_id),
            cheese: true,
            index: offset as i64 + 1,
            stem: file_title(&episode.title),
            cover_url: episode.cover.clone(),
            label: episode.title.clone(),
        })
        .collect();
    download_jobs(&cheese.title, Some(cheese.title.clone()), jobs).await
}

pub(crate) async fn download_media_list(biz_id: &str, series: bool) -> i32 {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let info = match client.fetch_media_list(biz_id, series).await {
        Ok(info) => info,
        Err(err) => {
            error(
                format!(
                    "无法获取{}信息: {:?}",
                    if series { "系列" } else { "合集" },
                    err
                )
                .as_str(),
            );
            return 1;
        }
    };
    download_media_items(&info.title, info.items).await
}

pub(crate) async fn download_fav(fid: &str, mid: &str) -> i32 {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let info = match client.fetch_fav_list(fid, mid).await {
        Ok(info) => info,
        Err(err) => {
            error(format!("无法获取收藏夹信息: {:?}", err).as_str());
            return 1;
        }
    };
    download_media_items(&info.title, info.items).await
}

async fn download_media_items(title: &str, items: Vec<bbdd::fetcher::MediaListItem>) -> i32 {
    info(format!("匹配到列表: {} (共{}个视频)", title, items.len()).as_str());
    let total = items.len();
    let jobs = items
        .into_iter()
        .enumerate()
        .map(|(offset, item)| {
            let index = offset as i64 + 1;
            let stem = if total > 1 {
                format!("[P{:02}]{}", index, file_title(&item.title))
            } else {
                file_title(&item.title)
            };
            PageJob {
                aid: item.aid,
                cid: item.cid,
                ep_id: None,
                cheese: false,
                index,
                stem,
                cover_url: item.cover,
                label: item.title,
            }
        })
        .collect();
    download_jobs(title, Some(title.to_string()), jobs).await
}

async fn download_jobs(title: &str, folder: Option<String>, jobs: Vec<PageJob>) -> i32 {
    if jobs.is_empty() {
        error("没有可下载的视频");
        return 1;
    }
    let total = jobs.len() as i64;
    let spec = super::SELECT_PAGE
        .get()
        .map(|value| value.as_str())
        .unwrap_or("ALL");
    let selected = match select_page_indexes(spec, total) {
        Ok(selected) => selected,
        Err(err) => {
            error(format!("{}", err).as_str());
            return 1;
        }
    };
    let jobs: Vec<PageJob> = jobs
        .into_iter()
        .filter(|job| selected.contains(&job.index))
        .collect();
    info(
        format!(
            "“{}” 共计 {} 项, 已选择: {}",
            title,
            total,
            selected
                .iter()
                .map(|index| index.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
        .as_str(),
    );
    if let Some(folder) = folder {
        if let Err(message) = enter_folder(&folder).await {
            error(message.as_str());
            return 1;
        }
    }
    let client = super::client::CLIENT_CELL.get().unwrap();
    if client.cookie.is_empty() {
        warn("未登录，跳过字幕下载");
    }
    let mut failed = 0usize;
    let mut succeeded = 0usize;
    let _busy = super::Busy::enter();
    for job in jobs {
        if !continue_download(&format!("{}.mp4", job.stem)) {
            continue;
        }
        match download_job_with_retry(&job).await {
            Ok(()) => succeeded += 1,
            Err(message) => {
                error(format!("“{}” 下载失败: {}", job.label, message).as_str());
                failed += 1;
                if super::is_cancelled() {
                    break;
                }
            }
        }
    }
    if failed == 0 {
        0
    } else if succeeded == 0 {
        1
    } else {
        2
    }
}

async fn download_job_with_retry(job: &PageJob) -> Result<(), String> {
    let mut last_error = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        match download_job(job).await {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_error = err;
                if super::is_cancelled() {
                    return Err("已取消".to_string());
                }
                if attempt == MAX_ATTEMPTS {
                    break;
                }
                warn(
                    format!(
                        "“{}” 下载出现异常, 3秒后重试 ({}/{}): {}",
                        job.label, attempt, MAX_ATTEMPTS, last_error
                    )
                    .as_str(),
                );
                let cancel = super::cancel_token();
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return Err("已取消".to_string()),
                    _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                }
            }
        }
    }
    Err(last_error)
}

async fn download_job(job: &PageJob) -> Result<(), String> {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let quality = super::QUALITY_PREFERENCE.get().copied().unwrap_or(None);
    let qn = quality.unwrap_or(127);
    let cancel = super::cancel_token();
    let play_url = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err("已取消".to_string()),
        play_url = async {
            if job.cheese {
                client
                    .play_url_cheese(job.aid, job.cid, job.ep_id.unwrap_or(0), qn)
                    .await
            } else if let Some(ep_id) = job.ep_id {
                client.play_url_ep(job.aid, job.cid, ep_id, qn).await
            } else {
                client.play_url_with_qn(job.aid, job.cid, qn).await
            }
        } => play_url.map_err(|err| format!("无法获取播放地址: {:?}", err))?,
    };
    let video = select_video(&play_url, quality).map_err(|err| format!("{:?}", err))?;
    let audio = select_audio(&play_url).map_err(|err| format!("{:?}", err))?;
    if video.id < qn {
        warn(
            format!(
                "请求清晰度 {}，接口实际可下载: {}",
                bbdd::util::video_quality_to_string(qn as i32),
                available_video_qualities(&play_url)
            )
            .as_str(),
        );
    }
    info(
        format!(
            "选择清晰度: {} ({}x{}, {}, video_bandwidth={}, audio_bandwidth={}, audio_codecs={})",
            bbdd::util::video_quality_to_string(video.id as i32),
            video.width,
            video.height,
            video.codecs,
            video.bandwidth,
            audio.bandwidth,
            audio.codecs
        )
        .as_str(),
    );
    let video_urls = bbdd::download::prepare_media_urls(&bbdd::select::media_urls(&video));
    let audio_urls = bbdd::download::prepare_media_urls(&bbdd::select::media_urls(&audio));
    let video_file = format!("{}.video.{}", job.stem, video.id);
    let audio_file = format!("{}.audio.{}", job.stem, audio.id);
    let merge_file = format!("{}.mp4", job.stem);
    info(format!("开始下载: “{}”", job.label).as_str());
    download_and_cache_files(vec![
        (audio_file.as_str(), audio_urls, "音频"),
        (video_file.as_str(), video_urls, "视频"),
    ])
    .await
    .map_err(|err| format!("{:?}", err))?;
    let cover_path = if super::download_cover() {
        download_cover(job).await
    } else {
        None
    };
    download_subtitles(job).await;
    merge_files(
        vec![video_file.as_str(), audio_file.as_str()],
        merge_file.as_str(),
        cover_path.as_deref(),
    )
    .await
    .map_err(|err| format!("{:?}", err))
}

async fn download_cover(job: &PageJob) -> Option<String> {
    let url = normalize_resource_url(&job.cover_url);
    if url.is_empty() {
        warn("没有封面地址，跳过");
        return None;
    }
    let path = format!("{}.{}", job.stem, cover_extension(&url));
    let client = super::client::CLIENT_CELL.get().unwrap();
    match download_bytes(client, &url).await {
        Ok(bytes) => match fs::write(&path, bytes).await {
            Ok(()) => Some(path),
            Err(err) => {
                let _ = fs::remove_file(&path).await;
                warn(format!("保存封面失败: {}", err).as_str());
                None
            }
        },
        Err(err) => {
            warn(format!("下载封面失败: {}", err).as_str());
            None
        }
    }
}

async fn download_subtitles(job: &PageJob) {
    let client = super::client::CLIENT_CELL.get().unwrap();
    if client.cookie.is_empty() {
        return;
    }
    let tracks = match client.fetch_subtitles(job.aid, job.cid).await {
        Ok(tracks) => tracks,
        Err(err) => {
            warn(format!("获取字幕失败: {:?}", err).as_str());
            return;
        }
    };
    if tracks.is_empty() {
        info(format!("“{}” 未找到字幕，跳过", job.label).as_str());
        return;
    }
    for track in tracks {
        let url = normalize_resource_url(&track.url);
        let bytes = match download_bytes(client, &url).await {
            Ok(bytes) => bytes,
            Err(err) => {
                warn(format!("下载字幕 {} 失败: {}", track.lan, err).as_str());
                continue;
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        let (extension, body) = if url.contains(".ass") && !text.trim_start().starts_with('{') {
            ("ass", text.to_string())
        } else {
            match bbdd::fetcher::subtitle_json_to_srt(&text) {
                Ok(srt) => ("srt", srt),
                Err(err) => {
                    warn(format!("转换字幕 {} 失败: {:?}", track.lan, err).as_str());
                    continue;
                }
            }
        };
        let path = format!("{}.{}.{}", job.stem, file_title(&track.lan), extension);
        if let Err(err) = fs::write(&path, body).await {
            warn(format!("保存字幕失败: {}", err).as_str());
        } else {
            info(format!("字幕已保存: {}", path).as_str());
        }
    }
}

async fn merge_now(
    input_files: &[&str],
    output_file: &str,
    cover: Option<&str>,
    control: &Control,
) -> Result<bool, Box<dyn std::error::Error>> {
    #[cfg(feature = "rsmpeg")]
    {
        return bbdd::merge::merge_files(input_files, output_file, cover, control)
            .await
            .map_err(|err| Box::new(err) as _);
    }
    #[cfg(not(feature = "rsmpeg"))]
    {
        if cover.is_some() {
            match super::ffmpeg::merge_command(input_files, output_file, cover, &control.cancel)
                .await
            {
                Ok(()) => return Ok(true),
                Err(err) if control.cancel.is_cancelled() => return Err(err),
                Err(err) => {
                    warn(format!("写入封面失败，改为只合并音视频: {}", err).as_str());
                    let _ = fs::remove_file(output_file).await;
                }
            }
        }
        super::ffmpeg::merge_command(input_files, output_file, None, &control.cancel).await?;
        Ok(false)
    }
}

pub(crate) async fn download_bytes(
    client: &bbdd::BBDD,
    url: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let cancel = super::cancel_token();
    let response = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(BBDDError::Cancelled.into()),
        response = client.download_resource(url) => response?.error_for_status()?,
    };
    let bytes = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(BBDDError::Cancelled.into()),
        bytes = response.bytes() => bytes?,
    };
    Ok(bytes.to_vec())
}

async fn enter_folder(name: &str) -> Result<(), String> {
    let folder_name = file_title(name);
    let folder_name = if folder_name.is_empty() {
        "download".to_string()
    } else {
        folder_name
    };
    let folder_path = Path::new(&folder_name);
    if !folder_path.exists() {
        fs::create_dir(folder_path)
            .await
            .map_err(|err| format!("无法创建目录 {}: {}", folder_name, err))?;
    }
    std::env::set_current_dir(folder_path).map_err(|err| format!("无法切换工作目录: {}", err))?;
    success(format!("工作目录切换到: {}", folder_name).as_str());
    Ok(())
}

fn available_video_qualities(play_url: &bbdd::fetcher::VideoPlayUrl) -> String {
    let mut ids: Vec<i64> = play_url.dash.video.iter().map(|video| video.id).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.reverse();
    if ids.is_empty() {
        return "无".to_string();
    }
    ids.iter()
        .map(|id| bbdd::util::video_quality_to_string(*id as i32))
        .collect::<Vec<_>>()
        .join(", ")
}

fn select_video(
    play_url: &bbdd::fetcher::VideoPlayUrl,
    quality: Option<i64>,
) -> BBDDResult<bbdd::fetcher::VideoMedia> {
    bbdd::select::select_video(play_url, quality)
}

fn select_audio(play_url: &bbdd::fetcher::VideoPlayUrl) -> BBDDResult<bbdd::fetcher::VideoMedia> {
    bbdd::select::select_audio(play_url)
}

async fn merge_files(
    input_files: Vec<&str>,
    output_file: &str,
    cover: Option<&str>,
) -> BBDDResult<()> {
    info(format!("开始合并文件到: {}", output_file).as_str());
    let (control, rx) = Control::channel(super::cancel_token());
    let ui = tokio::spawn(render_progress(rx));
    let result = merge_now(&input_files, output_file, cover, &control).await;
    drop(control);
    let _ = ui.await;
    match result {
        Ok(attached) => {
            for file in &input_files {
                let _ = fs::remove_file(file).await;
            }
            finish_cover(cover, attached).await;
            success(format!("合并完成: {}", output_file).as_str());
            Ok(())
        }
        Err(err) => {
            let _ = fs::remove_file(output_file).await;
            if let Some(path) = cover {
                let _ = fs::remove_file(path).await;
            }
            cleanup_temp_files_on_fail(input_files).await;
            error(format!("合并失败: {:?}", err).as_str());
            Err(BBDDError::StateError(format!("合并失败: {:?}", err)))
        }
    }
}

async fn finish_cover(cover: Option<&str>, attached: bool) {
    let Some(path) = cover else {
        return;
    };
    if attached && super::keep_cover() {
        info(format!("封面已保存: {}", path).as_str());
        return;
    }
    if !attached {
        warn("封面未能写入视频，已跳过");
    }
    let _ = fs::remove_file(path).await;
}

async fn cleanup_temp_files_on_fail(files: Vec<&str>) {
    let continue_cache = super::CONTINUE_CACHE.get().copied().unwrap_or(true);
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
        &str,        // file_name
        Vec<String>, // urls (primary + backup)
        &str,        // label
    )>,
) -> Result<(), Box<dyn std::error::Error>> {
    let result = download_files(files.clone()).await;
    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let message = e
                .downcast_ref::<BBDDError>()
                .map(|err| match err {
                    BBDDError::Cancelled => "已取消".to_string(),
                    other => other.to_string(),
                })
                .unwrap_or_else(|| e.to_string());
            error(format!("下载失败: {message}").as_str());
            cleanup_temp_files_on_fail(files.iter().map(|(file_name, _, _)| *file_name).collect())
                .await;
            Err(e)
        }
    }
}

async fn download_files(
    files: Vec<(
        &str,        // file_name
        Vec<String>, // urls
        &str,        // label
    )>,
) -> Result<(), Box<dyn std::error::Error>> {
    let continue_cache = super::CONTINUE_CACHE.get().copied().unwrap_or(true);
    let client = super::client::CLIENT_CELL.get().unwrap();
    let (control, rx) = Control::channel(super::cancel_token());
    let ui = tokio::spawn(render_progress(rx));
    let tasks: Vec<_> = files
        .into_iter()
        .map(|(file_name, urls, label)| {
            let control = control.clone();
            async move {
                let status = client
                    .download_file(&urls, Path::new(file_name), continue_cache, label, &control)
                    .await?;
                if status.already_complete {
                    info(
                        format!(
                            "文件 “{}” 已下载, 文件大小 {}",
                            file_name, status.bytes
                        )
                        .as_str(),
                    );
                }
                Ok::<_, BBDDError>(())
            }
        })
        .collect();
    let result = future::try_join_all(tasks).await;
    drop(control);
    let _ = ui.await;
    result?;
    Ok(())
}

struct ProgressRow {
    stage: Stage,
    name: String,
    done: u64,
    total: Option<u64>,
    started: Instant,
    initial: u64,
}

async fn render_progress(mut rx: mpsc::Receiver<Update>) {
    let tty = std::io::stderr().is_terminal();
    let mut rows: Vec<ProgressRow> = Vec::new();
    let mut drawn = 0usize;
    let mut last_log = Instant::now() - Duration::from_secs(1);
    while let Some(update) = rx.recv().await {
        let new_row = !rows
            .iter()
            .any(|row| row.stage == update.stage && row.name == update.name);
        if let Some(row) = rows
            .iter_mut()
            .find(|row| row.stage == update.stage && row.name == update.name)
        {
            row.done = update.done;
            row.total = update.total;
        } else {
            if tty && drawn == 0 && update.done > 0 {
                info(
                    format!("{} 从 {} 继续", update.name, human_bytes(update.done)).as_str(),
                );
            }
            rows.push(ProgressRow {
                stage: update.stage,
                name: update.name,
                done: update.done,
                total: update.total,
                started: Instant::now(),
                initial: update.done,
            });
        }
        if tty {
            paint_progress(&rows, &mut drawn);
        } else if new_row || last_log.elapsed() >= Duration::from_secs(1) {
            last_log = Instant::now();
            for row in &rows {
                eprintln!("{}", progress_line(row, 20));
            }
        }
    }
    if tty && drawn > 0 {
        let mut err = std::io::stderr().lock();
        let _ = writeln!(err);
        let _ = err.flush();
    }
}

fn paint_progress(rows: &[ProgressRow], drawn: &mut usize) {
    let cols = console::Term::stderr().size().1 as usize;
    let cols = if cols < 40 { 40 } else { cols };
    let bar_width = (cols / 5).clamp(8, 24);
    let mut err = std::io::stderr().lock();
    if *drawn > 0 {
        let _ = write!(err, "\x1b[{}A", drawn);
    }
    for row in rows {
        let line = fit_width(&progress_line(row, bar_width), cols.saturating_sub(1));
        let _ = write!(err, "\r\x1b[2K{line}\n");
    }
    if rows.len() < *drawn {
        let extra = *drawn - rows.len();
        for _ in 0..extra {
            let _ = write!(err, "\r\x1b[2K\n");
        }
        let _ = write!(err, "\x1b[{}A", extra);
    }
    let _ = err.flush();
    *drawn = rows.len();
}

fn progress_line(row: &ProgressRow, bar_width: usize) -> String {
    let label = match row.stage {
        Stage::Download => row.name.as_str(),
        Stage::Merge => "合并",
    };
    let (done, total) = match row.stage {
        Stage::Download => (
            human_bytes(row.done),
            row.total
                .map(human_bytes)
                .unwrap_or_else(|| "?".to_string()),
        ),
        Stage::Merge => (
            row.done.to_string(),
            row.total
                .map(|total| format!("{total} 包"))
                .unwrap_or_else(|| "?".to_string()),
        ),
    };
    format!(
        "{label} [{}] {done}/{total} ({})",
        progress_bar(row.done, row.total, bar_width),
        progress_eta(row)
    )
}

fn progress_bar(done: u64, total: Option<u64>, width: usize) -> String {
    let Some(total) = total.filter(|total| *total > 0) else {
        return "-".repeat(width);
    };
    let filled = ((done.min(total) as f64 / total as f64) * width as f64).round() as usize;
    let filled = filled.min(width);
    format!("{}{}", "#".repeat(filled), "-".repeat(width - filled))
}

fn progress_eta(row: &ProgressRow) -> String {
    let Some(total) = row.total else {
        return "--".to_string();
    };
    let gained = row.done.saturating_sub(row.initial);
    let elapsed = row.started.elapsed().as_secs_f64();
    if row.done >= total {
        return "完成".to_string();
    }
    if gained == 0 || elapsed < 0.2 {
        return "--".to_string();
    }
    let left = total.saturating_sub(row.done) as f64 / (gained as f64 / elapsed);
    format!("{}s", left.ceil() as u64)
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

fn fit_width(text: &str, width: usize) -> String {
    if console::measure_text_width(text) <= width {
        text.to_string()
    } else {
        console::truncate_str(text, width, "").into_owned()
    }
}

pub(crate) fn file_title(title: &str) -> String {
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut file_title = title.to_string();
    for ch in invalid_chars.iter() {
        file_title = file_title.replace(*ch, "_");
    }
    file_title.trim().to_string()
}

pub(crate) fn normalize_resource_url(url: &str) -> String {
    let url = url.trim();
    if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url.to_string()
    }
}

pub(crate) fn cover_extension(url: &str) -> &'static str {
    let path = url.split('?').next().unwrap_or(url);
    let ext = path.rsplit('.').next().unwrap_or("jpg");
    if ext.eq_ignore_ascii_case("png") {
        "png"
    } else if ext.eq_ignore_ascii_case("webp") {
        "webp"
    } else if ext.eq_ignore_ascii_case("gif") {
        "gif"
    } else {
        "jpg"
    }
}
