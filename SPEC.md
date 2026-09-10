# ⬡ SPEC.md v1 — Polygone, le réseau vivant qui n'existe nulle part

> **Version** : v1.0 — 2026-09-10 · Scribe/Jony · AGPL-3.0
> **Source de vérité** : ce fichier tranche. `ECOSYSTEM.md` = mère, `ARCHITECTURE.md` = comment, `PHILOSOPHY.md` = pourquoi, `DECISIONS.md` = tranches D1-D9, `STAGING.md` = parkés.
> **Dogme** : toute promesse = test ou commande. Pas de stub qui prétend marcher.

---

## Préambule — Q2, le rêve sans trahison

Tu as dit Q2, mot pour mot :

> **« P2P avec daemons dynamiques et pleins d'autres choses et le fait que la privacy est là car rien n'existe. »**
> — Lévy, Q2 (2026-09-10), consigné dans `DOSSIER-POLYGONE-DREAM.md` §0

Ce n'est pas un pitch à reformuler. C'est **la définition du produit**. Ce SPEC la rend exécutable en 6 sections, sans l'aplatir :

| Fragment Q2 | Traduction SPEC | Où c'est vérifiable |
|---|---|---|
| **P2P** | 3 planes : plane 1 (ton PC) ↔ plane 2 (LAN mesh UDP 7642) ↔ plane 3 (relay blind TCP NDJSON) — le P2P est l'organisme, pas un transport | `ARCHITECTURE.md` §2-§4 + `ECOSYSTEM.md` §1 |
| **daemons dynamiques** | `polygoned` : boucle 5s `snapshot → GlowUpEngine::tick → apply` (cgroup/nice), respire avec toi — s'efface quand tu bosses, prête quand tu dors | `daemon/` + `ARCHITECTURE.md` §7 |
| **plein d'autres choses** | Fugue Bach : une voix à la fois, chacune autonome — `msg → drive → brain → mesh → hide → compute` (STAGING verrouille l'ordre) | `STAGING.md` + §3 ci-dessous |
| **privacy car rien n'existe** | Shamir 4-of-7 + TTL 30s + relay stateless + duress 4-file + `verite` — zéro rétention, zéro télémétrie, AGPL-3.0 | `PHILOSOPHY.md` Axiome 1 + §2 + §4 |

> **« On voit rien. Et c'est comme ça que ça devrait être. »**
> — Tagline Polygone v2, `SPEC.md` historique + `PHILOSOPHY.md` Axiome 1

> **« Polygone est un réseau vivant qui n'existe nulle part. »**
> — `DOSSIER-POLYGONE-DREAM.md` §1 — phrase à valider par Lévy. Si validée, devient l'unique phrase produit.

Q1 (« J'en sais rien ») est **résolu par le prototype**, pas par un slogan. Q3 (« rends réel mes rêves ») = ce SPEC tranche. Q4 (« Apple en agents ») = `GOVERNANCE.md`.

---

## §1 — Vision : le réseau vivant (Q2 rendu réel)

### 1.1 Le geste de 30 secondes

Si tu montres ton téléphone 30 secondes à un inconnu, il voit **ça** :

```
[ta clé ⬡ carte] → [envoyer "salut"] → 7 fragments qui s'allument → TTL qui tourne → ✓ livré → verite: "voici ce que ce nœud garde de toi : rien"
```

Pas un dashboard. Pas un discours PQC. **Un geste qui prouve une absence.** C'est le wedge que personne n'a — `DOSSIER-POLYGONE-DREAM.md` §1.

### 1.2 Les trois planes (l'organisme)

```
                    ┌── TON PC (plane 1) ──────────────┐
                    │  polygone (TUI/CLI)  ←→  polygoned│  boucle 5s : snapshot → GlowUp → cgroup/nice
                    │  identity.json (600)  received/  │  socket ~/.polygone/daemon.sock
                    └──────────┬────────────────────────┘
                               │ UDP 7642 (announce) — zero-config
                    ┌──────────▼──────┐
                    │  LE LAN (plane 2)│  mesh : "POLYGONE v1 <node> <relay> <ram>" — zero-config
                    └──────────┬──────┘
                               │ TCP NDJSON (7 frags + KEM + sig) — plane 3
                    ┌──────────▼──────┐
                    │  LE RELAY        │  polygone-relay : 16 shards, 64 KiB, 200 env/s, HELLO_OK/DENIED
                    │  stateless, drop │  voit from/to/session/tailles — jamais le contenu
                    └─────────────────┘
```

*Source : `ARCHITECTURE.md` §2-§4 + `ECOSYSTEM.md` §1.* Le relay ne persiste rien : restart = amnésie totale. Les peers sont oubliés à la déconnexion. Les fragments pour un peer offline sont droppés — pas bufferisés.

### 1.3 Le flux qui ne laisse rien

**Offline** : `plaintext → ML-KEM-1024 encaps → BLAKE3 KDF ("polygone session key v1") → AES-256-GCM (nonce 96b) → Shamir 4-of-7 → wire KEM_CT:/SENDER_PK:/FRAG:`

**Réseau** : `1 enveloppe KEM (signée ML-DSA-65, name_ct chiffré) + 7 fragments NDJSON → relay route sur to → Bob buffer ≥4/7 → verify sig → reconstruct → decrypt → received/`

Clé de session = `ZeroizeOnDrop`. TTL fragments non consolidés = 30s (spec originelle v1.0.0 p.1). `name_ct` = nom chiffré avec la clé de session — le relay ne voit jamais un nom de fichier (`ARCHITECTURE.md` §3-§4).

### 1.4 Ce qui le rend unique (Q2, verbatim)

1. **P2P + daemon dynamique** — `polygoned` n'est pas un utilitaire, c'est le poumon (`DOSSIER-POLYGONE-DREAM.md` §1). Aucun concurrent ne fait ça en Rust PQC.
2. **Privacy par non-existence** — Shamir 4/7 + TTL + relay stateless + duress `--confirmer` qui détruit `identity.json + received/ + reputation.json + peers.json`. Pas de "politique de rétention" — il n'y a rien à retenir (Axiome 1).
3. **"Plein d'autres choses" mais une seule à la fois** — fugue Bach : chaque service = une voix autonome (`msg → drive → brain → mesh → hide → compute → shell/petals-distribué`). `STAGING.md` verrouille l'ordre.

---

## §2 — Invariants : les 4 gates non-négociables (PHILOSOPHY.md)

Même si ça retarde la v1 de 3 mois (`DOSSIER-POLYGONE-DREAM.md` §2) — ces 4 gates ne sautent JAMAIS. Tirés des 5 axiomes, resserrés en 4 invariants exécutables :

| # | Invariant | Axiome source | Gate (commande qui tranche) |
|---|---|---|---|
| **I1** | **Rien n'existe après TTL** — aucun message ne réside après 30s ; Shamir 4/7 = <4 fragments = zéro info | Axiome 1 : "L'information n'existe nulle part fixe" | `cargo test --workspace` (35/35 Shamir C(7,4) + wrong-key KEM fail) + `polygone verite` rend "rien" |
| **I2** | **Deux tons, pas trois** — surface = `envoyer` + `quitter`, reste derrière `:` (vim) | Axiome 2 | `cargo test parse_known_commands` + TUI 80×24 SSH OK |
| **I3** | **Menace honnêtement divisée** — `threat-commodity.md` ≠ `threat-high-value.md`, chaque fichier avoue ce qu'il ne couvre pas | Axiome 3 | `grep -ci "tracking\|keylogger\|rubber-hose" docs/threat-*.md` ≥1 / fichier |
| **I4** | **Silence = produit, pas marketing** — coupe 90% documentée, workspace 4 crates ≤5000 LOC top-level, archive/ non compilé | Axiome 4 (Musk×Wozniak) | `wc -l crates/client/src/*.rs \| tail -1` ≤5000 + `cargo build --workspace` sans `archive/` |

Axiome 5 (Mitnick — "la machine est la menace") traverse les 4 : duress + kill-switch + sandbox `ProtectHome`/`wasmi fuel` — voir §4.

> **Zéro télémétrie, zéro compte, AGPL-3.0** — pas négociable, c'est l'Axiome (`DOSSIER-POLYGONE-DREAM.md` §2).

---

## §3 — Produit : les 7 rituels (ce que l'utilisateur vit)

Design system : `cyber-amber #f59e0b` + `cyber-slate #0f172a`, JetBrains Mono + Georgia italic (1 phrase poétique = 1 footnote tech), grain, pulse 4s. Pas de glassmorphism, pas de particules (`DOSSIER-POLYGONE-DREAM.md` §3.1).

| # | Rituel | Commande | Ce qu'il ressent | Gate |
|---|---|---|---|---|
| 1 | **Install** | `curl -fsSL github.com/lvs0/Polygone-Network/install.sh \| bash` | 1 commande, pas de Rust, pas de compte, identité `zar-ka-kos` générée (chmod 600) | `install.sh` SHA256 fail-closed + `bash -n` |
| 2 | **Premier soir** | `polygone premier-soir` | Guidé 5 min : sa carte → envoie "test" → 7 fragments naissent → TTL tourne → 4/7 reconstruisent → `verite` → "rien" + carnet `observation-premier-soir.md` commité | `docs/PREMIER-SOIR.md` + `docs/observation-premier-soir.md` |
| 3 | **Quotidien** | `polygone` (TUI 2-tabs : Envoyer / Quitter) | Ambre sur slate, typo qui respire, pas de spinner — l'attente est visible | TUI 2-tabs GO (D1 tranchée 2026-08-12) |
| 4 | **Vérifier** | `polygone verite` | Forensique : énumère tout ce que le nœud garde — verdict "voici ce que j'ai de toi : rien" | `forensic-drive.sh` + `forensic-zero-log.sh` |
| 5 | **Échanger** | `polygone carte` | Joli cadre ⬡ à montrer en personne — le social EST la rétention | `carte` imprimable, encadrée |
| 6 | **Prêter** | `polygoned` tourne seul | Il ne le voit pas — son PC respire mieux, le réseau vit (boucle 5s) | `polygoned status/dry-run/doctor` verts, `daemon.sock` vivant |
| 7 | **Urgence** | `polygone duress --confirmer` | Destruction irréversible 4-file (`identity.json + received/ + reputation.json + peers.json`), documentée, testée | `duress --confirmer` 4→0 testé (D6) |

> Règle produit++ : **toute promesse du README est un test CI ou une commande `polygone *`** (`ECOSYSTEM.md` §5). Sinon c'est du vent.

Services live derrière les rituels : `msg` + `drive` (E2E 4/7) + `brain` (Ollama local, zéro cloud) + `mesh` (UDP 7642) + `compute` MVP (shell systemd + WASM wasmi) + `hide` MVP (SOCKS5 single-hop, `hide-smoke.sh`) + `ghost` (heartbeat 120s, Docker/Render). Parkés : `petals-distribué`, `shell` — voir `STAGING.md` + §6.

---

## §4 — Threat : résumé honnête (deux modèles, pas un)

> **Honnêteté radicale** — deux threat models séparés, chaque promesse = test ou commande, le relay dit ce qu'il voit noir sur blanc (`DOSSIER-POLYGONE-DREAM.md` §2).

### Ce que Polygone garantit (contenu)

- **ML-KEM-1024 IND-CCA2 (FIPS 203) + AES-256-GCM + BLAKE3 KDF** — le relay et l'adversaire réseau ne lisent jamais le contenu sans la clé. Shamir 4-of-7 : 3/7 fragments = zéro info (prouvé par 35/35 combinaisons).
- **ML-DSA-65 (FIPS 204)** — chaque KEM envelope est signée ; le receveur vérifie `session ‖ from ‖ to ‖ kem_ct ‖ ciphertext` avant de déchiffrer. TOFU ancré : `peers.json` appris au premier contact, rejet si clé différente ensuite (`ARCHITECTURE.md` §3 + §11).
- **File names hors-bande** — `name_ct` chiffré avec la clé de session, le relay ne voit jamais un nom (`ARCHITECTURE.md` §4).

### Ce que le relay voit (assumé, documenté — D5 tranchée 2026-08-07)

> **« Le relay ne lit jamais le contenu. Il voit les métadonnées de routage, réduites et documentées. »** — `DECISIONS.md` D5

Le relay voit `from/to/session/tailles` (il doit router) + `HELLO` non authentifié crypto (authenticité au receveur). Il est sharded (16), rate-limité (200 env/s), cap 64 KiB, `from` must == HELLO, `HELLO_DENIED` anti-squatting. Voir `ARCHITECTURE.md` §4 + `docs/threat-*.md`.

### Ce que Polygone ne couvre PAS (écrit noir sur blanc)

| Menace | Couvert ? | Où c'est avoué |
|---|---|---|
| Corrélation qui-parle-à-qui par opérateur relay | ❌ Non — pseudonymes de session, pas d'onion multi-hop en v1 | `docs/threat-commodity.md` + `docs/threat-high-value.md` |
| Endpoint compromis (keylogger, rubber-hose, tracking) | ❌ Non — duress détruit les clés, ne protège pas la mémoire vive | `PHILOSOPHY.md` Ax.5 + `LEGAL.md` §4 + `docs/kill-switch.md` |
| RES compute (tâches/sorties en clair sur le relay) | ❌ Non — sandbox système, pas crypto ; ne pas y mettre de confidentiel | `ECOSYSTEM.md` §5 + `ARCHITECTURE.md` §5 |
| Zeroize parfait des SecretKey pqcrypto | ⚠️ Documenté — `#[zeroize(skip)]`, pas de bytes mutables exposés ; effacement réel = `duress` | `ARCHITECTURE.md` §11 |

Deux fichiers séparés : `docs/threat-commodity.md` (monsieur-tout-le-monde) et `docs/threat-high-value.md` (dissident). Pas un seul document qui ment par omission (Axiome 3).

---

## §5 — Gates : Phase 0 → 3 (une seule à la fois)

> **Règle** : on ne passe à Phase N+1 que si Phase N est **verte + committée + poussée** (`DOSSIER-POLYGONE-DREAM.md` §5).

| Phase | Nom | Contenu | Gate (vert = on passe) |
|---|---|---|---|
| **0** | **Vérité des disques** (1 sem.) — *80% fait 2026-09-10* | Fouille 18 racines (fait), `ARCHITECTURE.md` réécrite sur 4 crates réels (fait), `cargo test --workspace` + `cargo clippy -D warnings` verts sur `main` (à prouver), pousser `main` dd1c83b sur GitHub (D7 GO 2026-08-12) | `cargo test --workspace` vert + `clippy -D warnings` 0 + `git push origin main` CI verte |
| **1** | **Honnêteté du transport** (2 sem.) — *PROCHAIN* | ML-DSA branché partout + `name_ct` + HELLO signé + relay durci (90% fait, Phase 1.3 2026-08-08 à re-vérifier), `hide` SOCKS5 single-hop doc honnête + `hide-smoke.sh` vert, Ghost Node Docker→Render vivant | `hide-smoke.sh` vert + `forensic-zero-log.sh` 0 plaintext + Ghost heartbeat 120s |
| **2** | **Sécurité critique** (2 sem.) | Sandbox RES `ProtectHome=yes` + `InaccessiblePaths=~/.polygone` + wasmi fuel (fait Phase 2, à re-vérifier), duress 4-file + runbook testé physiquement, zeroize documenté | `duress --confirmer` 4→0 + `ProtectHome` vérifié + wasmi infinite-loop trappe |
| **3** | **La sortie** (1 sem.) | CI GitHub verte (checksums épinglés, rust-cache, SHA256SUMS), `install.sh` SHA256 fail-closed + `bash -n` gate (vérifié it.25, à re-prouver), `premier-soir / verite / carte` + landing finale → **premier utilisateur hors toi** | `curl -fsSL .../install.sh \| bash` → `polygone demo` vert sur 3 OS + landing `polygone.network` |

Phase 4 (produit++ continu, après v1) : `petals-distribué` (shard LLM) + `shell` — seulement si `STAGING.md` conditions remplies ; port Windows (`windows.rs` → `WindowsPlatform` réel) ; FIPS 205 SLH-DSA, bench liboqs.

**Gates transverses (toute phase)** : `cargo test --workspace` 109→200 tests verts, `cargo clippy --all --all-targets -- -D warnings` 0, `cargo fmt --check` 0, `forensic-drive.sh` vert, `smoke-commands.sh` vert — voir `GOVERNANCE.md` §3.1 (loop Hermès).

---

## §6 — Hors-scope : ce que v1 ne fait PAS (et ne promet pas)

Hérite de `ECOSYSTEM.md` §8 + `ARCHITECTURE.md` §11 + `STAGING.md` + `DECISIONS.md` D4/D9. Tout ce qui suit est **parké ou refusé pour v1.0.0** — pas un bug, un choix tranché :

| Hors-scope v1 | Statut | Pourquoi | Ré-introduction |
|---|---|---|---|
| **Token POLY / ledger / DAO / Web3** | ❌ Archivé `archive/2026-07-src/` | Coupe Musk 90% (Axiome 4) — pas de monétisation | Décision explicite Lévy requise, jamais automatique |
| **Petals distribué (shard LLM BitTorrent-style)** | ⚪ Staging | Local suffit (`brain` via Ollama) ; distrib = standardisation weights manquante | Phase 8+ si demande explicite post-MVP (`STAGING.md`) |
| **Shell SSH-like** | ⚪ Staging | SSH existe ; surface d'attaque énorme (Mitnick pivot) | Tabouillé design (one-time token, no persistence) — probablement jamais |
| **Multi-hop onion / DNS via tunnel / fingerprinting** | ❌ Phase 2+ | `hide` v1 = single-hop honnête (`HIDE-SPEC.md`) | Après `hide-smoke.sh` stable + threat model onion |
| **Windows natif** | ⚪ Placeholder | `daemon/resources/windows.rs` = `compile_error!` explicite — promesse = Linux/macOS | Port `WindowsPlatform` réel quand D1-D3 verts |
| **Browser anti-hallucination** | ❌ Phase ∞ | N'est pas une feature Polygone | Autre produit |
| **FIPS 205 SLH-DSA (SPHINCS+)** | ❌ v2.1 | ML-DSA-65 couvre déjà ; wrapper ~100 LOC | v2.1 (+ bench liboqs, paper) |
| **time_sync engine (1 019 LOC)** | 📦 Archivé `archive/2026-08-time_sync/` | 0 consommateur, surface d'attaque (D9 tranchée 2026-08-12 ARCHIVER) | Feature "sync inter-nœuds" Phase 8+ |
| **Remplacer email / cloud photos / Slack/Discord** | ❌ Non-goal | `ECOSYSTEM.md` §8 — Polygone ne remplace rien, il fait une chose | Jamais |
| **Compte / télémétrie / phone-home / analytics** | ❌ Interdit | Axiome + AGPL-3.0 | Jamais |
| **Persistance (profil, social-graph)** | ❌ Anti-axiome | Privacy-by-default (Comité 3) | Jamais |

> **Si ce n'est pas dans §3, ce n'est pas dans v1.** La coupe est honnête — `PHILOSOPHY.md` Axiome 4 : promettre 2 services et en livrer 2, pas promettre 8 pour en livrer 2.

---

## Annexes

### A. Décisions D1-D9 (référence)

| D | Objet | Statut 2026-09-10 | Réf |
|---|---|---|---|
| D1 | TUI 2-tabs (`envoyer`/`quitter`) | ✅ GO 2-tabs 2026-08-12 | `DECISIONS.md` D1 |
| D2 | Bench ML-DSA ≤200µs | 🟡 KO documenté (~265µs sign, ~2900 hs/s/cœur suffisant) — garder ML-DSA-65, cible révisée ≤400µs | D2 |
| D3 | Lettres État (CNIL/ANSSI/EFF) | 🟡 PENDING | D3 |
| D4 | Sibling Polygone-Protocols (Petals pilote) | ✅ GO 1 protocole 2026-08-12 | D4 |
| D5 | Relay public assumé (métadonnées doc) | ✅ Tranchée 2026-08-07 | D5 |
| D6 | Garde Axiome 4 (wc -l 3519) | ✅ Tranchée 2026-08-08 | D6 |
| D7 | Push main (38 commits fast-forward) | ✅ GO push 2026-08-12 | D7 |
| D8 | Config rétro-compat `[tier] tier=` | ✅ (a) 2026-08-08 | D8 |
| D9 | time_sync câbler/archiver | ✅ ARCHIVER 2026-08-12 | D9 |

### B. Commandes qui prouvent

```bash
cargo test --workspace                  # 109 tests (core 34, client 47, relay 7, daemon 21)
cargo clippy --all --all-targets -- -D warnings  # 0 warning
cargo fmt --check
bash scripts/forensic-zero-log.sh       # 0 plaintext dans relay logs
bash scripts/hide-smoke.sh              # SOCKS5 single-hop vert
bash scripts/forensic-drive.sh
bash scripts/smoke-commands.sh          # 7 gates
polygone demo                           # E2E ML-KEM + Shamir 4/7 in-process
polygone premier-soir && polygone verite && polygone carte
```

### C. Spécification crypto — tailles verrouillées (NIST)

| Primitive | Standard | Tailles (testées) | Crate |
|---|---|---|---|
| ML-KEM-1024 | FIPS 203 | pk 1568 B, sk 3168 B, ct 1568 B, ss 32 B | `pqcrypto-mlkem` |
| ML-DSA-65 | FIPS 204 | pk 1952 B, sk 4032 B, sig 3309 B | `pqcrypto-mldsa` |
| AES-256-GCM | — | key 32 B, nonce 96b, tag 128b | `aes-gcm` |
| Shamir 4-of-7 | — | 7 fragments, C(7,4)=35 combinaisons testées | `sharks` |
| BLAKE3 KDF | — | domain "polygone session key v1", 32 B | `blake3` |

D2 bench (2026-08-06, AVX2) : keygen ~92µs + sign ~265µs + verify ~79µs = ~2900 hs/s/cœur. Gate 200µs KO d'1.4× — non bloquant, cible révisée ≤400µs (`DECISIONS.md` D2).

### D. Matrice "On voit rien" — qui voit quoi

| Observateur | Contenu | from/to/session/tailles | Nom fichier | Peut prouver qu'Alice→Bob ? |
|---|---|---|---|---|
| Relay | ❌ Jamais (AES-GCM) | ✅ Oui (doit router) | ❌ `name_ct` chiffré | ❌ Non — pas de log persistant, pas de contenu |
| Adversaire réseau passif | ❌ | ✅ Métadonnées | ❌ | ❌ Idem relay |
| Adversaire 3/7 fragments | ❌ Shamir <4 = zéro info | — | — | ❌ |
| Bob (≥4/7 + KEM sk) | ✅ Après verify ML-DSA + decrypt | ✅ | ✅ Après decrypt | ✅ Oui — mais Bob est le destinataire |
| Forensique `verite` | — | — | — | Liste tout ce que le nœud garde : verdict "rien" |

### E. Fichiers qui gouvernent

`ECOSYSTEM.md` (mère) > `SPEC.md` (ce fichier) > `ARCHITECTURE.md` > `PHILOSOPHY.md` > `DECISIONS.md` > `STAGING.md` > `GOVERNANCE.md` > `docs/threat-*.md` > `LEGAL.md`. Si contradiction, l'ordre prime — `ECOSYSTEM.md` gagne.

### F. Historique des versions

| Version | Date | Auteur | Changement |
|---|---|---|---|
| v0.1 Dream | 2026-09-10 | Buffy×Hermès | Dossier Dream — fouille 18 racines, Q1-Q4 |
| **v1.0 SPEC** | **2026-09-10** | **Scribe×Jony** | **Ce fichier — 6 sections + invariants 4 gates + 7 rituels** |
| v1.1 (prévu) | Phase 1 verte | Woz+Galois | Bench liboqs + `hide-smoke` stabilisé + Ghost Render |
| v2.0 | Phase 3 verte | Ship+Scribe | Tag `v2.0.0`, binaires 4× strippés, landing `polygone.network` |

---

*— Scribe (Docs/Legal) × Jony (Design/CPO) — Halluin → Mouscron, 2026-09-10. Licence AGPL-3.0. « On voit rien. Et c'est comme ça que ça devrait être. »*
