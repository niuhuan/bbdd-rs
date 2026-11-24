use crate::BBDD;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

impl BBDD {
    async fn fix_avid(&self, avid: i64) -> Result<VideoType> {
        let api = format!("https://www.bilibili.com/video/av{}/", avid);
        let location = self.get_302_location(&api).await?;
        if let Some(loc) = location {
            if loc.contains("/ep") {
                let re = regex::Regex::new(r"/ep(\d+)").unwrap();
                if let Some(caps) = re.captures(&loc) {
                    let ep_id = caps.get(1).unwrap().as_str();
                    match ep_id.parse::<i64>() {
                        Ok(id) => return Ok(VideoType::EPID(id)),
                        Err(_) => return Err(Error::ParamError("ep_id 不是数字".to_string())),
                    }
                }
            }
        }
        Ok(VideoType::AVID(avid))
    }

    pub async fn parse_input(&self, input: &str) -> Result<VideoType> {
        let mut input = input.to_string();
        if input.starts_with("http") {
            if input.contains("b23.tv") {
                if let Some(tmp) = self.get_302_location(input.as_str()).await? {
                    if tmp == input {
                        return Err(Error::ParamError("无限重定向".to_string()));
                    }
                    input = tmp;
                } else {
                    return Err(Error::ParamError("无法解析b23.tv链接".to_string()));
                }
            }
            if input.contains("video/av") {
                let re = regex::Regex::new(r"av(\d+)").unwrap();
                if let Some(caps) = re.captures(input.as_str()) {
                    let avid = caps
                        .get(1)
                        .unwrap()
                        .as_str()
                        .parse::<i64>()
                        .map_err(|e| Error::ParamError(format!("av号解析错误: {}", e)))?;
                    return Ok(self.fix_avid(avid).await?);
                }
                return Err(Error::ParamError("无法解析av号".to_string()));
            }
            if input.to_lowercase().contains("video/bv") {
                let re = regex::Regex::new(r"[Bb][Vv]1(\w+)").unwrap();
                if let Some(caps) = re.captures(input.as_str()) {
                    let bv = caps.get(0).unwrap().as_str();
                    let avid = abv::bv2av(bv)
                        .map_err(|e| Error::ParamError(format!("bv号解析错误: {}", e)))?;
                    return Ok(VideoType::AVID(avid as i64));
                }
                return Err(Error::ParamError("无法解析bv号".to_string()));
            }
            if input.contains("/cheese/") {
                let re_ep = regex::Regex::new(r"/ep(\d+)").unwrap();
                let re_ss = regex::Regex::new(r"/ss(\d+)").unwrap();
                let ep_id = if input.contains("/ep") {
                    if let Some(caps) = re_ep.captures(input.as_str()) {
                        let ep_id = caps.get(1).unwrap().as_str();
                        if let Ok(ep_id_num) = ep_id.parse::<i64>() {
                            ep_id_num
                        } else {
                            return Err(Error::ParamError("无法解析ep_id".to_string()));
                        }
                    } else {
                        return Err(Error::ParamError("无法解析ep_id".to_string()));
                    }
                } else if input.contains("/ss") {
                    if let Some(caps) = re_ss.captures(input.as_str()) {
                        let ss_id = caps.get(1).unwrap().as_str();
                        if let Ok(ss_id_num) = ss_id.parse::<i64>() {
                            self.get_epid_by_ssid(ss_id_num).await?
                        } else {
                            return Err(Error::ParamError("无法解析ss_id".to_string()));
                        }
                    } else {
                        return Err(Error::ParamError("无法解析ss_id".to_string()));
                    }
                } else {
                    return Err(Error::ParamError("无法解析cheese链接".to_string()));
                };
                return Ok(VideoType::CHEESE(ep_id));
            }
            if input.contains("/ep") {
                let re = regex::Regex::new(r"/ep(\d+)").unwrap();
                if let Some(caps) = re.captures(input.as_str()) {
                    let ep_id = caps.get(1).unwrap().as_str();
                    match ep_id.parse::<i64>() {
                        Ok(id) => return Ok(VideoType::EPID(id)),
                        Err(_) => return Err(Error::ParamError("ep_id 不是数字".to_string())),
                    }
                } else {
                    return Err(Error::ParamError("无法解析ep_id".to_string()));
                }
            }
            if input.contains("/ss") {
                let re = regex::Regex::new(r"/ss(\d+)").unwrap();
                if let Some(caps) = re.captures(input.as_str()) {
                    let ss_id = caps.get(1).unwrap().as_str();
                    let ss_id = if let Ok(id) = ss_id.parse::<i64>() {
                        id
                    } else {
                        return Err(Error::ParamError("ss_id 不是数字".to_string()));
                    };
                    let ep_id = self.get_epid_by_bangumi_ssid(ss_id).await?;
                    return Ok(VideoType::EPID(ep_id));
                } else {
                    return Err(Error::ParamError("无法解析ss_id".to_string()));
                }
            }
            if input.contains("/medialist/")
                && input.contains("business_id=")
                && input.contains("business=space_collection")
            {
                let re = regex::Regex::new(r"business_id=([^&]+)").unwrap();
                if let Some(caps) = re.captures(input.as_str()) {
                    return Ok(VideoType::LISTBIZID(
                        caps.get(1).unwrap().as_str().to_string(),
                    ));
                } else {
                    return Err(Error::ParamError("无法解析business_id".to_string()));
                }
            }
            if input.contains("/medialist/")
                && input.contains("business_id=")
                && input.contains("business=space_series")
            {
                let re = regex::Regex::new(r"business_id=([^&]+)").unwrap();
                if let Some(caps) = re.captures(input.as_str()) {
                    return Ok(VideoType::SERIESBIZID(
                        caps.get(1).unwrap().as_str().to_string(),
                    ));
                } else {
                    return Err(Error::ParamError("无法解析business_id".to_string()));
                }
            }

            if input.contains("/channel/collectiondetail?sid=") {
                let re = regex::Regex::new(r"sid=([^&]+)").unwrap();
                if let Some(caps) = re.captures(input.as_str()) {
                    return Ok(VideoType::LISTBIZID(
                        caps.get(1).unwrap().as_str().to_string(),
                    ));
                } else {
                    return Err(Error::ParamError("无法解析sid".to_string()));
                }
            }
            if input.contains("/channel/seriesdetail?sid=") {
                let re = regex::Regex::new(r"sid=([^&]+)").unwrap();
                if let Some(caps) = re.captures(input.as_str()) {
                    return Ok(VideoType::SERIESBIZID(
                        caps.get(1).unwrap().as_str().to_string(),
                    ));
                } else {
                    return Err(Error::ParamError("无法解析sid".to_string()));
                }
            }
            if let Some(list_type) = parse_space_lists_url(input.as_str())? {
                return Ok(list_type);
            }
            if input.contains("/space.bilibili.com/") && input.contains("/favlist") {
                let re_uid = regex::Regex::new(r"space\.bilibili\.com/(\d+)").unwrap();
                let re_fid = regex::Regex::new(r"fid=([^&]+)").unwrap();
                let mid = if let Some(caps) = re_uid.captures(input.as_str()) {
                    caps.get(1).unwrap().as_str().to_string()
                } else {
                    return Err(Error::ParamError("无法解析mid".to_string()));
                };
                let fid = if let Some(caps) = re_fid.captures(input.as_str()) {
                    caps.get(1).unwrap().as_str().to_string()
                } else {
                    return Err(Error::ParamError("无法解析fid".to_string()));
                };
                return Ok(VideoType::FAVID { fid, mid });
            }
            if input.contains("/space.bilibili.com/") {
                let re = regex::Regex::new(r"space\.bilibili\.com/(\d+)").unwrap();
                if let Some(caps) = re.captures(input.as_str()) {
                    return Ok(VideoType::MID(caps.get(1).unwrap().as_str().to_string()));
                } else {
                    return Err(Error::ParamError("无法解析mid".to_string()));
                }
            }
            if input.contains("ep_id=") {
                let re = regex::Regex::new(r"ep_id=([^&]+)").unwrap();
                if let Some(caps) = re.captures(input.as_str()) {
                    let ep_id = caps.get(1).unwrap().as_str();
                    if let Ok(ep_id_num) = ep_id.parse::<i64>() {
                        return Ok(VideoType::EPID(ep_id_num));
                    } else {
                        return Err(Error::ParamError("ep_id 不是数字".to_string()));
                    }
                } else {
                    return Err(Error::ParamError("无法解析ep_id".to_string()));
                }
            }
            {
                let re_global =
                    regex::Regex::new(r"\.bilibili\.tv\/\w+\/play\/\d+\/(\d+)").unwrap();
                if let Some(caps) = re_global.captures(input.as_str()) {
                    let ep_id = caps.get(1).unwrap().as_str();
                    if let Ok(ep_id_num) = ep_id.parse::<i64>() {
                        return Ok(VideoType::EPID(ep_id_num));
                    } else {
                        return Err(Error::ParamError("ep_id 不是数字".to_string()));
                    }
                }
                let re_bangumi = regex::Regex::new(r"bangumi/media/(md\d+)").unwrap();
                if let Some(caps) = re_bangumi.captures(input.as_str()) {
                    let md_id = caps.get(1).unwrap().as_str();
                    let ep_id = self.get_epid_by_md(md_id).await?;
                    return Ok(VideoType::EPID(ep_id));
                }
                let web = self.get_web_source(&input).await?;
                let re_state =
                    regex::Regex::new(r#"window.__INITIAL_STATE__=([\s\S].*?);\(function\(\)"#)
                        .unwrap();
                if let Some(caps) = re_state.captures(&web) {
                    let json_str = caps.get(1).unwrap().as_str();
                    let json: serde_json::Value = serde_json::from_str(json_str)
                        .map_err(|e| Error::ParamError(format!("JSON解析错误: {}", e)))?;
                    if let Some(ep_list) = json.get("epList").and_then(|v| v.as_array()) {
                        if let Some(first_ep) = ep_list.first() {
                            if let Some(id) = first_ep.get("id").and_then(|v| v.as_i64()) {
                                return Ok(VideoType::EPID(id));
                            }
                        }
                    }
                }
            }
        }
        if input.to_lowercase().starts_with("bv") {
            let avid = abv::bv2av(input.as_str())
                .map_err(|e| Error::ParamError(format!("bv号解析错误: {}", e)))?;
            return Ok(VideoType::AVID(avid as i64));
        }
        if input.to_lowercase().starts_with("av") {
            let re = regex::Regex::new(r"av(\d+)").unwrap();
            if let Some(caps) = re.captures(input.as_str()) {
                let avid = caps
                    .get(1)
                    .unwrap()
                    .as_str()
                    .parse::<i64>()
                    .map_err(|e| Error::ParamError(format!("av号解析错误: {}", e)))?;
                return Ok(self.fix_avid(avid).await?);
            } else {
                return Err(Error::ParamError("无法解析av号".to_string()));
            }
        }
        if input.starts_with("cheese/") {
            let re_ep = regex::Regex::new(r"cheese/ep(\d+)").unwrap();
            let re_ss = regex::Regex::new(r"cheese/ss(\d+)").unwrap();
            let ep_id: i64 = if input.contains("/ep") {
                if let Some(caps) = re_ep.captures(input.as_str()) {
                    let ep_id = caps.get(1).unwrap().as_str();
                    if let Ok(ep_id_num) = ep_id.parse::<i64>() {
                        ep_id_num
                    } else {
                        return Err(Error::ParamError("无法解析ep_id".to_string()));
                    }
                } else {
                    return Err(Error::ParamError("无法解析ep_id".to_string()));
                }
            } else if input.contains("/ss") {
                if let Some(caps) = re_ss.captures(input.as_str()) {
                    let ss_id = caps.get(1).unwrap().as_str();
                    let ss_id = if let Ok(id) = ss_id.parse::<i64>() {
                        id
                    } else {
                        return Err(Error::ParamError("ss_id 不是数字".to_string()));
                    };
                    self.get_epid_by_ssid(ss_id).await?
                } else {
                    return Err(Error::ParamError("无法解析ss_id".to_string()));
                }
            } else {
                return Err(Error::ParamError("无法解析cheese链接".to_string()));
            };
            return Ok(VideoType::CHEESE(ep_id));
        }

        if input.starts_with("ep") {
            if let Ok(ep_id) = input[2..].parse::<i64>() {
                return Ok(VideoType::EPID(ep_id));
            } else {
                return Err(Error::ParamError("ep_id 不是数字".to_string()));
            }
        }
        if input.starts_with("ss") {
            let ss_id = &input[2..];
            let ss_id = if let Ok(id) = ss_id.parse::<i64>() {
                id
            } else {
                return Err(Error::ParamError("ss_id 不是数字".to_string()));
            };
            let ep_id = self.get_epid_by_bangumi_ssid(ss_id).await?;
            return Ok(VideoType::EPID(ep_id));
        }
        if input.starts_with("md") {
            let re = regex::Regex::new(r"md(\d+)").unwrap();
            if let Some(caps) = re.captures(input.as_str()) {
                let md_id = caps.get(1).unwrap().as_str();
                let ep_id = self.get_epid_by_md(md_id).await?;
                return Ok(VideoType::EPID(ep_id));
            } else {
                return Err(Error::ParamError("无法解析md_id".to_string()));
            }
        }

        {
            return Err(Error::ParamError("无法解析用户输入".to_string()));
        }
    }

    pub async fn get_epid_by_ssid(&self, ssid: i64) -> Result<i64> {
        let api = format!(
            "https://api.bilibili.com/pugv/view/web/season?season_id={}",
            ssid
        );
        let json: serde_json::Value = self.get_data(&api, None).await?;
        if let Some(ep_id) = json
            .get("episodes")
            .and_then(|e| e.as_array())
            .and_then(|arr| arr.first())
            .and_then(|first| first.get("id"))
            .and_then(|id| id.as_i64())
        {
            Ok(ep_id)
        } else {
            Err(Error::ParamError("无法解析ep_id".to_string()))
        }
    }

    async fn get_epid_by_bangumi_ssid(&self, ssid: i64) -> Result<i64> {
        let api = format!(
            "https://api.bilibili.com/pgc/web/season/section?season_id={}",
            ssid
        );
        let json: serde_json::Value = self.get_json(&api, None).await?;
        if let Some(ep_id) = json
            .get("result")
            .and_then(|r| r.get("main_section"))
            .and_then(|ms| ms.get("episodes"))
            .and_then(|e| e.as_array())
            .and_then(|arr| arr.first())
            .and_then(|first| first.get("id"))
            .and_then(|id| id.as_i64())
        {
            Ok(ep_id)
        } else {
            Err(Error::ParamError("无法解析ep_id".to_string()))
        }
    }

    async fn get_epid_by_md(&self, md_id: &str) -> Result<i64> {
        let api = format!(
            "https://api.bilibili.com/pgc/review/user?media_id={}",
            md_id
        );
        let json: serde_json::Value = self.get_json(&api, None).await?;
        if let Some(ep_id) = json
            .get("result")
            .and_then(|r| r.get("media"))
            .and_then(|m| m.get("new_ep"))
            .and_then(|ne| ne.get("id"))
            .and_then(|id| id.as_i64())
        {
            Ok(ep_id)
        } else {
            Err(Error::ParamError("无法解析ep_id".to_string()))
        }
    }
}

pub fn select_page_indexes(spec: &str, total: i64) -> Result<Vec<i64>> {
    if total <= 0 {
        return Err(Error::ParamError("没有可选择的分P".to_string()));
    }
    let spec = spec.trim();
    if spec.is_empty() || spec.eq_ignore_ascii_case("ALL") {
        return Ok((1..=total).collect());
    }
    let mut selected = Vec::new();
    for token in spec.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        if token.eq_ignore_ascii_case("LAST")
            || token.eq_ignore_ascii_case("LATEST")
            || token.eq_ignore_ascii_case("NEW")
        {
            push_page(&mut selected, total);
            continue;
        }
        if let Some((start, end)) = token.split_once('-') {
            let start = parse_page_number(start)?;
            let end = parse_page_number(end)?;
            if start > end {
                return Err(Error::ParamError(format!("分P范围无效: {token}")));
            }
            for index in start..=end {
                ensure_page_in_range(index, total)?;
                push_page(&mut selected, index);
            }
        } else {
            let index = parse_page_number(token)?;
            ensure_page_in_range(index, total)?;
            push_page(&mut selected, index);
        }
    }
    if selected.is_empty() {
        return Err(Error::ParamError("没有选中任何分P".to_string()));
    }
    Ok(selected)
}

fn parse_page_number(token: &str) -> Result<i64> {
    token
        .trim()
        .parse::<i64>()
        .map_err(|_| Error::ParamError(format!("无法解析分P参数: {token}")))
}

fn ensure_page_in_range(index: i64, total: i64) -> Result<()> {
    if index < 1 || index > total {
        Err(Error::ParamError(format!("分P {index} 超出范围 1-{total}")))
    } else {
        Ok(())
    }
}

fn push_page(selected: &mut Vec<i64>, index: i64) {
    if !selected.contains(&index) {
        selected.push(index);
    }
}

fn parse_space_lists_url(input: &str) -> Result<Option<VideoType>> {
    if !(input.contains("/space.bilibili.com/") && input.contains("/lists/")) {
        return Ok(None);
    }
    let path = input.split(['?', '#']).next().unwrap_or(input);
    let sid = path.rsplit('/').next().unwrap_or("");
    if sid.is_empty() || !sid.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(Error::ParamError("无法解析空间列表 id".to_string()));
    }
    let list_type = query_param(input, "type")
        .unwrap_or_default()
        .to_ascii_lowercase();
    Ok(Some(if list_type == "series" {
        VideoType::SERIESBIZID(sid.to_string())
    } else {
        VideoType::LISTBIZID(sid.to_string())
    }))
}

fn query_param<'a>(input: &'a str, key: &str) -> Option<&'a str> {
    let query = input.split(['?', '#']).nth(1)?;
    for pair in query.split('&') {
        if let Some((name, value)) = pair.split_once('=') {
            if name == key {
                return Some(value);
            }
        }
    }
    None
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum VideoType {
    AVID(i64),
    CHEESE(i64),
    EPID(i64),
    LISTBIZID(String),
    SERIESBIZID(String),
    MID(String),
    FAVID { fid: String, mid: String },
}

#[cfg(test)]
mod tests {
    use crate::parse::VideoType;

    const BV_URL: &'static str = "https://www.bilibili.com/video/BV1P4411T73c/?spm_id_from=333.1387.upload.video_card.click&vd_source=5c131bccac814abb97bb5a4df65ac42b";
    const BV_ID: &'static str = "BV1P4411T73c";
    const SS_URL: &'static str =
        "https://www.bilibili.com/bangumi/play/ss29325?spm_id_from=333.337.0.0";

    #[tokio::test]
    async fn test_parse_bv() {
        crate::tests::log_init();
        let client = &crate::tests::BBDD;
        let target = VideoType::AVID(abv::bv2av(BV_ID).unwrap() as i64);
        let parse = client.parse_input(BV_URL).await.unwrap();
        assert!(target.eq(&parse));
        let parse = client.parse_input(BV_ID).await.unwrap();
        assert!(target.eq(&parse));
    }

    #[tokio::test]
    async fn test_parse_ss() {
        crate::tests::log_init();
        let client = &crate::tests::BBDD;
        let target = VideoType::EPID(307247);
        let parse = client.parse_input(SS_URL).await.unwrap();
        println!("{:#?}", parse);
        assert!(target.eq(&parse));
        let parse = client.parse_input("ss29325").await.unwrap();
        assert!(target.eq(&parse));
    }

    #[test]
    fn test_parse_space_lists() {
        let season = "https://space.bilibili.com/392959666/lists/1560264?type=season";
        assert_eq!(
            super::parse_space_lists_url(season).unwrap(),
            Some(VideoType::LISTBIZID("1560264".to_string()))
        );
        let series = "https://space.bilibili.com/392959666/lists/1560264?type=series";
        assert_eq!(
            super::parse_space_lists_url(series).unwrap(),
            Some(VideoType::SERIESBIZID("1560264".to_string()))
        );
        let unknown = "https://space.bilibili.com/392959666/lists/99";
        assert_eq!(
            super::parse_space_lists_url(unknown).unwrap(),
            Some(VideoType::LISTBIZID("99".to_string()))
        );
    }

    #[test]
    fn test_select_page_indexes() {
        assert_eq!(super::select_page_indexes("ALL", 3).unwrap(), vec![1, 2, 3]);
        assert_eq!(super::select_page_indexes("8", 8).unwrap(), vec![8]);
        assert_eq!(super::select_page_indexes("1,2", 4).unwrap(), vec![1, 2]);
        assert_eq!(super::select_page_indexes("3-5", 6).unwrap(), vec![3, 4, 5]);
        assert_eq!(super::select_page_indexes("LAST", 4).unwrap(), vec![4]);
        assert!(super::select_page_indexes("9", 3).is_err());
    }
}
