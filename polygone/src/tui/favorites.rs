//! Favorites persistence — saved to ~/.config/polygone/favorites.json

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// Default favorites file location.
pub fn favorites_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"));
    path.push("polygone");
    path.push("favorites.json");
    path
}

/// Load favorites from disk.
pub fn load_favorites() -> HashSet<String> {
    let path = favorites_path();
    if !path.exists() {
        return HashSet::new();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

/// Save favorites to disk.
pub fn save_favorites(favorites: &HashSet<String>) {
    let path = favorites_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string(favorites) {
        let _ = fs::write(&path, content);
    }
}