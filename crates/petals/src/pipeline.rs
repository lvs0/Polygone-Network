//! Planification d'inférence distribuée (pipeline de couches).
//!
//! Ce module répond à une seule question, et il faut être précis sur ce qu'il
//! fait : **où les couches d'un modèle s'exécuteraient** si plusieurs pairs
//! acceptaient d'en porter chacun une tranche, et **à quelle vitesse** le
//! pipeline tournerait avec ces pairs.
//!
//! Ce qu'il ne fait pas, et qu'il serait malhonnête de laisser croire :
//! - il ne transfère aucun poids — les modèles restent sur les machines qui les
//!   ont déjà, ce module ne connaît que des capacités déclarées ;
//! - il n'appelle aucun pair, n'ouvre aucune connexion, ne vérifie aucune
//!   preuve : l'exécution distante n'est pas branchée ;
//! - l'estimation de débit est un modèle de pipeline (somme des temps de
//!   traversée par couche), pas une mesure. Elle sert à comparer deux
//!   affectations, pas à promettre un chiffre.
//!
//! Autrement dit : la découpe est réelle et testée, l'exécution ne l'est pas.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::DeviceType;

/// Nombre de pairs recommandé comme plafond : au-delà, le gain d'un hop
/// supplémentaire ne compense plus son coût de latence. `plan` n'applique pas
/// ce plafond (il couvre le modèle si la capacité existe) ; c'est aux appelants
/// de `plan_with_peers` de le passer s'ils veulent borner le pipeline.
pub const MAX_PEERS: usize = 8;

/// Capacité déclarée d'un pair : combien de couches il peut porter, et à
/// quelle vitesse il génère des tokens quand il tourne seul.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PeerCapacity {
    /// Identifiant du pair (nom lisible ou NodeId sérialisé).
    pub node: String,
    /// Nombre de couches du modèle que ce pair peut héberger.
    pub layers: u32,
    /// RAM disponible pour l'inférence, en Go (informatif pour l'instant).
    pub ram_gb: f32,
    /// Débit mesuré ou déclaré, en tokens par seconde, sur le modèle entier.
    pub tokens_per_second: f32,
    /// Périphérique d'exécution : sert à comparer deux pairs, pas à router.
    pub device: DeviceType,
}

/// Tranche de couches `[from_layer, to_layer)` — borne haute exclue,
/// comme partout ailleurs dans le projet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerRange {
    pub from_layer: u32,
    pub to_layer: u32,
}

impl LayerRange {
    /// Nombre de couches couvertes. Zéro pour une tranche vide.
    pub fn len(&self) -> u32 {
        self.to_layer.saturating_sub(self.from_layer)
    }

    /// Vrai si la tranche ne contient aucune couche.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Affectation d'une tranche de couches à un pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardAssignment {
    pub node: String,
    pub range: LayerRange,
}

/// Plan complet : quelles couches chez qui, et le débit attendu du pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelinePlan {
    pub model: String,
    pub total_layers: u32,
    pub assignments: Vec<ShardAssignment>,
    /// Estimation, pas une mesure : voir la doc du module.
    pub estimated_tokens_per_second: f32,
}

/// Raisons pour lesquelles un plan ne peut pas être construit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// Aucun pair candidat.
    NoPeers,
    /// Modèle sans couche : rien à répartir.
    NoLayers,
    /// Deux pairs portent le même nom — on ne peut pas voter deux fois.
    DuplicateNode(String),
    /// La capacité cumulée ne couvre pas le modèle.
    InsufficientCapacity { needed: u32, available: u32 },
    /// Le plafond de pairs demandé ne permet pas de couvrir le modèle.
    NotEnoughPeers { needed: usize, available: usize },
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::NoPeers => write!(f, "aucun pair candidat"),
            PlanError::NoLayers => write!(f, "le modèle ne déclare aucune couche"),
            PlanError::DuplicateNode(node) => {
                write!(f, "le pair « {node} » apparaît plusieurs fois")
            }
            PlanError::InsufficientCapacity { needed, available } => write!(
                f,
                "capacité insuffisante : {needed} couches à placer, {available} disponibles"
            ),
            PlanError::NotEnoughPeers { needed, available } => write!(
                f,
                "pas assez de pairs : {needed} nécessaires, {available} autorisés"
            ),
        }
    }
}

impl std::error::Error for PlanError {}

/// Étape ordonnée du pipeline, prête à être affichée ou envoyée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineStage {
    /// Position dans le pipeline, à partir de 0.
    pub index: usize,
    pub node: String,
    pub range: LayerRange,
    /// Premier pair du pipeline (reçoit le prompt).
    pub is_first: bool,
    /// Dernier pair du pipeline (produit les tokens).
    pub is_last: bool,
}

/// Construit un plan sans plafond de pairs : si la capacité existe, le modèle
/// est couvert, quel que soit le nombre de pairs que cela demande.
pub fn plan(
    model: &str,
    total_layers: u32,
    peers: &[PeerCapacity],
) -> Result<PipelinePlan, PlanError> {
    plan_with_peers(model, total_layers, peers, peers.len().max(1))
}

/// Construit un plan avec au plus `max_peers` pairs.
///
/// Déterministe : l'ordre d'entrée n'influence pas le résultat. Les pairs sont
/// triés par débit décroissant puis par nom, les plus rapides d'abord, et les
/// tranches sont contiguës, sans trou ni recouvrement, dans l'ordre des couches.
pub fn plan_with_peers(
    model: &str,
    total_layers: u32,
    peers: &[PeerCapacity],
    max_peers: usize,
) -> Result<PipelinePlan, PlanError> {
    if total_layers == 0 {
        return Err(PlanError::NoLayers);
    }
    if peers.is_empty() {
        return Err(PlanError::NoPeers);
    }

    // Un nom en double, c'est deux votes pour la même machine : refusé avant
    // tout le reste, y compris si les doublons n'ont pas de capacité.
    let mut seen: Vec<&str> = Vec::with_capacity(peers.len());
    for peer in peers {
        if seen.contains(&peer.node.as_str()) {
            return Err(PlanError::DuplicateNode(peer.node.clone()));
        }
        seen.push(peer.node.as_str());
    }

    // Ordre stable : débit décroissant, puis nom. Le tri par nom départage les
    // égalités pour que deux machines interchangeables donnent le même plan.
    let mut candidates: Vec<&PeerCapacity> = peers.iter().filter(|p| p.layers > 0).collect();
    candidates.sort_by(|a, b| {
        b.tokens_per_second
            .partial_cmp(&a.tokens_per_second)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.node.cmp(&b.node))
    });

    let total_capacity: u64 = candidates.iter().map(|p| u64::from(p.layers)).sum();

    // Sélection : on prend les plus rapides jusqu'à couvrir le modèle.
    let cap = max_peers.max(1);
    let mut selected: Vec<&PeerCapacity> = Vec::new();
    let mut accumulated: u64 = 0;
    for peer in &candidates {
        if selected.len() >= cap || accumulated >= u64::from(total_layers) {
            break;
        }
        selected.push(*peer);
        accumulated += u64::from(peer.layers);
    }

    if accumulated < u64::from(total_layers) {
        // Deux causes distinctes, et l'utilisateur doit savoir laquelle :
        // le plafond de pairs a coupé la sélection alors que la capacité
        // globale suffisait, ou bien il n'y a réellement pas assez de place.
        return if selected.len() >= cap && total_capacity >= u64::from(total_layers) {
            Err(PlanError::NotEnoughPeers {
                needed: selected.len() + 1,
                available: selected.len(),
            })
        } else {
            Err(PlanError::InsufficientCapacity {
                needed: total_layers,
                available: u32::try_from(total_capacity).unwrap_or(u32::MAX),
            })
        };
    }

    let shares = distribute(total_layers, &selected);
    let assignments = build_assignments(&selected, &shares, total_layers);

    let plan = PipelinePlan {
        model: model.to_string(),
        total_layers,
        estimated_tokens_per_second: estimate_throughput(total_layers, &assignments, &selected),
        assignments,
    };

    validate(&plan)?;
    Ok(plan)
}

/// Répartit `total_layers` entre les pairs sélectionnés, au prorata de leur
/// capacité, avec au moins une couche chacun et un total exactement égal.
///
/// Méthode du plus fort reste : chaque pair reçoit d'abord une couche (le
/// plancher), puis les couches restantes au prorata exact de leur capacité ;
/// les reliquats d'arrondi vont aux plus grands restes, départagés par ordre de
/// sélection pour rester déterministe.
fn distribute(total_layers: u32, selected: &[&PeerCapacity]) -> Vec<u32> {
    let count = selected.len();
    let capacity_sum: u64 = selected.iter().map(|p| u64::from(p.layers)).sum();
    let capacity_sum = capacity_sum.max(1);
    let total = u64::from(total_layers);

    // Une couche garantie par pair ; le reste est proportionnel.
    let remaining = total.saturating_sub(count as u64);
    let mut shares = vec![1u32; count];
    let mut remainders: Vec<(usize, u64)> = Vec::with_capacity(count);
    let mut given = 0u64;

    for (i, peer) in selected.iter().enumerate() {
        let scaled = remaining * u64::from(peer.layers);
        let extra = scaled / capacity_sum;
        shares[i] += u32::try_from(extra).unwrap_or(u32::MAX);
        remainders.push((i, scaled % capacity_sum));
        given += extra;
    }

    // Les couches non attribuées par les parties entières vont aux plus grands
    // restes : c'est ce qui garantit un total exactement égal à `total_layers`.
    let mut leftover = remaining.saturating_sub(given);
    remainders.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut cursor = 0usize;
    while leftover > 0 && !remainders.is_empty() {
        let (index, _) = remainders[cursor % remainders.len()];
        shares[index] += 1;
        leftover -= 1;
        cursor += 1;
    }

    shares
}

/// Transforme les parts en tranches contiguës qui couvrent `0..total_layers`.
fn build_assignments(
    selected: &[&PeerCapacity],
    shares: &[u32],
    total_layers: u32,
) -> Vec<ShardAssignment> {
    let mut assignments = Vec::with_capacity(selected.len());
    let mut cursor = 0u32;
    for (peer, share) in selected.iter().zip(shares.iter()) {
        let from_layer = cursor;
        let to_layer = (cursor + share).min(total_layers);
        cursor = to_layer;
        assignments.push(ShardAssignment {
            node: peer.node.clone(),
            range: LayerRange {
                from_layer,
                to_layer,
            },
        });
    }
    assignments
}

/// Estimation de débit : temps de traversée d'un token = somme, sur les pairs,
/// de la part du modèle qu'ils portent divisée par leur débit solo. Le débit
/// est l'inverse de ce temps. C'est pessimiste (il ignore le recouvrement entre
/// hops) et c'est voulu : mieux vaut sous-promettre.
fn estimate_throughput(
    total_layers: u32,
    assignments: &[ShardAssignment],
    selected: &[&PeerCapacity],
) -> f32 {
    let total = f32::from(u16::try_from(total_layers).unwrap_or(u16::MAX));
    let mut seconds_per_token = 0.0f32;

    for (assignment, peer) in assignments.iter().zip(selected.iter()) {
        let share = f32::from(u16::try_from(assignment.range.len()).unwrap_or(u16::MAX)) / total;
        // Un débit nul ou négatif est traité comme très lent, jamais comme
        // une division par zéro silencieuse.
        let rate = if peer.tokens_per_second > 0.0 {
            peer.tokens_per_second
        } else {
            1.0e-3
        };
        seconds_per_token += share / rate;
    }

    if seconds_per_token > 0.0 {
        1.0 / seconds_per_token
    } else {
        0.0
    }
}

/// Vérifie qu'un plan couvre exactement `0..total_layers`, sans trou ni
/// recouvrement, sans pair en double.
pub fn validate(plan: &PipelinePlan) -> Result<(), PlanError> {
    if plan.total_layers == 0 {
        return Err(PlanError::NoLayers);
    }
    if plan.assignments.is_empty() {
        return Err(PlanError::NoPeers);
    }

    let mut seen: Vec<&str> = Vec::with_capacity(plan.assignments.len());
    let mut covered = 0u32;

    for assignment in &plan.assignments {
        if seen.contains(&assignment.node.as_str()) {
            return Err(PlanError::DuplicateNode(assignment.node.clone()));
        }
        seen.push(assignment.node.as_str());

        // La tranche doit reprendre exactement là où la précédente s'arrête.
        if assignment.range.from_layer != covered {
            return Err(PlanError::InsufficientCapacity {
                needed: plan.total_layers,
                available: covered,
            });
        }
        covered = assignment.range.to_layer;
    }

    if covered != plan.total_layers {
        return Err(PlanError::InsufficientCapacity {
            needed: plan.total_layers,
            available: covered.min(plan.total_layers),
        });
    }

    Ok(())
}

/// Étapes ordonnées d'un plan, avec les drapeaux de bord.
pub fn stages(plan: &PipelinePlan) -> Vec<PipelineStage> {
    let last = plan.assignments.len().saturating_sub(1);
    plan.assignments
        .iter()
        .enumerate()
        .map(|(index, assignment)| PipelineStage {
            index,
            node: assignment.node.clone(),
            range: assignment.range,
            is_first: index == 0,
            is_last: index == last,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(name: &str, layers: u32, rate: f32) -> PeerCapacity {
        PeerCapacity {
            node: name.to_string(),
            layers,
            ram_gb: 8.0,
            tokens_per_second: rate,
            device: DeviceType::Cpu,
        }
    }

    fn three_peers() -> Vec<PeerCapacity> {
        vec![
            peer("a", 12, 30.0),
            peer("b", 12, 20.0),
            peer("c", 12, 10.0),
        ]
    }

    #[test]
    fn couvre_toutes_les_couches_sans_trou() {
        let plan = plan("qwen3-32b", 32, &three_peers()).expect("plan");
        assert_eq!(plan.total_layers, 32);
        assert_eq!(plan.assignments[0].range.from_layer, 0);
        assert_eq!(plan.assignments.last().unwrap().range.to_layer, 32);
        validate(&plan).expect("plan valide");

        let mut cursor = 0;
        for assignment in &plan.assignments {
            assert_eq!(assignment.range.from_layer, cursor);
            assert!(!assignment.range.is_empty());
            cursor = assignment.range.to_layer;
        }
        assert_eq!(cursor, 32);
    }

    #[test]
    fn deterministe_et_independant_de_l_ordre_d_entree() {
        let direct = plan("m", 32, &three_peers()).unwrap();

        let mut permuted = three_peers();
        permuted.reverse();
        let reversed = plan("m", 32, &permuted).unwrap();

        assert_eq!(direct, reversed);
        assert_eq!(plan("m", 32, &three_peers()).unwrap(), direct);
    }

    #[test]
    fn refuse_un_pair_en_double() {
        let peers = vec![peer("a", 8, 10.0), peer("a", 8, 12.0)];
        assert_eq!(
            plan("m", 4, &peers),
            Err(PlanError::DuplicateNode("a".to_string()))
        );
    }

    #[test]
    fn refuse_une_capacite_insuffisante() {
        let peers = vec![peer("a", 2, 10.0), peer("b", 2, 10.0)];
        match plan("m", 32, &peers) {
            Err(PlanError::InsufficientCapacity { needed, available }) => {
                assert_eq!(needed, 32);
                assert_eq!(available, 4);
            }
            other => panic!("attendu InsufficientCapacity, obtenu {other:?}"),
        }
    }

    #[test]
    fn respecte_le_plafond_de_pairs() {
        let plan = plan_with_peers("m", 16, &three_peers(), 2).expect("plan");
        assert_eq!(plan.assignments.len(), 2);
        validate(&plan).expect("plan valide");
        assert_eq!(plan.assignments.last().unwrap().range.to_layer, 16);
    }

    #[test]
    fn plafond_trop_bas_est_refuse() {
        // Capacité globale suffisante (3 × 12 = 36 ≥ 32), mais un seul pair
        // autorisé : c'est bien le plafond qui bloque, pas la capacité.
        let peers = vec![
            peer("a", 12, 10.0),
            peer("b", 12, 10.0),
            peer("c", 12, 10.0),
        ];
        match plan_with_peers("m", 32, &peers, 1) {
            Err(PlanError::NotEnoughPeers { needed, available }) => {
                assert_eq!(available, 1);
                assert_eq!(needed, 2);
            }
            other => panic!("attendu NotEnoughPeers, obtenu {other:?}"),
        }
    }

    #[test]
    fn les_etapes_ont_les_bons_drapeaux() {
        let plan = plan("m", 32, &three_peers()).unwrap();
        let stages = stages(&plan);
        assert_eq!(stages.len(), plan.assignments.len());
        assert!(stages[0].is_first);
        assert!(!stages[0].is_last);
        let mid = &stages[1];
        assert!(!mid.is_first && !mid.is_last);
        assert!(stages.last().unwrap().is_last);
        for (index, stage) in stages.iter().enumerate() {
            assert_eq!(stage.index, index);
        }
    }

    #[test]
    fn valide_refuse_un_recouvrement() {
        let broken = PipelinePlan {
            model: "m".to_string(),
            total_layers: 8,
            assignments: vec![
                ShardAssignment {
                    node: "a".to_string(),
                    range: LayerRange {
                        from_layer: 0,
                        to_layer: 5,
                    },
                },
                ShardAssignment {
                    node: "b".to_string(),
                    range: LayerRange {
                        from_layer: 4,
                        to_layer: 8,
                    },
                },
            ],
            estimated_tokens_per_second: 0.0,
        };
        assert!(validate(&broken).is_err());
    }

    #[test]
    fn un_seul_pair_prend_tout_le_modele() {
        let peers = vec![peer("solo", 40, 12.0)];
        let plan = plan("m", 32, &peers).unwrap();
        assert_eq!(plan.assignments.len(), 1);
        assert_eq!(
            plan.assignments[0].range,
            LayerRange {
                from_layer: 0,
                to_layer: 32
            }
        );
        // Une traversée unique : le débit estimé est celui du pair.
        assert!((plan.estimated_tokens_per_second - 12.0).abs() < 0.001);
    }

    #[test]
    fn l_estimation_suit_le_modele_de_pipeline() {
        // Deux pairs à 100 et 50 tok/s, parts égales : 1 / (0,5/100 + 0,5/50).
        let peers = vec![peer("a", 16, 100.0), peer("b", 16, 50.0)];
        let plan = plan("m", 32, &peers).unwrap();
        assert!((plan.estimated_tokens_per_second - 66.666).abs() < 0.01);
    }

    #[test]
    fn un_modele_sans_couche_est_refuse() {
        assert_eq!(plan("m", 0, &three_peers()), Err(PlanError::NoLayers));
    }

    #[test]
    fn aucun_pair_est_refuse() {
        assert_eq!(plan("m", 8, &[]), Err(PlanError::NoPeers));
    }

    #[test]
    fn les_pairs_sans_capacite_sont_ignores() {
        let peers = vec![peer("a", 0, 100.0), peer("b", 8, 10.0)];
        let plan = plan("m", 8, &peers).unwrap();
        assert_eq!(plan.assignments.len(), 1);
        assert_eq!(plan.assignments[0].node, "b");
    }
}
