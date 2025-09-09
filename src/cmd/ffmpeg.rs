#[cfg(not(feature = "rsmpeg"))]
pub(crate) mod ffmpeg_api {
    use std::process::{exit, Stdio};
    use crate::cmd::out::{error, info};

    pub(crate) fn ffmpeg_run_version()  {
        let mut cmd = std::process::Command::new("ffmpeg");
        cmd.stderr(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.arg("-version");
        match cmd.status() {
            Ok(_) => {}
            Err(_) => {
                error("未找到ffmpeg, 请先安装ffmpeg.");
                exit(1);
            },
        }
    }

    pub(crate) fn ffmpeg_merge_files(list: Vec<&str>, output: &str) -> Result<(), Box<dyn std::error::Error>> {
        info("正在合并文件");
        let mut cmd = std::process::Command::new("ffmpeg");
        cmd.stderr(Stdio::null());
        cmd.stdout(Stdio::null());
        for x in list {
            cmd.arg("-i");
            cmd.arg(x);
        }
        cmd.arg("-vcodec");
        cmd.arg("copy");
        cmd.arg("-acodec");
        cmd.arg("copy");
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
    use rsmpeg::{
        self,
        avcodec::{AVCodec, AVCodecContext},
        avformat::{AVFormatContextInput, AVFormatContextOutput},
    };
    use std::collections::HashMap;
    use std::ffi::CString;
    use std::os::raw::c_int;

    pub fn ffmpeg_merge_files(
        list: Vec<&str>,
        output: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut total_packets = 0;
        for input in &list {
            let input = CString::new(*input)?;
            let mut input_format_context = AVFormatContextInput::open(&input)?;
            loop {
                match input_format_context.read_packet()? {
                    Some(_) => total_packets += 1,
                    None => break,
                }
            }
        }

        let deps = total_packets;
        let pb = ProgressBar::new(deps);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
                .progress_chars("##-"),
        );
        pb.set_message("合并".to_owned());

        let output = CString::new(output)?;
        let mut output_format_context = AVFormatContextOutput::create(&output)?;
        let mut inputs = vec![];
        for input in list {
            let input = CString::new(input).unwrap();
            let input_format_context = AVFormatContextInput::open(&input)?;
            let mut stream_index_map = HashMap::new();
            for av_stream_ref in input_format_context.streams() {
                let stream_codecpar = av_stream_ref.codecpar();
                let codec_id = stream_codecpar.codec_id;
                let decoder = AVCodec::find_decoder(codec_id).ok_or(
                    bbdd::BBDDError::StateError(format!("Unsupported codec_id: {:?}", codec_id)),
                )?;
                let mut decode_context = AVCodecContext::new(&decoder);
                decode_context.apply_codecpar(&stream_codecpar)?;
                decode_context.set_time_base(av_stream_ref.time_base);
                if let Some(framerate) = av_stream_ref.guess_framerate() {
                    decode_context.set_framerate(framerate);
                }
                let mut out_stream = output_format_context.new_stream();
                out_stream.set_codecpar(decode_context.extract_codecpar());
                out_stream.set_time_base(decode_context.time_base);
                stream_index_map.insert(av_stream_ref.index as i32, out_stream.index as i32);
            }
            inputs.push((input_format_context, stream_index_map));
        }
        let mut dict = None;
        output_format_context.write_header(&mut dict)?;
        for (mut input_format_context, stream_index_map) in inputs {
            loop {
                let mut packet = match input_format_context.read_packet()? {
                    Some(x) => x,
                    None => break,
                };
                pb.inc(1);
                packet.set_stream_index(
                    stream_index_map
                        .get(&(packet.stream_index as i32))
                        .unwrap()
                        .clone() as c_int,
                );
                output_format_context
                    .interleaved_write_frame(&mut packet)
                    .unwrap();
            }
        }
        output_format_context.write_trailer()?;
        pb.finish_with_message("合并完成".to_owned());
        Ok(())
    }
}
