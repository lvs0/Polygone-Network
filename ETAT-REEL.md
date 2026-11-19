# ETAT-REEL — ce que le code fait, prouvé

> Consolidation de `AUDIT_VERACITE_VISION.md` (2 nov. 2026) et des vérifications
> du même jour. Règle : une fonction définie ou testée isolément ne vaut pas
> intégration produit. Il faut un appelant de production.
> **Ce fichier est la seule source autorisée pour le README et le site.**

## 1. Score

> **Instantané historique du 2026-11-02** : les nombres 11/18/4 ci-dessous
> sont le score de l'audit de base, pas un recomptage après les lots du 5 novembre.
> Voir l'addendum courant §8 avant toute réutilisation publique.

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
| 10 | Latence mesurée en production | GW.2 écrit `latency_ms` après ping/pong signé sur socket réel ; preuve loopback, pas encore inter-hôtes |
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
| Linux | oui | 432 | `--isoler` systemd/cgroup v2 | **Supporté** |
| macOS | oui | partiels | aucune | **Expérimental** |
| Windows | portage | non testé runner | aucune | **Non supporté** |

## 7. Réseau : ce qui est prouvé et ce qui ne l'est pas

- **Prouvé :** deux processus locaux, loopback, relay TCP, mesh LAN local,
  transfert de fichier comparé octet à octet, compute sandboxé.
- **Prouvé depuis le 2026-11-04/05 :** anti-rejeu et refus de l'usurpation
  `from` à travers le vrai relais sur loopback ; Hide SOCKS5 mono-saut avec
  grant réel et contrôle du contenu observé par le relais.
- **Non prouvé :** deux hôtes physiques sur LAN réel, deux réseaux distincts,
  NAT, perte de paquets, reconnexion inter-hôtes, analyse de trafic, multi-hop Hide.
- **Prouvé le 2026-11-10 (WAN, deux noyaux) :** nœud A sur la machine locale
  (`Linux 7.2.5-3-omarchy`), nœud B dans un conteneur Modal (`4.19.0-gvisor`), relais
  dans le conteneur exposé par un tunnel TCP brut. Message livré par le relais,
  marqueur retrouvé chez B, ancre de confiance apprise
  (`BLAKE3-256 0ea5d646…`), dates de démarrage distinctes (`1794231716` ≠
  `1794347093`). Journal : `docs/proofs/modal-two-host-20261110T214504Z.md`.
  Ce n'est pas deux machines personnelles : le second hôte est virtualisé (gVisor)
  et aucun pair n'est derrière une box, donc le NAT résidentiel reste non prouvé.
- **Prouvé le 2026-11-10 (trois noyaux, deux sauts de WAN) :** A local, relais R et
  nœud B dans **deux conteneurs Modal distincts**. Trois `boot_id` différents
  (`96607c82…`, `1acb281c…`, `99d41715…`), marqueur livré par R, ancre apprise
  (`BLAKE3-256 89e939b6…`). Le relais ne partage plus le noyau du destinataire.
  Journal : `docs/proofs/modal-three-kernels-20261110T215027Z.md`.
- **Prouvé le 2026-11-10 (coupure et reprise) :** le tunnel public est fermé en pleine
  session ; l'écouteur de B tombe (`died=1`), B se réenregistre sur un nouveau tunnel
  avec le **même `node_id`, la même clé KEM et la même ancre de confiance**, et le
  message suivant est livré. Aucun `duplicate` : le slot du pair coupé est
  effectivement libéré, pas conservé. Journal :
  `docs/proofs/modal-reconnect-20261110T215246Z.md`. Non prouvé : la reprise si le
  relais lui-même redémarre (état des slots en mémoire seulement).
- **Mesuré avant correctif le 2026-11-10 (redémarrage du relais) :** R1 tué par SIGKILL,
  R2 relancé sur le même port derrière le même tunnel. Aucun état ne survit chez le relais.
  Côté pair, l'écouteur sortait avec le code 0 et ne se reconnectait pas. Journal historique :
  `docs/proofs/modal-relay-restart-20261110T215706Z.md`.
- **Corrigé et re-mesuré le 2026-11-10 :** `receive_network()` conserve ses buffers, ses
  caches anti-rejeu et ses ancres pendant la reconnexion ; il refait le handshake avec la
  même identité, avec un backoff de 1 à 30 s. Après SIGKILL de R1 et redémarrage de R2 sur
  la même adresse, le même processus B reste vivant (`exit=None`), se réauthentifie
  automatiquement (`auto_reconnected=true`) et reçoit la sonde. L'ancre n'est pas réapprise.
  Journal final : `docs/proofs/modal-relay-restart-20261110T221636Z.md`.
  Régression : `cargo test -p polygone-client --bin polygone net::tests:: -- --test-threads=1`
  — **38 tests, 0 échec** ; `cargo build -p polygone-client --bin polygone` : PASS.
- **Limites restantes :** le relais ne persiste toujours pas ses slots ; ils sont recréés
  quand le pair se réauthentifie. Aucune livraison différée des messages envoyés pendant
  la coupure n'a été prouvée. NAT résidentiel et hôtes physiques restent hors de cette preuve.
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


## 8. Addendum de vérification — 2026-11-05

- Workspace Linux : `cargo test --workspace -j 2` **432 tests, 0 échec** ;
  `cargo fmt --all -- --check` et Clippy `-D warnings` réussis.
- Identité locale : `~/.polygone` est vérifié en `0700`, les fichiers
  identité/ancre en `0600`, écritures atomiques ; chmod et erreurs de lecture
  propagés, symlinks refusés. Un magasin de pairs corrompu ne réactive pas TOFU.
- Relais : SEC.10 mesure les métadonnées visibles (source, destination,
  fragments, tailles, cadence et horodatages) ; le contenu du marqueur n'est
  pas observé en clair. Cela ne cache pas les métadonnées ni l'adresse IP.
- Hide : smoke local SOCKS5/grant validé ; il s'agit d'un **mono-saut**, pas
  d'une garantie d'anonymat ou d'une preuve multi-hop.
- Identités : 30 créations mesurées (médiane 2,85 ms, P95 3,07 ms,
  21 641 octets). Ce coût faible ne constitue pas une résistance Sybil.
- Réseau : toujours aucune preuve entre deux machines physiques ; les tests
  loopback ne ferment pas BLOCKS.9, MSG.6 ou FV-INF-04.
