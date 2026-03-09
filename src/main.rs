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
use anyhow::Ok;
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc::{self, Receiver, Sender};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (stream_sender, stream_receiver): (Sender<RawFrame>, Receiver<RawFrame>) =
        mpsc::channel::<RawFrame>(100);
    let (encoder_sender, encoder_receiver): (Sender<Vec<u8>>, Receiver<Vec<u8>>) =
        mpsc::channel::<Vec<u8>>(100);
    let app = Arc::new(App::new(stream_sender, encoder_receiver).await?);

    tokio::spawn({
        let app = Arc::clone(&app);
        async move { app.discover_peers().await }
    });

    tokio::spawn(async move {
        let mut video_encoder = VideoEncoder::new(2560, 1440, 8).await.unwrap();
        video_encoder
            .encode_stream(stream_receiver, encoder_sender)
            .await
    });

    std::thread::sleep(Duration::from_secs(2));

    // start recording and streaming to another instance
    app.start_recording().await?;
    app.start_streaming().await?;

    while let Some(raw) = app.encoder_receiver.lock().await.recv().await {
        println!("{:?}", raw);
    }

    println!("{:?}", app.discovered_peers);

    Ok(())
}
