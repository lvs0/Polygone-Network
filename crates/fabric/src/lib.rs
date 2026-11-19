//! Polygone Fabric — la tranche verticale du calcul bidirectionnel.
//!
//! Ce crate relie quatre capacités qui existaient séparément :
//!
//! 1. un **modèle de nœud** avec deux périmètres (`local` / `global`), un
//!    consentement explicite et des ressources mesurées ou déclarées ;
//! 2. un **planificateur déterministe** qui refuse de déporter une tâche quand
//!    le transfert coûte plus que l'exécution locale, et qui ne choisit jamais
//!    un nœud hors périmètre ou sans consentement ;
//! 3. un **exécuteur** qui ne déclare un succès qu'après vérification du
//!    [`polygone_core::task::TaskResult`] signé (ML-DSA-65) et qui réessaie un
//!    autre nœud après un échec ;
//! 4. un **apprentissage local** (EWMA par nœud) qui ajuste le risque sans
//!    jamais appliquer une connaissance collective non vérifiée.
//!
//! Le stockage distribué vit dans [`store`] : c'est un magasin adressé par
//! contenu et un plan de réplication. **Ce n'est pas Babel.** Les sources
//! historiques ne contiennent aucune spécification de Babel (seulement une
//! référence à la bibliothèque de Babel) et `ADR-014` garde cette décision
//! ouverte. Le module est donc nommé pour ce qu'il fait, et ne revendique ni
//! compression, ni stockage infini, ni le nom du concept non retrouvé.

pub mod engine;
pub mod learning;
pub mod model;
pub mod planner;
pub mod store;
pub mod transport;

pub use engine::{ExecutionOutcome, FabricEngine, ObservationRecord, Transport, TransportError};
pub use learning::{CollectiveProposal, LearningError, LearningStore, NodeHealth};
pub use model::{NodeCapabilities, Scope, TaskEnvelope, WIRE_VERSION};
pub use planner::{Placement, PlacementPlan, PlacementPolicy, RemoteEstimate};
pub use store::{chunk_data, reconstruct, replica_plan, BlockId, ContentManifest, ReplicaPlan};
pub use transport::{serve, MemoryNode, MemoryTransport, ServerNode, TcpTransport};

use std::time::{SystemTime, UNIX_EPOCH};

/// Secondes Unix, sans panique si l'horloge est antérieure à l'époque.
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
