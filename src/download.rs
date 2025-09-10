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

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt;

    use crate::tests::log_init;

    #[tokio::test]
    async fn test_bili_download() {
        log_init();
        let client = &crate::tests::BBDD;
        let aid = 54916636;
        let cid = 96040706;
        let play_url = client.play_url(aid, cid).await.unwrap();
        // video
        let download_url = play_url.dash.video.first().unwrap().base_url.clone();
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
        let download_url = play_url.dash.audio.first().unwrap().base_url.clone();
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
