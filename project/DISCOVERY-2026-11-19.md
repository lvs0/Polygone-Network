# Discovery et analyse d'écarts — mission prototype complet

Date : 2026-11-19. Dépôt : `lvs0/Polygone-Network`, branche `main`, HEAD `5252e76`.
Arbre propre. CI verte (run 44585430237). Release v2.1.0 publiée.
Méthode : ASD-STE100. Une idée par phrase. Voix active. Aucun secret.

Ce document enregistre la Phase 1 (Discovery) et la Phase 2 (Gap Analysis).
Il ne réécrit aucune décision produit. Il signale les conflits et choisit le
chemin le moins destructif.

---

## 1. Carte des sources de vérité

Priorité appliquée : décisions owner > vision produit > architecture/sécurité >
contrats API > implémentation vérifiée > tests/preuves > historique > propositions.

| Document | Objet | Date | Autorité | État | Implémentation conforme |
|---|---|---|---|---|---|
| `product/PRODUCT-FOUNDATION.md` | vision, promesse, 6 espaces, limites | 09/11 (maj git 13/11) | Vision produit | Courant (vision) | Oui pour la vision. §2/§5.3 « état » périmé (voir conflit 1 et 2) |
| `docs/P0-COMPUTE-SECURITY-2026-11-14.md` | modèle sécurité compute, niveaux L0–L3, sandbox | 14/11 | Sécurité | Courant (modèle) | Oui : L0 défaut, L1 opt-in, cgroup+RLIMIT_AS, fail-closed. §5/§6 « deux machines bloqué » périmé (conflit 1) |
| `docs/proofs/P0-DEUX-HOTES-2026-11-19.md` | preuve deux hôtes Hope↔Cue | 19/11 | Preuve vérifiée | Courant | Oui : 8 sections PASS, logs, artefacts ML-DSA signés, L1 recalculé |
| `docs/decisions/ADR-0001-base-canonique-et-reconciliation.md` | base canonique | 19/11 | Décision | Courant | Oui |
| `docs/adr/0001-authentification-du-relais.md` | relais authentifié ML-DSA-65 | 19/11 | Sécurité | Courant | Oui : HELLO signé, challenge, refus relais nu |
| `docs/decisions/ADR-0002-identite-noeud-cle-signature.md` | node_id = BLAKE3(ctx‖ML-DSA-65) | 19/11 | Décision | Courant | Oui |
| `docs/decisions/ADR-0003-preuve-l1-persistee.md` | preuve L1 persistée `polygone.evidence.v1` | 19/11 | Décision | Courant | Oui : `~/.polygone/evidence-*.json` |
| `docs/decisions/ADR-0004-reconciliation-github-main.md` | github/main réconcilié | 19/11 | Décision | Courant | Oui : PR #4 fusionnée |
| `docs/architecture/API-CONTRACTS.md` | contrats API, §2.4 routes produit, §8 domaines | 19/11 | Contrats API | Courant | Oui : §2.4 livré (PR #6), 6 routes + ledger + events |
| `ARCHITECTURE.md` | structure crates, LOC | 19/11 | Architecture | Courant | Oui : 105 fichiers / 45 853 lignes / 612 tests re-mesurés |
| `STATE.md`, `project/STATUS.md` §13 | état courant | 19/11 | État | Courant | Oui |
| `product/PRODUCT-READINESS.md`, `product/UI-API-MAPPING.md` | readiness, mapping écrans | 19/11 | Produit | Courant | Oui |
| `project/FOUNDATION-ROADMAP.md`, `project/AUTONOMOUS_EXECUTION.md` | tickets F1/F2 | 19/11 | Plan | Courant | Oui |
| `docs/P0-DEUX-MACHINES.md`, `P0-RAPPORT.md`, `MISSION-P0-*`, `PLAN_*`, `letters/`, `atlas/` | historique sept–nov | 09–14/11 | Historique | Référencé | N/A — ne pas réimplémenter |
| PR #3 `fix/ci-rustfmt` (+6411/−492, 29 fichiers) | fmt/clippy lignée octobre | 15/10 | Historique | Superseded | main fmt/clippy déjà verts. **[OWNER-DECISION]** recommandation : fermer, ne pas fusionner |

### Conflits enregistrés

**Conflit 1 — état « deux machines physiques ».**
`PRODUCT-FOUNDATION` §2 et `P0-COMPUTE-SECURITY` §5/§6 (14/11) disent « pas de
test / bloqué ». La preuve datée `P0-DEUX-HOTES-2026-11-19.md` + ADR-0003 disent
PASS sur Hope↔Cue. Résolution : la preuve vérifiée (priorité 6) l'emporte sur
l'état historique (priorité 7). Chemin le moins destructif : note datée dans les
sections périmées. Le modèle de sécurité 14/11 reste intact.

**Conflit 2 — état « pas d'API de tâches ni d'événements ».**
`PRODUCT-FOUNDATION` §3/§5.3 (09/11) vs gateway tasks + events livrés le 19/11
(PR #6, STATUS §13, contrats §2.4). Résolution : implémentation vérifiée +
STATUS courant l'emportent. Note datée. La vision et les limites restent intactes.

**Conflit 3 — données simulées dans l'interface.**
`web/mesh.js` contient des pairs fictifs (dell-e6540, raspi-4, macbook-air,
phone-ble) avec métriques inventées. `web/drive.js` se déclare « local
simulation » avec fichiers fictifs. Règle produit : aucune donnée fictive,
aucun faux appareil connecté. Résolution : rebrancher `mesh` sur le backend
réel ; `drive` est hors des 6 espaces produit → étiquette « démo non connectée »
explicite, pas de faux statut. Pas de nouveau backend drive (hors parcours cœur).

---

## 2. Analyse d'écarts (parcours §8 / critères §16)

| Élément | État | Preuve / source | Écart restant |
|---|---|---|---|
| A. Installation | WORKING | `install.sh` + release v2.1.0 testée 19/11 | — |
| B. Premier lancement | PARTIAL | `index.html` (app shell), `landing.html` | Pas d'explication guidée (ce que c'est, ce qui reste local, pairer, partager, pause) |
| C. Identité locale | WORKING | CLI `identite` + `GET /identity` sans secret (test contrat) | — |
| D. Pairing | WORKING | CLI TOFU `peers.json` + gateway QR/token `/pair/*` ; preuve 19/11 §1/§6 | Confirmer le flux UI de pairing |
| E. Réseau réel | WORKING | Relais authentifié ML-DSA, E2E ML-KEM ; preuve 19/11 §2 ; `/network/status`, `/nodes/ping` | Mode = relais (pas de P2P direct). Étiqueter honnêtement. LAN/NAT non prouvé |
| F. Partage ressources | WORKING | Daemon 133 tests, bail 60 s, pause = arrêt processus servant + intention | Contrôle pause absent de l'UI (route gateway à ajouter) |
| G. Exécution distante | WORKING via CLI ; **MISSING via produit** | `net::borrow_compute` + sandbox + E2E, preuve 19/11 §3 ; gateway tasks = exécuteur local seul | **Pont gateway → `borrow_compute` + ciblage pair dans l'UI** (écart central) |
| H. Vérification résultat | WORKING | L0 défaut, L1 opt-in `--verifier-local`, preuve persistée ADR-0003 ; preuve 19/11 §7 | Reporter le niveau dans l'UI sans surclaim : « signé » ≠ « vérifié » |
| I. Dashboard | PARTIAL | 3 écrans prototype (node/compute/activity) + shell | mesh/drive fictifs → réel/étiqueté ; tâches distantes + pause + mode réseau à intégrer |
| J. Récupération erreur | PARTIAL | `borrow_compute` fail-closed (EXIT=1, preuve 19/11 §5) | Surfacer timeout/déconnexion dans l'UI tasks |

### Fait technique clé pour le cycle 1

`net`, `exec`, `identity`, `msg` sont des modules **privés du binaire**
(`main.rs`), absents de `lib.rs`. Leurs dépendances (`polygone_core::crypto`,
`anyhow`, `tokio`) sont déjà dans le `Cargo.toml` du client. Les exposer en
`pub mod` dans `lib.rs` permet au gateway de réutiliser le chemin compute
**prouvé** sans le réécrire. `identity` et `exec` sont autonomes ; `net` dépend
de `identity` + `msg` ; `msg` dépend de `polygone_core`. Aucun déplacement de
fichier nécessaire.

`NodeInfo` (gateway) porte l'id relais (16 hex) + clé de signature, **pas la clé
KEM complète**. Le compute distant utilise donc `peers.json` (ancres TOFU
`node_id → clé KEM`) via `net::load_peers`, même magasin de confiance que le CLI.
Un seul magasin de confiance compute. Cohérent.

---

## 3. Plan de cycles (≤ 6 après discovery)

1. **Pont compute distant gateway.** Exposer `net/exec/identity/msg` en lib.
   Route `POST /api/v1/tasks` avec cible pair → `borrow_compute` → reçu avec
   niveau de vérification. Tests contrat (relais réel en boucle locale).
2. **Interface.** compute : cible local/pair + niveau de vérif honnête.
   mesh : pairs réels. drive : étiquette démo. node : bouton pause.
   Premier lancement : panneau d'explication court.
3. **Contrôle pause + récupération erreur.** Route pause/reprendre gateway
   (réutilise `pause.rs` déjà en lib). Timeout/déconnexion → état failed, pas de
   faux succès.
4. **Preuve deux hôtes Hope↔Cue du nouveau chemin.** Build sur Cue (glibc 2.39),
   ghost B sur Cue, gateway A sur Hope, tâche distante L1, artefacts persistés.
   Si SSH Cue reste en check mode : preuve datée 19/11 + preuve protocole deux
   nœuds sur Hope, et blocage documenté.
5. **Gates + revue sécurité + notes datées + rapport final.** fmt/clippy/tests
   workspace, security-check, smoke, contrats §8, revue §11, rapport §17.

Recherche externe §7 : leçons d'ingénierie only (Iroh, libp2p, Syncthing, Akash,
Ollama, NIST FIPS 203/204). Transport actuel prouvé sur deux hôtes : pas de
remplacement sans preuve mesurée. Pas de nouvelle dépendance par mimétisme.
Crypto actuelle : `pqcrypto-mlkem 0.1` (ML-KEM, FIPS 203), `pqcrypto-mldsa 0.1`
(ML-DSA, FIPS 204). Alignement NIST à confirmer en cycle 5.

---

## 4. Limites connues (ne pas surclaim)

- LAN/NAT traversal non prouvé. Le chemin réel passe par le relais (Tailscale
  entre hôtes). L'interface doit dire « via relais », pas « connexion directe ».
- Preuve deux hôtes physique : datée 19/11 (Hope↔Cue). Rejeu ce soir dépend de
  l'accès SSH Cue (check mode Tailscale intermittent).
- L2 (quorum) hors scope : second opérateur requis. L1 = recalcul local, même
  source que l'exécution distante → indépendance limitée, documentée ADR-0003.
- Drive hors des 6 espaces produit : démo étiquetée, pas de backend réel.
- Historique de consommation `F1-USG` : ledger usage présent, écran à confirmer.