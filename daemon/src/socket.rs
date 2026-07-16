//! Socket writer — pushes allocation decisions to the Polygone node.
//!
//! Polygone node reads from ~/.polygone/daemon.sock (Unix socket).
//! Each line is a JSON command. Simple, no protocol overhead.

use std::{fs, io::Write, path::PathBuf, sync::OnceLock};
use std::os::unix::net::UnixStream;
use anyhow::{Context, Result};
use serde::{Serialize, Serializer};

use crate::allocator::Allocation;

static SOCKET_PATH: OnceLock<PathBuf> = OnceLock::new();

fn socket_path() -> PathBuf {
    SOCKET_PATH
        .get_or_init(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".polygone")
        })
        .join("daemon.sock")
}

#[derive(Serialize)]
#[serde(tag = "cmd")]
pub enum DaemonMsg {
    #[serde(rename = "set_alloc")]
    SetAlloc {
        ram_mb: u64,
        bandwidth_mbps: u32,
        #[serde(serialize_with = "ts_secs")]
        timestamp: i64,
    },
    #[serde(rename = "shrink")]
    Shrink {
        reason: String,
        #[serde(serialize_with = "ts_secs")]
        timestamp: i64,
    },
    #[serde(rename = "grow")]
    Grow {
        headroom_mb: u64,
        #[serde(serialize_with = "ts_secs")]
        timestamp: i64,
    },
    #[serde(rename = "status")]
    Status {
        ram_mb: u64,
        bandwidth_mbps: u32,
        shrinking: bool,
        #[serde(serialize_with = "ts_secs")]
        timestamp: i64,
    },
}

fn ts_secs<S>(_: &i64, s: S) -> Result<S::Ok, S::Error>
where S: Serializer,
{
    s.serialize_i64(chrono_lite_timestamp())
}

fn chrono_lite_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Ensure the .polygone directory exists.
pub fn ensure_dir() -> Result<()> {
    let path = socket_path();
    let dir = path.parent().unwrap();
    fs::create_dir_all(dir)
        .context(format!("failed to create {}", dir.display()))?;
    Ok(())
}

/// Write a message to the daemon socket.
/// If socket doesn't exist yet, that's fine — Polygone node just won't receive updates.
pub fn send(msg: &DaemonMsg) -> Result<()> {
    let path = socket_path();
    if !path.exists() {
        // Socket not yet created by the Polygone node — skip silently
        return Ok(());
    }

    let json = serde_json::to_string(msg)
        .context("serialize daemon msg")?;
    let line = format!("{}\n", json);

    match UnixStream::connect(&path) {
        Ok(mut stream) => {
            stream.write_all(line.as_bytes())?;
            stream.flush()?;
        }
        Err(e) => {
            // Socket exists but node isn't listening — this is fine,
            // just means the Polygone node hasn't started yet
            log::debug!("daemon socket unreachable (node not running): {}", e);
        }
    }
    Ok(())
}

/// Notify the Polygone node of a new allocation.
pub fn notify_allocation(allocation: &Allocation, _shrinking: bool) -> Result<()> {
    let msg = DaemonMsg::SetAlloc {
        ram_mb: allocation.ram_mb(),
        bandwidth_mbps: allocation.bandwidth_mbps,
        timestamp: chrono_lite_timestamp(),
    };
    send(&msg)
}

pub fn notify_shrink(reason: &str) -> Result<()> {
    send(&DaemonMsg::Shrink {
        reason: reason.to_string(),
        timestamp: chrono_lite_timestamp(),
    })
}

pub fn notify_grow(headroom_mb: u64) -> Result<()> {
    send(&DaemonMsg::Grow {
        headroom_mb,
        timestamp: chrono_lite_timestamp(),
    })
}

pub fn notify_status(ram_mb: u64, bandwidth_mbps: u32, shrinking: bool) -> Result<()> {
    send(&DaemonMsg::Status {
        ram_mb,
        bandwidth_mbps,
        shrinking,
        timestamp: chrono_lite_timestamp(),
    })
}