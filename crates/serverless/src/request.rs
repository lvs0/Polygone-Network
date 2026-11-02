//! Requête serverless, fragments Shamir et reconstruction.
//!
//! Le découpage est un vrai partage de secret (4-of-7 par défaut) : les
//! fragments voyagent séparément, `threshold` d'entre eux suffisent, et le hash
//! BLAKE3 de l'enveloppe complète permet de rejeter tout fragment altéré.

use anyhow::{anyhow, bail, Context, Result};
use polygone_core::crypto::shamir;
use serde::{Deserialize, Serialize};

/// Nombre de fragments produits par défaut.
pub const DEFAULT_TOTAL: u8 = 7;
/// Nombre de fragments nécessaires par défaut.
pub const DEFAULT_THRESHOLD: u8 = 4;

/// Ce que les nœuds doivent exécuter.
///
/// Les trois variantes décrivent la même chose — une fonction déterministe et
/// son entrée — avec des niveaux de contrainte différents. Aucune n'est
/// exécutée par ce crate : il transporte le descriptif, pas le moteur.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FunctionKind {
    /// Commande shell, exécutée dans la sandbox du nœud fantôme.
    Shell { command: String },
    /// Module WASM, identifié par le hash de son binaire (le module voyage à part).
    Wasm { module_hash: [u8; 32] },
    /// Fonction nommée connue des deux côtés (hash, tri, etc.).
    Deterministic { name: String },
}

/// Une requête d'exécution, avant découpage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerlessRequest {
    /// Identifiant de requête, unique côté client.
    pub id: String,
    /// Fonction à exécuter.
    pub function: FunctionKind,
    /// Entrée de la fonction.
    pub payload: Vec<u8>,
    /// Fragments nécessaires à la reconstruction.
    pub threshold: u8,
    /// Fragments produits.
    pub total: u8,
}

/// Un fragment Shamir prêt à partir vers un nœud.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shard {
    /// Index du fragment, `1..=total`.
    pub index: u8,
    /// Seuil annoncé par le producteur (vérifié à la reconstruction).
    pub threshold: u8,
    /// Nombre total de fragments produits.
    pub total: u8,
    /// BLAKE3 de l'enveloppe complète (identifiant + charge utile).
    pub content_hash: [u8; 32],
    /// Octets du partage Shamir.
    pub payload: Vec<u8>,
}

/// Enveloppe réellement découpée : l'identifiant est lié à la charge utile,
/// sinon un fragment pourrait être rejoué sous une autre requête.
#[derive(Serialize, Deserialize)]
struct PayloadEnvelope {
    id: String,
    payload: Vec<u8>,
}

impl ServerlessRequest {
    /// Nouvelle requête en 4-of-7.
    pub fn new(function: FunctionKind, payload: Vec<u8>) -> Self {
        let id = random_id();
        Self {
            id,
            function,
            payload,
            threshold: DEFAULT_THRESHOLD,
            total: DEFAULT_TOTAL,
        }
    }

    /// Vérifie les bornes du partage avant tout découpage.
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            bail!("identifiant de requête vide");
        }
        if self.threshold < 2 {
            bail!(
                "seuil {} invalide : un seuil de 1 ne protège rien",
                self.threshold
            );
        }
        if self.total < self.threshold {
            bail!("total {} inférieur au seuil {}", self.total, self.threshold);
        }
        if self.total > DEFAULT_TOTAL {
            bail!(
                "total {} dépasse le maximum du protocole ({})",
                self.total,
                DEFAULT_TOTAL
            );
        }
        Ok(())
    }

    /// Découpe la requête en `total` fragments Shamir réels.
    pub fn shards(&self) -> Result<Vec<Shard>> {
        self.validate()?;

        let envelope = serde_json::to_vec(&PayloadEnvelope {
            id: self.id.clone(),
            payload: self.payload.clone(),
        })
        .context("sérialisation de l'enveloppe serverless")?;

        let content_hash = hash_bytes(&envelope);

        let shares = shamir::split(&envelope, self.threshold, self.total)
            .map_err(|e| anyhow!("découpage Shamir impossible : {e}"))?;

        Ok(shares
            .into_iter()
            .map(|fragment| Shard {
                index: fragment.id.0,
                threshold: self.threshold,
                total: self.total,
                content_hash,
                payload: fragment.data,
            })
            .collect())
    }

    /// Empreinte stable de la requête (identifiant, fonction, entrée, bornes).
    ///
    /// Sert à rapprocher les résultats d'exécution de leur demande sans
    /// transmettre la requête elle-même.
    pub fn request_hash(&self) -> [u8; 32] {
        let function = serde_json::to_vec(&self.function).unwrap_or_default();
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"polygone serverless request v1");
        hasher.update(&(self.id.len() as u64).to_be_bytes());
        hasher.update(self.id.as_bytes());
        hasher.update(&(function.len() as u64).to_be_bytes());
        hasher.update(&function);
        hasher.update(&(self.payload.len() as u64).to_be_bytes());
        hasher.update(&self.payload);
        hasher.update(&[self.threshold, self.total]);
        *hasher.finalize().as_bytes()
    }
}

/// Reconstruit la charge utile à partir d'au moins `threshold` fragments.
///
/// Vérifie, dans cet ordre : nombre de fragments, cohérence du seuil annoncé,
/// unicité du hash de contenu, hash du secret reconstruit, lisibilité de
/// l'enveloppe. Chaque échec a son message — pas de « erreur inconnue ».
pub fn reassemble(shards: &[Shard], threshold: u8) -> Result<Vec<u8>> {
    if threshold < 2 {
        bail!("seuil {threshold} invalide : un seuil de 1 ne protège rien");
    }
    if shards.len() < threshold as usize {
        bail!(
            "il faut {threshold} fragments pour reconstruire, reçu {}",
            shards.len()
        );
    }

    let first = shards
        .first()
        .ok_or_else(|| anyhow!("aucun fragment à reconstruire"))?;

    for shard in shards {
        if shard.threshold != threshold {
            bail!(
                "fragment {} : seuil {} incohérent avec le seuil {threshold}",
                shard.index,
                shard.threshold
            );
        }
        if shard.content_hash != first.content_hash {
            bail!(
                "fragments incohérents : le fragment {} vient d'une autre requête",
                shard.index
            );
        }
    }

    let shares: Vec<shamir::Fragment> = shards
        .iter()
        .map(|shard| shamir::Fragment {
            id: shamir::FragmentId(shard.index),
            data: shard.payload.clone(),
        })
        .collect();

    let secret = shamir::reconstruct(&shares, threshold)
        .map_err(|e| anyhow!("reconstruction Shamir impossible : {e}"))?;

    let actual = hash_bytes(&secret);
    if actual != first.content_hash {
        bail!(
            "contenu reconstruit corrompu : hash attendu {}…, obtenu {}…",
            hex::encode(&first.content_hash[..4]),
            hex::encode(&actual[..4])
        );
    }

    let envelope: PayloadEnvelope = serde_json::from_slice(&secret)
        .context("enveloppe serverless illisible après reconstruction")?;

    Ok(envelope.payload)
}

fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

fn random_id() -> String {
    let bytes: [u8; 16] = rand::random();
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ServerlessRequest {
        ServerlessRequest::new(
            FunctionKind::Shell {
                command: "echo ok".into(),
            },
            b"charge utile".to_vec(),
        )
    }

    #[test]
    fn quatre_fragments_sur_sept_reconstruisent() {
        let request = request();
        let shards = request.shards().unwrap();
        assert_eq!(shards.len(), DEFAULT_TOTAL as usize);

        let recovered =
            reassemble(&shards[..DEFAULT_THRESHOLD as usize], DEFAULT_THRESHOLD).unwrap();
        assert_eq!(recovered, request.payload);
    }

    #[test]
    fn trois_fragments_ne_suffisent_pas() {
        let request = request();
        let shards = request.shards().unwrap();
        assert!(reassemble(&shards[..3], DEFAULT_THRESHOLD).is_err());
    }

    #[test]
    fn fragment_altere_rejete() {
        let request = request();
        let mut shards = request.shards().unwrap();

        // Corruption d'un octet du partage : la reconstruction donne autre
        // chose, le hash de contenu doit le voir.
        shards[0].payload[0] ^= 0xFF;

        assert!(reassemble(&shards[..DEFAULT_THRESHOLD as usize], DEFAULT_THRESHOLD).is_err());
    }

    #[test]
    fn fragments_portent_des_index_distincts_et_le_meme_hash() {
        let request = request();
        let shards = request.shards().unwrap();

        let mut indexes: Vec<u8> = shards.iter().map(|s| s.index).collect();
        indexes.sort_unstable();
        indexes.dedup();
        assert_eq!(indexes.len(), shards.len());

        let first_hash = shards[0].content_hash;
        assert!(shards.iter().all(|s| s.content_hash == first_hash));
    }

    #[test]
    fn validation_refuse_un_total_superieur_au_protocole() {
        let mut request = request();
        request.total = 8;
        request.threshold = 4;
        assert!(request.validate().is_err());

        request.total = 3;
        request.threshold = 4;
        assert!(request.validate().is_err());
    }

    #[test]
    fn empreinte_change_avec_la_charge_utile() {
        let a = request();
        let mut b = request();
        b.payload = b"autre chose".to_vec();
        assert_ne!(a.request_hash(), b.request_hash());
    }

    #[test]
    fn fragments_de_deux_requetes_ne_se_melangent_pas() {
        let a = request();
        let mut b = request();
        b.payload = b"autre chose".to_vec();

        let mut mixed = a.shards().unwrap();
        let other = b.shards().unwrap();
        mixed[0] = other[0].clone();

        assert!(reassemble(&mixed[..DEFAULT_THRESHOLD as usize], DEFAULT_THRESHOLD).is_err());
    }
}
