# TRACEABILITY-2026-11-14 — concepts Polygone → sources → implémentation → état

> **Instantané daté — 2026-11-14.** Cette matrice décrit cette exécution, pas
> l'état courant du dépôt. Voir [`ETAT-REEL.md`](../../ETAT-REEL.md) et
> [`STATE.md`](../../STATE.md).
>
> Matrice de la tranche `polygone-fabric`, 2026-11-14, dépôt
> `~/Polygone-canonical`, HEAD `f6382e6`.
> Elle ne remplace aucune documentation canonique : elle relie des concepts à
> leurs sources et dit, sans complaisance, ce qui existe.

## Légende

| Code | Signification |
|---|---|
| **IT** | Implémenté et testé dans cette tranche |
| **I** | Implémenté mais non testé |
| **P** | Partiellement implémenté (chemin incomplet ou non intégré) |
| **S** | Spécifié uniquement |
| **N** | Non commencé |

## Matrice

| # | Concept | Source historique (chemin) | Implémentation actuelle | État | Écart / prochaine action |
|---|---|---|---|---|---|
| 1 | **Babel** | `Polygone-Atlas/06-PRODUCT-SPEC/02-PRODUCT-ARCHITECTURE/POLYGONE-COMPUTE-SPEC.md` §9 ; `ARCHITECTURE-DECISIONS.md` ADR-014 / `DEC-028` ; `Polygone-Atlas/_work/full-pass-20261107/founder-direct-113.md` (message 182434) ; corpus `_work/corpus/user-55/65/88/90/91/92/93.md` cité par la spec | Aucune implémentation Babel. `crates/fabric/src/store.rs` fournit un CAS BLAKE3, un manifeste, un plan de réplication et une reconstruction, explicitement **hors Babel**. | **N** (Babel) / **IT** (CAS expérimental) | Les sources ne définissent ni structure, ni sharding, ni reconstruction, ni tolérance aux pannes, ni confidentialité, ni coûts. `DEC-028` reste ouverte ; ne rien revendiquer avant décision humaine. |
| 2 | **Calcul résiduel** | Message fondateur 2026-08-22 (`founder-direct-113.md`) ; `VISION.md` §3 et §5 ; `POLYGONE-COMPUTE-SPEC.md` C1–C7 | `planner.rs` : coût local vs transfert + exécution + RTT, marge 900 ‰. `model.rs` : refus selon charge, batterie, température, activité. | **P** | Aucune collecte réelle des ressources inutilisées ; les capacités sont déclarées par la CLI, pas mesurées par le daemon. |
| 3 | **Réseau mesh local** | `Polygone-Atlas/06-PRODUCT-SPEC/02-PRODUCT-ARCHITECTURE/NETWORK-AND-MESH-SPEC.md` §5 ; `crates/client/src/mesh.rs` ; `STAGING.md` | `TcpTransport` avec pairs explicites et `local_fabric`. Aucune découverte UDP. | **N** (dans fabric) / existant **I** (mesh) | Câbler la découverte 7642 à `list_nodes`, avec appairage et séparation local/global. |
| 4 | **Réseau global** | `NETWORK-AND-MESH-SPEC.md` §1 et §3 ; `crates/relay` ; `VISION.md` §3 | `Scope::Global` exige `global_consent` du demandeur et du nœud **distant** ; le nœud local peut exécuter sa propre demande. Aucun partage par défaut. Transport TCP direct, sans relais. | **P** | Pas de découverte globale, pas de NAT traversal, pas de relais, pas de fédération. |
| 5 | **Daemon universel** | `Polygone-Atlas/06-PRODUCT-SPEC/02-PRODUCT-ARCHITECTURE/DAEMON-SPEC.md` ; `daemon/src/policy/glow_up.rs` ; `daemon/src/socket.rs` | Modèle `NodeCapabilities` (CPU/RAM/stockage/VRAM, charge, batterie, température, activité). | **P** | Aucune intégration : pas de bail, pas de mesure réelle, pas de protocole IPC, pas de démarrage système. |
| 6 | **Orchestration des ressources** | `DAEMON-SPEC.md` §1–2 ; `POLYGONE-COMPUTE-SPEC.md` §7 | `FabricEngine` : plan, exclusion du nœud fautif, `max_attempts`, observation de latence. | **P** | Aucune limite OS appliquée ; le bail reste une licence, pas une contrainte. |
| 7 | **Polygone Compute bidirectionnel** | `POLYGONE-COMPUTE-SPEC.md` ; `STAGING.md` (compute live) ; `crates/client` (grant, sandbox, réputation) | `crates/fabric` : enveloppe de tâche, placement, TCP NDJSON, exécution `echo`/`blake3`, résultat signé, failover. | **P** | Ne réutilise pas la sandbox systemd/wasmi ni la réputation existantes. Deux chemins concurrents ; décision d'intégration requise. |
| 8 | **Vérification des résultats** | `crates/core/src/task.rs` (`TaskResult::verify`) ; `POLYGONE-COMPUTE-SPEC.md` C4 | `engine.rs` vérifie hash BLAKE3, signature ML-DSA-65, `NodeId` dérivé, `task_id` ; un résultat faux n'est jamais un succès. | **IT** | Pas de quorum, pas de preuve d'exécution, pas de détection de résultat correct mais malveillant. |
| 9 | **Intelligence collective** | Message fondateur (« le système apprend de tout ») ; aucun module canonique dédié | `learning.rs` : EWMA locale 7/8 ; `CollectiveProposal` acceptée seulement avec ≥ 2 vérificateurs distincts. | **P** | Aucune diffusion réseau, aucune persistance, aucun protocole de consensus ni d'audit. |
| 10 | **Device Reuse** | `salvage/13-documentation-polygone/01-glow-up-and-network-expansion-2026-07-18.md` ; `VISION.md` §5 | Champs de capacités (CPU, RAM, stockage, VRAM, batterie, température, activité). | **P** | Aucun rôle (relais, stockage, supervision), aucune PWA, aucun profil « ancienne machine ». |
| 11 | **Polygone ID** | `crates/core/src/sign.rs` (`node_id_from_sign_pk_hex`) ; `crates/client/src/identity.rs` ; `CHANGELOG.md` (node_id = BLAKE3(sign_pk)) | `FabricEngine::new` vérifie que le `NodeId` local dérive de la clé ML-DSA ; `TaskResult::verify` vérifie le producteur. | **IT** (vérification) / **P** (identité) | Clés éphémères par exécution CLI ; pas de persistance, pas d'appairage, pas de rotation/révocation. |
| 12 | **Polygone Platform / Server** | `checklists/polygone.md` (« Polygone Platform ») ; `crates/gateway` ; `crates/serverless` | `polygone-fabric serve` : serveur TCP de tâches. | **P** (prototype) | Ce n'est pas le gateway HTTP ni l'API produit ; aucun écran, aucune API de supervision. |
| 13 | **Petals** | `crates/petals/src/pipeline.rs` ; `STAGING.md` (planner partiel, poids non distribués) | Non touché. | **N** | Aucun pipeline d'inférence, aucun transfert de poids, aucune parité de sortie. |
| 14 | **Architecture hybride** | `VISION.md` §2 ; `ECOSYSTEM.md` ; `DECISIONS.md` D4 ; `STAGING.md` | Crate séparé `crates/fabric`, binaire `polygone-fabric`. | **P** | Le crate n'est pas une sous-commande de `polygone`. « Un seul produit visible » n'est pas respecté tant qu'il n'est pas intégré ou retiré. |
| 15 | **Expérience utilisateur / PWA** | `VISION.md` §2 ; `docs/` (PWA existante) ; `product/UI-API-MAPPING.md` | Aucun changement d'UI. | **N** | Aucune supervision du placement, du `verified`, de la latence ou des refus. |
| 16 | **Computational Design / Living Network / Generative Engine** | Directive de mission ; aucune source canonique retrouvée dans `Polygone-canonical` | Aucun rendu, aucune animation, aucun WebGPU/WebGL. | **N** | Concepts non spécifiés dans le dépôt ; ne pas inventer une architecture graphique. |
| 17 | **Sécurité commune** | `docs/security/THREAT-MODEL.md` ; `NETWORK-AND-MESH-SPEC.md` §3 ; `crates/core/src/sign.rs` | Signature des résultats, `NodeId` dérivé, séparation local/global, consentement double. | **P** | Pas de TLS, pas d'authentification de la demande, pas d'anti-rejeu, pas de sandbox, pas de révocation. |
| 18 | **Stockage distribué (hors Babel)** | `STORAGE-AND-DRIVE-SPEC.md` §2 (blocs BLAKE3, pas de réplication existante) | `store.rs` : découpage ≤ 1 Mio, manifeste, vérification, reconstruction, plan de réplication à 2 copies. | **IT** (plan local) | Aucun placement réel sur nœuds, aucun transfert, aucune réparation, aucun chiffrement au repos. |

## Sources lues pour cette matrice

- `Polygone-canonical/VISION.md`
- `Polygone-canonical/ETAT-REEL.md`
- `Polygone-canonical/STAGING.md`
- `Polygone-canonical/ECOSYSTEM.md`
- `Polygone-canonical/DECISIONS.md`
- `Polygone-canonical/ARCHITECTURE.md`
- `Polygone-canonical/project/STATUS.md`
- `Polygone-canonical/product/PRODUCT-READINESS.md`
- `Polygone-canonical/docs/testing/VALIDATION-RESULTS.md`
- `Polygone-canonical/crates/core/src/task.rs`
- `Polygone-canonical/crates/core/src/sign.rs`
- `Polygone-canonical/crates/gateway/src/tasks.rs`
- `Polygone-Atlas/06-PRODUCT-SPEC/02-PRODUCT-ARCHITECTURE/POLYGONE-COMPUTE-SPEC.md`
- `Polygone-Atlas/06-PRODUCT-SPEC/02-PRODUCT-ARCHITECTURE/ARCHITECTURE-DECISIONS.md`
- `Polygone-Atlas/06-PRODUCT-SPEC/02-PRODUCT-ARCHITECTURE/DAEMON-SPEC.md`
- `Polygone-Atlas/06-PRODUCT-SPEC/02-PRODUCT-ARCHITECTURE/NETWORK-AND-MESH-SPEC.md`
- `Polygone-Atlas/06-PRODUCT-SPEC/02-PRODUCT-ARCHITECTURE/STORAGE-AND-DRIVE-SPEC.md`
- `Polygone-Atlas/_work/full-pass-20261107/founder-direct-113.md`

## Hypothèses non vérifiées

1. Le terme « Babel » désignerait un adressage par contenu plutôt qu'un
   mécanisme de stockage spécifique. **Non tranché** : `DEC-028` est ouverte.
2. Les valeurs `rtt_ms`, `bandwidth_kbps` et `failure_rate_permille` fournies
   par la CLI seraient représentatives d'un réseau réel. **Non mesuré** : ce sont
   des déclarations de prototype.
3. Le coût d'exécution du modèle (`2 ms + 8 ms/Mio` pour `echo`, `1 ms + 2
   ms/Mio` pour `blake3`) serait proche du coût réel. **Non benchmarké** : c'est
   un ordre de grandeur déterministe, pas une mesure.
4. Le service `compute` existant resterait le chemin produit officiel.
   **Documenté comme live dans `STAGING.md`, non rejoué dans cette session.**
