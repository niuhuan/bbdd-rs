use crate::Error;

pub fn random_user_agent() -> String {
    format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{} Safari/537.36",
        rand::random::<u8>() % 31 + 80
    )
}

pub fn url_to_cookie(url: &str) -> crate::Result<String> {
    Ok(
        url.split("?")
            .last()
            .ok_or(Error::ParamError(
                "Invalid URL, missing query parameters".to_string(),
            ))?
            .replace("&", ";"), //.replace(",", "%2C")
    )
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
