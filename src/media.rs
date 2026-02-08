extern crate ffmpeg_next as ffmpeg;

use ffmpeg::{codec, encoder};

use ffmpeg_next::{format, frame};
use tokio::sync::mpsc::Receiver;

use crate::recorder::RawFrame;

use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};

// Create encoder
pub struct VideoEncoder {
    encoder: encoder::Video,
    scaler: ScalingContext,
    frame_count: usize,
}

pub fn parse_format(format: u32) -> format::Pixel {
    use format::Pixel;
    match format {
        2  => Pixel::YUV420P, // I420
        4  => Pixel::YUYV422, // YUY2
        7  => Pixel::RGB32,   // RGBx
        8  => Pixel::BGR32,   // BGRx (Wayland native)
        11 => Pixel::RGBA,    // RGBA
        15 => Pixel::RGB24,   // RGB
        _  => unreachable!(),
    }
}

impl VideoEncoder {
    pub fn new(width: u32, height: u32, pixel_format: u32) -> Result<Self, ffmpeg::Error> {          
        // build the ffmpeg video encoder context
        let context = codec::context::Context::new();
        let mut video = context.encoder().video()?;

        video.set_width(width);
        video.set_height(height);
        video.set_format(format::Pixel::YUV420P);
        video.set_max_b_frames(0);      // NO B-frames
        video.set_gop(60);              // keyframe every 1s @60fps
        video.set_time_base((1,60));

        let codec = encoder::find(codec::Id::H264)
            .ok_or(ffmpeg::Error::EncoderNotFound)?
            .video()?;

        let video: encoder::Video = video.open_as(codec)?;
        
        let scaling_context = ScalingContext::get(
            parse_format(pixel_format),
            width,
            height,
            format::Pixel::YUV420P,
            width,
            height,
            Flags::FAST_BILINEAR,
        )?;

        let video_encoder = VideoEncoder {
            encoder: video,
            scaler: scaling_context,
            frame_count: 0,
        };

        Ok(video_encoder)
    }

    pub async fn encode_stream(
        &mut self, 
        mut rx: Receiver<RawFrame>) 
    -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        while let Some(raw) = rx.recv().await {
            let mut src = frame::Video::new(
                parse_format(raw.format),
                raw.width,
                raw.height,
            );

            src.data_mut(0).copy_from_slice(&raw.data);
            src.set_pts(Some(self.frame_count as i64));

            let mut dst = frame::Video::new(
                format::Pixel::YUV420P,
                raw.width,
                raw.height,
            );

            self.scaler.run(&src, &mut dst)?;

            self.encoder.send_frame(&dst)?;

            let mut packet = ffmpeg::Packet::empty();
            while self.encoder.receive_packet(&mut packet).is_ok() {
                packet.set_stream(0);

                // 🚀 THIS is what you send over the network
                // self.send_packet(packet.data())?;
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
