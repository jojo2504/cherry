pub mod recorder;
pub mod media;

use std::sync::Arc;

use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::{media::VideoEncoder, recorder::{RawFrame, ScreenRecorder}};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    let (stream_sender, stream_receiver): (Sender<RawFrame>, Receiver<RawFrame>) = mpsc::channel::<RawFrame>(100);
    let recorder = Arc::new(ScreenRecorder::new().await?);
    let media = Arc::new(VideoEncoder::new(2560, 1440).unwrap());
    
    let task_recorder = recorder.clone();
    let mut handles = vec![];
    handles.push(tokio::spawn(async move {
        task_recorder.start_recording(stream_sender).await
    }));

    let task_media = media.clone();
    handles.push(tokio::spawn(async move {
        task_media.encode_stream(stream_receiver).await
    }));    

    for handle in handles {
        handle.await??;
    }

    Ok(())
}
