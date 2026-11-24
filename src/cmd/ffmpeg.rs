#[cfg(not(feature = "rsmpeg"))]
pub(crate) mod ffmpeg_api {
    use crate::cmd::out::{error, info};
    use std::process::{Stdio, exit};

    pub(crate) fn ffmpeg_run_version() {
        let mut cmd = std::process::Command::new("ffmpeg");
        cmd.stderr(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.arg("-version");
        match cmd.status() {
            Ok(_) => {}
            Err(_) => {
                error("未找到ffmpeg, 请先安装ffmpeg.");
                exit(1);
            }
        }
    }

    pub(crate) fn ffmpeg_merge_files(
        list: Vec<&str>,
        output: &str,
        cover: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info("正在合并文件");
        let mut cmd = std::process::Command::new("ffmpeg");
        cmd.stderr(Stdio::null());
        cmd.stdout(Stdio::null());
        for x in &list {
            cmd.arg("-i");
            cmd.arg(x);
        }
        if let Some(cover) = cover {
            cmd.arg("-i");
            cmd.arg(cover);
            for index in 0..list.len() {
                cmd.arg("-map");
                cmd.arg(index.to_string());
            }
            cmd.arg("-map");
            cmd.arg(list.len().to_string());
            cmd.arg("-disposition:v:1");
            cmd.arg("attached_pic");
        }
        cmd.arg("-vcodec");
        cmd.arg("copy");
        cmd.arg("-acodec");
        cmd.arg("copy");
        cmd.arg("-y");
        cmd.arg(output);
        let status = cmd.status().unwrap();
        if status.code().unwrap() == 0 {
            Ok(())
        } else {
            Err(Box::new(bbdd::BBDDError::StateError(format!(
                "FFMPEG 未能成功运行 : EXIT CODE : {}",
                status.code().unwrap()
            ))))
        }
    }
}

#[cfg(feature = "rsmpeg")]
pub(crate) mod ffmpeg_api {
    use indicatif::ProgressBar;
    use std::collections::HashMap;

    pub fn ffmpeg_merge_files(
        list: Vec<&str>,
        output: &str,
        _cover: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        ffmpeg_next::init()?;
        ffmpeg_next::log::set_level(ffmpeg_next::log::Level::Error);

        let mut total_packets = 0u64;
        for input in &list {
            let mut input_format_context = ffmpeg_next::format::input(input)?;
            for _ in input_format_context.packets() {
                total_packets += 1;
            }
        }

        let pb = ProgressBar::new(total_packets);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars("##-"),
        );
        pb.set_message("合并".to_owned());

        let copy_codec = ffmpeg_next::encoder::find(ffmpeg_next::codec::Id::None)
            .ok_or_else(|| bbdd::BBDDError::StateError("无法创建流复制编码器".to_owned()))?;
        let mut output_format_context = ffmpeg_next::format::output(output)?;
        let mut inputs = Vec::new();
        for input in &list {
            let input_format_context = ffmpeg_next::format::input(input)?;
            let mut stream_index_map = HashMap::new();
            for av_stream_ref in input_format_context.streams() {
                let time_base = av_stream_ref.time_base();
                let mut out_stream = output_format_context.add_stream(copy_codec)?;
                out_stream.set_parameters(av_stream_ref.parameters());
                // 换容器时沿用源 codec tag 会让封装器拒绝这个流。
                unsafe {
                    (*out_stream.parameters().as_mut_ptr()).codec_tag = 0;
                }
                out_stream.set_time_base(time_base);
                stream_index_map.insert(av_stream_ref.index(), out_stream.index());
            }
            inputs.push((input_format_context, stream_index_map));
        }
        output_format_context.write_header()?;
        for (mut input_format_context, stream_index_map) in inputs {
            for (stream, mut packet) in input_format_context.packets() {
                let Some(&out_index) = stream_index_map.get(&stream.index()) else {
                    continue;
                };
                let out_time_base = output_format_context
                    .stream(out_index)
                    .ok_or_else(|| {
                        bbdd::BBDDError::StateError(format!("输出流 {out_index} 不存在"))
                    })?
                    .time_base();
                packet.rescale_ts(stream.time_base(), out_time_base);
                packet.set_position(-1);
                packet.set_stream(out_index);
                packet.write_interleaved(&mut output_format_context)?;
                pb.inc(1);
            }
        }
        output_format_context.write_trailer()?;
        pb.finish_with_message("合并完成".to_owned());
        Ok(())
    }
}
