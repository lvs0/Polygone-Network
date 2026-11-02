//! Vérification des limites OS d'un consommateur de blocs sur Linux/cgroup v2.
//!
//! Ce module ne crée ni ne modifie lui-même un cgroup. Le CLI peut demander à
//! systemd de lancer un processus dans un scope transitoire dédié ; ce module
//! vérifie ensuite que le processus est bien dans CE scope et que le noyau expose
//! les plafonds attendus. Un bail sans cette preuve reste un engagement logiciel.

#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};

const CGROUP_ROOT: &str = "/sys/fs/cgroup";
const MEMORY_FILE: &str = "memory.max";
const CPU_FILE: &str = "cpu.max";

/// Convertit une demande RAM en octets selon la même convention Gio que le daemon.
pub fn requested_memory_bytes(ram_gb: f32) -> Result<u64, String> {
    if !ram_gb.is_finite() || ram_gb <= 0.0 {
        return Err("la RAM demandée doit être finie et strictement positive".into());
    }
    let bytes = (f64::from(ram_gb) * 1_073_741_824.0) as u64;
    if bytes == 0 {
        return Err("la RAM demandée est inférieure à un octet après conversion".into());
    }
    if bytes == u64::MAX {
        return Err("la RAM demandée dépasse la capacité représentable en octets".into());
    }
    Ok(bytes)
}

/// Extrait le chemin unifié cgroup v2 depuis `/proc/<pid>/cgroup`.
#[cfg(any(target_os = "linux", test))]
fn unified_path(contents: &str) -> Result<&str, String> {
    contents
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .filter(|path| path.starts_with('/'))
        .ok_or_else(|| "aucune entrée cgroup v2 unifiée dans /proc/self/cgroup".to_string())
}

/// Accepte uniquement un scope Polygone créé sous le gestionnaire systemd
/// utilisateur courant, dans `app.slice`, jamais dans `session-*.scope`.
#[cfg(any(target_os = "linux", test))]
fn is_polygone_app_scope(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if parts.len() != 5 || parts[0] != "user.slice" || parts[3] != "app.slice" {
        return false;
    }
    let user_slice = parts[1]
        .strip_prefix("user-")
        .and_then(|part| part.strip_suffix(".slice"));
    let user_service = parts[2]
        .strip_prefix("user@")
        .and_then(|part| part.strip_suffix(".service"));
    if user_slice.is_none() || user_slice != user_service {
        return false;
    }
    parts[4].starts_with("polygone-node-") && parts[4].ends_with(".scope")
}

#[cfg(any(target_os = "linux", test))]
fn parse_limit(value: &str, name: &str) -> Result<u64, String> {
    let value = value.trim();
    if value == "max" {
        return Err(format!("{name} n'a pas de limite finie (valeur `max`)"));
    }
    value
        .parse::<u64>()
        .map_err(|error| format!("valeur {name} illisible « {value} » : {error}"))
}

#[cfg(any(target_os = "linux", test))]
fn parse_cpu_max(value: &str) -> Result<(u64, u64), String> {
    let mut fields = value.split_whitespace();
    let quota = fields
        .next()
        .ok_or_else(|| "cpu.max ne contient pas de quota".to_string())?;
    let period = fields
        .next()
        .ok_or_else(|| "cpu.max ne contient pas de période".to_string())?;
    if fields.next().is_some() {
        return Err("cpu.max contient des champs surnuméraires".into());
    }
    let quota = parse_limit(quota, "cpu.max quota")?;
    let period = parse_limit(period, "cpu.max période")?;
    if period == 0 {
        return Err("cpu.max a une période nulle".into());
    }
    Ok((quota, period))
}

/// Renvoie `true` seulement si le processus courant est dans un scope dédié
/// Polygone et si MemoryMax/CPUQuota correspondent exactement à sa demande.
/// `false` signifie qu'il faut encore lancer le CLI dans un scope ; les autres
/// erreurs indiquent un scope Polygone mal configuré et doivent faire échouer.
#[cfg(target_os = "linux")]
pub fn current_scope_matches(ram_gb: f32, cpu_cores: u32) -> Result<bool, String> {
    if cpu_cores == 0 {
        return Err("le nombre de cœurs doit être supérieur à zéro".into());
    }
    let requested_memory = requested_memory_bytes(ram_gb)?;
    let proc_cgroup = std::fs::read_to_string("/proc/self/cgroup")
        .map_err(|error| format!("lecture de /proc/self/cgroup impossible : {error}"))?;
    let relative = unified_path(&proc_cgroup)?;
    if !is_polygone_app_scope(relative) {
        return Ok(false);
    }

    let cgroup_dir = Path::new(CGROUP_ROOT).join(relative.trim_start_matches('/'));
    let actual_memory = std::fs::read_to_string(cgroup_dir.join(MEMORY_FILE))
        .map_err(|error| format!("lecture de memory.max impossible : {error}"))?;
    let actual_memory = parse_limit(&actual_memory, "memory.max")?;
    if actual_memory != requested_memory {
        return Err(format!(
            "MemoryMax du scope incohérent : {} octets, attendu {}",
            actual_memory, requested_memory
        ));
    }

    let actual_cpu = std::fs::read_to_string(cgroup_dir.join(CPU_FILE))
        .map_err(|error| format!("lecture de cpu.max impossible : {error}"))?;
    let (quota, period) = parse_cpu_max(&actual_cpu)?;
    if u128::from(quota) != u128::from(cpu_cores) * u128::from(period) {
        return Err(format!(
            "CPUQuota du scope incohérent : {quota}/{period}, attendu {} cœur(s)",
            cpu_cores
        ));
    }

    // Les plafonds hérités peuvent être plus stricts, mais jamais plus souples
    // que la réservation portée par le bail. Vérifier tous les ancêtres protège
    // aussi contre une configuration systemd englobante inattendue.
    let mut ancestor = cgroup_dir.parent();
    while let Some(directory) = ancestor {
        if !directory.starts_with(CGROUP_ROOT) {
            break;
        }
        let memory_file = directory.join(MEMORY_FILE);
        if memory_file.is_file() {
            let limit = std::fs::read_to_string(&memory_file).map_err(|error| {
                format!("lecture de {} impossible : {error}", memory_file.display())
            })?;
            if limit.trim() != "max"
                && parse_limit(&limit, "limite mémoire héritée")? < requested_memory
            {
                return Err(format!(
                    "un cgroup parent limite la mémoire à moins que le bail : {}",
                    directory.display()
                ));
            }
        }
        let cpu_file = directory.join(CPU_FILE);
        if cpu_file.is_file() {
            let value = std::fs::read_to_string(&cpu_file).map_err(|error| {
                format!("lecture de {} impossible : {error}", cpu_file.display())
            })?;
            // `max` signifie sans plafond ; toute autre valeur doit être
            // syntaxiquement valide, sinon la vérification échoue fermée.
            if value.split_whitespace().next() != Some("max") {
                let (parent_quota, parent_period) = parse_cpu_max(&value)?;
                if u128::from(parent_quota) < u128::from(cpu_cores) * u128::from(parent_period) {
                    return Err(format!(
                        "un cgroup parent limite le CPU à moins que le bail : {}",
                        directory.display()
                    ));
                }
            }
        }
        ancestor = directory.parent();
    }

    Ok(true)
}

/// Chemin de cgroup actuel (utile au diagnostic ; jamais renvoyé comme preuve
/// d'isolation à lui seul).
#[cfg(target_os = "linux")]
pub fn current_cgroup_path() -> Result<PathBuf, String> {
    let contents = std::fs::read_to_string("/proc/self/cgroup")
        .map_err(|error| format!("lecture de /proc/self/cgroup impossible : {error}"))?;
    Ok(PathBuf::from(unified_path(&contents)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_the_unified_cgroup_v2_entry() {
        assert_eq!(
            unified_path("12:cpu:/legacy\n0::/user.slice/user-1000.slice/session-4.scope\n"),
            Ok("/user.slice/user-1000.slice/session-4.scope")
        );
        assert!(unified_path("1:cpu:/legacy\n").is_err());
    }

    #[test]
    fn accepts_only_a_dedicated_scope_in_the_user_app_slice() {
        assert!(is_polygone_app_scope(
            "/user.slice/user-1000.slice/user@1000.service/app.slice/polygone-node-42.scope"
        ));
        assert!(!is_polygone_app_scope(
            "/user.slice/user-1000.slice/session-19.scope"
        ));
        assert!(!is_polygone_app_scope(
            "/user.slice/user-1000.slice/user@1000.service/app.slice/other.scope"
        ));
        assert!(!is_polygone_app_scope(
            "/user.slice/user-1001.slice/user@1000.service/app.slice/polygone-node-42.scope"
        ));
    }

    #[test]
    fn parses_cpu_quota_and_period_strictly() {
        assert_eq!(parse_cpu_max("100000 100000\n"), Ok((100000, 100000)));
        assert_eq!(parse_cpu_max("200000 100000"), Ok((200000, 100000)));
        assert!(parse_cpu_max("max 100000").is_err());
        assert!(parse_cpu_max("100000 0").is_err());
        assert!(parse_cpu_max("100000 100000 extra").is_err());
    }

    #[test]
    fn requested_ram_bytes_use_gib_and_reject_invalid_values() {
        assert_eq!(requested_memory_bytes(0.25), Ok(268_435_456));
        assert!(requested_memory_bytes(0.0).is_err());
        assert!(requested_memory_bytes(f32::NAN).is_err());
        assert!(requested_memory_bytes(f32::INFINITY).is_err());
        assert!(requested_memory_bytes(f32::MAX).is_err());
    }
}
