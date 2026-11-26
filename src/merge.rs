//! 进程内流复制。只在 `rsmpeg` 打开时编译。
//! `ffmpeg6`–`ffmpeg9` 和 `rsmpeg-build` 都会打开 `rsmpeg`。
//! 封面作为 attached picture 写进成品。打不开封面时跳过，不让音视频合并失败。

use std::collections::HashMap;

use crate::progress::{self, Control, Stage, Throttle, PROGRESS_INTERVAL};
use crate::{Error, Result};

/// 合并音视频。`cover` 能打开时写成封面，返回是否写进去了。
pub async fn merge_files(
    inputs: &[&str],
    output: &str,
    cover: Option<&str>,
    control: &Control,
) -> Result<bool> {
    let inputs: Vec<String> = inputs.iter().map(|item| (*item).to_string()).collect();
    let output = output.to_string();
    let cover = cover.map(str::to_string);
    let control = control.clone();
    tokio::task::spawn_blocking(move || merge_blocking(inputs, output, cover, control))
        .await
        .map_err(|err| Error::StateError(format!("合并任务中断: {err}")))?
}

fn merge_blocking(
    inputs: Vec<String>,
    output: String,
    cover: Option<String>,
    control: Control,
) -> Result<bool> {
    if control.is_cancelled() {
        return Err(Error::Cancelled);
    }
    ffmpeg_next::init().map_err(ff_err)?;
    ffmpeg_next::log::set_level(ffmpeg_next::log::Level::Error);

    let mut total_packets = 0u64;
    for input in &inputs {
        if control.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let mut input_format_context = ffmpeg_next::format::input(input).map_err(ff_err)?;
        for _ in input_format_context.packets() {
            if control.is_cancelled() {
                return Err(Error::Cancelled);
            }
            total_packets += 1;
        }
    }

    let mut output_format_context = ffmpeg_next::format::output(&output).map_err(ff_err)?;
    let mut opened = Vec::new();
    for input in &inputs {
        if control.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let input_format_context = ffmpeg_next::format::input(input).map_err(ff_err)?;
        let mut stream_index_map = HashMap::new();
        for av_stream_ref in input_format_context.streams() {
            let time_base = av_stream_ref.time_base();
            let mut out_stream = output_format_context
                .add_stream(None::<ffmpeg_next::Codec>)
                .map_err(ff_err)?;
            out_stream.set_parameters(av_stream_ref.parameters());
            unsafe {
                (*out_stream.parameters().as_mut_ptr()).codec_tag = 0;
            }
            out_stream.set_time_base(time_base);
            stream_index_map.insert(av_stream_ref.index(), out_stream.index());
        }
        opened.push((input_format_context, stream_index_map));
    }
    let cover_input = match cover.as_deref() {
        Some(path) => match prepare_cover(&mut output_format_context, path) {
            Ok(opened) => opened,
            Err(err) => {
                tracing::warn!("无法读取封面，跳过: {err}");
                None
            }
        },
        None => None,
    };
    output_format_context.write_header().map_err(ff_err)?;

    let mut throttle = Throttle::new(PROGRESS_INTERVAL);
    let mut done = 0u64;
    let emit = |done: u64, force: bool, throttle: &mut Throttle| {
        if throttle.allow(force) {
            control.emit(progress::update(
                Stage::Merge,
                "",
                done,
                Some(total_packets),
            ));
        }
    };
    emit(0, true, &mut throttle);

    for (mut input_format_context, stream_index_map) in opened {
        for (stream, mut packet) in input_format_context.packets() {
            if control.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let Some(&out_index) = stream_index_map.get(&stream.index()) else {
                continue;
            };
            let out_time_base = output_format_context
                .stream(out_index)
                .ok_or_else(|| Error::StateError(format!("输出流 {out_index} 不存在")))?
                .time_base();
            packet.rescale_ts(stream.time_base(), out_time_base);
            packet.set_position(-1);
            packet.set_stream(out_index);
            packet
                .write_interleaved(&mut output_format_context)
                .map_err(ff_err)?;
            done += 1;
            emit(done, false, &mut throttle);
        }
    }
    let attached = write_cover(&mut output_format_context, cover_input, &control)?;
    output_format_context.write_trailer().map_err(ff_err)?;
    control.emit_final_blocking(progress::update(
        Stage::Merge,
        "",
        done,
        Some(total_packets),
    ));
    Ok(attached)
}

fn prepare_cover(
    output: &mut ffmpeg_next::format::context::Output,
    path: &str,
) -> std::result::Result<
    Option<(ffmpeg_next::format::context::Input, usize, usize)>,
    ffmpeg_next::Error,
> {
    let input = ffmpeg_next::format::input(path)?;
    let time_base;
    let in_index;
    let out_index;
    {
        let Some(stream) = input
            .streams()
            .best(ffmpeg_next::media::Type::Video)
            .or_else(|| input.streams().next())
        else {
            return Ok(None);
        };
        time_base = stream.time_base();
        in_index = stream.index();
        let mut out_stream = output.add_stream(None::<ffmpeg_next::Codec>)?;
        out_stream.set_parameters(stream.parameters());
        unsafe {
            (*out_stream.parameters().as_mut_ptr()).codec_tag = 0;
            (*out_stream.as_mut_ptr()).disposition = ffmpeg_next::ffi::AV_DISPOSITION_ATTACHED_PIC;
        }
        out_stream.set_time_base(time_base);
        out_index = out_stream.index();
    }
    Ok(Some((input, in_index, out_index)))
}

fn write_cover(
    output: &mut ffmpeg_next::format::context::Output,
    cover: Option<(ffmpeg_next::format::context::Input, usize, usize)>,
    control: &Control,
) -> Result<bool> {
    let Some((mut input, in_index, out_index)) = cover else {
        return Ok(false);
    };
    if control.is_cancelled() {
        return Err(Error::Cancelled);
    }
    let mut attached = false;
    for (stream, mut packet) in input.packets() {
        if stream.index() != in_index {
            continue;
        }
        let out_time_base = output
            .stream(out_index)
            .ok_or_else(|| Error::StateError(format!("封面流 {out_index} 不存在")))?
            .time_base();
        packet.rescale_ts(stream.time_base(), out_time_base);
        packet.set_position(-1);
        packet.set_stream(out_index);
        if packet.write_interleaved(output).is_err() {
            tracing::warn!("写入封面失败，视频仍会完成");
            return Ok(false);
        }
        attached = true;
    }
    Ok(attached)
}

fn ff_err(err: ffmpeg_next::Error) -> Error {
    Error::StateError(err.to_string())
}
