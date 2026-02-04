use cherry::screen_recorder::recorder::ScreenRecorder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let recorder = ScreenRecorder::new().await?;
    recorder.start_recording().await?;
    Ok(())
}
