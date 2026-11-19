//! Acceptation du résultat par quorum.
//!
//! Une requête part vers `total` nœuds ; chacun exécute la requête entière et
//! renvoie sa sortie (et, s'il le souhaite, un reçu signé ML-DSA-65). Le
//! résultat n'est accepté que si au moins `threshold` nœuds **distincts**
//! produisent exactement la même sortie.
//!
//! `tally` ne vérifie pas les signatures : il ne connaît pas les clés publiques
//! des nœuds. C'est le rôle de [`verify_receipts`], à qui l'appelant fournit le
//! moyen de résoudre un `NodeId` en clé publique. L'ordre recommandé est
//! `verify_receipts` **puis** `tally` : on ne compte pas les votes qu'on n'a pas
//! vérifiés.

use std::collections::{HashMap, HashSet};

use polygone_core::receipt::Receipt;
use polygone_core::{NodeId, Verifier};
use serde::{Deserialize, Serialize};

/// Ce qu'un nœud a répondu à une requête serverless.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeOutcome {
    /// Nœud qui a répondu.
    pub node: NodeId,
    /// Fragment qu'il avait reçu, pour tracer la dispersion.
    pub shard_index: u8,
    /// Sortie brute de la fonction.
    pub output: Vec<u8>,
    /// Reçu signé, non vérifié ici.
    pub receipt: Option<Receipt>,
    /// Durée d'exécution annoncée par le nœud.
    pub elapsed_ms: u64,
}

impl NodeOutcome {
    /// BLAKE3 de la sortie — la seule chose comparée entre nœuds.
    pub fn output_hash(&self) -> [u8; 32] {
        *blake3::hash(&self.output).as_bytes()
    }
}

/// Un groupe de nœuds ayant produit la même sortie.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumGroup {
    /// Hash de la sortie partagée par le groupe.
    pub output_hash: [u8; 32],
    /// Nœuds du groupe, dans l'ordre d'arrivée.
    pub nodes: Vec<NodeId>,
}

/// Verdict d'un tour d'exécution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuorumVerdict {
    /// Au moins `threshold` nœuds distincts sont d'accord : la sortie est retenue.
    Agreed {
        /// Sortie retenue.
        output: Vec<u8>,
        /// Son hash.
        output_hash: [u8; 32],
        /// Nœuds qui l'ont produite.
        signers: Vec<NodeId>,
    },
    /// Assez de nœuds ont répondu pour trancher, mais aucun groupe n'atteint le seuil.
    Disagreement {
        /// Tous les groupes, du plus nombreux au moins nombreux.
        groups: Vec<QuorumGroup>,
    },
    /// Pas assez de nœuds distincts pour décider quoi que ce soit.
    Insufficient {
        /// Seuil requis.
        needed: usize,
        /// Nœuds distincts ayant répondu.
        got: usize,
    },
}

/// Compte les votes et décide si le résultat est accepté.
///
/// Règles, dans cet ordre :
/// 1. un nœud ne vote qu'une fois (première réponse retenue) ;
/// 2. les sorties sont groupées par hash ;
/// 3. un groupe de `threshold` nœuds distincts ou plus l'emporte ;
/// 4. sinon, si assez de nœuds distincts ont répondu, c'est un désaccord ;
/// 5. sinon, c'est insuffisant.
pub fn tally(outcomes: Vec<NodeOutcome>, threshold: usize) -> QuorumVerdict {
    let needed = threshold.max(1);

    let mut seen: HashSet<NodeId> = HashSet::new();
    let mut groups: HashMap<[u8; 32], QuorumGroup> = HashMap::new();
    let mut outputs: HashMap<[u8; 32], Vec<u8>> = HashMap::new();

    for outcome in outcomes {
        if !seen.insert(outcome.node) {
            continue;
        }
        let hash = outcome.output_hash();
        groups
            .entry(hash)
            .or_insert_with(|| QuorumGroup {
                output_hash: hash,
                nodes: Vec::new(),
            })
            .nodes
            .push(outcome.node);
        outputs.entry(hash).or_insert(outcome.output);
    }

    let mut ordered: Vec<QuorumGroup> = groups.into_values().collect();
    ordered.sort_by(|a, b| {
        b.nodes
            .len()
            .cmp(&a.nodes.len())
            .then_with(|| a.output_hash.cmp(&b.output_hash))
    });

    if let Some(best) = ordered.first() {
        if best.nodes.len() >= needed {
            let output = outputs.get(&best.output_hash).cloned().unwrap_or_default();
            return QuorumVerdict::Agreed {
                output,
                output_hash: best.output_hash,
                signers: best.nodes.clone(),
            };
        }
    }

    let distinct = seen.len();
    if distinct >= needed && !ordered.is_empty() {
        return QuorumVerdict::Disagreement { groups: ordered };
    }

    QuorumVerdict::Insufficient {
        needed,
        got: distinct,
    }
}

/// Ne garde que les sorties dont le reçu ML-DSA-65 est valide **et porte sur
/// cette sortie**.
///
/// Un reçu ne prouve pas qu'un nœud a bien exécuté la fonction : il prouve
/// qu'il a signé `(sender, receiver, ts, payload_hash)`. Deux contrôles sont
/// donc faits en plus de la signature :
/// - `receipt.sender == outcome.node` — un nœud ne peut pas présenter le reçu
///   d'un autre ;
/// - `receipt.payload_hash == BLAKE3(outcome.output)` — un reçu valide pour une
///   autre sortie est rejeté, sinon n'importe qui recyclerait une signature.
///
/// `resolve` rend la clé publique du nœud (`None` = inconnu, donc rejeté).
/// Les reçus absents sont écartés : le quorum doit se calculer sur des votes
/// vérifiables.
pub fn verify_receipts<F>(outcomes: Vec<NodeOutcome>, resolve: F) -> Vec<NodeOutcome>
where
    F: Fn(&NodeId) -> Option<Verifier>,
{
    outcomes
        .into_iter()
        .filter(|outcome| {
            let Some(receipt) = outcome.receipt.as_ref() else {
                return false;
            };
            if receipt.sender != outcome.node {
                return false;
            }
            if receipt.payload_hash != outcome.output_hash() {
                return false;
            }
            match resolve(&outcome.node) {
                Some(verifier) => receipt.verify(&verifier),
                None => false,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(node: NodeId, output: &[u8]) -> NodeOutcome {
        NodeOutcome {
            node,
            shard_index: 1,
            output: output.to_vec(),
            receipt: None,
            elapsed_ms: 0,
        }
    }

    /// Un reçu correct signé par la clé du nœud est conservé ; une sortie sans
    /// reçu ne l'est pas. Un vote non vérifié ne doit pas compter.
    #[test]
    fn verify_receipts_keeps_only_signed_outcomes() {
        let keys = polygone_core::sign::generate_keypair().expect("keypair");
        let node = NodeId::random();
        let output = b"resultat".to_vec();

        let mut receipt = Receipt {
            sender: node,
            receiver: NodeId::random(),
            ts: 1,
            payload_hash: Receipt::hash_payload(&output),
            signature: Vec::new(),
        };
        receipt.sign(&keys.signer);

        let signed = NodeOutcome {
            node,
            shard_index: 0,
            output: output.clone(),
            receipt: Some(receipt),
            elapsed_ms: 1,
        };
        let anonymous = outcome(NodeId::random(), b"resultat");

        let verifier = keys.verifier.clone();
        let kept = verify_receipts(vec![signed, anonymous], |id| {
            if *id == node {
                Some(verifier.clone())
            } else {
                None
            }
        });

        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].node, node);
    }

    /// Un reçu valide pour une autre sortie ne doit pas blanchir un résultat
    /// modifié : c'est le scénario du reçu recyclé.
    #[test]
    fn verify_receipts_rejects_receipt_for_another_output() {
        let keys = polygone_core::sign::generate_keypair().expect("keypair");
        let node = NodeId::random();
        let signed_output = b"resultat".to_vec();

        let mut receipt = Receipt {
            sender: node,
            receiver: NodeId::random(),
            ts: 1,
            payload_hash: Receipt::hash_payload(&signed_output),
            signature: Vec::new(),
        };
        receipt.sign(&keys.signer);

        let forged = NodeOutcome {
            node,
            shard_index: 0,
            output: b"autre chose".to_vec(),
            receipt: Some(receipt),
            elapsed_ms: 1,
        };

        let verifier = keys.verifier.clone();
        let kept = verify_receipts(vec![forged], |_| Some(verifier.clone()));
        assert!(kept.is_empty());
    }

    #[test]
    fn accord_avec_quatre_noeuds_distincts() {
        let outcomes = vec![
            outcome(NodeId::random(), b"ok"),
            outcome(NodeId::random(), b"ok"),
            outcome(NodeId::random(), b"ok"),
            outcome(NodeId::random(), b"ok"),
            outcome(NodeId::random(), b"different"),
        ];

        match tally(outcomes, 4) {
            QuorumVerdict::Agreed {
                output, signers, ..
            } => {
                assert_eq!(output, b"ok".to_vec());
                assert_eq!(signers.len(), 4);
            }
            other => panic!("attendu Agreed, obtenu {other:?}"),
        }
    }

    #[test]
    fn trois_identiques_ne_suffisent_pas() {
        let outcomes = vec![
            outcome(NodeId::random(), b"ok"),
            outcome(NodeId::random(), b"ok"),
            outcome(NodeId::random(), b"ok"),
        ];

        assert_eq!(
            tally(outcomes, 4),
            QuorumVerdict::Insufficient { needed: 4, got: 3 }
        );
    }

    #[test]
    fn un_noeud_ne_vote_qu_une_fois() {
        let node = NodeId::random();
        let outcomes: Vec<NodeOutcome> = (0..5).map(|_| outcome(node, b"ok")).collect();

        assert_eq!(
            tally(outcomes, 4),
            QuorumVerdict::Insufficient { needed: 4, got: 1 }
        );
    }

    #[test]
    fn desaccord_liste_les_groupes() {
        let outcomes = vec![
            outcome(NodeId::random(), b"a"),
            outcome(NodeId::random(), b"a"),
            outcome(NodeId::random(), b"b"),
            outcome(NodeId::random(), b"b"),
        ];

        match tally(outcomes, 4) {
            QuorumVerdict::Disagreement { groups } => {
                assert_eq!(groups.len(), 2);
                assert_eq!(groups[0].nodes.len(), 2);
                assert_eq!(groups[1].nodes.len(), 2);
            }
            other => panic!("attendu Disagreement, obtenu {other:?}"),
        }
    }

    #[test]
    fn quatre_sorties_differentes_sont_un_desaccord() {
        let outcomes = vec![
            outcome(NodeId::random(), b"a"),
            outcome(NodeId::random(), b"b"),
            outcome(NodeId::random(), b"c"),
            outcome(NodeId::random(), b"d"),
        ];

        match tally(outcomes, 4) {
            QuorumVerdict::Disagreement { groups } => assert_eq!(groups.len(), 4),
            other => panic!("attendu Disagreement, obtenu {other:?}"),
        }
    }

    #[test]
    fn aucun_vote_est_insuffisant() {
        assert_eq!(
            tally(Vec::new(), 4),
            QuorumVerdict::Insufficient { needed: 4, got: 0 }
        );
    }
}
