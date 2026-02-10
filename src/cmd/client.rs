use std::path::Path;
use std::sync::Arc;
use tokio::sync::OnceCell;
#[cfg(feature = "http2")]
use std::time::Duration;

pub(crate) static CONFIG_DIR: OnceCell<String> = OnceCell::const_new();
pub(crate) static CLIENT_CELL: OnceCell<bbdd::BBDD> = OnceCell::const_new();

pub(crate) async fn init_client(client_dir: String) {
    CONFIG_DIR.get_or_init(|| async { client_dir }).await;
    CLIENT_CELL.get_or_init(|| async { bbdd().await }).await;
}

async fn bbdd() -> bbdd::BBDD {
    let ua = ua().await;
    let builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
    #[cfg(feature = "http2")]
    let builder = builder
        .http2_adaptive_window(true)
        .http2_keep_alive_interval(Some(Duration::from_secs(20)))
        .http2_keep_alive_timeout(Duration::from_secs(20));
    #[cfg(feature = "http3")]
    let builder = builder.http3_prior_knowledge();
    let agent = builder.build().unwrap();
    let cookie = cookie().await;
    bbdd::BBDD {
        agent: Arc::new(agent),
        ua,
        cookie,
    }
}

async fn ua() -> String {
    let ua_path = format!("{}/{}", CONFIG_DIR.get().unwrap(), "ua.txt");
    let ua_path = Path::new(&ua_path);
    if ua_path.exists() {
        tokio::fs::read_to_string(ua_path)
            .await
            .expect(format!("无法读取配置文件 : {}", ua_path.display()).as_str())
    } else {
        let ua = bbdd::util::random_user_agent();
        std::fs::write(ua_path, &ua).unwrap();
        ua
    }
}

async fn cookie() -> String {
    let cookie_path = format!("{}/{}", CONFIG_DIR.get().unwrap(), "web_login.json");
    let cookie_path = Path::new(&cookie_path);
    if cookie_path.exists() {
        let json = tokio::fs::read_to_string(cookie_path)
            .await
            .expect(format!("无法读取配置文件 : {}", cookie_path.display()).as_str());
        let data: bbdd::auth::web::WebLoginQRVerifyData = serde_json::from_str(&json).unwrap();
        bbdd::util::url_to_cookie(&data.url).unwrap()
    } else {
        String::new()
    }
}

pub(crate) fn store_login(data: &bbdd::auth::web::WebLoginQRVerifyData)  {
    let cookie_path = format!("{}/{}", CONFIG_DIR.get().unwrap(), "web_login.json");
    std::fs::write(cookie_path.as_str(), serde_json::to_string_pretty(data).unwrap()).expect(
        format!("无法写入配置文件 : {}", cookie_path).as_str(),
    );
}
