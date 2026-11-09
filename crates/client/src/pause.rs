//! Pause bornée du service — `polygone pause` / `polygone reprendre`.
//!
//! # Ce que ce module est, et ce qu'il n'est pas
//!
//! Une pause est une **intention horodatée**, pas un état du système. Elle vit
//! dans `~/.polygone/pause.json` avec deux instants : quand elle a commencé et
//! quand elle expire. Elle ne survit pas à un redémarrage de la machine tant
//! que CLI.8 n'existe pas — l'écrire autrement serait promettre une persistance
//! qu'aucun processus ne tient.
//!
//! Le bail, lui, est réel : `pause` arrête le processus qui sert (le bail meurt
//! avec sa connexion, `LedgerGuard` côté daemon) et `reprendre` redemande un
//! bail au daemon. Un fichier d'intention sans bail rendu serait un mensonge.
//!
//! # Bornes
//!
//! 10 minutes à 4 heures. En dessous, la pause ne libère rien d'utile ; au
//! dessus, ce n'est plus une pause mais un retrait, et le produit n'a pas encore
//! d'interface pour ça.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Pause minimale acceptée.
pub const MIN_PAUSE: Duration = Duration::from_secs(10 * 60);
/// Pause maximale acceptée.
pub const MAX_PAUSE: Duration = Duration::from_secs(4 * 60 * 60);

/// Intention de pause telle qu'elle est écrite sur disque.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PauseIntent {
    /// Instant où la pause a été demandée (secondes Unix).
    pub paused_at_unix: u64,
    /// Instant où elle cesse d'être valable (secondes Unix).
    pub resumes_at_unix: u64,
    /// Durée demandée, conservée pour l'affichage et le diagnostic.
    pub duration_secs: u64,
}

/// Où en est une intention à un instant donné.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseStatus {
    /// Encore valable ; contient les secondes restantes.
    Active { remaining_secs: u64 },
    /// Dépassée : `reprendre` doit refuser de la consommer comme une pause.
    Expired,
}

/// Emplacement par défaut de l'intention (`~/.polygone/pause.json`).
pub fn default_intent_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".polygone")
        .join("pause.json")
}

/// Maintenant, en secondes Unix. Zéro si l'horloge est antérieure à l'époque —
/// une pause ne peut alors pas être datée, et [`intent`] refusera.
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Lit `30m` ou `2h`. Refuse tout le reste, y compris les valeurs hors bornes :
/// une pause de 9 minutes ou de 5 heures n'est pas « presque » valable.
pub fn parse_duration(raw: &str) -> Result<Duration, String> {
    let raw = raw.trim();
    let (number, unit) = raw.split_at(
        raw.find(|c: char| !c.is_ascii_digit())
            .ok_or_else(|| format!("durée « {raw} » sans unité : utilisez `30m` ou `2h`"))?,
    );
    if number.is_empty() {
        return Err(format!(
            "durée « {raw} » sans nombre : utilisez `30m` ou `2h`"
        ));
    }
    let value: u64 = number
        .parse()
        .map_err(|error| format!("durée « {raw} » illisible : {error}"))?;
    let seconds = match unit {
        "m" => value.saturating_mul(60),
        "h" => value.saturating_mul(60 * 60),
        other => {
            return Err(format!(
                "unité « {other} » inconnue : utilisez `m` (minutes) ou `h` (heures)"
            ))
        }
    };
    let duration = Duration::from_secs(seconds);
    if duration < MIN_PAUSE || duration > MAX_PAUSE {
        return Err(format!(
            "durée hors bornes : {} min demandées, entre 10 min et 4 h",
            seconds / 60
        ));
    }
    Ok(duration)
}

/// Construit l'intention datée. Refuse un horodatage nul : sans instant de
/// départ, l'expiration ne serait pas calculable.
pub fn intent(now_secs: u64, duration: Duration) -> Result<PauseIntent, String> {
    if now_secs == 0 {
        return Err("horloge antérieure à l'époque Unix : la pause ne peut pas être datée".into());
    }
    if duration < MIN_PAUSE || duration > MAX_PAUSE {
        return Err("durée de pause hors bornes (10 min à 4 h)".into());
    }
    Ok(PauseIntent {
        paused_at_unix: now_secs,
        resumes_at_unix: now_secs.saturating_add(duration.as_secs()),
        duration_secs: duration.as_secs(),
    })
}

/// Où en est l'intention à `now_secs`. L'égalité exacte est déjà expirée : à
/// l'instant prévu, la pause n'a plus à être respectée.
pub fn status_at(intent: &PauseIntent, now_secs: u64) -> PauseStatus {
    if now_secs >= intent.resumes_at_unix {
        PauseStatus::Expired
    } else {
        PauseStatus::Active {
            remaining_secs: intent.resumes_at_unix - now_secs,
        }
    }
}

/// Écrit l'intention **atomiquement** : fichier temporaire dans le même dossier,
/// puis renommage. Un lecteur ne doit jamais voir un JSON à moitié écrit.
pub fn write_intent(path: &Path, intent: &PauseIntent) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("chemin d'intention sans dossier : {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("création de {} impossible : {error}", parent.display()))?;
    let json = serde_json::to_string_pretty(intent)
        .map_err(|error| format!("sérialisation de l'intention impossible : {error}"))?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, json)
        .map_err(|error| format!("écriture de {} impossible : {error}", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("renommage vers {} impossible : {error}", path.display()))
}

/// Lit l'intention si elle existe. Un fichier corrompu est une **erreur**, pas
/// une absence : le confondre avec « aucune pause » relancerait le service
/// pendant une pause que l'utilisateur croit active.
pub fn read_intent(path: &Path) -> Result<Option<PauseIntent>, String> {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "lecture de {} impossible : {error}",
                path.display()
            ))
        }
    };
    let intent: PauseIntent = serde_json::from_str(&raw)
        .map_err(|error| format!("intention {} illisible : {error}", path.display()))?;
    Ok(Some(intent))
}

/// Supprime l'intention. `false` si elle n'existait déjà plus — idempotent, comme
/// `reprendre`.
pub fn clear_intent(path: &Path) -> Result<bool, String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "suppression de {} impossible : {error}",
            path.display()
        )),
    }
}

/// Consomme l'intention et redemande un bail via `allocate`.
///
/// L'allocation est injectée : la décision (pause valable, expirée, absente) est
/// testable sans daemon, et le chemin de production ne fait que brancher le vrai
/// client. L'intention n'est supprimée **qu'après** un bail obtenu : une
/// allocation refusée laisse la pause en place, pour qu'un nouvel essai soit
/// possible sans perdre la durée restante.
pub fn resume<A>(path: &Path, now_secs: u64, allocate: A) -> Result<String, String>
where
    A: FnOnce() -> Result<String, String>,
{
    let intent =
        read_intent(path)?.ok_or_else(|| "aucune pause en cours : rien à reprendre".to_string())?;
    match status_at(&intent, now_secs) {
        PauseStatus::Expired => {
            clear_intent(path)?;
            Err(format!(
                "pause expirée depuis {} s : aucun bail redemandé",
                now_secs - intent.resumes_at_unix
            ))
        }
        PauseStatus::Active { remaining_secs } => {
            let bail = allocate()?;
            clear_intent(path)?;
            Ok(format!(
                "bail obtenu après une pause ({remaining_secs} s restants) : {bail}"
            ))
        }
    }
}

/// Description lisible d'une intention pour la sortie CLI.
pub fn describe(intent: &PauseIntent) -> String {
    format!(
        "{} min, jusqu'à {} (epoch)",
        intent.duration_secs / 60,
        intent.resumes_at_unix
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("polygone-{tag}-{}-{nonce}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn pause_writes_expiring_intent() {
        let dir = TempDir::new("pause-ecrit");
        let path = dir.join("pause.json");
        let written = intent(1_000, Duration::from_secs(30 * 60)).unwrap();
        write_intent(&path, &written).unwrap();

        let read = read_intent(&path).unwrap().expect("intention présente");
        assert_eq!(read, written);
        assert_eq!(read.resumes_at_unix, 1_000 + 1_800);
        assert_eq!(
            status_at(&read, 1_500),
            PauseStatus::Active {
                remaining_secs: 1_300
            }
        );
        assert_eq!(
            status_at(&read, read.resumes_at_unix),
            PauseStatus::Expired,
            "à l'instant prévu, la pause est finie — pas une seconde de plus"
        );
    }

    #[test]
    fn resume_clears_intent_and_reallocates() {
        let dir = TempDir::new("pause-reprend");
        let path = dir.join("pause.json");
        write_intent(&path, &intent(2_000, Duration::from_secs(10 * 60)).unwrap()).unwrap();

        let called = Cell::new(false);
        let message = resume(&path, 2_100, || {
            called.set(true);
            Ok("0.25 Gio · 1 cœur(s) — alloc-1".to_string())
        })
        .unwrap();

        assert!(called.get(), "le bail doit être redemandé");
        assert!(message.contains("alloc-1"), "{message}");
        assert!(
            read_intent(&path).unwrap().is_none(),
            "une pause consommée ne doit pas rester sur disque"
        );
    }

    #[test]
    fn resume_refuses_expired_intent_without_allocating() {
        let dir = TempDir::new("pause-expiree");
        let path = dir.join("pause.json");
        write_intent(&path, &intent(1_000, Duration::from_secs(10 * 60)).unwrap()).unwrap();

        let called = Cell::new(false);
        let error = resume(&path, 1_000 + 601, || {
            called.set(true);
            Ok("jamais".to_string())
        })
        .expect_err("une pause expirée ne se reprend pas");

        assert!(!called.get(), "aucun bail ne doit être demandé");
        assert!(error.contains("pause expirée"), "{error}");
        assert!(
            read_intent(&path).unwrap().is_none(),
            "l'intention expirée est consommée par le refus"
        );
    }

    #[test]
    fn resume_without_intent_is_a_named_refusal() {
        let dir = TempDir::new("pause-absente");
        let error = resume(&dir.join("pause.json"), 5_000, || Ok("jamais".into()))
            .expect_err("sans intention, rien à reprendre");
        assert!(error.contains("aucune pause en cours"), "{error}");
    }

    #[test]
    fn resume_keeps_the_intent_when_allocation_fails() {
        let dir = TempDir::new("pause-refusee");
        let path = dir.join("pause.json");
        write_intent(&path, &intent(3_000, Duration::from_secs(20 * 60)).unwrap()).unwrap();

        let error = resume(&path, 3_100, || Err("daemon indisponible".to_string()))
            .expect_err("allocation refusée");
        assert!(error.contains("daemon indisponible"), "{error}");
        assert!(
            read_intent(&path).unwrap().is_some(),
            "un refus du daemon ne doit pas consommer la pause"
        );
    }

    #[test]
    fn duration_parsing_accepts_bounds_and_rejects_the_rest() {
        assert_eq!(parse_duration("10m"), Ok(Duration::from_secs(600)));
        assert_eq!(parse_duration("30m"), Ok(Duration::from_secs(1_800)));
        assert_eq!(parse_duration("4h"), Ok(Duration::from_secs(14_400)));
        assert_eq!(parse_duration(" 2h "), Ok(Duration::from_secs(7_200)));

        for raw in ["9m", "0m", "5h", "30", "abc", "m", "-30m", "30s"] {
            assert!(parse_duration(raw).is_err(), "« {raw} » doit être refusé");
        }
    }

    #[test]
    fn intent_refuses_an_epoch_zero_clock() {
        assert!(intent(0, Duration::from_secs(600)).is_err());
    }
}
