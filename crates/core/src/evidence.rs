//! Preuve canonique `polygone.evidence.v1` — primitive pure, sans E/S.
//!
//! Ce module fixe **ce qu'est une preuve** et **comment deux preuves se
//! comparent**. Il ne lit aucun fichier, n'écrit rien, ne signe rien et
//! n'ouvre aucun socket. La persistance est portée par le client
//! (`net::persist_l1_evidence`, ADR-0003) : la vérification L1 de
//! `--verifier-local` émet une preuve canonique sur disque, accord comme
//! divergence. Le module reste pur ; l'E/S vit ailleurs.
//!
//! **Pourquoi.** `net::annotate_local_recomputation` (L1) compare deux sorties
//! en mémoire et annote un grant. Tant qu'une preuve n'a pas de forme
//! canonique, deux opérateurs ne peuvent pas comparer leurs mesures (L2) :
//! chacun produit un JSON différent et un désaccord ne se distingue pas d'un
//! artefact de sérialisation. Ce module donne un octet-à-octet comparable.
//!
//! **Ce que ce module ne décide pas** :
//! - l'attribution de faute (producteur / demandeur / partagée) : décision
//!   humaine, à porter par un ADR si un quorum est engagé ;
//! - la persistance : tranchée par `docs/decisions/ADR-0003-preuve-l1-persistee.md`
//!   (`~/.polygone/evidence-*.json`, écriture atomique 0600, un fichier par vérification) ;
//! - le transport : aucun champ de ce module n'entre dans une enveloppe, et
//!   `docs/architecture/API-CONTRACTS.md` n'est pas modifié par ce module ;
//! - les niveaux de preuve : le mapping méthode → L0/L1/L2 appartient à
//!   `docs/testing/TEST-STRATEGY.md`, pas au type.
//!
//! **Canonique** : `serde_json::to_vec` sur une structure à champs ordonnés,
//! avec `deny_unknown_fields` ; le digest est `BLAKE3` de ces octets. Deux
//! preuves identiques produisent le même JSON et le même digest, sur toute
//! machine et toute version de la bibliothèque.

use blake3::Hasher;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Version du schéma. Toute rupture incrémente la version et ne réutilise pas
/// ce nom.
pub const EVIDENCE_SCHEMA_V1: &str = "polygone.evidence.v1";
/// Longueur maximale d'un identifiant de sujet mesuré.
pub const MAX_SUBJECT_ID_LEN: usize = 80;
/// Longueur maximale d'un producteur de preuve (identifiant de nœud ou label).
pub const MAX_PRODUCER_LEN: usize = 128;
/// Longueur maximale d'un label d'hôte.
pub const MAX_HOST_LEN: usize = 64;
/// Longueur d'un digest hexadécimal BLAKE3-256.
pub const DIGEST_HEX_LEN: usize = 64;

/// Erreurs de construction, de validation et de lecture d'une preuve.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvidenceError {
    #[error("schéma inconnu : {found} (attendu {EVIDENCE_SCHEMA_V1})")]
    SchemaMismatch { found: String },
    #[error("JSON invalide : {0}")]
    MalformedJson(String),
    #[error("champ vide ou blanc : {field}")]
    EmptyField { field: &'static str },
    #[error("champ trop long : {field} ({size} octets, maximum {max})")]
    FieldTooLong {
        field: &'static str,
        size: usize,
        max: usize,
    },
    #[error("caractère de contrôle interdit : {field}")]
    ControlCharacter { field: &'static str },
    #[error("digest invalide : {field} (64 hexadécimaux minuscules attendus)")]
    InvalidHash { field: &'static str },
    #[error("digest nul : {field} ne peut pas être un BLAKE3 de contenu")]
    ZeroHash { field: &'static str },
    #[error("horodatage nul : provenance.measured_at doit être > 0")]
    ZeroTimestamp,
    #[error("sérialisation impossible : {0}")]
    Serialization(String),
}

/// Nature de ce qui est mesuré.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// Sortie d'une tâche déterministe (`echo`, `blake3`).
    TaskOutput,
    /// Inventaire de ressources d'un hôte.
    InventorySnapshot,
    /// Preuve de vie (challenge/réponse), sans mesure de charge.
    Liveness,
}

/// Méthode qui a produit la mesure. C'est elle qui distingue un recalcul local
/// d'une observation signée ; deux méthodes différentes ne se comparent pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceMethod {
    /// Le demandeur réexécute la même tâche dans le même sandbox (L1).
    LocalRecompute,
    /// Le producteur répond à un challenge à nonce et signe sa réponse.
    SignedChallenge,
    /// Un opérateur tiers rapporte une mesure (candidat L2, pas un quorum).
    OperatorAttestation,
}

/// Ce qui est mesuré, indépendamment de qui le mesure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSubject {
    /// Identifiant de la tâche ou du sujet (nœud, hôte) mesuré.
    pub subject_id: String,
}

/// Engagements de la mesure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commitments {
    /// Digest publié **avant** la mesure, s'il existe. `None` signifie « aucun
    /// engagement antérieur » : la comparaison le dira, elle ne le devinera pas.
    pub committed_blake3: Option<String>,
    /// Digest de ce qui a été observé pendant cette mesure.
    pub observed_blake3: String,
}

/// Qui a mesuré, quand, et par quel procédé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub method: EvidenceMethod,
    /// Secondes depuis l'epoch Unix.
    pub measured_at: u64,
    /// Producteur de la preuve (nœud ou label d'opérateur).
    pub producer: String,
    /// Hôte d'exécution, si connu. Un conteneur sur le même noyau n'est pas un
    /// second hôte (voir `TEST-STRATEGY.md` §1).
    pub host: Option<String>,
}

/// Preuve versionnée. Sérialisée en JSON canonique, digest BLAKE3-256.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceV1 {
    pub schema: String,
    pub kind: EvidenceKind,
    pub subject: EvidenceSubject,
    pub commitments: Commitments,
    pub provenance: Provenance,
}

/// Résultat d'une comparaison entre deux preuves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOutcome {
    /// Même sujet, même méthode, même sortie observée.
    Match,
    /// Même sujet et même méthode, mais les mesures se contredisent.
    Divergent,
    /// La comparaison n'a pas de sens : on ne conclut pas.
    Inconclusive,
}

/// Ce qui s'est passé, en vocabulaire fermé. Aucune chaîne libre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonReason {
    SameSubjectSameOutput,
    OutputDiffers,
    SubjectDiffers,
    KindDiffers,
    MethodDiffers,
    CommittedHashDisagrees,
    CommitmentUnavailable,
}

/// Comparaison typée : un résultat et sa raison, jamais l'un sans l'autre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Comparison {
    pub outcome: ComparisonOutcome,
    pub reason: ComparisonReason,
}

fn validate_text(field: &'static str, value: &str, max: usize) -> Result<(), EvidenceError> {
    if value.trim().is_empty() {
        return Err(EvidenceError::EmptyField { field });
    }
    if value.chars().count() > max {
        return Err(EvidenceError::FieldTooLong {
            field,
            size: value.chars().count(),
            max,
        });
    }
    if value.chars().any(|c| c.is_control()) {
        return Err(EvidenceError::ControlCharacter { field });
    }
    Ok(())
}

fn validate_digest(field: &'static str, value: &str) -> Result<(), EvidenceError> {
    let is_lower_hex = value.len() == DIGEST_HEX_LEN
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !is_lower_hex {
        return Err(EvidenceError::InvalidHash { field });
    }
    if value.bytes().all(|b| b == b'0') {
        return Err(EvidenceError::ZeroHash { field });
    }
    Ok(())
}

impl EvidenceV1 {
    /// Construit une preuve valide. Le schéma est toujours
    /// [`EVIDENCE_SCHEMA_V1`] : il ne se choisit pas à l'appel.
    pub fn new(
        kind: EvidenceKind,
        subject: EvidenceSubject,
        commitments: Commitments,
        provenance: Provenance,
    ) -> Result<Self, EvidenceError> {
        let evidence = Self {
            schema: EVIDENCE_SCHEMA_V1.to_string(),
            kind,
            subject,
            commitments,
            provenance,
        };
        evidence.validate()?;
        Ok(evidence)
    }

    /// Vérifie le schéma, les champs textuels et les digests.
    pub fn validate(&self) -> Result<(), EvidenceError> {
        if self.schema != EVIDENCE_SCHEMA_V1 {
            return Err(EvidenceError::SchemaMismatch {
                found: self.schema.clone(),
            });
        }
        validate_text(
            "subject.subject_id",
            &self.subject.subject_id,
            MAX_SUBJECT_ID_LEN,
        )?;
        validate_text(
            "provenance.producer",
            &self.provenance.producer,
            MAX_PRODUCER_LEN,
        )?;
        if let Some(host) = &self.provenance.host {
            validate_text("provenance.host", host, MAX_HOST_LEN)?;
        }
        if self.provenance.measured_at == 0 {
            return Err(EvidenceError::ZeroTimestamp);
        }
        validate_digest(
            "commitments.observed_blake3",
            &self.commitments.observed_blake3,
        )?;
        if let Some(committed) = &self.commitments.committed_blake3 {
            validate_digest("commitments.committed_blake3", committed)?;
        }
        Ok(())
    }

    /// Octets JSON canoniques : ordre des champs fixé par la structure,
    /// séparateurs compacts, aucun champ inconnu.
    pub fn canonical_json(&self) -> Result<Vec<u8>, EvidenceError> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|e| EvidenceError::Serialization(e.to_string()))
    }

    /// Digest BLAKE3-256 des octets canoniques, en hexadécimal minuscule.
    pub fn digest_hex(&self) -> Result<String, EvidenceError> {
        let bytes = self.canonical_json()?;
        let mut hasher = Hasher::new();
        hasher.update(&bytes);
        Ok(hasher.finalize().to_hex().to_string())
    }

    /// Vrai si `expected_hex` est le digest de cette preuve. Un digest mal
    /// formé renvoie `false` : on ne lève pas, on ne devine pas.
    pub fn digest_matches(&self, expected_hex: &str) -> bool {
        match self.digest_hex() {
            Ok(actual) => actual == expected_hex,
            Err(_) => false,
        }
    }

    /// Relit une preuve depuis un JSON, en refusant un schéma inconnu, un champ
    /// inconnu et tout champ invalide.
    pub fn from_json(bytes: &[u8]) -> Result<Self, EvidenceError> {
        let parsed: Self = serde_json::from_slice(bytes)
            .map_err(|e| EvidenceError::MalformedJson(e.to_string()))?;
        parsed.validate()?;
        Ok(parsed)
    }

    /// Compare deux preuves sans conclure au-delà de ce qu'elles disent.
    ///
    /// - sujet, nature ou méthode différents ⇒ `Inconclusive` : comparer
    ///   n'aurait pas de sens, et un désaccord ne prouverait rien ;
    /// - engagements présents et différents ⇒ `Divergent` ;
    /// - un seul engagement présent ⇒ `Inconclusive` (on ne peut pas vérifier) ;
    /// - sorties observées différentes ⇒ `Divergent` ;
    /// - sinon ⇒ `Match`.
    pub fn compare(&self, other: &Self) -> Comparison {
        let c = |outcome, reason| Comparison { outcome, reason };

        if self.kind != other.kind {
            return c(
                ComparisonOutcome::Inconclusive,
                ComparisonReason::KindDiffers,
            );
        }
        if self.subject != other.subject {
            return c(
                ComparisonOutcome::Inconclusive,
                ComparisonReason::SubjectDiffers,
            );
        }
        if self.provenance.method != other.provenance.method {
            return c(
                ComparisonOutcome::Inconclusive,
                ComparisonReason::MethodDiffers,
            );
        }
        match (
            &self.commitments.committed_blake3,
            &other.commitments.committed_blake3,
        ) {
            (Some(left), Some(right)) if left != right => {
                return c(
                    ComparisonOutcome::Divergent,
                    ComparisonReason::CommittedHashDisagrees,
                );
            }
            (Some(_), None) | (None, Some(_)) => {
                return c(
                    ComparisonOutcome::Inconclusive,
                    ComparisonReason::CommitmentUnavailable,
                );
            }
            _ => {}
        }
        if self.commitments.observed_blake3 != other.commitments.observed_blake3 {
            return c(
                ComparisonOutcome::Divergent,
                ComparisonReason::OutputDiffers,
            );
        }
        c(
            ComparisonOutcome::Match,
            ComparisonReason::SameSubjectSameOutput,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HASH: &str = "abababababababababababababababababababababababababababababababab";

    fn subject() -> EvidenceSubject {
        EvidenceSubject {
            subject_id: "t-1".to_string(),
        }
    }

    fn commitments() -> Commitments {
        Commitments {
            committed_blake3: None,
            observed_blake3: SAMPLE_HASH.to_string(),
        }
    }

    fn provenance(producer: &str) -> Provenance {
        Provenance {
            method: EvidenceMethod::LocalRecompute,
            measured_at: 1_763_000_000,
            producer: producer.to_string(),
            host: None,
        }
    }

    fn sample() -> EvidenceV1 {
        EvidenceV1::new(
            EvidenceKind::TaskOutput,
            subject(),
            commitments(),
            provenance("node-a"),
        )
        .expect("preuve valide")
    }

    #[test]
    fn canonical_json_is_stable_and_ordered() {
        let expected = concat!(
            r#"{"schema":"polygone.evidence.v1","kind":"task_output","#,
            r#""subject":{"subject_id":"t-1"},"#,
            r#""commitments":{"committed_blake3":null,"observed_blake3":"#,
            r#""abababababababababababababababababababababababababababababababab"},"#,
            r#""provenance":{"method":"local_recompute","measured_at":1763000000,"#,
            r#""producer":"node-a","host":null}}"#
        );
        let bytes = sample().canonical_json().expect("json");
        assert_eq!(String::from_utf8(bytes.clone()).unwrap(), expected);

        let mut hasher = Hasher::new();
        hasher.update(expected.as_bytes());
        assert_eq!(
            sample().digest_hex().expect("digest"),
            hasher.finalize().to_hex().to_string(),
            "le digest doit porter sur les octets canoniques"
        );
    }

    #[test]
    fn digest_is_reproducible_across_constructions() {
        assert_eq!(
            sample().digest_hex().unwrap(),
            sample().digest_hex().unwrap()
        );
        assert!(sample().digest_matches(&sample().digest_hex().unwrap()));
        assert!(!sample().digest_matches("00"));
    }

    #[test]
    fn new_always_uses_the_v1_schema() {
        assert_eq!(sample().schema, EVIDENCE_SCHEMA_V1);
    }

    #[test]
    fn rejects_blank_subject_id() {
        let mut s = subject();
        s.subject_id = "   ".to_string();
        let err = EvidenceV1::new(
            EvidenceKind::TaskOutput,
            s,
            commitments(),
            provenance("node-a"),
        )
        .unwrap_err();
        assert_eq!(
            err,
            EvidenceError::EmptyField {
                field: "subject.subject_id"
            }
        );
    }

    #[test]
    fn rejects_control_character_in_producer() {
        let err = EvidenceV1::new(
            EvidenceKind::Liveness,
            subject(),
            commitments(),
            provenance("node\na"),
        )
        .unwrap_err();
        assert_eq!(
            err,
            EvidenceError::ControlCharacter {
                field: "provenance.producer"
            }
        );
    }

    #[test]
    fn rejects_uppercase_or_short_digest() {
        let mut c = commitments();
        c.observed_blake3 = SAMPLE_HASH.to_uppercase();
        assert_eq!(
            EvidenceV1::new(EvidenceKind::TaskOutput, subject(), c, provenance("node-a"))
                .unwrap_err(),
            EvidenceError::InvalidHash {
                field: "commitments.observed_blake3"
            }
        );

        let mut short = commitments();
        short.observed_blake3 = "ab".to_string();
        assert_eq!(
            EvidenceV1::new(
                EvidenceKind::TaskOutput,
                subject(),
                short,
                provenance("node-a")
            )
            .unwrap_err(),
            EvidenceError::InvalidHash {
                field: "commitments.observed_blake3"
            }
        );
    }

    #[test]
    fn rejects_zero_digest() {
        let mut c = commitments();
        c.observed_blake3 = "0".repeat(DIGEST_HEX_LEN);
        assert_eq!(
            EvidenceV1::new(EvidenceKind::TaskOutput, subject(), c, provenance("node-a"))
                .unwrap_err(),
            EvidenceError::ZeroHash {
                field: "commitments.observed_blake3"
            }
        );
    }

    #[test]
    fn rejects_zero_timestamp() {
        let mut p = provenance("node-a");
        p.measured_at = 0;
        assert_eq!(
            EvidenceV1::new(EvidenceKind::TaskOutput, subject(), commitments(), p).unwrap_err(),
            EvidenceError::ZeroTimestamp
        );
    }

    #[test]
    fn from_json_rejects_unknown_schema() {
        let json = String::from_utf8(sample().canonical_json().unwrap())
            .unwrap()
            .replace("polygone.evidence.v1", "polygone.evidence.v2");
        assert_eq!(
            EvidenceV1::from_json(json.as_bytes()).unwrap_err(),
            EvidenceError::SchemaMismatch {
                found: "polygone.evidence.v2".to_string()
            }
        );
    }

    #[test]
    fn from_json_rejects_unknown_fields() {
        let json = String::from_utf8(sample().canonical_json().unwrap())
            .unwrap()
            .replace(
                r#""kind":"task_output","#,
                r#""kind":"task_output","extra":1,"#,
            );
        match EvidenceV1::from_json(json.as_bytes()).unwrap_err() {
            EvidenceError::MalformedJson(_) => {}
            other => panic!("attendu MalformedJson, obtenu {other:?}"),
        }
    }

    #[test]
    fn from_json_round_trips() {
        let bytes = sample().canonical_json().unwrap();
        assert_eq!(EvidenceV1::from_json(&bytes).unwrap(), sample());
    }

    #[test]
    fn compare_matches_same_subject_and_output() {
        let comparison = sample().compare(&sample());
        assert_eq!(comparison.outcome, ComparisonOutcome::Match);
        assert_eq!(comparison.reason, ComparisonReason::SameSubjectSameOutput);
    }

    #[test]
    fn compare_accepts_two_producers_of_the_same_subject() {
        // Deux opérateurs distincts mesurent le même sujet : c'est le cas L2.
        let other = EvidenceV1::new(
            EvidenceKind::TaskOutput,
            subject(),
            commitments(),
            provenance("node-b"),
        )
        .unwrap();
        assert_eq!(sample().compare(&other).outcome, ComparisonOutcome::Match);
    }

    #[test]
    fn compare_diverges_on_output() {
        let mut c = commitments();
        c.observed_blake3 = "cd".repeat(32);
        let other =
            EvidenceV1::new(EvidenceKind::TaskOutput, subject(), c, provenance("node-b")).unwrap();
        let comparison = sample().compare(&other);
        assert_eq!(comparison.outcome, ComparisonOutcome::Divergent);
        assert_eq!(comparison.reason, ComparisonReason::OutputDiffers);
    }

    #[test]
    fn compare_is_inconclusive_on_other_subject_kind_or_method() {
        let mut s = subject();
        s.subject_id = "t-2".to_string();
        let other_subject = EvidenceV1::new(
            EvidenceKind::TaskOutput,
            s,
            commitments(),
            provenance("node-a"),
        )
        .unwrap();
        assert_eq!(
            sample().compare(&other_subject).reason,
            ComparisonReason::SubjectDiffers
        );

        let other_kind = EvidenceV1::new(
            EvidenceKind::Liveness,
            subject(),
            commitments(),
            provenance("node-a"),
        )
        .unwrap();
        assert_eq!(
            sample().compare(&other_kind).reason,
            ComparisonReason::KindDiffers
        );

        let mut p = provenance("node-a");
        p.method = EvidenceMethod::SignedChallenge;
        let other_method =
            EvidenceV1::new(EvidenceKind::TaskOutput, subject(), commitments(), p).unwrap();
        assert_eq!(
            sample().compare(&other_method).reason,
            ComparisonReason::MethodDiffers
        );
    }

    #[test]
    fn compare_reports_commitment_disagreement() {
        let mut c = commitments();
        c.committed_blake3 = Some("11".repeat(32));
        let a =
            EvidenceV1::new(EvidenceKind::TaskOutput, subject(), c, provenance("node-a")).unwrap();
        let mut c2 = commitments();
        c2.committed_blake3 = Some("22".repeat(32));
        let b = EvidenceV1::new(
            EvidenceKind::TaskOutput,
            subject(),
            c2,
            provenance("node-b"),
        )
        .unwrap();
        let comparison = a.compare(&b);
        assert_eq!(comparison.outcome, ComparisonOutcome::Divergent);
        assert_eq!(comparison.reason, ComparisonReason::CommittedHashDisagrees);
    }

    #[test]
    fn compare_without_both_commitments_is_inconclusive() {
        let mut c = commitments();
        c.committed_blake3 = Some("11".repeat(32));
        let with_commitment =
            EvidenceV1::new(EvidenceKind::TaskOutput, subject(), c, provenance("node-a")).unwrap();
        let comparison = with_commitment.compare(&sample());
        assert_eq!(comparison.outcome, ComparisonOutcome::Inconclusive);
        assert_eq!(comparison.reason, ComparisonReason::CommitmentUnavailable);
    }
}
