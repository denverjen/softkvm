mod clipboard;
mod edge;
mod inject;
mod network;

use anyhow::Result;
use softkvm_common::Config;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "softkvm.toml".to_string());
    let config = Config::load_or_default(&config_path);

    tracing::info!(
        "SoftKVM Client connecting to {}:{}",
        config.client.host,
        config.client.port
    );

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { network::run(&config).await })
}
