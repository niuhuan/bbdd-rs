use std::collections::HashSet;

use bbdd::parse::VideoType;
use tokio::fs;

use crate::cmd::download::{cover_extension, download_bytes, file_title, normalize_resource_url};
use crate::cmd::out::{error, info, success, warn};

struct CoverSave {
    stem: String,
    url: String,
}

pub(crate) async fn save(url: &str) -> i32 {
    let url = url.trim();
    if url.is_empty() {
        error("请提供视频、番剧、课堂、合集、系列或收藏夹链接");
        return 1;
    }
    let client = super::client::CLIENT_CELL.get().unwrap();
    let parsed = match client.parse_input(url).await {
        Ok(parsed) => parsed,
        Err(err) => {
            error(format!("无法识别链接: {:?}", err).as_str());
            return 1;
        }
    };
    match parsed {
        VideoType::AVID(avid) => save_avid(avid).await,
        VideoType::EPID(ep_id) => save_ep(ep_id).await,
        VideoType::CHEESE(ep_id) => save_cheese(ep_id).await,
        VideoType::LISTBIZID(biz_id) => save_list(biz_id.as_str(), false).await,
        VideoType::SERIESBIZID(biz_id) => save_list(biz_id.as_str(), true).await,
        VideoType::FAVID { fid, mid } => save_fav(fid.as_str(), mid.as_str()).await,
        VideoType::MID(_) => {
            error("暂时不支持 UP 主投稿批量下载");
            1
        }
    }
}

async fn save_avid(avid: i64) -> i32 {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let video = match client.fetch_video_info(avid).await {
        Ok(video) => video,
        Err(err) => {
            error(format!("无法获取视频信息: {:?}", err).as_str());
            return 1;
        }
    };
    info(format!("匹配到视频: {}", video.title).as_str());
    write_covers(vec![CoverSave {
        stem: file_title(&video.title),
        url: video.pic,
    }])
    .await
}

async fn save_ep(ep_id: i64) -> i32 {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let season = match client.fetch_ep_info(ep_id).await {
        Ok(season) => season,
        Err(err) => {
            error(format!("无法获取EP信息: {:?}", err).as_str());
            return 1;
        }
    };
    info(format!("匹配到EP: {}", season.season_title).as_str());
    let mut jobs = Vec::new();
    let mut seen_url = HashSet::new();
    for episode in season.episodes {
        let title = if !episode.show_title.is_empty() {
            episode.show_title
        } else if !episode.title.is_empty() {
            episode.title
        } else {
            episode.long_title
        };
        let cover = if episode.cover.is_empty() {
            season.cover.clone()
        } else {
            episode.cover
        };
        push_unique_url(&mut jobs, &mut seen_url, &title, &cover);
    }
    write_covers(jobs).await
}

async fn save_cheese(ep_id: i64) -> i32 {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let cheese = match client.fetch_cheese_info(ep_id).await {
        Ok(cheese) => cheese,
        Err(err) => {
            error(format!("无法获取课堂信息: {:?}", err).as_str());
            return 1;
        }
    };
    info(format!("匹配到课堂: {}", cheese.title).as_str());
    let mut jobs = Vec::new();
    let mut seen_url = HashSet::new();
    for episode in cheese.episodes {
        push_unique_url(&mut jobs, &mut seen_url, &episode.title, &episode.cover);
    }
    write_covers(jobs).await
}

async fn save_list(biz_id: &str, series: bool) -> i32 {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let kind = if series { "系列" } else { "合集" };
    let info = match client.fetch_media_list(biz_id, series).await {
        Ok(info) => info,
        Err(err) => {
            error(format!("无法获取{kind}信息: {:?}", err).as_str());
            return 1;
        }
    };
    save_media_list(kind, info).await
}

async fn save_fav(fid: &str, mid: &str) -> i32 {
    let client = super::client::CLIENT_CELL.get().unwrap();
    let info = match client.fetch_fav_list(fid, mid).await {
        Ok(info) => info,
        Err(err) => {
            error(format!("无法获取收藏夹信息: {:?}", err).as_str());
            return 1;
        }
    };
    save_media_list("收藏夹", info).await
}

async fn save_media_list(kind: &str, list: bbdd::fetcher::MediaListInfo) -> i32 {
    info(format!("匹配到{kind}: {}", list.title).as_str());
    let mut jobs = Vec::new();
    let mut seen_aid = HashSet::new();
    for item in list.items {
        if !seen_aid.insert(item.aid) {
            continue;
        }
        let title = if item.video_title.is_empty() {
            item.title
        } else {
            item.video_title
        };
        jobs.push(CoverSave {
            stem: file_title(&title),
            url: item.cover,
        });
    }
    jobs.push(CoverSave {
        stem: file_title(&list.title),
        url: list.cover,
    });
    write_covers(jobs).await
}

fn push_unique_url(jobs: &mut Vec<CoverSave>, seen: &mut HashSet<String>, title: &str, url: &str) {
    let normalized = normalize_resource_url(url);
    if normalized.is_empty() {
        let label = if title.is_empty() { "封面" } else { title };
        warn(format!("{label} 没有封面地址，跳过").as_str());
        return;
    }
    if !seen.insert(normalized.clone()) {
        return;
    }
    jobs.push(CoverSave {
        stem: file_title(title),
        url: normalized,
    });
}

async fn write_covers(jobs: Vec<CoverSave>) -> i32 {
    if jobs.is_empty() {
        warn("没有可保存的封面");
        return 1;
    }
    let client = super::client::CLIENT_CELL.get().unwrap();
    let mut used = HashSet::new();
    let mut saved = 0u32;
    let mut failed = 0u32;
    for job in jobs {
        if super::is_cancelled() {
            warn("已取消");
            return 1;
        }
        let url = normalize_resource_url(&job.url);
        let label = if job.stem.is_empty() {
            "cover".to_string()
        } else {
            job.stem.clone()
        };
        if url.is_empty() {
            warn(format!("{label} 没有封面地址，跳过").as_str());
            failed += 1;
            continue;
        }
        let path = allocate_name(&label, cover_extension(&url), &mut used);
        match download_bytes(client, &url).await {
            Ok(bytes) => match fs::write(&path, &bytes).await {
                Ok(()) => {
                    info(format!("封面已保存: {path}").as_str());
                    saved += 1;
                }
                Err(err) => {
                    let _ = fs::remove_file(&path).await;
                    warn(format!("保存封面失败 {label}: {err}").as_str());
                    failed += 1;
                }
            },
            Err(err) => {
                warn(format!("下载封面失败 {label}: {err}").as_str());
                failed += 1;
            }
        }
    }
    if saved == 0 {
        error("没有保存成功的封面");
        return 1;
    }
    success(format!("封面保存完成: {saved} 张，失败 {failed} 张").as_str());
    0
}

fn allocate_name(stem: &str, ext: &str, used: &mut HashSet<String>) -> String {
    let mut candidate = format!("{stem}.{ext}");
    let mut index = 2u32;
    while !used.insert(candidate.clone()) {
        candidate = format!("{stem}_{index}.{ext}");
        index += 1;
    }
    candidate
}
