//! Polygone Serverless — exécuter une fonction sur le réseau.
//!
//! ## Ce que ce crate fait réellement
//!
//! Deux mécanismes distincts, et il faut les tenir séparés pour rester honnête :
//!
//! 1. **Transit — Shamir 4-of-7 réel.** La requête (identifiant + charge utile)
//!    est sérialisée, puis découpée par le partage de secret de `polygone-core`
//!    en `total` fragments dont `threshold` suffisent à reconstruire. Chaque
//!    fragment porte le hash BLAKE3 de l'enveloppe complète : un fragment isolé
//!    ne dit rien, et un fragment modifié est rejeté à la reconstruction.
//! 2. **Calcul — quorum, pas reconstruction.** Une fonction ne se calcule pas
//!    sur une part de secret : `f(part) ≠ f(secret)`. La requête est donc
//!    envoyée à `total` nœuds, chacun exécute **la requête entière**, et le
//!    résultat n'est accepté que si `threshold` nœuds **distincts** renvoient
//!    exactement la même sortie (même hash BLAKE3). Un nœud ne vote qu'une fois.
//!
//! ## Limites, dites noir sur blanc
//!
//! - Le quorum est une garantie de **cohérence entre nœuds**, pas une preuve de
//!   correction : `threshold` nœuds malveillants qui s'accordent sur un faux
//!   résultat l'emportent. `tally` ne vérifie aucune signature : il ne connaît
//!   pas les clés publiques. [`verify_receipts`] fait ce contrôle avant le
//!   décompte, quand l'appelant sait résoudre un `NodeId` en clé publique — un
//!   reçu ne prouve d'ailleurs que la signature, pas la correction du calcul.
//! - Aucune exécution distante n'est branchée ici : ce crate est le modèle, les
//!   types et les règles d'acceptation. Le transport vers les nœuds fantômes est
//!   le protocole `compute` existant, côté client.
//! - Le découpage Shamir porte sur la requête en transit, jamais sur la sortie.
//!
//! ```text
//! requête ──Shamir──► fragments ──relais aveugle──► nœuds (requête entière)
//!                                                        │
//!                    sortie acceptée ◄── quorum ≥ 4/7 ◄──┘
//! ```

mod quorum;
mod request;

pub use quorum::{tally, verify_receipts, NodeOutcome, QuorumGroup, QuorumVerdict};
pub use request::{
    reassemble, FunctionKind, ServerlessRequest, Shard, DEFAULT_THRESHOLD, DEFAULT_TOTAL,
};

#[cfg(test)]
mod end_to_end {
    use super::*;
    use polygone_core::NodeId;

    /// Une requête traverse les trois étages : découpage, reconstruction,
    /// puis acceptation du résultat par quorum. Aucun réseau, aucun fichier.
    #[test]
    fn requete_complete_de_bout_en_bout() {
        let request = ServerlessRequest::new(
            FunctionKind::Deterministic {
                name: "blake3".into(),
            },
            b"entree".to_vec(),
        );
        assert_eq!(request.threshold, DEFAULT_THRESHOLD);
        assert_eq!(request.total, DEFAULT_TOTAL);

        let shards = request.shards().unwrap();
        let recovered =
            reassemble(&shards[..DEFAULT_THRESHOLD as usize], request.threshold).unwrap();
        assert_eq!(recovered, request.payload);

        // Quatre nœuds rendent la même sortie : le résultat est accepté.
        let mut outcomes = Vec::new();
        for index in 0..DEFAULT_THRESHOLD {
            outcomes.push(NodeOutcome {
                node: NodeId::random(),
                shard_index: shards[index as usize].index,
                output: b"sortie".to_vec(),
                receipt: None,
                elapsed_ms: u64::from(index),
            });
        }

        match tally(outcomes, DEFAULT_THRESHOLD as usize) {
            QuorumVerdict::Agreed {
                output,
                output_hash,
                signers,
            } => {
                assert_eq!(output, b"sortie".to_vec());
                assert_eq!(output_hash, *blake3::hash(b"sortie").as_bytes());
                assert_eq!(signers.len(), DEFAULT_THRESHOLD as usize);
            }
            other => panic!("attendu Agreed, obtenu {other:?}"),
        }
    }

    /// Un seul nœud qui répond quatre fois ne fait pas un quorum.
    #[test]
    fn un_seul_noeud_ne_fait_pas_un_quorum() {
        let node = NodeId::random();
        let outcomes: Vec<NodeOutcome> = (0..DEFAULT_THRESHOLD)
            .map(|index| NodeOutcome {
                node,
                shard_index: index,
                output: b"sortie".to_vec(),
                receipt: None,
                elapsed_ms: 0,
            })
            .collect();

        assert_eq!(
            tally(outcomes, DEFAULT_THRESHOLD as usize),
            QuorumVerdict::Insufficient {
                needed: DEFAULT_THRESHOLD as usize,
                got: 1,
            }
        );
    }
}
