use std::cmp::Reverse;

use crate::fetcher::{VideoMedia, VideoPlayUrl};
use crate::{Error, Result};

pub fn select_video(play_url: &VideoPlayUrl, quality: Option<i64>) -> Result<VideoMedia> {
    let selected = if let Some(quality) = quality {
        play_url
            .dash
            .video
            .iter()
            .filter(|video| video.id == quality)
            .max_by_key(|video| (codec_rank(&video.codecs), Reverse(video.bandwidth)))
            .or_else(|| {
                play_url
                    .dash
                    .video
                    .iter()
                    .filter(|video| video.id <= quality)
                    .max_by_key(|video| (video.id, codec_rank(&video.codecs), Reverse(video.bandwidth)))
            })
            .or_else(|| play_url.dash.video.iter().min_by_key(|video| video.id))
    } else {
        play_url.dash.video.iter().max_by_key(|video| {
            (
                video.id,
                codec_rank(&video.codecs),
                Reverse(video.bandwidth),
            )
        })
    };
    selected
        .cloned()
        .ok_or_else(|| Error::StateError("视频下载地址列表为空".to_string()))
}

pub fn select_audio(play_url: &VideoPlayUrl) -> Result<VideoMedia> {
    play_url
        .dash
        .audio
        .iter()
        .max_by_key(|audio| (audio_codec_rank(&audio.codecs), audio.bandwidth))
        .cloned()
        .ok_or_else(|| Error::StateError("音频下载地址列表为空".to_string()))
}

pub fn media_urls(media: &VideoMedia) -> Vec<String> {
    let mut urls = vec![media.base_url.clone()];
    urls.extend(media.backup_url.clone());
    urls
}

fn codec_rank(codecs: &str) -> i32 {
    let codecs = codecs.to_ascii_lowercase();
    if codecs.contains("av01") {
        3
    } else if codecs.contains("hev1") || codecs.contains("hvc1") || codecs.contains("hevc") {
        2
    } else if codecs.contains("avc") || codecs.contains("h264") {
        1
    } else {
        0
    }
}

fn audio_codec_rank(codecs: &str) -> i32 {
    let codecs = codecs.to_ascii_lowercase();
    if codecs.contains("flac") {
        3
    } else if codecs.contains("ec-3") || codecs.contains("eac3") || codecs.contains("ac-3") {
        2
    } else if codecs.contains("mp4a") || codecs.contains("aac") {
        1
    } else {
        0
    }
}
