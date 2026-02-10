const LOGIN_URL: &'static str =
    "https://passport.bilibili.com/x/passport-login/web/qrcode/generate?source=main-fe-header";

const VERIFY_URL: &'static str = "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={qrcodeKey}&source=main-fe-header";

const NAV_URL: &'static str = "https://api.bilibili.com/x/web-interface/nav";

pub use crate::BBDD;
pub use crate::error::{Error, Result};

pub use serde::{Deserialize, Serialize};

impl BBDD {
    pub async fn web_nav(&self) -> Result<WebNavData> {
        self.get_data(NAV_URL, None).await
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
        self.get_data(&url, None).await
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

#[cfg(test)]
mod tests {

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
