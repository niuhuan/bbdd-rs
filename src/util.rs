use crate::Error;

pub fn random_user_agent() -> String {
    format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{} Safari/537.36",
        rand::random::<u8>() % 31 + 80
    )
}

pub fn url_to_cookie(url: &str) -> crate::Result<String> {
    Ok(url
        .split("?")
        .last()
        .ok_or(Error::ParamError(
            "Invalid URL, missing query parameters".to_string(),
        ))?
        .replace("&", ";")
        .replace(",", "%2C"))
}

const LOGIN_COOKIE_NAMES: &[&str] = &[
    "DedeUserID",
    "DedeUserID__ckMd5",
    "SESSDATA",
    "bili_jct",
    "sid",
];

/// 按 BBDownT `NormalizeLoginCookie` 组装登录 cookie。
/// 查询串只保留登录字段；`Set-Cookie` 覆盖同名值；`refresh_token` 存成 `ac_time_value`。
pub fn normalize_login_cookie(
    login_url: &str,
    refresh_token: &str,
    set_cookie_headers: &[String],
) -> String {
    let mut refresh_token = refresh_token.to_string();
    let mut query = match login_url.split_once('?') {
        Some((_, query)) => query,
        None => login_url,
    };
    if let Some((query_without_fragment, _)) = query.split_once('#') {
        query = query_without_fragment;
    }

    let mut cookies: Vec<(String, String)> = Vec::new();
    for part in query.split('&') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let Some((name, value)) = part.split_once('=') else {
            continue;
        };
        let name = name.trim();
        let value = value.trim();
        if name.is_empty() {
            continue;
        }
        if name.eq_ignore_ascii_case("refresh_token") {
            if refresh_token.trim().is_empty() {
                refresh_token = value.to_string();
            }
            continue;
        }
        if LOGIN_COOKIE_NAMES
            .iter()
            .any(|allowed| name.eq_ignore_ascii_case(allowed))
        {
            upsert_cookie(&mut cookies, name, value.replace(',', "%2C"));
        }
    }

    for header in set_cookie_headers {
        let first = header.split(';').next().unwrap_or("").trim();
        let Some((name, value)) = first.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        upsert_cookie(&mut cookies, name, value.trim().to_string());
    }

    if !refresh_token.trim().is_empty() {
        upsert_cookie(
            &mut cookies,
            "ac_time_value",
            refresh_token.replace(',', "%2C"),
        );
    }

    cookies
        .into_iter()
        .filter(|(name, _)| !name.trim().is_empty())
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(";")
}

pub fn has_required_login_cookies(cookie: &str) -> bool {
    cookie_value(cookie, "SESSDATA").is_some_and(|value| !value.trim().is_empty())
        && cookie_value(cookie, "bili_jct").is_some_and(|value| !value.trim().is_empty())
}

fn upsert_cookie(cookies: &mut Vec<(String, String)>, name: &str, value: String) {
    if let Some((_, existing)) = cookies
        .iter_mut()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
    {
        *existing = value;
    } else {
        cookies.push((name.to_string(), value));
    }
}

fn cookie_value<'a>(cookie: &'a str, name: &str) -> Option<&'a str> {
    let mut found = None;
    for part in cookie.split([';', '&']) {
        let part = part.trim();
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case(name) {
            found = Some(value.trim());
        }
    }
    found
}

pub fn take_json_field<T: for<'de> serde::Deserialize<'de>>(
    json: &serde_json::Value,
    field: &str,
) -> crate::Result<T> {
    let value = json
        .get(field)
        .ok_or(Error::StateError(format!("Missing field: {}", field)))?;
    let result: T = serde_json::from_value(value.clone())?;
    Ok(result)
}

pub fn video_quality_to_string(quality: i32) -> String {
    match quality {
        127 => "8K 超高清".to_string(),
        126 => "杜比视界".to_string(),
        125 => "HDR 真彩".to_string(),
        120 => "4K 超清".to_string(),
        116 => "1080P 高帧率".to_string(),
        112 => "1080P 高码率".to_string(),
        100 => "智能修复".to_string(),
        80 => "1080P 高清".to_string(),
        74 => "720P 高帧率".to_string(),
        64 => "720P 高清".to_string(),
        48 => "720P 高清".to_string(),
        32 => "480P 清晰".to_string(),
        16 => "360P 流畅".to_string(),
        5 => "144P 流畅".to_string(),
        6 => "240P 流畅".to_string(),
        _ => format!("QUALITY-{}", quality),
    }
}

pub fn audio_quality_to_string(quality: i32) -> String {
    match quality {
        30216 => "64K".to_string(),
        30232 => "132K".to_string(),
        30280 => "192K".to_string(),
        _ => format!("AUDIO-{}", quality),
    }
}

#[cfg(test)]
mod tests {
    use super::{has_required_login_cookies, normalize_login_cookie};

    const TICKET_URL: &str = "https://passport.biligame.com/crossDomain?ticket=ticket-value&gourl=https%3A%2F%2Fwww.bilibili.com&first_domain=.bilibili.com";

    #[test]
    fn normalize_login_cookie_uses_set_cookie() {
        let headers = vec![
            "SESSDATA=session-value; Path=/; Domain=.bilibili.com; HttpOnly; Secure".to_string(),
            "bili_jct=csrf-value; Path=/; Domain=.bilibili.com".to_string(),
            "DedeUserID=12345; Path=/; Domain=.bilibili.com".to_string(),
        ];
        let cookie = normalize_login_cookie(TICKET_URL, "refresh-value", &headers);

        assert!(cookie.contains("SESSDATA=session-value"));
        assert!(cookie.contains("bili_jct=csrf-value"));
        assert!(cookie.contains("DedeUserID=12345"));
        assert!(cookie.contains("ac_time_value=refresh-value"));
        assert!(!cookie.contains("ticket="));
        assert!(!cookie.contains("gourl="));
        assert!(!cookie.contains("first_domain="));
        assert!(has_required_login_cookies(&cookie));
    }

    #[test]
    fn normalize_login_cookie_rejects_redirect_metadata() {
        let cookie = normalize_login_cookie(TICKET_URL, "refresh-value", &[]);

        assert_eq!(cookie, "ac_time_value=refresh-value");
        assert!(!has_required_login_cookies(&cookie));
    }

    #[test]
    fn stored_login_without_cookie_field_falls_back_to_url() {
        let json = r#"{"url":"https://passport.bilibili.com/x/passport-login/web/crossDomain?DedeUserID=1&SESSDATA=abc&bili_jct=jct","refresh_token":"","timestamp":0,"code":0,"message":""}"#;
        let data: crate::auth::web::WebLoginQRVerifyData = serde_json::from_str(json).unwrap();
        assert!(data.cookie.is_empty());
        assert_eq!(
            data.stored_cookie().unwrap(),
            "DedeUserID=1;SESSDATA=abc;bili_jct=jct"
        );
    }
}
