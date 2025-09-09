use crate::{
    error::{Error, Result},
    util::take_json_field,
};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::BBDD;

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
        let prefix = "https://api.bilibili.com/x/player/wbi/playurl?";
        let api = format!(
            "support_multi_audio=true&from_client=BROWSER&avid={}&cid={}&fnval=4048&fnver=0&fourk=1&wts={}",
            aid,
            cid,
            chrono::Utc::now().timestamp()
        );
        let md5 = md5::compute(api.as_bytes());
        let hex = hex::encode(md5.0);
        let sign = format!("&w_rid={}", hex);
        let url = format!("{}{}{}", prefix, api, sign);
        let json: serde_json::Value = self.get_data(url.as_str(), None).await?;
        let paly_url: VideoPlayUrl = serde_json::from_value(json)?;
        Ok(paly_url)
    }

    pub async fn play_url_ep(&self, aid: i64, cid: i64, ep_id: i64) -> Result<VideoPlayUrl> {
        let prefix = "https://api.bilibili.com/pgc/player/web/v2/playurl?";
        let api = format!(
            "support_multi_audio=true&from_client=BROWSER&avid={}&cid={}&fnval=4048&fnver=0&fourk=1&module=bangumi&ep_id={}&session=&wts={}",
            aid,
            cid,
            ep_id,
            chrono::Utc::now().timestamp()
        );
        let url = format!("{}{}", prefix, api);
        let json: serde_json::Value = self.get_result(url.as_str(), None).await?;
        let video_info = json
            .get("video_info")
            .ok_or(Error::StateError("Missing field: video_info".to_string()))?;
        let paly_url: VideoPlayUrl = serde_json::from_value(video_info.clone())?;
        Ok(paly_url)
    }
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

#[cfg(test)]
mod tests {
    use crate::parse::VideoType;

    const BV_ID: &'static str = "BV1P4411T73c";

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
        let play_url = bbdd.play_url_ep(aid, cid, ep_id).await.unwrap();
        println!("{:#?}", play_url);
    }
}
