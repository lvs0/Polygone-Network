# ETAT-REEL — ce que le code fait, prouvé

> Consolidation de `AUDIT_VERACITE_VISION.md` (2 nov. 2026) et des vérifications
> du même jour. Règle : une fonction définie ou testée isolément ne vaut pas
> intégration produit. Il faut un appelant de production.
> **Ce fichier est la seule source autorisée pour le README et le site.**

## 1. Score

| Verdict | Nombre |
|---|---:|
| **VRAI** — présent et appelé | 11 |
| **FAUX / NON CÂBLÉ** | 18 |
| **INDÉTERMINÉ** | 4 |
| **Total** | 33 |

## 2. Vrai (avec preuve)

| # | Capacité | Preuve |
|---:|---|---|
| 1 | Transport de messages chiffrés et signés (ML-KEM-1024, AES-256-GCM, Shamir 4/7, ML-DSA-65) | `crates/client/src/net/mod.rs:198-290`, `:650-860` |
| 2 | Relais TCP aveugle (route sans lire le payload, refuse l'usurpation de `from`) | `crates/relay/src/relay.rs:14-27`, `:165-230` |
| 3 | Exécution shell distante sandboxée systemd | `net/mod.rs:601-620`, `exec.rs:105-189` |
| 4 | Exécution WASM (`wasmi`, fuel + plafonds) | `exec.rs:36-103` |
| 5 | IA locale via Ollama | `crates/client/src/petals.rs:87-144` |
| 6 | PWA installable + coque hors ligne | `web/pwa.js`, `web/sw.js` |
| 7 | Gateway HTTP sur socket TCP réel | `crates/gateway/tests/serve.rs:53-173` |
| 8 | Hide mono-saut (SOCKS5 via relais) | `crates/client/src/hide.rs:263-265` |
| 9 | État local du nœud de stockage | `crates/client/src/node.rs:435-508` |
| 10 | Destruction locale manuelle (`duress --confirmer`) | `crates/client/src/duress.rs:37-80` |
| 11 | Licence + canal de divulgation sécurité | `LICENSE`, `SECURITY.md`, `.well-known/security.txt` |

**Ajout du 2 novembre, vérifié après l'audit :** isolation `--isoler` réelle
sous systemd/cgroup v2 (`memory.max=268435456`, `cpu.max=100000 100000`, scope
déchargé après SIGINT, aucun cgroup résiduel) ; IPC daemon `0600` ; smokes
`compute-smoke.sh`, `forensic-drive.sh`, mesh LAN.

## 3. Faux ou non câblé

| # | Promesse | Réalité |
|---:|---|---|
| 1 | Serverless exécute sur plusieurs nœuds | `serverless::run` reconstitue localement et écrit qu'aucun nœud n'a été contacté |
| 2 | Quorum Serverless en production | `tally`/`verify_receipts` existent, aucun appel CLI |
| 3 | Petals distribue les couches | `PipelinePlan` sans consommateur hors tests |
| 4 | Réputation choisit les pairs | Score affiché, aucun sélecteur ne l'utilise |
| 5 | Kill-switch automatique | Seulement `duress` manuel |
| 6 | Déclencheurs USB/clavier/GPIO | Aucune occurrence dans le code |
| 7 | Téléphone comme nœud | PWA = interface + cache |
| 8 | Pause batterie / réseau mobile | Aucune API batterie |
| 9 | Statut Gateway = état réseau | Table en mémoire, aucun heartbeat relay |
| 10 | Latence mesurée en production | `latency_ms` reste `None` hors tests |
| 11 | Comptes / login / multi-appareils | Identité locale seulement |
| 12 | Appairage par jeton porteur | MVP : 256 bits, TTL 5 min, usage unique — mais pas d'authentification mutuelle, HTTP sans TLS |
| 13 | Dashboard global messages/fichiers | Compte seulement blocs/octets du nœud |
| 14 | Gateway HTTPS publique intégrée | HTTP loopback uniquement |
| 15 | Déploiement automatique Pages/Vercel/CF | Aucun workflow de déploiement |
| 16 | Site généré depuis les docs | Fichiers statiques |
| 17 | « Logs locaux uniquement, pas de serveur central » | **Trompeur** : le relais journalise les IP source |
| 18 | Bug bounty rémunéré | `LEGAL.md:79` dit explicitement le contraire |

## 4. Indéterminé

1. Déploiement public / TLS effectif (aucun workflow, hébergeur non inspecté).
2. Interopérabilité entre **deux machines physiques** (tests locaux seulement).
3. Installation PWA testée sur iOS/Android (aucun résultat appareil).
4. Objectifs de disponibilité, latence, 100 000 utilisateurs (aucun benchmark).

## 5. Contrat de ressources — l'écart central

| Ce qui est calculé | Ce qui est appliqué |
|---|---|
| Politique d'allocation (CPU, RAM, réseau) | `nice` seulement, **sans `--isoler`** |
| Bail avec durée, heartbeat, révocation | Bail = engagement, pas contrainte OS |
| Bande passante estimée et plafonnée en policy | Pas de shaping réseau effectif |
| Activité utilisateur | `Unknown` sur Linux/macOS (volontairement conservateur) |
| Capacité stockage | `0` dans le snapshot |

**Phrase autorisée :** « le daemon mesure, calcule et propose ; avec `--isoler`
sous Linux, la limite est appliquée et vérifiée par cgroup v2. Sans `--isoler`,
le consommateur respecte lui-même son plafond. »

**Phrase interdite :** « Polygone limite automatiquement les ressources
partagées sur toutes les plateformes. »

## 6. Matrice de support

| Plateforme | Compilation | Tests | Isolation OS | Statut autorisé |
|---|---|---|---|---|
| Linux | oui | 333 | `--isoler` systemd/cgroup v2 | **Supporté** |
| macOS | oui | partiels | aucune | **Expérimental** |
| Windows | portage | non testé runner | aucune | **Non supporté** |

## 7. Réseau : ce qui est prouvé et ce qui ne l'est pas

- **Prouvé :** deux processus locaux, loopback, relay TCP, mesh LAN local,
  transfert de fichier comparé octet à octet, compute sandboxé.
- **Non prouvé :** deux hôtes physiques sur LAN réel, deux réseaux distincts,
  NAT, perte de paquets, reconnexion, pair malveillant, replay, analyse de
  trafic, multi-hop Hide.
- **Conséquence :** le mot « réseau décentralisé » est autorisé en architecture,
  pas en démonstration publique.

## 8. Divergence Git (bloquant)

| Élément | Valeur |
|---|---|
| HEAD local | `f3d09c0` (29 oct. 2026) |
| `origin/main` | `23d27ac` (11 sept. 2026) |
| Commits locaux absents du remote | 6 |
| Commits distants absents du local | 17 |
| Fichiers non commités | 65 |

**Aucun merge, reset, clean ou push automatique.** La réconciliation se fait
commit par commit, sur une branche de sauvegarde.

## 9. Comment rejouer ces preuves

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
git diff --check
bash scripts/compute-smoke.sh
bash scripts/forensic-drive.sh
```

Les résultats doivent être cités avec leur date et la machine. Un test vert
d'hier ne prouve rien sur le code d'aujourd'hui.
