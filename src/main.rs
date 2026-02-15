pub mod recorder;
pub mod media;
pub mod menu;
pub mod network;

use std::sync::Arc;

use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::{media::VideoEncoder, recorder::{RawFrame, ScreenRecorder}};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    // crate::menu::main().await;
    
    crate::network::main().await;
    
    let (stream_sender, stream_receiver): (Sender<RawFrame>, Receiver<RawFrame>) = mpsc::channel::<RawFrame>(100);
    // let recorder = Arc::new(ScreenRecorder::new().await?);
    // let media = Arc::new(VideoEncoder::new(2560, 1440, 8).unwrap());
    
    // let mut handles = vec![];

    // handles.push(tokio::spawn(async move {
    //     crate::network::main().await
    // }));
    
    // handles.push(tokio::spawn(async move {
    //     // crate::menu::main().await
    // }));

    // let task_recorder = recorder.clone();
    // handles.push(tokio::spawn(async move {
    //     task_recorder.start_recording(stream_sender).await
    // }));

    // let task_media = media.clone();
    // handles.push(tokio::spawn(async move {
    //     task_media.encode_stream(stream_receiver).await
    // }));    

    // for handle in handles {
    //     handle.await??;
    // }

    Ok(())
}
