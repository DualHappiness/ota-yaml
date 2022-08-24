use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    ota_yaml::Ota::run().await.map_err(|e| {
        tracing::error!("ota yaml run failed: {}", e);
        e
    })
}
