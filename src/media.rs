extern crate ffmpeg_next as ffmpeg;

use tokio::sync::mpsc::Receiver;

use crate::recorder::RawFrame;

use ffmpeg::software::scaling::{context::Context as ScalingContext, flag::Flags};

// Create encoder
pub struct VideoEncoder {
    encoder: ffmpeg::encoder::Video,
    scaler: Option<ScalingContext>,
    frame_count: usize,
}

impl VideoEncoder {
    pub fn new(width: u32, height: u32) -> Result<Self, ffmpeg::Error> {                
        let codec = ffmpeg::encoder::find(ffmpeg::codec::Id::H264)
            .ok_or(ffmpeg::Error::EncoderNotFound)?
            .video()?;
        todo!()
    }

    pub async fn encode_stream(
        &self, 
        mut rx: Receiver<RawFrame>) 
    -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
        while let Some(data) = rx.recv().await {

        }
        Ok(())
    }
}
