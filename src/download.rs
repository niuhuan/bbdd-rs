use reqwest::Method;

use crate::{BBDD, Result};

impl BBDD {
    pub async fn download_resource_head(&self, url: &str) -> Result<reqwest::Response> {
        let response = self
            .agent
            .request(Method::HEAD, url)
            .header("Referer", "https://www.bilibili.com")
            .header("User-Agent", &self.ua)
            .header("Accept-Encoding", "gzip, deflate")
            .header("Cookie", &self.cookie)
            .send()
            .await?;
        Ok(response)
    }

    // if file size is 1024, range_start is 0, range_end is 1023
    // contiue download from file len = 30, range_start is 30, range_end is 1023 or None
    pub async fn download_resource_with_range(
        &self,
        url: &str,
        range_start: impl Into<Option<u64>>,
        range_end: impl Into<Option<u64>>,
    ) -> Result<reqwest::Response> {
        let range_start = range_start.into();
        let range_end = range_end.into();
        let mut request_builder = self
            .agent
            .request(Method::GET, url)
            .header("Referer", "https://www.bilibili.com")
            .header("User-Agent", &self.ua)
            .header("Accept-Encoding", "gzip, deflate")
            .header("Cookie", &self.cookie);
        if range_start.is_some() || range_end.is_some() {
            let range_header = match (range_start, range_end) {
                (Some(start), Some(end)) => format!("bytes={}-{}", start, end),
                (Some(start), None) => format!("bytes={}-", start),
                (None, Some(end)) => format!("bytes=-{}", end),
                (None, None) => "".to_string(),
            };
            if !range_header.is_empty() {
                request_builder = request_builder.header("Range", range_header);
            }
        }
        let response = request_builder.send().await?;
        Ok(response)
    }

    pub async fn download_resource(&self, url: &str) -> Result<reqwest::Response> {
        self.download_resource_with_range(url, None, None).await
    }
}

pub const MIRROR_HOST: &str = "upos-sz-mirrorcoso1.bilivideo.com";

pub fn prepare_media_urls(urls: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut originals = Vec::new();
    for url in urls {
        if let Some(rewritten) = rewrite_media_url(url) {
            push_unique(&mut out, rewritten);
        }
        originals.push(url.clone());
    }
    for url in originals {
        push_unique(&mut out, url);
    }
    out
}

fn rewrite_media_url(url: &str) -> Option<String> {
    if url.contains(".mcdn.bilivideo.cn:") {
        return None;
    }
    let rest = url.split_once("://")?.1;
    let (_host, path) = rest.split_once('/')?;
    if path.is_empty() {
        return None;
    }
    Some(format!("http://{MIRROR_HOST}/{path}"))
}

fn push_unique(urls: &mut Vec<String>, url: String) {
    if !urls.iter().any(|item| item == &url) {
        urls.push(url);
    }
}

#[cfg(test)]
mod tests {
    use super::prepare_media_urls;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn test_prepare_media_urls_rewrites_host_and_keeps_original() {
        let urls = vec!["https://upos-sz-estgoss.bilivideo.com/upgcxcode/a.m4s?e=1".to_string()];
        let out = prepare_media_urls(&urls);
        assert_eq!(
            out[0],
            "http://upos-sz-mirrorcoso1.bilivideo.com/upgcxcode/a.m4s?e=1"
        );
        assert_eq!(out[1], urls[0]);
    }

    #[test]
    fn test_prepare_media_urls_keeps_mcdn_port() {
        let url = "https://xy123.mcdn.bilivideo.cn:8082/v.m4s".to_string();
        let out = prepare_media_urls(&[url.clone()]);
        assert_eq!(out, vec![url]);
    }

    #[test]
    fn test_prepare_media_urls_rewrites_pcdn() {
        let url = "https://127.0.0.1:8080/upgcxcode/a.m4s".to_string();
        let out = prepare_media_urls(&[url.clone()]);
        assert_eq!(
            out[0],
            "http://upos-sz-mirrorcoso1.bilivideo.com/upgcxcode/a.m4s"
        );
        assert_eq!(out[1], url);
    }

    use crate::tests::log_init;

    #[tokio::test]
    async fn test_bili_download() {
        log_init();
        let client = &crate::tests::BBDD;
        let aid = 54916636;
        let cid = 96040706;
        let play_url = client.play_url_with_qn(aid, cid, 127).await.unwrap();
        // video
        let download_url = play_url
            .dash
            .video
            .iter()
            .max_by_key(|v| (v.id, v.bandwidth))
            .unwrap()
            .base_url
            .clone();
        let mut response = client
            .download_resource(download_url.as_str())
            .await
            .unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("video.mp4");
        let mut file = tokio::fs::File::create(path).await.unwrap();
        while let Some(chunk) = response.chunk().await.unwrap() {
            file.write_all(&chunk).await.unwrap();
        }
        drop(file);
        drop(response);
        // audio
        let download_url = play_url
            .dash
            .audio
            .iter()
            .max_by_key(|a| a.bandwidth)
            .unwrap()
            .base_url
            .clone();
        let mut response = client
            .download_resource(download_url.as_str())
            .await
            .unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("audio.m4a");
        let mut file = tokio::fs::File::create(path).await.unwrap();
        while let Some(chunk) = response.chunk().await.unwrap() {
            file.write_all(&chunk).await.unwrap();
        }
        drop(file);
        drop(response);
    }
}
