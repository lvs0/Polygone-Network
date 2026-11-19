//! Polygone Gateway binary.
//!
//! Gateway is intentionally loopback-only until TLS and mutual device
//! authentication are implemented. The packaged static site is resolved
//! relative to the installed executable, not the caller's working directory.

use anyhow::{Result, bail};
use polygone_gateway::Gateway;
use std::path::{Path, PathBuf};

fn default_site_dir(executable: &Path, working_dir: &Path) -> PathBuf {
    let candidates = [
        executable
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("../share/polygone/web"),
        working_dir.join("web"),
        working_dir.join("site"),
    ];
    candidates
        .into_iter()
        .find(|candidate| candidate.join("index.html").is_file())
        .unwrap_or_else(|| {
            executable
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("../share/polygone/web")
        })
}

fn resolve_site_dir(explicit: Option<PathBuf>, executable: &Path, working_dir: &Path) -> PathBuf {
    explicit.unwrap_or_else(|| default_site_dir(executable, working_dir))
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args()
        .skip(1)
        .any(|arg| arg == "--version" || arg == "-V")
    {
        println!("polygone-gateway {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let addr = std::env::var("POLYGONE_GATEWAY_ADDR").unwrap_or_else(|_| "127.0.0.1:8088".into());
    let executable = std::env::current_exe()?;
    let working_dir = std::env::current_dir()?;
    let explicit_site = std::env::var_os("POLYGONE_SITE_DIR").map(PathBuf::from);
    let site_dir = resolve_site_dir(explicit_site, &executable, &working_dir);

    if !site_dir.join("index.html").is_file() {
        bail!(
            "site introuvable : {} (attendu un index.html ; définir POLYGONE_SITE_DIR ou réinstaller les ressources web)",
            site_dir.display()
        );
    }

    Gateway::run(&addr, site_dir).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "polygone-gateway-main-{label}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn installed_site_is_found_relative_to_executable_even_from_another_cwd() {
        let root = temp_dir("installed");
        let bin = root.join("bin");
        let web = root.join("share/polygone/web");
        let cwd = root.join("caller");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&web).unwrap();
        std::fs::create_dir_all(&cwd).unwrap();
        std::fs::create_dir_all(cwd.join("web")).unwrap();
        std::fs::write(web.join("index.html"), b"installed").unwrap();
        std::fs::write(cwd.join("web/index.html"), b"working directory").unwrap();

        let selected = default_site_dir(&bin.join("polygone-gateway"), &cwd);
        assert_eq!(selected, bin.join("../share/polygone/web"));
        assert_eq!(
            resolve_site_dir(None, &bin.join("polygone-gateway"), &cwd),
            selected
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn repository_working_directory_site_is_a_fallback_when_not_installed() {
        let root = temp_dir("repository");
        let cwd = root.join("repo");
        std::fs::create_dir_all(cwd.join("web")).unwrap();
        std::fs::write(cwd.join("web/index.html"), b"repo").unwrap();
        let executable = root.join("target/debug/polygone-gateway");
        assert_eq!(default_site_dir(&executable, &cwd), cwd.join("web"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_site_directory_overrides_all_defaults() {
        let root = temp_dir("explicit");
        let explicit = root.join("custom-site");
        std::fs::create_dir_all(&explicit).unwrap();
        std::fs::write(explicit.join("index.html"), b"custom").unwrap();
        let resolved = resolve_site_dir(
            Some(explicit.clone()),
            &root.join("bin/polygone-gateway"),
            &root.join("caller"),
        );
        assert_eq!(resolved, explicit);
        std::fs::remove_dir_all(root).unwrap();
    }
}
