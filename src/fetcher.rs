use crate::{
    error::{Error, Result},
    util::take_json_field,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::BBDD;

static WBI_MIXIN_KEY: OnceLock<String> = OnceLock::new();

impl BBDD {
    pub async fn fetch_video_info(&self, aid: i64) -> Result<VideoInfo> {
        let url = format!("https://api.bilibili.com/x/web-interface/view?aid={}", aid);
        let json: serde_json::Value = self.get_data(url.as_str(), None).await?;
        let bvid: String = take_json_field(&json, "bvid")?;
        let cid: i64 = take_json_field(&json, "cid")?;
        let title: String = take_json_field(&json, "title")?;
        let desc: String = take_json_field(&json, "desc")?;
        let pic: String = take_json_field(&json, "pic")?;
        let pubdate: i64 = take_json_field(&json, "pubdate")?;
        let owner: VideoInfoOwner = take_json_field(&json, "owner")?;
        let is_stein_gate = if let Some(rights) = json.get("rights") {
            if let Some(is_stein_gate_value) = rights.get("is_stein_gate") {
                if let Some(is_stein_gate_int) = is_stein_gate_value.as_i64() {
                    is_stein_gate_int
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        };
        let mut pages = vec![];
        if let Some(pages_json_value) = json.get("pages") {
            if let Some(pages_array) = pages_json_value.as_array() {
                for page_json in pages_array {
                    let page: i64 = take_json_field(&page_json, "page")?;
                    let cid: i64 = take_json_field(&page_json, "cid")?;
                    let part: String = take_json_field(&page_json, "part")?;
                    let part = part.trim().to_string();
                    let duration: i64 = take_json_field(&page_json, "duration")?;
                    let dimension: VideoDimension = take_json_field(&page_json, "dimension")?;
                    pages.push(VideoPage {
                        page,
                        aid: aid,
                        cid: cid,
                        epid: "".to_string(),
                        part,
                        duration,
                        dimension,
                        pubdate,
                        cover: "".to_string(),
                        desc: "".to_string(),
                        owner: owner.clone(),
                    });
                }
            }
        }
        let mut is_bangumi = false;
        if let Some(redirect_url_value) = json.get("redirect_url") {
            if let Some(redirect_url) = redirect_url_value.as_str() {
                if redirect_url.contains("bangumi") {
                    is_bangumi = true;
                    let re = Regex::new(r"ep(\d+)").map_err(|e| {
                        Error::StateError(format!("Failed to compile regex: {}", e.to_string()))
                    })?;
                    if let Some(caps) = re.captures(redirect_url) {
                        if let Some(epid_match) = caps.get(1) {
                            let epid = epid_match.as_str().to_string();
                            if pages.len() == 1 {
                                for p in pages.iter_mut() {
                                    p.epid = epid.clone();
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(VideoInfo {
            bvid,
            cid: cid,
            title: title.trim().to_string(),
            desc: desc.trim().to_string(),
            pic,
            owner,
            pubdate,
            is_bangumi,
            is_cheese: false,
            is_bangumi_end: false,
            index: 0,
            pages,
            is_stein_gate,
        })
    }

    pub async fn fetch_ep_info(&self, ep_id: i64) -> Result<EpInfo> {
        let url = format!(
            "https://api.bilibili.com/pgc/view/web/season?ep_id={}",
            ep_id
        );
        self.get_result(url.as_str(), None).await
    }

    pub async fn play_url(&self, aid: i64, cid: i64) -> Result<VideoPlayUrl> {
        self.play_url_with_qn(aid, cid, 127).await
    }

    pub async fn play_url_with_qn(&self, aid: i64, cid: i64, qn: i64) -> Result<VideoPlayUrl> {
        let prefix = "https://api.bilibili.com/x/player/wbi/playurl?";
        let mut api = format!(
            "support_multi_audio=true&from_client=BROWSER&avid={aid}&cid={cid}&fnval=4048&fnver=0&fourk=1&otype=json&qn={qn}"
        );
        if self.cookie.is_empty() {
            api.push_str("&try_look=1");
        }
        api.push_str(format!("&wts={}", chrono::Utc::now().timestamp()).as_str());
        let signed = match wbi_sign(self, api.as_str()).await {
            Ok(signed) => signed,
            Err(err) => {
                tracing::warn!("WBI 签名失败，尝试网页源码兜底: {}", err);
                return recover_play(Err(err), self, aid, None, false).await;
            }
        };
        let url = format!("{}{}", prefix, signed);
        let primary = match self.get_data::<serde_json::Value>(url.as_str(), None).await {
            Ok(json) => match parse_play_url(json) {
                Ok(play_url) if !play_url.dash.video.is_empty() => return Ok(play_url),
                Ok(_) => Err(Error::StateError("播放地址里没有视频流".to_string())),
                Err(err) => Err(err),
            },
            Err(err) => Err(err),
        };
        if let Err(err) = &primary {
            tracing::warn!("playurl 接口失败，尝试网页源码兜底: {}", err);
        }
        recover_play(primary, self, aid, None, false).await
    }

    pub async fn play_url_ep(
        &self,
        aid: i64,
        cid: i64,
        ep_id: i64,
        qn: i64,
    ) -> Result<VideoPlayUrl> {
        self.play_url_bangumi(aid, cid, ep_id, qn, false).await
    }

    pub async fn play_url_cheese(
        &self,
        aid: i64,
        cid: i64,
        ep_id: i64,
        qn: i64,
    ) -> Result<VideoPlayUrl> {
        self.play_url_bangumi(aid, cid, ep_id, qn, true).await
    }

    async fn play_url_bangumi(
        &self,
        aid: i64,
        cid: i64,
        ep_id: i64,
        qn: i64,
        cheese: bool,
    ) -> Result<VideoPlayUrl> {
        let prefix = if cheese {
            "https://api.bilibili.com/pugv/player/web/v2/playurl?"
        } else {
            "https://api.bilibili.com/pgc/player/web/v2/playurl?"
        };
        let mut api = format!(
            "avid={aid}&cid={cid}&fnval=4048&fnver=0&fourk=1&otype=json&qn={qn}&module=bangumi&ep_id={ep_id}&session="
        );
        if self.cookie.is_empty() {
            api.push_str("&try_look=1");
        }
        api.push_str(format!("&wts={}", chrono::Utc::now().timestamp()).as_str());
        let url = format!("{}{}", prefix, api);
        let primary = match self.get_json(url.as_str(), None).await {
            Ok(json) => bangumi_play(json),
            Err(err) => Err(err),
        };
        if let Err(err) = &primary {
            tracing::warn!("番剧/课堂 playurl 失败，尝试网页源码兜底: {}", err);
            return recover_play(primary, self, aid, Some(ep_id), cheese).await;
        }
        primary
    }

    pub async fn fetch_cheese_info(&self, ep_id: i64) -> Result<CheeseInfo> {
        let url = format!("https://api.bilibili.com/pugv/view/web/season?ep_id={ep_id}");
        let data: serde_json::Value = self.get_data(url.as_str(), None).await?;
        let title = json_string(&data, "title");
        let cover = json_string(&data, "cover");
        let episodes_json = data
            .get("episodes")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut episodes = Vec::new();
        for ep in episodes_json {
            let aid = json_i64(&ep, "aid").unwrap_or(0);
            let cid = json_i64(&ep, "cid").unwrap_or(0);
            let episode_id = json_i64(&ep, "id").unwrap_or(0);
            if aid == 0 || cid == 0 || episode_id == 0 {
                continue;
            }
            let ep_title = json_string(&ep, "title");
            let ep_cover = json_string(&ep, "cover");
            episodes.push(CheeseEpisode {
                aid,
                cid,
                ep_id: episode_id,
                title: ep_title,
                cover: if ep_cover.is_empty() {
                    cover.clone()
                } else {
                    ep_cover
                },
            });
        }
        if episodes.is_empty() {
            return Err(Error::StateError("课堂课时列表为空".to_string()));
        }
        Ok(CheeseInfo {
            title,
            cover,
            episodes,
        })
    }

    pub async fn fetch_media_list(&self, biz_id: &str, series: bool) -> Result<MediaListInfo> {
        match self.fetch_media_list_typed(biz_id, series).await {
            Ok(info) => Ok(info),
            Err(err) if !series => {
                tracing::warn!("合集解析失败，尝试按系列解析: {}", err);
                self.fetch_media_list_typed(biz_id, true).await
            }
            Err(err) => Err(err),
        }
    }

    async fn fetch_media_list_typed(&self, biz_id: &str, series: bool) -> Result<MediaListInfo> {
        let list_type = if series { 5 } else { 8 };
        let kind = if series { "系列" } else { "合集" };
        let info_url = format!(
            "https://api.bilibili.com/x/v1/medialist/info?type={list_type}&biz_id={biz_id}&tid=0"
        );
        let info: serde_json::Value = self.get_data(info_url.as_str(), None).await?;
        if !info.is_object() {
            return Err(Error::StateError(format!("获取{kind}信息失败")));
        }
        let title = json_string(&info, "title");
        let list_cover = json_string(&info, "cover");
        let desc = if series { "true" } else { "false" };
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut oid = String::new();
        let mut has_more = true;
        let mut index = 1i64;
        for _ in 0..200 {
            if !has_more {
                break;
            }
            let list_url = format!(
                "https://api.bilibili.com/x/v2/medialist/resource/list?type={list_type}&oid={oid}&otype=2&biz_id={biz_id}&with_current=true&mobi_app=web&ps=20&direction=false&sort_field=1&tid=0&desc={desc}"
            );
            let data: serde_json::Value = self.get_data(list_url.as_str(), None).await?;
            if !data.is_object() {
                return Err(Error::StateError(format!("获取{kind}视频列表失败")));
            }
            has_more = data
                .get("has_more")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let media_list = data
                .get("media_list")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if media_list.is_empty() {
                break;
            }
            for media in media_list {
                if json_i64(&media, "attr").unwrap_or(0) != 0 {
                    continue;
                }
                let aid = json_i64(&media, "id").unwrap_or(0);
                let media_title = json_string(&media, "title");
                let cover = json_string(&media, "cover");
                let page_count = json_i64(&media, "page").unwrap_or(1);
                let pages = media
                    .get("pages")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                for page in pages {
                    let cid = json_i64(&page, "id").unwrap_or(0);
                    if aid == 0 || cid == 0 || !seen.insert((aid, cid)) {
                        continue;
                    }
                    let page_no = json_i64(&page, "page").unwrap_or(1);
                    let part = json_string(&page, "title");
                    let item_title = if page_count <= 1 {
                        media_title.clone()
                    } else {
                        format!("{media_title}_P{page_no}_{part}")
                    };
                    items.push(MediaListItem {
                        aid,
                        cid,
                        index,
                        title: item_title,
                        video_title: media_title.clone(),
                        cover: cover.clone(),
                    });
                    index += 1;
                }
                if aid != 0 {
                    oid = aid.to_string();
                }
            }
        }
        if items.is_empty() {
            return Err(Error::StateError(format!("{kind}里没有可下载的视频")));
        }
        Ok(MediaListInfo {
            title: if title.is_empty() {
                kind.to_string()
            } else {
                title
            },
            cover: list_cover,
            items,
        })
    }

    pub async fn fetch_fav_list(&self, fid: &str, _mid: &str) -> Result<MediaListInfo> {
        let page_size = 20i64;
        let first_url = format!(
            "https://api.bilibili.com/x/v3/fav/resource/list?media_id={fid}&pn=1&ps={page_size}&order=mtime&type=2&tid=0&platform=web"
        );
        let first: serde_json::Value = self.get_data(first_url.as_str(), None).await?;
        let title = first
            .get("info")
            .map(|info| json_string(info, "title"))
            .unwrap_or_default();
        let list_cover = first
            .get("info")
            .map(|info| json_string(info, "cover"))
            .unwrap_or_default();
        let media_count = first
            .get("info")
            .and_then(|info| json_i64(info, "media_count"))
            .unwrap_or(0);
        let total_page = ((media_count + page_size - 1) / page_size).max(1);
        let mut medias = first
            .get("medias")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for page in 2..=total_page {
            let url = format!(
                "https://api.bilibili.com/x/v3/fav/resource/list?media_id={fid}&pn={page}&ps={page_size}&order=mtime&type=2&tid=0&platform=web"
            );
            let data: serde_json::Value = self.get_data(url.as_str(), None).await?;
            if let Some(arr) = data.get("medias").and_then(|v| v.as_array()) {
                medias.extend(arr.iter().cloned());
            }
        }
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut index = 1i64;
        for media in medias {
            if media.is_null() {
                continue;
            }
            if json_i64(&media, "attr").unwrap_or(0) != 0 {
                continue;
            }
            let aid = json_i64(&media, "id").unwrap_or(0);
            if aid == 0 {
                continue;
            }
            let media_title = json_string(&media, "title");
            let cover = json_string(&media, "cover");
            let page_count = json_i64(&media, "page").unwrap_or(1);
            if page_count > 1 {
                let info = self.fetch_video_info(aid).await?;
                for page in info.pages {
                    if !seen.insert((aid, page.cid)) {
                        continue;
                    }
                    items.push(MediaListItem {
                        aid,
                        cid: page.cid,
                        index,
                        title: format!("{media_title}_P{}_{}", page.page, page.part),
                        video_title: media_title.clone(),
                        cover: if cover.is_empty() {
                            info.pic.clone()
                        } else {
                            cover.clone()
                        },
                    });
                    index += 1;
                }
            } else {
                let cid = media
                    .get("ugc")
                    .and_then(|ugc| json_i64(ugc, "first_cid"))
                    .unwrap_or(0);
                if cid == 0 || !seen.insert((aid, cid)) {
                    continue;
                }
                items.push(MediaListItem {
                    aid,
                    cid,
                    index,
                    title: media_title.clone(),
                    video_title: media_title,
                    cover,
                });
                index += 1;
            }
        }
        if items.is_empty() {
            return Err(Error::StateError("收藏夹里没有可下载的视频".to_string()));
        }
        Ok(MediaListInfo {
            title: if title.is_empty() {
                "收藏夹".to_string()
            } else {
                title
            },
            cover: list_cover,
            items,
        })
    }

    pub async fn fetch_subtitles(&self, aid: i64, cid: i64) -> Result<Vec<SubtitleTrack>> {
        if self.cookie.is_empty() {
            return Ok(Vec::new());
        }
        if let Some(list) = self.subtitles_from_player(aid, cid).await {
            let list = skip_ai_subtitles(list);
            if !list.is_empty() {
                return Ok(list);
            }
        }
        if let Some(list) = self.subtitles_from_view(aid, cid).await {
            return Ok(skip_ai_subtitles(list));
        }
        Ok(Vec::new())
    }

    async fn subtitles_from_player(&self, aid: i64, cid: i64) -> Option<Vec<SubtitleTrack>> {
        let url = format!("https://api.bilibili.com/x/player/wbi/v2?cid={cid}&aid={aid}");
        let data: serde_json::Value = self.get_data(url.as_str(), None).await.ok()?;
        let subs = data.get("subtitle")?.get("subtitles")?.as_array()?.clone();
        Some(parse_subtitle_list(&subs, "subtitle_url"))
    }

    async fn subtitles_from_view(&self, aid: i64, cid: i64) -> Option<Vec<SubtitleTrack>> {
        let url = format!("https://api.bilibili.com/x/web-interface/view?aid={aid}&cid={cid}");
        let data: serde_json::Value = self.get_data(url.as_str(), None).await.ok()?;
        let subs = data.get("subtitle")?.get("list")?.as_array()?.clone();
        Some(parse_subtitle_list(&subs, "subtitle_url"))
    }
}

fn bangumi_play(json: serde_json::Value) -> Result<VideoPlayUrl> {
    let code = json.get("code").and_then(|value| value.as_i64()).unwrap_or(0);
    if code != 0 {
        let message = json
            .get("message")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string();
        return Err(Error::ApiError {
            code: code as i32,
            message,
        });
    }
    let result = json
        .get("result")
        .cloned()
        .or_else(|| json.get("data").cloned())
        .unwrap_or(json);
    let result = if let Some(video_info) = result.get("video_info") {
        video_info.clone()
    } else {
        result
    };
    match parse_play_url(result) {
        Ok(play_url) if !play_url.dash.video.is_empty() => Ok(play_url),
        Ok(_) => Err(Error::StateError("播放地址里没有视频流".to_string())),
        Err(err) => Err(err),
    }
}

async fn recover_play(
    primary: Result<VideoPlayUrl>,
    client: &BBDD,
    aid: i64,
    ep_id: Option<i64>,
    cheese: bool,
) -> Result<VideoPlayUrl> {
    match play_url_from_html(client, aid, ep_id, cheese).await {
        Ok(play) if !play.dash.video.is_empty() => Ok(play),
        _ => primary,
    }
}

fn parse_play_url(mut json: serde_json::Value) -> Result<VideoPlayUrl> {
    if let Some(dash) = json.get_mut("dash") {
        normalize_audio_list(dash);
    }
    let play_url: VideoPlayUrl = serde_json::from_value(json)?;
    Ok(play_url)
}

fn normalize_audio_list(dash: &mut serde_json::Value) {
    let dolby_audio = dash.get("dolby").and_then(|v| v.get("audio")).cloned();
    let flac_audio = dash.get("flac").and_then(|v| v.get("audio")).cloned();
    let Some(audio_array) = dash.get_mut("audio").and_then(|v| v.as_array_mut()) else {
        return;
    };
    let mut push_audio = |value: &serde_json::Value| {
        if let Some(arr) = value.as_array() {
            audio_array.extend(arr.iter().cloned());
        } else if value.is_object() {
            audio_array.push(value.clone());
        }
    };
    if let Some(dolby) = dolby_audio {
        push_audio(&dolby);
    }
    if let Some(flac) = flac_audio {
        push_audio(&flac);
    }
}

async fn wbi_sign(client: &BBDD, api: &str) -> Result<String> {
    let key = wbi_mixin_key(client).await?;
    let query = canonicalize_wbi_query(api);
    let md5 = md5::compute(format!("{query}{key}").as_bytes());
    let sign = hex::encode(md5.0);
    Ok(format!("{query}&w_rid={sign}"))
}

fn canonicalize_wbi_query(api: &str) -> String {
    let mut pairs = Vec::new();
    for part in api.split('&') {
        if part.is_empty() {
            continue;
        }
        let (key, value) = match part.split_once('=') {
            Some((key, value)) => (key.to_string(), filter_wbi_value(value)),
            None => (part.to_string(), String::new()),
        };
        pairs.push((key, value));
    }
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
        .into_iter()
        .map(|(key, value)| format!("{}={}", form_encode(&key), form_encode(&value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn filter_wbi_value(value: &str) -> String {
    value.chars().filter(|ch| !"!'()*".contains(*ch)).collect()
}

fn form_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

async fn wbi_mixin_key(client: &BBDD) -> Result<String> {
    if let Some(key) = WBI_MIXIN_KEY.get() {
        return Ok(key.clone());
    }
    let nav = client.web_nav().await?;
    let orig = format!(
        "{}{}",
        r_sub_string(nav.wbi_img.img_url.as_str())?,
        r_sub_string(nav.wbi_img.sub_url.as_str())?
    );
    let key = mixin_key(orig.as_str())?;
    let _ = WBI_MIXIN_KEY.set(key.clone());
    Ok(key)
}

fn r_sub_string(url: &str) -> Result<&str> {
    let last = url
        .rsplit('/')
        .next()
        .ok_or_else(|| Error::StateError("Invalid wbi url".to_string()))?;
    if let Some((stem, _ext)) = last.rsplit_once('.') {
        Ok(stem)
    } else {
        Err(Error::StateError("Invalid wbi url".to_string()))
    }
}

fn mixin_key(orig: &str) -> Result<String> {
    const MIXIN_KEY_ENC_TAB: [usize; 32] = [
        46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19,
        29, 28, 14, 39, 12, 38, 41, 13,
    ];
    let chars: Vec<char> = orig.chars().collect();
    let mut out = String::with_capacity(32);
    for idx in MIXIN_KEY_ENC_TAB {
        let ch = *chars
            .get(idx)
            .ok_or_else(|| Error::StateError("Invalid wbi key material".to_string()))?;
        out.push(ch);
    }
    Ok(out)
}

async fn play_url_from_html(
    client: &BBDD,
    aid: i64,
    ep_id: Option<i64>,
    cheese: bool,
) -> Result<VideoPlayUrl> {
    let url = if let Some(ep_id) = ep_id {
        if cheese {
            format!("https://www.bilibili.com/cheese/play/ep{ep_id}")
        } else {
            format!("https://www.bilibili.com/bangumi/play/ep{ep_id}")
        }
    } else {
        format!("https://www.bilibili.com/video/av{aid}")
    };
    let html = client.get_web_source(url.as_str()).await?;
    let playinfo = extract_playinfo_from_html(&html)
        .ok_or_else(|| Error::StateError("网页源码中未找到 __playinfo__".to_string()))?;
    let data = playinfo
        .get("data")
        .cloned()
        .ok_or_else(|| Error::StateError("playinfo 缺少 data 字段".to_string()))?;
    let play_url = parse_play_url(data)?;
    if play_url.dash.video.is_empty() {
        Err(Error::StateError(
            "playinfo 解析失败: dash.video 为空".to_string(),
        ))
    } else {
        Ok(play_url)
    }
}

fn json_i64(value: &serde_json::Value, field: &str) -> Option<i64> {
    let field = value.get(field)?;
    field
        .as_i64()
        .or_else(|| field.as_u64().and_then(|n| i64::try_from(n).ok()))
        .or_else(|| field.as_str().and_then(|text| text.parse().ok()))
}

fn json_string(value: &serde_json::Value, field: &str) -> String {
    value
        .get(field)
        .and_then(|item| match item {
            serde_json::Value::String(text) => Some(text.clone()),
            serde_json::Value::Number(number) => Some(number.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

fn parse_subtitle_list(subs: &[serde_json::Value], url_field: &str) -> Vec<SubtitleTrack> {
    let mut tracks = Vec::new();
    for sub in subs {
        let lan = json_string(sub, "lan");
        let url = normalize_subtitle_url(&json_string(sub, url_field));
        if lan.is_empty() || url.is_empty() {
            continue;
        }
        tracks.push(SubtitleTrack { lan, url });
    }
    tracks
}

fn skip_ai_subtitles(tracks: Vec<SubtitleTrack>) -> Vec<SubtitleTrack> {
    tracks
        .into_iter()
        .filter(|track| !track.lan.starts_with("ai-"))
        .collect()
}

fn normalize_subtitle_url(url: &str) -> String {
    if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url.to_string()
    }
}

pub fn subtitle_json_to_srt(json: &str) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    let body = value
        .get("body")
        .and_then(|item| item.as_array())
        .ok_or_else(|| Error::StateError("字幕 JSON 缺少 body".to_string()))?;
    let mut out = String::new();
    for (index, line) in body.iter().enumerate() {
        let from = line
            .get("from")
            .and_then(|item| item.as_f64())
            .unwrap_or(0.0);
        let to = line
            .get("to")
            .and_then(|item| item.as_f64())
            .unwrap_or(from);
        let content = line
            .get("content")
            .and_then(|item| item.as_str())
            .unwrap_or("");
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            index + 1,
            format_srt_time(from),
            format_srt_time(to),
            content
        ));
    }
    Ok(out)
}

fn format_srt_time(seconds: f64) -> String {
    let seconds = if seconds.is_sign_negative() {
        0.0
    } else {
        seconds
    };
    let total_ms = (seconds * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let total_s = total_ms / 1000;
    let s = total_s % 60;
    let total_m = total_s / 60;
    let m = total_m % 60;
    let h = total_m / 60;
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

fn extract_playinfo_from_html(html: &str) -> Option<serde_json::Value> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    let re = RE
        .get_or_init(|| Regex::new(r"__playinfo__=(\{.*?\})\s*</script>").ok())
        .as_ref()?;
    let matched = re.captures(html)?.get(1)?;
    serde_json::from_str(matched.as_str()).ok()
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct VideoInfo {
    pub bvid: String,
    pub cid: i64,
    pub title: String,
    pub desc: String,
    pub pic: String,
    pub owner: VideoInfoOwner,
    pub pubdate: i64,
    pub is_bangumi: bool,
    pub is_cheese: bool,
    pub is_bangumi_end: bool,
    pub index: i64,
    pub pages: Vec<VideoPage>,
    pub is_stein_gate: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct VideoInfoOwner {
    pub mid: i64,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct VideoPage {
    pub page: i64,
    pub aid: i64,
    pub cid: i64,
    pub epid: String,
    pub part: String,
    pub duration: i64,
    pub dimension: VideoDimension,
    pub pubdate: i64,
    pub cover: String,
    pub desc: String,
    pub owner: VideoInfoOwner,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct VideoDimension {
    pub width: i64,
    pub height: i64,
}

impl VideoDimension {
    pub fn resolution(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct VideoPlayUrl {
    pub from: String,
    pub result: String,
    pub message: String,
    pub quality: i64,
    pub format: String,
    pub timelength: i64,
    pub accept_format: String,
    pub accept_description: Vec<String>,
    pub accept_quality: Vec<i64>,
    pub video_codecid: i64,
    pub seek_param: String,
    pub seek_type: String,
    pub dash: VideoDash,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct VideoDash {
    pub duration: i64,
    pub min_buffer_time: f64,
    pub video: Vec<VideoMedia>,
    pub audio: Vec<VideoMedia>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct VideoMedia {
    pub id: i64,
    pub base_url: String,
    pub backup_url: Vec<String>,
    pub bandwidth: i64,
    pub mime_type: String,
    pub codecs: String,
    pub width: i64,
    pub height: i64,
    pub frame_rate: String,
    pub sar: String,
    pub start_with_sap: i64,
    pub codecid: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct EpInfo {
    pub actors: String,
    pub alias: String,
    pub cover: String,
    pub delivery_fragment_video: bool,
    pub enable_vt: bool,
    pub evaluate: String,
    pub record: String,
    pub episodes: Vec<EpisodeInfo>,
    #[serde(default)]
    pub season_id: i64,
    #[serde(default)]
    pub season_title: String,
    #[serde(default)]
    pub seasons: Vec<SeasonInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct EpisodeInfo {
    pub aid: i64,
    #[serde(default)]
    pub badge: String,
    pub bvid: String,
    pub cid: i64,
    pub cover: String,
    pub dimension: VideoDimension,
    pub duration: i64,
    pub enable_vt: bool,
    pub ep_id: i64,
    pub from: String,
    pub id: i64,
    pub long_title: String,
    pub pub_time: i64,
    pub pv: i64,
    pub section_type: i64,
    pub share_copy: String,
    pub share_url: String,
    pub short_link: String,
    pub show_title: String,
    pub title: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonInfo {
    pub season_id: i64,
    pub season_title: String,
    pub season_type: i64,
    pub media_id: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheeseInfo {
    pub title: String,
    pub cover: String,
    pub episodes: Vec<CheeseEpisode>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheeseEpisode {
    pub aid: i64,
    pub cid: i64,
    pub ep_id: i64,
    pub title: String,
    pub cover: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaListInfo {
    pub title: String,
    pub cover: String,
    pub items: Vec<MediaListItem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MediaListItem {
    pub aid: i64,
    pub cid: i64,
    pub index: i64,
    pub title: String,
    pub video_title: String,
    pub cover: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubtitleTrack {
    pub lan: String,
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::{mixin_key, r_sub_string};
    use crate::parse::VideoType;

    const BV_ID: &'static str = "BV1P4411T73c";

    #[test]
    fn test_r_sub_string_uses_last_dot() {
        let url = "https://i0.hdslb.com/bfs/wbi/abc.def.png";
        let stem = r_sub_string(url).unwrap();
        assert_eq!(stem, "abc.def");
    }

    #[test]
    fn test_mixin_key_length() {
        let orig = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ__";
        let key = mixin_key(orig).unwrap();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn test_wbi_query_sorted_and_filtered() {
        let query = super::canonicalize_wbi_query("z=1&a=b!*c&m=hello");
        assert_eq!(query, "a=bc&m=hello&z=1");
    }

    #[test]
    fn test_subtitle_json_to_srt() {
        let json = r#"{"body":[{"from":1.5,"to":3.25,"content":"你好"}]}"#;
        let srt = super::subtitle_json_to_srt(json).unwrap();
        assert_eq!(srt, "1\n00:00:01,500 --> 00:00:03,250\n你好\n\n");
    }

    #[test]
    fn test_extract_playinfo_from_html() {
        let html = r#"<script>window.__playinfo__={"code":0,"data":{"quality":80}}</script>"#;
        let value = super::extract_playinfo_from_html(html).unwrap();
        assert_eq!(value["data"]["quality"], 80);
        assert!(super::extract_playinfo_from_html("<html></html>").is_none());
    }

    #[tokio::test]
    async fn test_fetch_video_info() {
        crate::tests::log_init();
        let bbdd = &crate::tests::BBDD;
        let input = bbdd.parse_input(BV_ID).await.unwrap();
        let avid = match input {
            VideoType::AVID(avid) => avid,
            _ => panic!("Expected AVID, got {:?}", input),
        };
        let info = bbdd.fetch_video_info(avid).await.unwrap();
        println!("{:#?}", info);
    }

    #[tokio::test]
    async fn guest_play_url_without_login() {
        let client = crate::BBDD::new(
            "",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/85 Safari/537.36",
        )
        .unwrap();
        let play = client.play_url_with_qn(54916636, 96040706, 80).await.unwrap();
        assert!(!play.dash.video.is_empty());
    }

    #[tokio::test]
    async fn test_play_url() {
        crate::tests::log_init();
        let bbdd = &crate::tests::BBDD;
        let aid = 54916636;
        let cid = 96040706;
        let play_url = bbdd.play_url(aid, cid).await.unwrap();
        println!("{:#?}", play_url);
    }

    #[tokio::test]
    async fn test_fetch_ep_info() {
        crate::tests::log_init();
        let bbdd = &crate::tests::BBDD;
        let ep_id = 307247;
        let ep_info = bbdd.fetch_ep_info(ep_id).await.unwrap();
        println!("{:#?}", ep_info);
    }

    #[tokio::test]
    async fn test_play_url_ep() {
        crate::tests::log_init();
        let bbdd = &crate::tests::BBDD;
        let aid = 797201440;
        let cid = 238907859;
        let ep_id = 307247;
        let play_url = bbdd.play_url_ep(aid, cid, ep_id, 32).await.unwrap();
        println!("{:#?}", play_url);
    }
}
