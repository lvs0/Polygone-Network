//! GPU detection and allocation.
//!
//! ## What this does
//!
//! - **detect**: runs `nvidia-smi --query-gpu=memory.total,memory.used --format=csv,noheader,nounits`
//!   to get total and used VRAM in MiB.
//! - **fallback**: if `nvidia-smi` is not available or returns an error, reports no GPU.
//! - **allocation**: returns a `GpuAllocation` with total, used, free memory and an allocated
//!   portion (in MiB) based on a ratio (default 0.5) of the free memory.
//!
//! ## What this doesn't do
//!
//! - Does not actually reserve or lock VRAM (that would require CUDA/Vulkan context and root).
//!   This module only reports and suggests an allocation; the consumer (e.g., a polygone-client)
//!   would need to actually use the GPU within the allocated limits.
//! - Does not handle multiple GPUs (uses the first one).
//!
//! ## Safety
//!
//! This module does not perform any unsafe operations. It only runs an external command and
//! parses its output.

use std::process::Command;

/// GPU allocation state.
#[derive(Debug, Clone)]
pub struct GpuAllocation {
    /// Total VRAM in MiB.
    pub total_mb: u32,
    /// Used VRAM in MiB.
    pub used_mb: u32,
    /// Free VRAM in MiB.
    pub free_mb: u32,
    /// Allocated VRAM in MiB (our share).
    pub allocated_mb: u32,
}

impl GpuAllocation {
    /// Create an allocation indicating no GPU detected.
    pub fn none() -> Self {
        Self {
            total_mb: 0,
            used_mb: 0,
            free_mb: 0,
            allocated_mb: 0,
        }
    }

    /// True if a GPU was detected and we have some memory.
    pub fn is_available(&self) -> bool {
        self.total_mb > 0
    }

    /// Returns the suggested allocation ratio (allocated_mb / free_mb) if free_mb > 0.
    pub fn allocation_ratio(&self) -> f32 {
        if self.free_mb > 0 {
            self.allocated_mb as f32 / self.free_mb as f32
        } else {
            0.0
        }
    }
}

/// Try to detect GPU and allocate a fraction of free memory.
///
/// # Arguments
///
/// * `ratio` - Fraction of free memory to allocate (0.0-1.0). If None, uses 0.5.
///
/// # Returns
///
/// A `GpuAllocation` struct. If no GPU is detected, all fields are zero.
pub fn allocate(ratio: Option<f32>) -> GpuAllocation {
    let ratio = ratio.unwrap_or(0.5).clamp(0.0, 1.0);
    let output = if let Ok(out) = Command::new("nvidia-smi")
        .args(&[
            "--query-gpu=memory.total,memory.used",
            "--format=csv,noheader,nounits",
        ])
        .output()
    {
        out
    } else {
        return GpuAllocation::none();
    };

    if !output.status.success() {
        return GpuAllocation::none();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let first = lines.next().unwrap_or("");
    let parts: Vec<&str> = first.split(',').map(|s| s.trim()).collect();
    if parts.len() != 2 {
        return GpuAllocation::none();
    }

    let total_mb: u32 = match parts[0].parse() {
        Ok(v) => v,
        Err(_) => return GpuAllocation::none(),
    };
    let used_mb: u32 = match parts[1].parse() {
        Ok(v) => v,
        Err(_) => return GpuAllocation::none(),
    };

    if total_mb == 0 {
        return GpuAllocation::none();
    }

    let free_mb = total_mb.saturating_sub(used_mb);
    let allocated_mb = ((free_mb as f32) * ratio).round() as u32;

    GpuAllocation {
        total_mb,
        used_mb,
        free_mb,
        allocated_mb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_returns_something_when_nvidia_smi_available() {
        // This test will pass if nvidia-smi is present and returns valid output.
        // On CI without a GPU, it will return none.
        let alloc = allocate(None);
        // We just assert that the struct is well-formed; actual values depend on hardware.
        assert!(alloc.allocated_mb <= alloc.free_mb);
        assert!(alloc.used_mb + alloc.free_mb <= alloc.total_mb);
    }

    #[test]
    fn allocate_none_when_ratio_out_of_bounds() {
        // Clamp should bring it back into range.
        let alloc = allocate(Some(2.0));
        assert!(alloc.allocated_mb <= alloc.free_mb);
    }

    #[test]
    fn none_is_not_available() {
        let alloc = GpuAllocation::none();
        assert!(!alloc.is_available());
    }
}