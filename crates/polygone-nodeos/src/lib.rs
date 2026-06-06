//! `polygone-nodeos` — Minimal bootable OS image for running a node.
//!
//! Transforme n'importe quel PC en nœud Polygone pur, sans OS hôte.
//! Utilise un mini-linux (Alpine + busybox) qui boote directement
//! dans le daemon Polygone. Le tout tient en <64MB.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

/// Architecture de construction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetArch {
    X8664,
    Aarch64,
    Armv7,
}

impl TargetArch {
    pub fn label(&self) -> &str {
        match self {
            TargetArch::X8664 => "x86_64",
            TargetArch::Aarch64 => "aarch64",
            TargetArch::Armv7 => "armv7",
        }
    }
}

/// Image bootable NodeOS.
#[derive(Clone, Debug)]
pub struct NodeOsImage {
    pub arch: TargetArch,
    pub kernel_mb: u32,
    pub initrd_mb: u32,
    pub polygone_binary: Vec<u8>,
    pub config_toml: String,
}

impl NodeOsImage {
    pub fn estimated_size_mb(&self) -> u32 {
        self.kernel_mb + self.initrd_mb + (self.polygone_binary.len() as u32) / (1024 * 1024)
    }
}

/// Builder pour l'image NodeOS.
pub struct NodeOsBuilder;

impl NodeOsBuilder {
    /// Build script stub — in production, this would use `linuxkit` or
    /// `buildroot` to produce a bootable ISO/USB image.
    #[allow(unused_variables)]
    pub fn build(arch: TargetArch, binary_path: &str) -> String {
        format!("🎯 NodeOS pour {} — binaire à {} — build via make nodeos-{}",
            arch.label(), binary_path, arch.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arch_labels() {
        assert_eq!(TargetArch::X8664.label(), "x86_64");
        assert_eq!(TargetArch::Aarch64.label(), "aarch64");
    }

    #[test]
    fn image_size_estimate() {
        let img = NodeOsImage {
            arch: TargetArch::X8664,
            kernel_mb: 8,
            initrd_mb: 4,
            polygone_binary: vec![0u8; 10 * 1024 * 1024], // 10MB binary
            config_toml: "[polygone]\nenabled = true".into(),
        };
        assert!(img.estimated_size_mb() >= 22);
    }

    #[test]
    fn builder_returns_string() {
        let msg = NodeOsBuilder::build(TargetArch::Aarch64, "/tmp/polygone");
        assert!(msg.contains("NodeOS"));
        assert!(msg.contains("aarch64"));
    }
}