//! CLI.3 — profils d'usage, avec leur impact chiffré.
//!
//! Un profil n'est pas une politique : c'est un jeu de trois seuils (CPU, RAM,
//! débit, plus le GPU) écrit dans `~/.config/polygone/daemon.toml`, le fichier
//! que `polygoned` relit à son démarrage. Le module ne touche ni à
//! `BehaviorConfig` ni à `SafetyMargins` : les garde-fous restent ceux de la
//! politique Glow-Up.
//!
//! Ce que le module refuse de faire : annoncer une limite OS. Sans isolation
//! (`--isoler`), le plafond reste auto-imposé par le daemon ; le texte le dit à
//! chaque affichage.

use anyhow::{Context, Result};
use polygoned::{DaemonConfig, ResourceLimits, SystemSnapshot};
use std::path::{Path, PathBuf};

/// Les trois profils nommés par la spécification CLI.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Sobre,
    Equilibre,
    Genereux,
}

impl Profile {
    pub const ALL: [Profile; 3] = [Profile::Sobre, Profile::Equilibre, Profile::Genereux];

    /// Nom stable, celui qu'on tape et celui qu'on relit.
    pub fn name(self) -> &'static str {
        match self {
            Profile::Sobre => "sobre",
            Profile::Equilibre => "equilibre",
            Profile::Genereux => "genereux",
        }
    }

    /// Accepte les accents et la casse, refuse le reste en nommant les choix.
    pub fn parse(raw: &str) -> Result<Self> {
        let normalized = raw.trim().to_lowercase().replace(['é', 'è'], "e");
        match normalized.as_str() {
            "sobre" => Ok(Profile::Sobre),
            "equilibre" => Ok(Profile::Equilibre),
            "genereux" => Ok(Profile::Genereux),
            other => {
                anyhow::bail!("profil inconnu : « {other} » — choix : sobre, equilibre, genereux")
            }
        }
    }

    /// Seuils du profil. Ils sont volontairement distincts sur les trois
    /// grandeurs : c'est ce qu'un utilisateur vient chercher.
    pub fn limits(self) -> ResourceLimits {
        match self {
            Profile::Sobre => ResourceLimits {
                max_cpu_percent: 25,
                max_ram_percent: 30,
                max_bandwidth_percent: 40,
                max_gpu_percent: 20,
            },
            Profile::Equilibre => ResourceLimits {
                max_cpu_percent: 50,
                max_ram_percent: 50,
                max_bandwidth_percent: 60,
                max_gpu_percent: 40,
            },
            Profile::Genereux => ResourceLimits {
                max_cpu_percent: 75,
                max_ram_percent: 70,
                max_bandwidth_percent: 80,
                max_gpu_percent: 60,
            },
        }
    }

    /// Impact attendu, en clair — jamais une promesse de limite OS.
    pub fn impact(self) -> &'static str {
        match self {
            Profile::Sobre => {
                "discret : un quart du CPU, 30 % de la RAM, 40 % du débit — le nœud se fait oublier."
            }
            Profile::Equilibre => {
                "équilibré : la moitié du CPU et de la RAM, 60 % du débit — le poste reste confortable."
            }
            Profile::Genereux => {
                "généreux : trois quarts du CPU, 70 % de la RAM, 80 % du débit — le réseau profite du temps mort."
            }
        }
    }

    /// Écrit le profil dans une copie de la config : politique et garde-fous
    /// intacts, seuls `tier` et `custom_limits` bougent.
    pub fn apply_to(self, config: &DaemonConfig) -> DaemonConfig {
        let mut updated = config.clone();
        updated.tier = polygoned::AllocationTier::Custom;
        updated.custom_limits = Some(self.limits());
        updated
    }
}

/// Profil actuellement écrit, s'il correspond exactement à un profil nommé.
///
/// Un `custom_limits` qui ne correspond à aucun profil n'est pas « equilibre
/// par défaut » : c'est du personnalisé, et l'affichage le dit.
pub fn current(config: &DaemonConfig) -> Option<Profile> {
    if let Some(limits) = config.custom_limits {
        return Profile::ALL.into_iter().find(|profile| {
            let expected = profile.limits();
            limits.max_cpu_percent == expected.max_cpu_percent
                && limits.max_ram_percent == expected.max_ram_percent
                && limits.max_bandwidth_percent == expected.max_bandwidth_percent
                && limits.max_gpu_percent == expected.max_gpu_percent
        });
    }
    match config.tier {
        polygoned::AllocationTier::Eco => Some(Profile::Sobre),
        polygoned::AllocationTier::Balanced => Some(Profile::Equilibre),
        polygoned::AllocationTier::Performance => Some(Profile::Genereux),
        polygoned::AllocationTier::Max | polygoned::AllocationTier::Custom => None,
    }
}

/// Chemin réellement lu par `polygoned` (même règle que `Platform::config_dir`).
pub fn config_path() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(xdg).join("polygone").join("daemon.toml");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".config")
            .join("polygone")
            .join("daemon.toml");
    }
    PathBuf::from(".config")
        .join("polygone")
        .join("daemon.toml")
}

/// Lit la config du daemon ; un fichier absent vaut les defaults produit.
pub fn read(path: &Path) -> Result<DaemonConfig> {
    if !path.exists() {
        return Ok(DaemonConfig::default());
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("lecture de {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("config illisible : {}", path.display()))
}

/// Écrit la config de façon atomique : un daemon lancé pendant l'écriture ne
/// lit jamais un fichier à moitié écrit.
pub fn write(path: &Path, config: &DaemonConfig) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).with_context(|| format!("création de {}", parent.display()))?;

    let body = toml::to_string_pretty(config)?;
    let temporary = path.with_extension("toml.tmp");
    std::fs::write(
        &temporary,
        format!("# polygoned config — écrite par `polygone profil`\n\n{body}"),
    )
    .with_context(|| format!("écriture de {}", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .with_context(|| format!("remplacement de {}", path.display()))?;
    Ok(())
}

/// Seuils effectifs, y compris quand la config n'écrit qu'un tier.
fn effective_limits(config: &DaemonConfig) -> ResourceLimits {
    if let Some(limits) = config.custom_limits {
        return limits;
    }
    let mut platform = polygoned::create_platform();
    let _ = platform.init();
    let snapshot = SystemSnapshot::capture(&*platform);
    config.effective_limits(&snapshot)
}

/// Rendu utilisateur : profil courant, seuils, impact, limite OS, chemin.
pub fn render(config: &DaemonConfig, path: &Path) -> String {
    let limits = effective_limits(config);
    let current_line = match current(config) {
        // Les seuils personnalisés priment sur le tier : le nommer évite de
        // faire croire que le tier par défaut décrit encore l'allocation.
        Some(profile) if config.custom_limits.is_some() => {
            format!("{} (seuils personnalisés)", profile.name())
        }
        Some(profile) => format!("{} (tier {})", profile.name(), config.tier),
        None => format!("personnalisé (tier {})", config.tier),
    };
    let impact = current(config)
        .map(Profile::impact)
        .unwrap_or("seuils écrits à la main : le daemon les respecte tels quels.");

    format!(
        "Profil      : {current_line}\n\
         Seuils      : CPU {}% · RAM {}% · BW {}% · GPU {}%\n\
         Impact      : {impact}\n\
         Limite OS   : aucune limite OS n'est posée sans isolation (`--isoler`) — \
         le plafond reste auto-imposé par le daemon.\n\
         Config      : {}\n\
         Application : polygoned relit ce fichier à son démarrage ; redémarrez-le pour changer l'allocation.\n",
        limits.max_cpu_percent,
        limits.max_ram_percent,
        limits.max_bandwidth_percent,
        limits.max_gpu_percent,
        path.display()
    )
}

/// Entrée CLI : `polygone profil [nom] [--config <chemin>]`.
pub fn run(name: Option<&str>, explicit_path: Option<&Path>) -> Result<()> {
    let path = explicit_path.map(PathBuf::from).unwrap_or_else(config_path);

    let config = read(&path)?;
    let config = match name {
        Some(raw) => {
            let profile = Profile::parse(raw)?;
            let updated = profile.apply_to(&config);
            write(&path, &updated)?;
            updated
        }
        None => config,
    };
    print!("{}", render(&config, &path));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn scratch_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("horloge")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("polygone-profil-{label}-{stamp}"));
        std::fs::create_dir_all(&dir).expect("répertoire de test");
        dir
    }

    #[test]
    fn three_profiles_write_distinct_limits() {
        let dir = scratch_dir("trois");
        let path = dir.join("daemon.toml");

        // Une config existante dont les garde-fous ne doivent pas bouger.
        let mut base = DaemonConfig::default();
        base.behavior.tick_interval_secs = 7;
        base.safety.min_free_ram_gb = 9.5;
        write(&path, &base).expect("écriture initiale");

        let mut cpu_values = Vec::new();
        for profile in Profile::ALL {
            let config = read(&path).expect("relecture");
            let updated = profile.apply_to(&config);
            write(&path, &updated).expect("écriture du profil");

            let reread = read(&path).expect("config réellement relue");
            let limits = reread.custom_limits.expect("seuils écrits");
            let expected = profile.limits();
            assert_eq!(limits.max_cpu_percent, expected.max_cpu_percent);
            assert_eq!(limits.max_ram_percent, expected.max_ram_percent);
            assert_eq!(limits.max_bandwidth_percent, expected.max_bandwidth_percent);
            assert_eq!(limits.max_gpu_percent, expected.max_gpu_percent);
            assert_eq!(current(&reread), Some(profile));
            // La politique et les garde-fous restent ceux de l'utilisateur.
            assert_eq!(reread.behavior.tick_interval_secs, 7);
            assert_eq!(reread.safety.min_free_ram_gb, 9.5);
            cpu_values.push(limits.max_cpu_percent);
        }

        cpu_values.sort_unstable();
        cpu_values.dedup();
        assert_eq!(cpu_values.len(), 3, "les trois profils doivent différer");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn profil_without_arg_prints_current() {
        let rendered = render(&DaemonConfig::default(), Path::new("/tmp/daemon.toml"));
        assert!(
            rendered.contains("Profil      : equilibre"),
            "sans argument, le profil courant doit être nommé : {rendered}"
        );
        assert!(
            rendered.contains("Limite OS   : aucune limite OS"),
            "ne jamais annoncer une contrainte OS : {rendered}"
        );
        assert!(rendered.contains("Application : polygoned relit"));
    }

    #[test]
    fn unknown_profile_is_refused_with_the_choices() {
        let error = Profile::parse("turbo").expect_err("profil inconnu");
        let message = error.to_string();
        assert!(message.contains("turbo"));
        assert!(message.contains("sobre"));
        assert!(message.contains("equilibre"));
        assert!(message.contains("genereux"));
    }

    #[test]
    fn accented_name_is_accepted() {
        assert_eq!(
            Profile::parse("Équilibre").expect("accent"),
            Profile::Equilibre
        );
        assert_eq!(
            Profile::parse("généreux").expect("accent"),
            Profile::Genereux
        );
    }
}
