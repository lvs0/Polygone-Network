//! Polygone Gateway binary.

use anyhow::Result;
use polygone_gateway::Gateway;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let site_dir = std::env::var_os("POLYGONE_SITE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("docs/site"));

    Gateway::run("127.0.0.1:8787", site_dir).await
}
