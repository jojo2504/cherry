pub mod app;
pub mod media;
pub mod menu;
pub mod network;
pub mod recorder;
pub mod service;

use crate::{
    app::App,
    media::VideoEncoder,
    recorder::RawFrame,
    service::{Client, Server},
};
use std::sync::Arc;
use tokio::sync::mpsc::{self, Receiver, Sender};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (stream_sender, stream_receiver): (Sender<RawFrame>, Receiver<RawFrame>) =
        mpsc::channel::<RawFrame>(100);
    let (encoder_sender, encoder_receiver): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
        mpsc::channel::<Vec<u8>>(100);
    let app = Arc::new(App::new(stream_sender, encoder_receiver).await?);

    // Background: mDNS peer discovery
    tokio::spawn({
        let app = Arc::clone(&app);
        async move {
            if let Err(e) = app.discover_peers().await {
                eprintln!("[discover_peers] error: {e}");
            }
        }
    });

    // Background: H264 encoder — reads RawFrames, writes encoded packets
    tokio::spawn(async move {
        let mut video_encoder = VideoEncoder::new(2560, 1440, 8).await.unwrap();
        if let Err(e) = video_encoder
            .encode_stream(stream_receiver, encoder_sender)
            .await
        {
            eprintln!("[encoder] error: {e:?}");
        }
    });

    // Background: screen capture (spawns its own OS thread for the PipeWire mainloop)
    tokio::spawn({
        let app = Arc::clone(&app);
        async move {
            if let Err(e) = app.start_recording().await {
                eprintln!("[start_recording] error: {e}");
            }
        }
    });

    // Background: streaming to peers
    tokio::spawn({
        let app = Arc::clone(&app);
        async move {
            if let Err(e) = app.start_streaming().await {
                eprintln!("[start_streaming] error: {e}");
            }
        }
    });

    // Keep main alive, printing encoded packets as they arrive
    let mut rx = app.encoder_receiver.lock().await;
    while let Some(packet) = rx.recv().await {
        println!("[main] encoded packet: {} bytes", packet.len());
    }

    Ok(())
}
