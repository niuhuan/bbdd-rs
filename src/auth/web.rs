const LOGIN_URL: &'static str =
    "https://passport.bilibili.com/x/passport-login/web/qrcode/generate?source=main-fe-header";

const VERIFY_URL: &'static str = "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={qrcodeKey}&source=main-fe-header";

const NAV_URL: &'static str = "https://api.bilibili.com/x/web-interface/nav";

pub use crate::BBDD;
pub use crate::error::{Error, Result};

pub use serde::{Deserialize, Serialize};

impl BBDD {
    pub async fn web_nav(&self) -> Result<WebNavData> {
        let json = self.get_json(NAV_URL, None).await?;
        parse_nav(json)
    }

    pub async fn web_login_qr_url(&self) -> Result<WebLoginQRData> {
        self.get_data(LOGIN_URL, None).await
    }

    // {"code":0,"message":"0","ttl":1,"data":{"url":"","refresh_token":"","timestamp":0,"code":86101,"message":"未扫码"}}
    // {"code":0,"message":"0","ttl":1,"data":{"url":"","refresh_token":"","timestamp":0,"code":86090,"message":"二维码已扫码未确认"}}
    //  {"code":0,"message":"0","ttl":1,"data":{"url":"https://passport.biligame.com/x/passport-login/web/crossDomain?DedeUserID=1856****",
    //  "refresh_token":"85171a7","timestamp":1757342829939,"code":0,"message":""}}
    // {"code":0,"message":"0","ttl":1,"data":{"url":"","refresh_token":"","timestamp":0,"code":86038,"message":"二维码已失效"}}
    pub async fn web_login_qr_verify(&self, qrcode_key: &str) -> Result<WebLoginQRVerifyData> {
        let url = VERIFY_URL.replace("{qrcodeKey}", qrcode_key);
        let response = self
            .request(reqwest::Method::GET, &url, None, None)
            .header("Referer", "https://www.bilibili.com/")
            .send()
            .await?;
        let set_cookies = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let status = response.status();
        let text = response.text().await?;
        tracing::debug!("Response Status: {}", status);
        tracing::debug!("Response Body: {}", text);
        let response: crate::WebResponseData = serde_json::from_str(&text)?;
        if response.code != 0 {
            return Err(Error::ApiError {
                code: response.code,
                message: response.message,
            });
        }
        let mut data: WebLoginQRVerifyData = serde_json::from_value(response.data)?;
        data.cookie =
            crate::util::normalize_login_cookie(&data.url, &data.refresh_token, &set_cookies);
        Ok(data)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct WebLoginQRData {
    pub url: String,
    pub qrcode_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct WebLoginQRVerifyData {
    pub url: String,
    pub refresh_token: String,
    pub timestamp: i64,
    pub code: i32,
    pub message: String,
    /// 由查询串和 poll 的 Set-Cookie 组装出的登录 cookie。旧文件没有这个字段。
    #[serde(default)]
    pub cookie: String,
}

impl WebLoginQRVerifyData {
    pub fn stored_cookie(&self) -> Result<String> {
        if !self.cookie.trim().is_empty() {
            Ok(self.cookie.clone())
        } else {
            crate::util::url_to_cookie(&self.url)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct WebNavWbiImg {
    #[serde(default)]
    pub img_url: String,
    #[serde(default)]
    pub sub_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct WebNavVipLabel {
    #[serde(default)]
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct WebNavVip {
    #[serde(default, rename = "type")]
    pub vip_type: i32,
    #[serde(default)]
    pub status: i32,
    #[serde(default)]
    pub due_date: i64,
    #[serde(default)]
    pub label: WebNavVipLabel,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct WebNavData {
    #[serde(default, rename = "isLogin")]
    pub is_login: bool,
    #[serde(default)]
    pub mid: i64,
    #[serde(default)]
    pub uname: String,
    #[serde(default)]
    pub vip: WebNavVip,
    #[serde(default)]
    pub money: f64,
    #[serde(default, rename = "wbi_img")]
    pub wbi_img: WebNavWbiImg,
}

fn parse_nav(json: serde_json::Value) -> Result<WebNavData> {
    let data = json.get("data").cloned().unwrap_or(serde_json::Value::Null);
    let nav: WebNavData = serde_json::from_value(data).unwrap_or_default();
    if !nav.wbi_img.img_url.is_empty() && !nav.wbi_img.sub_url.is_empty() {
        return Ok(nav);
    }
    let code = json.get("code").and_then(|value| value.as_i64()).unwrap_or(-1) as i32;
    let message = json
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or("nav 没有返回 wbi 图片")
        .to_string();
    if code != 0 {
        return Err(Error::ApiError { code, message });
    }
    Err(Error::StateError(message))
}

#[cfg(test)]
mod tests {

    #[test]
    fn logged_out_nav_still_exposes_wbi() {
        let json = serde_json::json!({
            "code": -101,
            "message": "账号未登录",
            "data": {
                "isLogin": false,
                "wbi_img": {
                    "img_url": "https://i0.hdslb.com/bfs/wbi/abc.png",
                    "sub_url": "https://i0.hdslb.com/bfs/wbi/def.png"
                }
            }
        });
        let nav = super::parse_nav(json).unwrap();
        assert!(!nav.is_login);
        assert!(nav.wbi_img.img_url.contains("abc.png"));
    }

    #[tokio::test]
    async fn test_web_log_qr_url() {
        crate::tests::log_init();
        let url = crate::tests::BBDD.web_login_qr_url().await.unwrap();
        println!("{:?}", url);
        qr2term::print_qr(url.url.as_str()).unwrap();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            let verify = crate::tests::BBDD
                .web_login_qr_verify(&url.qrcode_key)
                .await
                .unwrap();
            if verify.code == 86101 || verify.code == 86090 {
                continue;
            }
            if verify.code == 0 {
                println!("Login success: {:?}", verify);
                crate::tests::store_login(&verify).unwrap();
                break;
            }
            panic!("Login failed: {:?}", verify);
        }
    }
}
