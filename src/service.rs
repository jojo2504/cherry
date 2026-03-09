use async_trait::async_trait;

#[async_trait]
pub trait Client {
    async fn start_recording(&self) -> anyhow::Result<()>;
    async fn draw_client(&self) -> anyhow::Result<()>;
}

#[async_trait]
pub trait Server {
    async fn discover_peers(&self) -> anyhow::Result<()>;
    async fn start_streaming(&self) -> anyhow::Result<()>;
}
