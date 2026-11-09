//! polygoned — Resource allocation daemon for Polygone P2P
//!
//! "On voit rien. Et c'est comme ça que ça devrait être."
//!
//! Embeddable library + standalone binary.

pub mod allocator;
// `cli.rs` n'était atteignable depuis aucune cible : ses tests ne tournaient
// jamais. Le module est déclaré ici pour que le chemin testé et le chemin
// compilé soient le même.
pub mod bandwidth;
pub mod cli;
pub mod client;
pub mod cpu;
pub mod gpu;
pub mod policy;
pub mod resources;
pub mod runtime;
pub mod socket;
pub mod system;

// Re-exports for embeddable use
//
// Each type lives in its own module — re-export with the canonical path so
// downstream consumers (binary, embedders, integration tests) can import
// everything from the crate root.
pub use allocator::Allocation;
pub use bandwidth::BandwidthAllocation;
pub use client::DaemonClient;
pub use cpu::CpuAllocation;
pub use gpu::GpuAllocation;
pub use resources::{
    create_platform,
    BandwidthInfo,
    CpuInfo,
    // Also expose resources types that glow_up.rs depends on
    CpuTopology,
    GpuInfo,
    IpcConnection,
    IpcEndpoint,
    MemoryInfo,
    NetInterface,
    Platform,
    PlatformCaps,
    ServiceConfig,
    UserActivity,
};
pub use system::{BandwidthSnapshot, CpuSnapshot, GpuSnapshot, MemorySnapshot, SystemSnapshot};

pub use policy::glow_up::{
    AllocationTier, BehaviorConfig, DaemonConfig, GlowUpEngine, ResourceLimits, SafetyMargins,
};
