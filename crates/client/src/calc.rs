//! calc — le cache de calcul : « un calcul n'est pas recalculé, il est repris »
//! (`docs/ARCHITECTURE_RESEAU.md` §3-§4). Chantier II.
//!
//! # Le piège qu'on refuse
//!
//! **Clé = BLAKE3(version du calculateur ‖ entrée), jamais BLAKE3(entrée)
//! seule** (§4). Sans la version dans la clé, le premier résultat serait servi
//! *pour toujours* — y compris après correction de l'algorithme — et rien ne le
//! signalerait : hash valide, bonne entrée, résultat périmé. C'est le seul
//! défaut qui rend un cache **pire** que pas de cache.
//!
//! # Pourquoi un index
//!
//! Un magasin adressé par contenu (`blocks`) nomme un fichier par le hash de son
//! **contenu**. La clé sémantique (`hash(version ‖ entrée)`) ne peut donc pas
//! *être* l'adresse d'un bloc : il faudrait `BLAKE3(résultat) == clé`, ce qui
//! n'est vrai que si le résultat *est* l'entrée. On sépare donc :
//!
//! - le **résultat** est un bloc ordinaire, adressé et **vérifié** par
//!   `BLAKE3(résultat)` (`blocks::put_in` / `get_in` / `fetch`) ;
//! - un **index** local `clé → hash_du_résultat` fait le lien sémantique.
//!
//! Conséquence voulue et conforme au §4 : « un bloc à version dépassée n'est pas
//! évincé : il est **introuvable** » — le fichier reste sur disque, mais aucune
//! clé ne le désigne plus.
//!
//! # Le chemin distant
//!
//! Le pair ne peut pas nous dire « donne-moi le bloc nommé <clé> » : `blocks`
//! exige une adresse content-addressed. On échange donc d'abord une
//! **référence** (`GET <clé>` → `HAVE 64\n<hash_du_résultat>`), puis on récupère
//! le contenu par `blocks::fetch` — qui **vérifie** `BLAKE3(contenu) == hash`.
//! Le même format de message `GET/HAVE/MISS` sert aux deux : ce n'est pas un
//! second protocole, c'est le même sur un autre espace de noms.
//!
//! # Honnêteté : le hash prouve l'intégrité, pas la justesse
//!
//! Un pair peut servir un résultat **faux dont le hash est valide**. Le hash ne
//! dit que « ces octets sont bien ceux qui portent ce nom » ; il ne dit pas « ce
//! résultat est le bon pour cette entrée ». Et l'index `clé→hash` n'étant pas
//! vérifiable, le lien entrée→résultat repose sur la **parole du pair**. Ce cache
//! est donc un cache de **confiance** : voir la section « Honnêteté du cache » de
//! [`get_or_compute`].
//!
//! # `hits_distant` : un zéro qui ne ment pas
//!
//! `hits_distant` compte les reprises **réellement obtenues chez un pair**. Un
//! zéro n'est pas un échec : c'est le plus souvent que le **chemin distant n'a
//! jamais été exercé** — aucun pair joignable (`polygone calc --chez`), et le
//! LAN entre deux machines n'a jamais tourné (2b-12c). L'affichage ne doit pas
//! laisser croire à un essai qui aurait échoué : voir `polygone calc --stats`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::blocks;

/// Port par défaut du service de références de calcul.
pub const DEFAULT_CALC_PORT: u16 = 7644;

/// **Version du calculateur.** Elle entre dans la clé, pas dans le résultat :
/// changer l'algorithme doit rendre l'ancien résultat **introuvable**, pas
/// servir un résultat périmé sous une clé inchangée.
pub const CALC_VERSION: u32 = 1;

/// Délai d'E/S, comme `blocks::IO_TIMEOUT`.
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Plafond de la ligne de requête (`GET <64 hex>` = 69 octets).
const MAX_REQUEST_LINE: usize = 64 * 1024;

// ── La clé ──────────────────────────────────────────────────────────────────

/// La clé de cache : `BLAKE3(version ‖ entrée)`, en hex minuscule.
///
/// Réutilise la seule implémentation BLAKE3 du dépôt (`blocks::hash_of`), pour
/// ne pas réintroduire un second hachage ni `Fragment::dummy_hash` (un XOR de
/// développement).
pub fn key_for(version: u32, input: &str) -> String {
    let mut buf = Vec::with_capacity(4 + input.len());
    buf.extend_from_slice(&version.to_be_bytes());
    buf.extend_from_slice(input.as_bytes());
    blocks::hash_of(&buf)
}

// ── Le calcul ───────────────────────────────────────────────────────────────

/// Le calcul : π(N), le nombre de premiers ≤ N. **Déterministe**, sans GPU,
/// sans dépendance nouvelle, borné en mémoire (crible segmenté). Le résultat
/// est petit (le texte `pi(<N>) = <compte>`) et **vérifiable à la main** :
/// π(10) = 4, π(1 000) = 168, π(10⁶) = 78 498.
pub fn compute(input: &str) -> Result<Vec<u8>> {
    let n: u64 = input
        .trim()
        .parse()
        .map_err(|e| anyhow::anyhow!("entrée illisible « {input} » (N entier attendu) : {e}"))?;
    let count = count_primes(n);
    Ok(format!("pi({n}) = {count}\n").into_bytes())
}

/// π(n) par crible d'Ératosthène **segmenté** : la mémoire ne dépend pas de `n`,
/// seulement de la taille d'un segment.
fn count_primes(n: u64) -> u64 {
    if n < 2 {
        return 0;
    }
    let sqrt = (n as f64).sqrt() as u64 + 1;

    // Primes de base jusqu'à √n.
    let mut base = vec![true; (sqrt + 1) as usize];
    let mut primes: Vec<u64> = Vec::new();
    for i in 2..=sqrt {
        if base[i as usize] {
            primes.push(i);
            let mut j = i * i;
            while j <= sqrt {
                base[j as usize] = false;
                j += i;
            }
        }
    }

    // Tous les `primes` sont ≤ √n ≤ n : on les compte une fois.
    let mut count = primes.len() as u64;

    const SEG: u64 = 1 << 20;
    let mut low = sqrt + 1;
    let mut seg = vec![true; SEG as usize];
    while low <= n {
        let high = (low + SEG - 1).min(n);
        let len = (high - low + 1) as usize;
        seg[..len].fill(true);
        for &p in &primes {
            if p * p > high {
                break;
            }
            let start = low.div_ceil(p) * p;
            let mut j = start;
            while j <= high {
                seg[(j - low) as usize] = false;
                j += p;
            }
        }
        count += seg[..len].iter().filter(|&&b| b).count() as u64;
        low = high + 1;
    }
    count
}

// ── L'index et les compteurs ────────────────────────────────────────────────

/// Une entrée d'index : où est le résultat, et combien il a coûté à calculer.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexEntry {
    result_hash: String,
    cost_ms: u64,
}

type Index = HashMap<String, IndexEntry>;

/// Compteurs du cache — durabes, dans le `HOME` du nœud.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CalcStats {
    /// Résultats trouvés **localement**.
    pub hits_local: u64,
    /// Résultats trouvés **chez un pair**.
    pub hits_distant: u64,
    /// Calculs réellement effectués.
    pub misses: u64,
    /// Temps cumulé passé à calculer, en millisecondes.
    pub ms_computed: u64,
    /// Temps cumulé **économisé** par les succès (coût mémorisé des résultats).
    pub ms_saved: u64,
    /// Horodatage (s) de la dernière mise à jour.
    pub last_update: u64,
}

/// D'où vient un résultat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Local,
    Distant,
    Computed,
}

impl Origin {
    /// Étiquette lisible, pour la ligne de commande.
    pub fn label(self) -> &'static str {
        match self {
            Origin::Local => "succès local (cache)",
            Origin::Distant => "succès distant (pair)",
            Origin::Computed => "manque (calculé)",
        }
    }
}

/// Le magasin de résultats : un index sémantique + les blocs qui les portent.
///
/// Le chemin est un **paramètre** (comme `blocks::put_in(root, ...)`) : les
/// tests visent un dossier temporaire et ne touchent jamais le vrai
/// `~/.polygone`.
#[derive(Debug, Clone)]
pub struct CalcStore {
    root: PathBuf,
    blocks_root: PathBuf,
}

impl CalcStore {
    /// Crée un magasin à racines explicites.
    pub fn new(root: PathBuf, blocks_root: PathBuf) -> Self {
        Self { root, blocks_root }
    }

    /// Racine par défaut : `~/.polygone/calc/`.
    pub fn default_root() -> PathBuf {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".polygone")
            .join("calc")
    }

    /// Magasin par défaut (index `~/.polygone/calc`, blocs `~/.polygone/blocks`).
    pub fn default_store() -> Self {
        Self::new(Self::default_root(), blocks::default_root())
    }

    /// Racine de l'index (utile aux tests et à `--stats`).
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    /// Racine du magasin de blocs.
    pub fn blocks_root(&self) -> &std::path::Path {
        &self.blocks_root
    }

    fn index_path(&self) -> PathBuf {
        self.root.join("index.json")
    }

    fn stats_path(&self) -> PathBuf {
        self.root.join("stats.json")
    }

    fn load_index(&self) -> Index {
        std::fs::read_to_string(self.index_path())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn save_index(&self, index: &Index) -> io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let raw = serde_json::to_string_pretty(index)?;
        std::fs::write(self.index_path(), raw)
    }

    /// Charge les compteurs (zéro si absents).
    pub fn load_stats(&self) -> CalcStats {
        std::fs::read_to_string(self.stats_path())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// Écrit les compteurs (au mieux).
    pub fn save_stats(&self, stats: &CalcStats) -> io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let raw = serde_json::to_string_pretty(stats)?;
        std::fs::write(self.stats_path(), raw)
    }

    /// Le hash du résultat déjà connu pour une clé, s'il l'est.
    pub fn result_hash(&self, key: &str) -> Option<String> {
        self.load_index().get(key).map(|e| e.result_hash.clone())
    }

    /// Cherche un résultat **localement** : index → bloc vérifié.
    ///
    /// Rend `None` si la clé est inconnue *ou* si le bloc a disparu/été altéré —
    /// un index qui pointe dans le vide vaut un manque, jamais un mensonge.
    pub fn lookup(&self, key: &str) -> Option<(u64, Vec<u8>)> {
        let entry = self.load_index().get(key).cloned()?;
        let bytes = blocks::get_in(&self.blocks_root, &entry.result_hash)?;
        Some((entry.cost_ms, bytes))
    }

    /// Enregistre un résultat : le bloc d'abord (vérifié par son hash), puis
    /// l'index. Si l'écriture du bloc échoue, aucune clé fantôme n'est écrite.
    pub fn record(&self, key: &str, result: &[u8], cost_ms: u64) -> io::Result<String> {
        let result_hash = blocks::put_in(&self.blocks_root, result)?;
        let mut index = self.load_index();
        index.insert(
            key.to_string(),
            IndexEntry {
                result_hash: result_hash.clone(),
                cost_ms,
            },
        );
        self.save_index(&index)?;
        Ok(result_hash)
    }
}

// ── Le chemin de décision ───────────────────────────────────────────────────

/// Calcule ou reprend un résultat.
///
/// Ordre : cache local → pair (`chez`) → calcul. Chaque issue incrémente un
/// compteur durable et renvoie son origine : « repris » et « calculé » ne sont
/// jamais confondus.
///
/// # Honnêteté du cache
///
/// Le hash prouve l'**intégrité**, pas la **justesse**. Un pair peut servir un
/// résultat faux dont le hash est valide ; l'index `clé→hash` n'étant pas
/// vérifiable, le lien entrée→résultat repose sur la parole du pair. Ce cache
/// est donc un cache de **confiance**. Seul un recalcul (qui annule le gain) ou
/// une signature (`receipt.rs` — qui prouve l'auteur, pas la justesse) le
/// rendrait vérifiable.
pub fn get_or_compute(
    store: &CalcStore,
    version: u32,
    input: &str,
    chez: Option<SocketAddr>,
) -> Result<(Vec<u8>, Origin)> {
    let key = key_for(version, input);
    let mut stats = store.load_stats();

    if let Some((cost_ms, bytes)) = store.lookup(&key) {
        stats.hits_local += 1;
        stats.ms_saved += cost_ms;
        stats.last_update = now_secs();
        store.save_stats(&stats)?;
        return Ok((bytes, Origin::Local));
    }

    if let Some(peer) = chez {
        if let Some(result_hash) = fetch_ref(&key, peer)? {
            if let Ok(bytes) = blocks::fetch(&result_hash, peer) {
                // Vérifié par `blocks::fetch` : le contenu correspond au hash
                // que la référence annonçait. On le mémorise localement.
                store.record(&key, &bytes, 0)?;
                stats.hits_distant += 1;
                stats.last_update = now_secs();
                store.save_stats(&stats)?;
                return Ok((bytes, Origin::Distant));
            }
            // Le fetch a échoué (bloc absent, ou contenu ne correspondant pas au
            // hash annoncé) → manque. Attention : un contenu FAUX dont le hash
            // est cohérent passerait quand même — le hash prouve l'intégrité,
            // pas la justesse (voir la section « Honnêteté du cache » ci-dessus).
        }
    }

    let started = Instant::now();
    let result = compute(input)?;
    let cost_ms = started.elapsed().as_millis() as u64;
    store.record(&key, &result, cost_ms)?;
    stats.misses += 1;
    stats.ms_computed += cost_ms;
    stats.last_update = now_secs();
    store.save_stats(&stats)?;
    Ok((result, Origin::Computed))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ── Le service de références (même protocole que `blocks`) ──────────────────

/// Sert, sur une connexion, une **référence** (`clé → hash du résultat`) ou un
/// bloc de résultat, selon ce que porte la requête.
///
/// Même format que `blocks` : `GET <64 hex>\n` → `HAVE <len>\n<octets>` ou
/// `MISS\n`. Deux espaces de noms, un seul protocole.
pub fn serve(listener: TcpListener, store: CalcStore) -> io::Result<()> {
    for incoming in listener.incoming() {
        let stream = match incoming {
            Ok(s) => s,
            Err(e) => {
                log::warn!("calc: accept impossible : {e}");
                continue;
            }
        };
        let store = store.clone();
        std::thread::spawn(move || {
            if let Err(e) = handle_ref(stream, &store) {
                log::debug!("calc: connexion terminée : {e}");
            }
        });
    }
    Ok(())
}

fn handle_ref(mut stream: TcpStream, store: &CalcStore) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;

    let line = match read_line(&mut stream)? {
        Some(l) => l,
        None => return Ok(()),
    };
    let mut parts = line.split_whitespace();
    let name = match (parts.next(), parts.next(), parts.next()) {
        (Some("GET"), Some(n), None) => n.to_string(),
        _ => {
            stream.write_all(b"MISS\n")?;
            return Ok(());
        }
    };

    // 1) une référence de calcul (clé sémantique) ?
    if let Some(rh) = store.result_hash(&name) {
        writeln!(stream, "HAVE {}", rh.len())?;
        stream.write_all(rh.as_bytes())?;
        return Ok(());
    }
    // 2) sinon, un bloc de résultat ordinaire (adressé par contenu).
    match blocks::get_in(store.blocks_root(), &name) {
        Some(bytes) => {
            writeln!(stream, "HAVE {}", bytes.len())?;
            stream.write_all(&bytes)?;
        }
        None => stream.write_all(b"MISS\n")?,
    }
    Ok(())
}

/// Demande la référence d'une clé à un pair. `None` si le pair ne la connaît pas.
pub fn fetch_ref(key: &str, peer: SocketAddr) -> io::Result<Option<String>> {
    if !is_hex64(key) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "clé invalide (64 caractères hexadécimaux minuscules attendus)",
        ));
    }
    let mut stream = TcpStream::connect_timeout(&peer, IO_TIMEOUT)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    writeln!(stream, "GET {key}")?;

    let line = read_line(&mut stream)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "réponse absente"))?;
    if line == "MISS" {
        return Ok(None);
    }
    let len: usize = match line
        .strip_prefix("HAVE ")
        .and_then(|n| n.trim().parse().ok())
    {
        Some(n) => n,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("réponse illisible : {line}"),
            ))
        }
    };
    if len != 64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("référence de taille inattendue : {len}"),
        ));
    }
    let mut bytes = vec![0u8; 64];
    stream.read_exact(&mut bytes)?;
    let hash = String::from_utf8_lossy(&bytes).to_string();
    if !is_hex64(&hash) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "référence non hexadécimale",
        ));
    }
    Ok(Some(hash))
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Lit une ligne terminée par `\n`, avec plafond — jamais une ligne tronquée.
fn read_line(stream: &mut TcpStream) -> io::Result<Option<String>> {
    let mut buf: Vec<u8> = Vec::with_capacity(64);
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return Ok(None),
            Ok(_) => {
                if byte[0] == b'\n' {
                    return Ok(Some(
                        String::from_utf8_lossy(&buf)
                            .trim_end_matches('\r')
                            .to_string(),
                    ));
                }
                buf.push(byte[0]);
                if buf.len() > MAX_REQUEST_LINE {
                    return Ok(None);
                }
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Dossier temporaire propre à un test — jamais le `~/.polygone` réel.
    /// Même motif que `blocks::tests::TempRoot`.
    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let p = std::env::temp_dir().join(format!("polygone-calc-{}-{n}", std::process::id()));
            std::fs::create_dir_all(&p).expect("dossier temporaire");
            Self(p)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store_at(tmp: &TempRoot) -> CalcStore {
        CalcStore::new(tmp.path().join("calc"), tmp.path().join("blocks"))
    }

    #[test]
    fn known_prime_counts() {
        assert_eq!(count_primes(1), 0);
        assert_eq!(count_primes(2), 1);
        assert_eq!(count_primes(10), 4);
        assert_eq!(count_primes(100), 25);
        assert_eq!(count_primes(1_000), 168);
        assert_eq!(count_primes(1_000_000), 78_498);
        assert_eq!(count_primes(10_000_000), 664_579);
    }

    #[test]
    fn compute_is_deterministic() {
        assert_eq!(compute("1000").unwrap(), compute("1000").unwrap());
        assert_eq!(
            String::from_utf8(compute("1000").unwrap()).unwrap(),
            "pi(1000) = 168\n"
        );
    }

    /// ⚑ Le test le plus important du chantier : la version entre dans la clé.
    #[test]
    fn version_is_in_the_key() {
        assert_ne!(
            key_for(1, "1000"),
            key_for(2, "1000"),
            "changer la version doit changer la clé"
        );
        assert_eq!(
            key_for(1, "1000"),
            key_for(1, "1000"),
            "mêmes entrée et version → même clé"
        );
    }

    /// ⚑ Une version différente ne doit JAMAIS servir le résultat de l'autre.
    #[test]
    fn different_version_recomputes() {
        let tmp = TempRoot::new();
        let store = store_at(&tmp);

        let (_, first) = get_or_compute(&store, 1, "1000", None).expect("calcul v1");
        assert_eq!(first, Origin::Computed);

        let (_, again) = get_or_compute(&store, 1, "1000", None).expect("reprise v1");
        assert_eq!(again, Origin::Local, "même version → reprise");

        let (_, forced) = get_or_compute(&store, 2, "1000", None).expect("calcul v2");
        assert_eq!(
            forced,
            Origin::Computed,
            "version différente → recalcul, jamais un résultat périmé"
        );
    }

    /// ⚑ Deuxième appel identique = succès de cache, et le compteur le prouve.
    #[test]
    fn second_call_is_a_cache_hit() {
        let tmp = TempRoot::new();
        let store = store_at(&tmp);

        let _ = get_or_compute(&store, CALC_VERSION, "1000", None).expect("premier");
        let (_, origin) = get_or_compute(&store, CALC_VERSION, "1000", None).expect("second");
        assert_eq!(origin, Origin::Local);

        let stats = store.load_stats();
        assert_eq!(stats.misses, 1, "un seul calcul");
        assert_eq!(stats.hits_local, 1, "une reprise locale");
    }

    /// ⚑ Un résultat de pair altéré est rejeté : la vérification par hash tient.
    #[test]
    fn tampered_peer_result_is_rejected() {
        let tmp = TempRoot::new();
        let store = store_at(&tmp);

        // Le pair a un résultat, mais sert un octet modifié.
        let good = compute("1000").expect("calcul");
        let good_hash = blocks::hash_of(&good);
        let mut tampered = good.clone();
        tampered[0] ^= 0x01;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("adresse");
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let _ = read_line(&mut s); // la requête GET
                let _ = writeln!(s, "HAVE {}", tampered.len());
                let _ = s.write_all(&tampered);
            }
        });

        let err = blocks::fetch(&good_hash, addr).expect_err("un bloc altéré doit être rejeté");
        assert_eq!(
            err.kind(),
            io::ErrorKind::InvalidData,
            "rejet au hash : {err}"
        );
        assert!(
            store.lookup(&key_for(CALC_VERSION, "1000")).is_none(),
            "aucun résultat mémorisé depuis un bloc rejeté"
        );
    }

    /// Le chemin distant complet : référence par clé, puis contenu vérifié.
    #[test]
    fn distant_ref_then_verified_fetch() {
        let peer_tmp = TempRoot::new();
        let peer_store = store_at(&peer_tmp);
        let _ = get_or_compute(&peer_store, CALC_VERSION, "1000", None).expect("le pair calcule");
        let expected = compute("1000").expect("calcul attendu");

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("adresse");
        let served = peer_store.clone();
        std::thread::spawn(move || {
            let _ = serve(listener, served);
        });

        // Un consommateur distinct, dossier distinct.
        let me_tmp = TempRoot::new();
        let me = store_at(&me_tmp);
        let (bytes, origin) =
            get_or_compute(&me, CALC_VERSION, "1000", Some(addr)).expect("reprise distante");
        assert_eq!(origin, Origin::Distant);
        assert_eq!(bytes, expected, "le résultat distant est celui du pair");
        assert_eq!(me.load_stats().hits_distant, 1);

        // Déjà en cache local : le second appel ne retourne plus sur le réseau.
        let (_, origin2) = get_or_compute(&me, CALC_VERSION, "1000", Some(addr)).expect("relocal");
        assert_eq!(origin2, Origin::Local);
    }

    /// Le magasin n'écrit jamais dans le vrai `~/.polygone`.
    #[test]
    fn store_never_touches_the_real_home() {
        let real = CalcStore::default_root();
        let before = std::fs::metadata(&real)
            .ok()
            .and_then(|m| m.modified().ok());

        let tmp = TempRoot::new();
        let store = store_at(&tmp);
        let _ = get_or_compute(&store, CALC_VERSION, "500", None).expect("calcul");

        let after = std::fs::metadata(&real)
            .ok()
            .and_then(|m| m.modified().ok());
        assert_eq!(before, after, "le vrai HOME ne doit pas bouger");
    }
}
