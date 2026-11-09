# SALVAGE — registre de fusion des 15 versions

> Chaque ligne dit d'où vient un élément, s'il est **gardé** ou **rejeté**, et
> pourquoi. « Rejeté » ne veut pas dire « supprimé du disque » : les originaux
> restent dans `Polygone-Atlas/versions/`. Cela veut dire **exclu de la version
> réalignée**, avec une raison écrite.

## 1. Socle de la version réalignée

| Source | Élément | Décision | Raison |
|---|---|---|---|
| `04-projects-polygone-real` | Workspace Rust complet (7 crates) | **GARDÉ — socle** | Seul code vivant, compilable, 333 tests |
| `04` | `daemon/` (snapshot, GlowUp, bail, ledger, IPC) | **GARDÉ** | Base de la couche ressources |
| `04` | `docs/VISION_CANONIQUE.md`, `AUDIT_VERACITE_VISION.md`, `FEATURE_SPEC`, `IMPL_PLAN`, `STAGING.md` | **GARDÉ** | Meilleure vérité documentaire du corpus |
| `04` | `scripts/` (smokes compute, drive, mesh, hide) | **GARDÉ** | Preuves reproductibles existantes |
| `04` | `install.sh`, `.github/workflows/` | **GARDÉ** | Installateur et CI déjà durcis |
| `04` | `archive/2026-07-src/` (monolithe libp2p) | **REJETÉ** | Explicitement « not built, not tested, not the product » |
| `04` | `graphify-out/`, `.claude-flow/`, `.freebuff-tasks/`, `.agents/` | **REJETÉ** | Sous-produits d'outillage, 13 Mo de bruit |
| `04` | `index.html.bak-pre-brand`, `*.bak` | **REJETÉ** | Sauvegardes obsolètes |

## 2. Apports ciblés d'autres versions

### Depuis `02-projets-polygone` (ligne produit v1, MIT, Ed25519)

| Fichier | Décision | Raison |
|---|---|---|
| `HISTOIRE.md` | **GARDÉ** | Seule narration continue de l'origine et de la métaphore de la vague |
| `MSH_SPEC.md` | **GARDÉ** | Spécification du mesh, utile pour la synchronisation sans temps |
| `DESIGN_PHILOSOPHY.md` | **GARDÉ** | Vocabulaire de conception encore valide |
| `ECOSYSTEM_MAP.md` | **GARDÉ** | Carte des 8 modules historiques, base de `VISION.md` §2 |
| `ALGORITHM.md` | **GARDÉ** | Description d'algorithme, à confronter au code actuel |
| `FICHE_TECHNIQUE.md` | **GARDÉ** | Résumé technique réutilisable pour un partenariat |
| `GATEWAY.md` | **GARDÉ** | Antécédent du Gateway, utile pour l'historique des décisions |
| `MANIFESTO.md` | **GARDÉ (archive)** | Ton d'origine ; ne pas republier tel quel (promesses non tenues) |
| `legal-guide-france.md` | **GARDÉ** | Contexte juridique FR, utile avant release |
| `business-model.md` | **REJETÉ du plan** | Conservé comme trace ; contredit `VISION.md` §6 |
| `tokenomics.md` | **REJETÉ du plan** | Jeton explicitement reporté |
| Le code Rust v1 (11 crates, Ed25519) | **REJETÉ** | Cryptographie en retard, architecture remplacée |

### Depuis `03-projects-polygone-network` (snapshot du remote)

| Élément | Décision | Raison |
|---|---|---|
| `crates/polygone-bench/` + `REPORT.md` | **GARDÉ — à porter** | **Seul banc d'essai chiffré du corpus** (p50/p95/p99, 500/500, chi²). Absent du canonique. Placé dans `salvage/`, hors workspace, car il cible l'ancienne API |
| Le reste des crates | **REJETÉ** | Doublon plus ancien du canonique |

### Depuis `05-projects-polygone-v2` (tentative orpheline)

| Élément | Décision | Raison |
|---|---|---|
| `AXIOMS.md` | **GARDÉ — à corriger** | Bonne formulation d'axiomes ; les axiomes 3 et 7 sont faux et doivent être réécrits |
| `LEGAL.md` | **GARDÉ** | Cadre légal, complémentaire du guide FR |
| `THREAT_MODEL.md` | **REJETÉ** | Prétend « aucune menace non mitigée » : auto-satisfait, donc dangereux |
| Le code (workspace concurrent) | **REJETÉ** | Doublon orphelin, non testé, API divergente |

### Depuis `13-documentation-polygone` (sources de vision datées)

| Fichier | Décision | Raison |
|---|---|---|
| `00-vision-source-2026-05-10.md` | **GARDÉ** | Message fondateur, source primaire (attention : contenu triplé dans le fichier) |
| `01-glow-up-and-network-expansion-2026-07-18.md` | **GARDÉ** | Spécification ressources/synchronisation la plus précise |
| `VISION-RECONSTRUITE-ET-ETAT-REEL.md` | **GARDÉ** | Synthèse utile, à confronter à `ETAT-REEL.md` |

## 3. Harnais : réutilisés comme modèles, pas comme code

| Source | Élément | Décision | Raison |
|---|---|---|---|
| `10-polygone-company` | Boucle `discover → dispatch → verify → state` | **MODÈLE** | 18 cycles réels ont mesuré 3 causes d'échec (502 `hope/code`, JSON invalide, fichier vide) |
| `10` | `backlog.json` / `backlog.auto.json` | **MODÈLE** | Format de file de travail exploitable |
| `10` | Le code Python et `worktree-product/` | **REJETÉ** | Faux positif prouvé (`reputation-exchange` `done` sans appelant) |
| `11-polygone-team` | Protocole dispatch → preuve → état tri-état, anti-doublon, `killpg`, `repo_busy` | **MODÈLE** | Le plus propre des deux harnais ; 3 tâches réellement prouvées |
| `11` | `queue/`, `state/`, `templates/` | **MODÈLE** | Structure réutilisable pour l'orchestrateur Atlas |
| `14-r-labs-polygone` | Recette Docker multi-stage, heartbeat 120 s / TTL 300 s | **MODÈLE** | Bonne hygiène de nœud conteneurisé ; binaire `zab` externe, lignée ZBS distincte |

## 4. Données : indexées, jamais copiées

| Source | Élément | Décision | Raison |
|---|---|---|---|
| `08-polygone-recovery` | Moteur de mémoire (42 955 messages, FTS5, provenance `content_sha1`) | **INDEXÉ** | Utile pour la continuité ; données personnelles |
| `09-projects-polygone-recovery` | Pipeline par lots mensuels + gabarit 7 sections | **INDEXÉ** | Méthode réutilisable (4 étapes sur 7 vides) |
| `12-polygone-knowledge-graph` | Schéma entités/relations/evidence | **INDEXÉ** | Base de la mémoire long terme ; 1,2 Go, secrets potentiels |
| `15-state-polygone` | `identity.json`, `ledger.json` | **NON COPIÉ** | **Clés privées en clair** ; état vivant |

## 5. Archives pures

| Source | Décision | Raison |
|---|---|---|
| `01-polygone-v1-agents` | **ARCHIVE** | Démon neuron/synapse/cortex avec mesures en dur (35/40) ; un seul mot à garder : le vocabulaire biomimétique |
| `06-polygone-v2-brand` | **ARCHIVE** | Un SVG, réserve de marque, pas une version |
| `07-polygone-resume` | **SOURCE UTILE (non copiée)** | 143 `.md` de reprise, redondants avec `docs/` ; matrice de provenance à consulter, pas à dupliquer |

## 6. Ce que la fusion a changé dans le code

**Aucun fichier Rust n'a été modifié.** La version réalignée est le workspace
canonique tel qu'il était au 2 novembre 2026, plus les documents ci-dessus et
`salvage/`. C'est volontaire : une fusion de code non testée serait exactement
le « faux-fix » que les audits reprochent. Les apports de code (banc d'essai)
sont **placés, documentés, et non branchés** jusqu'à un port vérifié.

## 7. Ce qu'il reste à porter explicitement

1. `polygone-bench` → membre du workspace, adapté à l'API actuelle, avec un
   `cargo bench` reproductible.
2. Réconciliation Git : 6 commits locaux vs 17 distants, **un par un**.
3. Règles d'orchestration de `polygone-company` + `polygone-team` → un seul
   orchestrateur, documenté dans `Polygone-Atlas/04-AGENT/`.
