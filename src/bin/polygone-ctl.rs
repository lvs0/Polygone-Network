//! polygone-ctl — minimal CLI client over the IPC Unix socket.
//!
//! Usage:
//!   polygone-ctl status
//!   polygone-ctl list
//!   polygone-ctl start <service>
//!   polygone-ctl stop  <service>

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use polygone::ipc::{self, Op, Request};

#[derive(Parser, Debug)]
#[command(name = "polygone-ctl", about = "Talk to the local Polygone Computer daemon")]
struct Cli {
    /// Path to the Computer socket.
    #[arg(long, env = "POLYGONE_SOCKET", default_value = "/tmp/polygone-computer.sock")]
    socket: PathBuf,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    Status,
    List,
    Start { service: String },
    Stop { service: String },
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let (op, service) = match &cli.cmd {
        Cmd::Status => (Op::Status, None),
        Cmd::List => (Op::List, None),
        Cmd::Start { service } => (Op::Start, Some(service.clone())),
        Cmd::Stop { service } => (Op::Stop, Some(service.clone())),
    };
    let req = Request { id: "ctl".into(), op, service };
    match ipc::call(&cli.socket, &req).await {
        Ok(resp) => {
            if resp.ok {
                if let Some(data) = resp.data {
                    println!("{}", serde_json::to_string_pretty(&data).unwrap_or_default());
                } else {
                    println!("ok");
                }
                ExitCode::SUCCESS
            } else {
                eprintln!("error: {}", resp.error.unwrap_or_default());
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("ipc: {e}");
            ExitCode::from(2)
        }
    }
}
