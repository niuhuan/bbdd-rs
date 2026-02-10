use std::{path::Path, sync::Arc};
#[cfg(feature = "http2")]
use std::time::Duration;

pub(crate) fn log_init() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();
}

pub(crate) static BBDD: std::sync::LazyLock<crate::BBDD> =
    std::sync::LazyLock::<crate::BBDD>::new(|| bbdd());

fn bbdd() -> crate::BBDD {
    let ua = ua();
    let builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
    #[cfg(feature = "http2")]
    let builder = builder
        .http2_adaptive_window(true)
        .http2_keep_alive_interval(Some(Duration::from_secs(20)))
        .http2_keep_alive_timeout(Duration::from_secs(20));
    #[cfg(feature = "http3")]
    let builder = builder.http3_prior_knowledge();
    let agent = builder.build().unwrap();
    let cookie = cookie();
    crate::BBDD {
        agent: Arc::new(agent),
        ua: ua,
        cookie: cookie,
    }
}

fn ua() -> String {
    let ua_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("ua.txt");
    if ua_path.exists() {
        std::fs::read_to_string(ua_path).unwrap()
    } else {
        let ua = crate::util::random_user_agent();
        std::fs::write(ua_path, &ua).unwrap();
        ua
    }
}

fn cookie() -> String {
    let cookie_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("login.json");
    if cookie_path.exists() {
        let json = std::fs::read_to_string(cookie_path).unwrap();
        let data: crate::auth::web::WebLoginQRVerifyData = serde_json::from_str(&json).unwrap();
        crate::util::url_to_cookie(&data.url).unwrap()
    } else {
        String::new()
    }
}

pub(crate) fn store_login(data: &crate::auth::web::WebLoginQRVerifyData) -> std::io::Result<()> {
    let login_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("login.json");
    std::fs::write(login_path, serde_json::to_string_pretty(data).unwrap())
}
