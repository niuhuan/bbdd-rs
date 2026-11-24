use crate::cmd::out::{error, info, success, warn};
use bbdd::parse::select_page_indexes;
use bbdd::{BBDDError, BBDDResult};
use dialoguer::Confirm;
use futures::future;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::cmp::Reverse;
use std::path::Path;
use std::time::Duration;
use tokio::fs;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

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
    for job in jobs {
        if !continue_download(&format!("{}.mp4", job.stem)) {
            continue;
        }
        match download_job_with_retry(&job).await {
            Ok(()) => succeeded += 1,
            Err(message) => {
                error(format!("“{}” 下载失败: {}", job.label, message).as_str());
                failed += 1;
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
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
    Err(last_error)
}

async fn download_job(job: &PageJob) -> Result<(), String> {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let quality = super::QUALITY_PREFERENCE.get().copied().unwrap_or(None);
    let qn = quality.unwrap_or(127);
    let play_url = if job.cheese {
        client
            .play_url_cheese(job.aid, job.cid, job.ep_id.unwrap_or(0), qn)
            .await
    } else if let Some(ep_id) = job.ep_id {
        client.play_url_ep(job.aid, job.cid, ep_id, qn).await
    } else {
        client.play_url_with_qn(job.aid, job.cid, qn).await
    }
    .map_err(|err| format!("无法获取播放地址: {:?}", err))?;
    let video = select_video(&play_url, quality).map_err(|err| format!("{:?}", err))?;
    let audio = select_audio(&play_url).map_err(|err| format!("{:?}", err))?;
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
    let video_urls = bbdd::download::prepare_media_urls(&media_urls(&video));
    let audio_urls = bbdd::download::prepare_media_urls(&media_urls(&audio));
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
    let cover_path = download_cover(job).await;
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
        return None;
    }
    let path = format!("{}.{}", job.stem, cover_extension(&url));
    let client = super::client::CLIENT_CELL.get().unwrap();
    match download_bytes(client, &url).await {
        Ok(bytes) => match fs::write(&path, bytes).await {
            Ok(()) => {
                info(format!("封面已保存: {}", path).as_str());
                Some(path)
            }
            Err(err) => {
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

async fn download_bytes(
    client: &bbdd::BBDD,
    url: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let response = client.download_resource(url).await?.error_for_status()?;
    Ok(response.bytes().await?.to_vec())
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

fn select_video(
    play_url: &bbdd::fetcher::VideoPlayUrl,
    quality: Option<i64>,
) -> BBDDResult<bbdd::fetcher::VideoMedia> {
    let selected = if let Some(q) = quality {
        play_url
            .dash
            .video
            .iter()
            .filter(|v| v.id == q)
            .max_by_key(|v| (codec_rank(&v.codecs), Reverse(v.bandwidth)))
            .or_else(|| {
                play_url
                    .dash
                    .video
                    .iter()
                    .filter(|v| v.id <= q)
                    .max_by_key(|v| (v.id, codec_rank(&v.codecs), Reverse(v.bandwidth)))
            })
            .or_else(|| play_url.dash.video.iter().min_by_key(|v| v.id))
    } else {
        play_url
            .dash
            .video
            .iter()
            .max_by_key(|v| (v.id, codec_rank(&v.codecs), Reverse(v.bandwidth)))
    };
    selected
        .cloned()
        .ok_or(BBDDError::StateError("视频下载地址列表为空".to_string()))
}

fn select_audio(play_url: &bbdd::fetcher::VideoPlayUrl) -> BBDDResult<bbdd::fetcher::VideoMedia> {
    play_url
        .dash
        .audio
        .iter()
        .max_by_key(|audio| (audio_codec_rank(&audio.codecs), audio.bandwidth))
        .cloned()
        .ok_or(BBDDError::StateError("音频下载地址列表为空".to_string()))
}

fn media_urls(media: &bbdd::fetcher::VideoMedia) -> Vec<String> {
    let mut urls = vec![media.base_url.clone()];
    urls.extend(media.backup_url.clone());
    urls
}

fn codec_rank(codecs: &str) -> i32 {
    let codecs_lower = codecs.to_ascii_lowercase();
    if codecs_lower.contains("av01") {
        3
    } else if codecs_lower.contains("hev1")
        || codecs_lower.contains("hvc1")
        || codecs_lower.contains("hevc")
    {
        2
    } else if codecs_lower.contains("avc") || codecs_lower.contains("h264") {
        1
    } else {
        0
    }
}

fn audio_codec_rank(codecs: &str) -> i32 {
    let codecs_lower = codecs.to_ascii_lowercase();
    if codecs_lower.contains("flac") {
        3
    } else if codecs_lower.contains("ec-3")
        || codecs_lower.contains("eac3")
        || codecs_lower.contains("ac-3")
    {
        2
    } else if codecs_lower.contains("mp4a") || codecs_lower.contains("aac") {
        1
    } else {
        0
    }
}

async fn merge_files(
    input_files: Vec<&str>,
    output_file: &str,
    cover: Option<&str>,
) -> BBDDResult<()> {
    info(format!("开始合并文件到: {}", output_file).as_str());
    let result =
        super::ffmpeg::ffmpeg_api::ffmpeg_merge_files(input_files.clone(), output_file, cover);
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
            error(format!("下载失败: {:?}", e).as_str());
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
    let mut bars = Vec::new();
    let m = MultiProgress::new();
    for (file_name, urls, label) in files {
        let path = Path::new(file_name);
        let Some((file, resp, file_len, len)) =
            open_with_backup(client, path, urls, continue_cache, file_name).await?
        else {
            continue;
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

async fn open_with_backup(
    client: &bbdd::BBDD,
    path: &Path,
    urls: Vec<String>,
    continue_cache: bool,
    file_name: &str,
) -> Result<Option<(fs::File, reqwest::Response, u64, u64)>, Box<dyn std::error::Error>> {
    let mut last_err: Option<String> = None;
    for url in urls {
        if continue_cache && path.exists() {
            let md = std::fs::metadata(path)?;
            let file_len = md.len();
            let resp_head = match client.download_resource_head(&url).await {
                Ok(v) => v,
                Err(e) => {
                    last_err = Some(format!("{:?}", e));
                    continue;
                }
            };
            let len_total = resp_head
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
                return Ok(None);
            }
            info(format!("文件 {} 续传 ({} -> {})", file_name, file_len, len_total).as_str());
            let mut file = tokio::fs::OpenOptions::new()
                .append(true)
                .open(path)
                .await?;
            file.seek(std::io::SeekFrom::Start(file_len)).await?;
            match client
                .download_resource_with_range(&url, file_len, None)
                .await
            {
                Ok(resp) => {
                    let resp = resp.error_for_status()?;
                    return Ok(Some((file, resp, file_len, len_total)));
                }
                Err(e) => {
                    last_err = Some(format!("{:?}", e));
                    continue;
                }
            }
        } else {
            let file = fs::File::create(path).await?;
            match client.download_resource(&url).await {
                Ok(resp) => {
                    let resp = resp.error_for_status()?;
                    let len = resp
                        .content_length()
                        .ok_or("无法获取文件长度，无法继续下载")?;
                    return Ok(Some((file, resp, 0, len)));
                }
                Err(e) => {
                    last_err = Some(format!("{:?}", e));
                    continue;
                }
            }
        }
    }
    let err_msg = last_err.unwrap_or_else(|| "未找到可用的下载链接".to_string());
    Err(err_msg.into())
}

fn file_title(title: &str) -> String {
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let mut file_title = title.to_string();
    for ch in invalid_chars.iter() {
        file_title = file_title.replace(*ch, "_");
    }
    file_title.trim().to_string()
}

fn normalize_resource_url(url: &str) -> String {
    let url = url.trim();
    if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url.to_string()
    }
}

fn cover_extension(url: &str) -> &'static str {
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
