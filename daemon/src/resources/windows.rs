//! Windows resource discovery — placeholder.
//!
//! The daemon currently ships Linux and macOS implementations (SPEC §1 lists
//! Windows as a target platform; the `#[cfg(target_os = "windows")]` branch
//! exists so the cross-platform split compiles on all three).
//!
//! Honest status: NOT implemented yet. A Windows build of `polygoned` will
//! link, but resource discovery returns empty defaults. Tracked as a
//! cross-platform gap, not silently claimed.
