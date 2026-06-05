//! polygone-server — stateless, zero-knowledge relay.
//!
//! Listens on $POLYGONE_BIND (default 127.0.0.1:4001) and forwards
//! encrypted fragments. No state survives a restart.

use std::net::SocketAddr;
use std::env;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind: SocketAddr = env::var("POLYGONE_BIND")
        .unwrap_or_else(|_| "127.0.0.1:4001".to_string())
        .parse()?;
    eprintln!("[polygone-server] starting on {bind}");
    polygone::server::serve(bind).await?;
    Ok(())
}
