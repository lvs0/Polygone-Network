//! Embedded static assets for the Polygone web dashboard.
//!
//! Every file in `web/` is included at compile time. The `get()` function
//! performs a flat-name lookup. No glob, no build script — just text.
//!
//! To regenerate, drop files in `web/` and add an `include_str!` line below.

/// Return the asset bytes for a flat filename, or None if unknown.
pub fn get(name: &str) -> Option<&'static [u8]> {
    Some(match name {
        "index.html"      => INDEX_HTML,
        "style.css"       => STYLE_CSS,
        "app.js"          => APP_JS,
        "node.html"       => NODE_HTML,
        "dashboard.css"   => DASHBOARD_CSS,
        "dashboard.js"    => DASHBOARD_JS,
        "drive.html"      => DRIVE_HTML,
        "drive.css"       => DRIVE_CSS,
        "drive.js"        => DRIVE_JS,
        "mesh.html"       => MESH_HTML,
        "mesh.css"        => MESH_CSS,
        "mesh.js"         => MESH_JS,
        "plan.html"       => PLAN_HTML,
        _ => return None,
    })
}

// ── Embed every file at compile time. ─────────────────────────────────────────

static INDEX_HTML:    &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/index.html"));
static STYLE_CSS:     &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/style.css"));
static APP_JS:        &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/app.js"));
static NODE_HTML:     &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/node.html"));
static DASHBOARD_CSS: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/dashboard.css"));
static DASHBOARD_JS:  &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/dashboard.js"));
static DRIVE_HTML:    &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/drive.html"));
static DRIVE_CSS:     &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/drive.css"));
static DRIVE_JS:      &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/drive.js"));
static MESH_HTML:     &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/mesh.html"));
static MESH_CSS:      &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/mesh.css"));
static MESH_JS: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/mesh.js"));
static PLAN_HTML: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/web/plan.html"));
