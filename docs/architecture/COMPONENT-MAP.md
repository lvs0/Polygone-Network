# COMPONENT-MAP — carte des composants

> Date : **2026-11-09**, HEAD `e2936a3` + correctif de persistance du ledger.
> Mesures : `find <crate> -name '*.rs' | wc -l`, `cat | wc -l`,
> `cargo test --workspace`. Les fichiers d'archive et de `salvage/` sont exclus :
> ils ne sont pas compilés.
>
> Cette carte décrit **le code compilé**. Une fonction publique sans appelant de
> production est signalée comme telle.

---

## 1. Vue d'ensemble mesurée

| Membre | Fichiers `.rs` | Lignes | Type | Tests | Binaire |
|---|---:|---:|---|---:|---|
| `crates/core` | 12 | 1 476 | lib | 32 | — |
| `crates/client` | 33 | 15 360 | bin + lib | 195 | `polygone` |
| `crates/relay` | 4 | 1 675 | lib + bin | 28 | `polygone-relay` |
| `crates/gateway` | 7 | 2 583 | lib + bin | 47 | `polygone-gateway` |
| `crates/petals` | 12 | 3 094 | lib | 19 | — |
| `crates/serverless` | 3 | 776 | lib | 17 | — |
| `daemon` | 17 | 10 613 | lib + bin | 127 | `polygoned` |
| **Total** | **88** | **35 577** | | **465** | **4 binaires** |

---

## 2. `crates/core` — primitives

| Fichier | Rôle | Points d'entrée notables |
|---|---|---|
| `crypto/kem.rs` | ML-KEM-1024 (FIPS 203) | `generate_keypair`, `encapsulate`, `decapsulate` |
| `crypto/symmetric.rs` | AES-256-GCM | `SessionKey`, `encrypt`, `decrypt` |
| `crypto/shamir.rs` | partage 4-of-7 | `split`, `reconstruct` |
| `sign.rs` | ML-DSA-65 (FIPS 204) | `Signer`, `Verifier`, `node_id_from_sign_pk_hex`, `relay_auth_transcript` |
| `identity.rs` | identifiants | `NodeId([u8;16])`, `SessionId` |
| `envelope.rs` | types de fil | `Envelope`, `Fragment`, `FRAGMENT_SHARED`/`THRESHOLD` |
| `receipt.rs` | reçu signé | `Receipt` |
| `error.rs` | erreurs | `PolygoneError` |

**Sans réseau.** Aucune dépendance à `tokio`, aucun socket. Toute la
cryptographie est testée ici (tailles exactes des clés, mauvaise clé refusée,
35 combinaisons Shamir).

`time_sync/` n'est **pas** dans le workspace actif : il a été archivé
(`archive/2026-08-time_sync/`, décision D9). Ne pas le réintroduire sans besoin.

---

## 3. `daemon` — autorité locale des ressources

| Fichier | Rôle | Persistance / effet |
|---|---|---|
| `main.rs` | boucle 5 s, signaux, socket | ouvre le socket, publie |
| `lib.rs` | exports de l'embeddable | — |
| `runtime.rs` | `runtime_state` pur, `run_tick`, `TickOutcome` | seul écrivain de l'état publié |
| `socket.rs` | protocole v1, serveur Unix, `Ledger` | `ledger.json` atomique 0600 |
| `system.rs` | `SystemSnapshot::capture` (CPU, RAM, réseau, GPU, stockage) | aucune |
| `resources/mod.rs` | types + trait `Platform`, `StorageInfo`, `mount_for_path` | aucune |
| `resources/linux.rs` | mesure réelle Linux, cgroups, services | aucune |
| `resources/macos.rs` | mesure réelle macOS | aucune |
| `resources/windows.rs` | placeholder ; `compile_error!` | — |
| `policy/glow_up.rs` | paliers, marges, décision, `shrinking` | config TOML |
| `cpu.rs`, `gpu.rs`, `bandwidth.rs` | calculs par ressource | aucune |
| `allocator.rs` | allocateur historique **non utilisé** par la politique active | aucune |
| `client.rs` | client du socket | — |
| `cli.rs` | sous-commandes `polygoned` | — |

**Invariants testés** : publication unique par le tick, aucune écriture de
plafond cgroup, mesure absente = zéro (fail-closed), ledger atomique et
quarantaine des fichiers illisibles.

---

## 4. `crates/client` — le produit

| Fichier | Rôle | État |
|---|---|---|
| `main.rs` | CLI clap + dispatch | OK |
| `identity.rs` | `~/.polygone/identity.json`, rotation/révocation | OK |
| `msg.rs` | pipeline hors ligne KEM → KDF → AES → Shamir | OK |
| `net/mod.rs` | transport réel via relais, enveloppes, anti-rejeu, grants | OK |
| `net/tests.rs`, `net/peers_tests.rs`, `net/s1_tests.rs` | tests du transport et de la migration | OK |
| `blocks.rs` | magasin BLAKE3, serveur/fetch TCP 7643 | OK (local) |
| `calc.rs` | cache de calcul, port 7644 | OK (local) |
| `node.rs` | bail du daemon, serveur de blocs, arrêt sur perte de bail | OK |
| `node_isolation.rs` | mesure cgroup, `--isoler` | OK (Linux) |
| `exec.rs` | shell `systemd-run`, WASM `wasmi` | OK (licence) |
| `hide.rs` | SOCKS5 via relais | PARTIEL (mono-saut) |
| `mesh.rs` | découverte UDP 7642, allowlist d'interfaces | OK (LAN local) |
| `reputation.rs` | registre local de confiance | **NON CÂBLÉ** comme sélecteur |
| `petals.rs` | Ollama local | OK |
| `serverless.rs` | découpage local | **NON CÂBLÉ** au réseau |
| `product.rs` | cartes, scénario « premier soir » | OK |
| `pause.rs` | intention bornée, reprise | OK |
| `duress.rs` | destruction locale | OK (manuel) |
| `uninstall.rs` | énumération + confirmation | OK |
| `ecosystem.rs` | sondes réelles | OK |
| `self_test.rs` | autotest crypto | OK |
| `demo.rs` | démonstration en processus | OK |
| `tui.rs` | TUI minimale | OK |
| `drive.rs`, `brain.rs` | **zéro appelant** | ne pas compter comme fonctionnalité |

---

## 5. `crates/relay` — routeur aveugle

| Fichier | Rôle |
|---|---|
| `relay.rs` | serveur TCP, handshake `AUTH_*`, 16 shards, limites |
| `relay/auth_tests.rs` | preuves d'authentification et d'usurpation |
| `main.rs` | CLI (`--port`, `--bind`, `--allow-insecure-hello`) |
| `lib.rs` | exports |

Limites : ligne 64 Kio, 200 enveloppes/s/connexion, 100/s/identité,
1 024 connexions, handshake 10 s, écriture vers destination 10 s. Aucune
persistance : redémarrage = amnésie.

---

## 6. `crates/gateway` — surface locale

| Fichier | Rôle |
|---|---|
| `lib.rs` | `Gateway`, appairage, `nodes.json`, verrou d'écriture |
| `routes.rs` | routes `/api/v1/*`, heartbeat, ping/pong |
| `http.rs` | parseur HTTP minimal, réponses, CORS |
| `static_files.rs` | `web/` (PWA) |
| `status.rs` | `NetworkStatus`, `NodeInfo`, fenêtre d'activité 300 s |
| `main.rs` | binaire |

Pas de TLS, pas d'authentification de l'opérateur local : la gateway est une
surface **loopback**. Un nœud distant n'est accepté qu'après appairage et ne
peut modifier que son `last_seen` / sa latence, avec une signature vérifiée
contre la clé épinglée.

---

## 7. `crates/petals` — inférence locale

| Fichier | Rôle | État |
|---|---|---|
| `lib.rs` | `PetalsEngine` | OK |
| `backends.rs` | sélection Ollama/vLLM/llama.cpp | OK |
| `backends/*.rs` | clients HTTP / chargeurs | OK (selon features) |
| `models.rs` | registre de modèles | OK |
| `benchmarks.rs` | configuration et résultats | OK (non branché CLI) |
| `pipeline.rs` | plan de découpage par couches | **sans consommateur de production** |
| `daemon.rs` | client du socket daemon | OK |
| `types.rs` | types d'inférence | OK |

---

## 8. `crates/serverless` — quorum

| Fichier | Rôle | État |
|---|---|---|
| `request.rs` | `ServerlessRequest`, `Shard`, `reassemble` | tests OK |
| `quorum.rs` | `tally`, `verify_receipts` | tests OK, **aucun appel CLI** |
| `lib.rs` | exports | — |

Le CLI `polygone serverless` prépare une exécution locale ; il ne contacte
aucun nœud. Ne pas le présenter comme du calcul distribué.

---

## 9. Composants hors workspace

| Chemin | Nature | Statut |
|---|---|---|
| `archive/2026-07-src/` | monolithe libp2p | archivé, non compilé |
| `archive/2026-08-time_sync/` | synchronisation d'horloge | archivé (D9) |
| `salvage/` | documents et benchmark historique | non compilé |
| `web/` | PWA statique | servi par la gateway |
| `landing/`, `site/`, `index.html` | pages statiques | hors produit |
| `phase0/` | preuves et script deux hôtes | outil de preuve |
| `brand/`, `reports/`, `checklists/` | documentation et assets | hors runtime |

---

## 10. Dépendances externes notables

| Dépendance | Usage | Remarque |
|---|---|---|
| `pqcrypto-mlkem` / `pqcrypto-mldsa` | KEM et signatures post-quantiques | tailles vérifiées par tests |
| `aes-gcm`, `blake3`, `sharks` | chiffrement, hachage, Shamir | primitives testées |
| `tokio` | réseau asynchrone (client, relais, gateway) | pas dans `core` |
| `sysinfo` | mesure système | daemon |
| `wasmi` / `wasmi_wasi` | bac à sable WASM | carburant borné |
| `crossterm` | TUI | client |
| `reqwest` (optionnel) | Ollama/vLLM | feature `ollama` par défaut |

Aucun audit de vulnérabilités n'a été exécuté dans ce lot ; voir
`project/FOUNDATION-ROADMAP.md` `F2-CI-02`.
