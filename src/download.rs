use crate::{BBDD, Result};

impl BBDD {
    // if file size is 1024, range_start is 0, range_end is 1023
    // contiue download from file len = 30, range_start is 30, range_end is 1023 or None
    pub async fn download_resource_with_range(
        &self,
        url: &str,
        range_start: Option<u64>,
        range_end: Option<u64>,
    ) -> Result<reqwest::Response> {
        let mut request_builder = self.request(reqwest::Method::GET, url, None, None);
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
