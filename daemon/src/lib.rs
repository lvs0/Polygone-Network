//! polygoned — Resource allocation daemon for Polygone P2P
//!
//! "On voit rien. Et c'est comme ça que ça devrait être."
//!
//! Embeddable library + standalone binary.

pub mod allocator;
pub mod bandwidth;
pub mod cpu;
pub mod gpu;
pub mod socket;
pub mod system;
pub mod resources;
pub mod policy;

// Re-exports for embeddable use
//
// Each type lives in its own module — re-export with the canonical path so
// downstream consumers (binary, embedders, integration tests) can import
// everything from the crate root.
pub use system::{
    SystemSnapshot, CpuSnapshot, MemorySnapshot, BandwidthSnapshot, GpuSnapshot,
};
pub use allocator::Allocation;
pub use cpu::CpuAllocation;
pub use gpu::GpuAllocation;
pub use bandwidth::BandwidthAllocation;
pub use resources::{
    Platform, create_platform, PlatformCaps, CpuAffinityMode,
    // Also expose resources types that glow_up.rs depends on
    CpuTopology, CpuInfo, MemoryInfo, BandwidthInfo, NetInterface, GpuInfo,
    IpcEndpoint, IpcConnection, ServiceConfig,
};

pub use policy::glow_up::{
    GlowUpEngine, DaemonConfig, AllocationTier, ResourceLimits, SafetyMargins, BehaviorConfig,
};