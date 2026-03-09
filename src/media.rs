extern crate ffmpeg_next as ffmpeg;

use ffmpeg::{codec, encoder};

use ffmpeg_next::{Dictionary, format, frame};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::recorder::RawFrame;

use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};

// Create encoder
pub struct VideoEncoder {
    encoder: encoder::Video,
    scaler: ScalingContext,
    frame_count: usize,
}

// SAFETY: VideoEncoder is only ever used from a single spawned task.
// The underlying *mut SwsContext (inside ScalingContext) is not shared
// across threads, so asserting Send here is sound.
unsafe impl Send for VideoEncoder {}

pub fn parse_format(format: u32) -> format::Pixel {
    use format::Pixel;
    // SPA format IDs -> ffmpeg pixel formats.
    // On little-endian x86, AV_PIX_FMT_BGR32 = AV_PIX_FMT_RGBA (wrong byte order!).
    // BGRx bytes in memory are [B, G, R, x] which is AV_PIX_FMT_BGR0.
    // RGBx bytes in memory are [R, G, B, x] which is AV_PIX_FMT_RGB0.
    match format {
        2 => Pixel::YUV420P, // SPA_VIDEO_FORMAT_I420
        4 => Pixel::YUYV422, // SPA_VIDEO_FORMAT_YUY2
        7 => Pixel::RGBZ,    // SPA_VIDEO_FORMAT_RGBx  [R, G, B, x] = AV_PIX_FMT_RGB0
        8 => Pixel::BGRZ, // SPA_VIDEO_FORMAT_BGRx  [B, G, R, x] = AV_PIX_FMT_BGR0 (Wayland native)
        11 => Pixel::RGBA, // SPA_VIDEO_FORMAT_RGBA
        15 => Pixel::RGB24, // SPA_VIDEO_FORMAT_RGB
        _ => unreachable!(),
    }
}

impl VideoEncoder {
    pub async fn new(width: u32, height: u32, pixel_format: u32) -> Result<Self, ffmpeg::Error> {
        // Find the codec first, then allocate the context *with* that codec so
        // libx264's private data (CRF, VBV, etc.) is properly initialized.
        let codec = encoder::find(codec::Id::H264).ok_or(ffmpeg::Error::EncoderNotFound)?;

        let context = codec::context::Context::new_with_codec(codec);
        let mut video = context.encoder().video()?;

        video.set_width(width);
        video.set_height(height);
        video.set_format(format::Pixel::YUV420P);
        video.set_bit_rate(2_000_000); // 2 Mbps ABR — satisfies x264 "no broken defaults"
        video.set_max_b_frames(0); // NO B-frames
        video.set_gop(60); // keyframe every 1s @60fps
        video.set_time_base((1, 60));

        let codec = encoder::find(codec::Id::H264)
            .ok_or(ffmpeg::Error::EncoderNotFound)?
            .video()?;

        let mut opts = Dictionary::new();
        opts.set("preset", "ultrafast");
        opts.set("tune", "zerolatency");
        let video: encoder::Video = video.open_as_with(codec, opts)?;

        println!("a");

        eprintln!(
            "[encoder] creating scaler: {:?} {}x{} -> YUV420P",
            parse_format(pixel_format),
            width,
            height
        );
        let scaling_context = ScalingContext::get(
            parse_format(pixel_format),
            width,
            height,
            format::Pixel::YUV420P,
            width,
            height,
            Flags::FAST_BILINEAR,
        );
        eprintln!("[encoder] scaler result: {:?}", scaling_context.is_ok());
        let scaling_context = scaling_context?;

        let video_encoder = VideoEncoder {
            encoder: video,
            scaler: scaling_context,
            frame_count: 0,
        };

        Ok(video_encoder)
    }

    pub async fn encode_stream(
        &mut self,
        mut rx: Receiver<RawFrame>,
        encoder_sender: Sender<Vec<u8>>,
    ) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        while let Some(raw) = rx.recv().await {
            eprintln!(
                "[enc] got frame {}x{} fmt={}",
                raw.width, raw.height, raw.format
            );

            let mut src = frame::Video::new(parse_format(raw.format), raw.width, raw.height);
            eprintln!(
                "[enc] src frame created, planes={} stride={}",
                src.planes(),
                src.stride(0)
            );

            let src_len = src.stride(0) * raw.height as usize;
            eprintln!(
                "[enc] src buf len={} raw.data len={}",
                src_len,
                raw.data.len()
            );
            src.data_mut(0)[..raw.data.len()].copy_from_slice(&raw.data);
            eprintln!("[enc] copy done");
            src.set_pts(Some(self.frame_count as i64));

            let mut dst = frame::Video::new(format::Pixel::YUV420P, raw.width, raw.height);
            eprintln!("[enc] dst frame created");

            if let Err(e) = self.scaler.run(&src, &mut dst) {
                eprintln!("[enc] scaler failed: {:?} (code {})", e, i32::from(e));
                eprintln!(
                    "[enc] src: fmt={:?} {}x{} stride={} planes={}",
                    parse_format(raw.format),
                    raw.width,
                    raw.height,
                    src.stride(0),
                    src.planes(),
                );
                eprintln!(
                    "[enc] raw.data.len()={} expected stride*h={}",
                    raw.data.len(),
                    src.stride(0) * raw.height as usize,
                );
                continue;
            }
            eprintln!("[enc] scaler done");

            self.encoder.send_frame(&dst)?;
            eprintln!("[enc] send_frame done");

            let mut packet = ffmpeg::Packet::empty();
            while self.encoder.receive_packet(&mut packet).is_ok() {
                packet.set_stream(0);

                // Copy into owned bytes so the borrow on `packet` ends
                // before the next receive_packet call mutably borrows it again.
                if let Some(data) = packet.data() {
                    let _ = encoder_sender.send(data.to_vec()).await;
                }
            }

            self.frame_count += 1;
        }

        self.encoder.send_eof()?;
        let mut packet = ffmpeg::Packet::empty();
        while self.encoder.receive_packet(&mut packet).is_ok() {
            // self.send_packet(packet.data())?;
        }

        Ok(())
    }
}
