use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

pub struct BBDD {
    pub agent: Arc<reqwest::Client>,
    pub ua: String,
    pub cookie: String,
}

impl BBDD {
    pub fn request(
        &self,
        method: reqwest::Method,
        url: &str,
        query: Option<serde_json::Value>,
        body: Option<serde_json::Value>,
    ) -> reqwest::RequestBuilder {
        let cookie = &self.cookie;
        let request = self
            .agent
            .request(method, url)
            .header("User-Agent", &self.ua)
            .header("Accept-Encoding", "gzip, deflate")
            .header(
                "Cookie",
                if url.contains("/ep") || url.contains("/ss") {
                    format!("{};CURRENT_FNVAL=4048;", cookie)
                } else {
                    cookie.to_string()
                },
            );
        let request = if url.contains("api.bilibili.com") {
            request.header("Referer", "https://www.bilibili.com/")
        } else if url.contains("api.bilibili.tv") {
            request.header(
                "sec-ch-ua",
                r#""Google Chrome";v="131", "Chromium";v="131", "Not_A Brand";v="24""#,
            )
        } else {
            request
        };
        let request = request.header("Cache-Control", "no-cache").query(&query);
        if let Some(body) = body {
            request.json(&body)
        } else {
            request
        }
    }

    pub async fn take_data<T: for<'de> serde::Deserialize<'de>>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T> {
        let response = request.send().await?;
        let code = response.status();
        let text = response.text().await?;
        debug!("Response Status: {}", code);
        debug!("Response Body: {}", text);
        let response: WebResponseData = serde_json::from_str(&text)?;
        if response.code != 0 {
            return Err(Error::ApiError {
                code: response.code,
                message: response.message,
            });
        }
        Ok(serde_json::from_value(response.data)?)
    }

    pub async fn take_result<T: for<'de> serde::Deserialize<'de>>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T> {
        let response = request.send().await?;
        let code = response.status();
        let text = response.text().await?;
        debug!("Response Status: {}", code);
        debug!("Response Body: {}", text);
        let response: WebResponseResult = serde_json::from_str(&text)?;
        if response.code != 0 {
            return Err(Error::ApiError {
                code: response.code,
                message: response.message,
            });
        }
        Ok(serde_json::from_value(response.result)?)
    }

    pub async fn take_json(&self, request: reqwest::RequestBuilder) -> Result<serde_json::Value> {
        let response = request.send().await?;
        let code = response.status();
        let text = response.text().await?;
        debug!("Response Status: {}", code);
        debug!("Response Body: {}", text);
        Ok(serde_json::from_str(&text)?)
    }

    pub async fn get_data<T: for<'de> serde::Deserialize<'de>>(
        &self,
        url: &str,
        query: Option<serde_json::Value>,
    ) -> Result<T> {
        self.take_data(self.request(reqwest::Method::GET, url, query, None))
            .await
    }

    pub async fn get_result<T: for<'de> serde::Deserialize<'de>>(
        &self,
        url: &str,
        query: Option<serde_json::Value>,
    ) -> Result<T> {
        self.take_result(self.request(reqwest::Method::GET, url, query, None))
            .await
    }

    pub async fn get_json(
        &self,
        url: &str,
        query: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        self.take_json(self.request(reqwest::Method::GET, url, query, None))
            .await
    }

    pub async fn get_302_location(&self, url: &str) -> Result<Option<String>> {
        let response = self
            .request(reqwest::Method::GET, url, None, None)
            .send()
            .await?;
        let code = response.status();
        let header = response.headers().clone();
        let response = response.text();
        debug!("Response Status: {}", code);
        debug!("Response Body: {}", response.await?);
        if code.is_redirection() {
            if let Some(location) = header.get("location") {
                if let Ok(location) = location.to_str() {
                    return Ok(Some(location.to_string()));
                }
            }
        }
        Ok(None)
    }

    pub async fn get_web_source(&self, url: &str) -> Result<String> {
        let request = self.request(reqwest::Method::GET, url, None, None);
        let response = request.send().await?;
        let code = response.status();
        let text = response.text().await?;
        debug!("Response Status: {}", code);
        debug!("Response Status: {}", code);
        Ok(text)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct WebResponseData {
    pub code: i32,
    pub message: String,
    pub data: serde_json::Value,
    #[serde(default)]
    pub ttl: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct WebResponseResult {
    pub code: i32,
    pub message: String,
    pub result: serde_json::Value,
}
