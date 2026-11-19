# ⬡ DOSSIER POLYGONE — Le produit dont tu rêves

> **Auteur** : Buffy × Hermès — fouille complète de 18 racines disque (2026-09-10)
> **État** : brouillon v0.1 à valider par Lévy — une fois validé, devient la source de vérité
> **Licence** : AGPL-3.0 — "On voit rien. Et c'est comme ça que ça devrait être."

---

## 0. Ce que tu m'as dit (Q1→Q4) — synthèse sans trahison

| Q | Ta réponse | Ce que j'en retiens |
|---|---|---|
| **Q1** | "J'en sais rien." | Tu ne veux pas choisir un pitch prématuré. Tu veux que le produit **parle de lui-même** quand on le voit — pas que je t'enferme dans une phrase marketing. Hypothèse ci-dessous à valider, pas à subir. |
| **Q2** | "P2P avec daemons dynamiques et pleins d'autres choses et le fait que la privacy est là car rien n'existe." | **C'est la définition dream** : pas "un chat chiffré", mais **un réseau vivant** où chaque machine prête ce qu'elle a (RAM/CPU/GPU/bande passante) via un daemon qui respire, et où la vie privée vient de **l'absence** — rien n'est stocké, rien à saisir. Le P2P n'est pas un transport, c'est l'organisme. |
| **Q3** | "Je veux que tu réussisses à rendre réel mes rêves." | Tu délègues la traduction rêve → réel. Mon job : **trancher** et **shipper**, pas te reposer 15 questions. Ce dossier tranche. |
| **Q4** | "L'expérience utilisateur, le fonctionnement, en fait il faut vraiment un système d'agent qui représente le nombre de salariés d'une grosse entreprise comme Apple pour qu'il travaille ensemble en continu chacun dans leurs rôles et qui réalise Polygone qui doit être à la hauteur la plus haute." | **L'exigence est l'organisation, pas le code.** Tu veux une **company d'agents** qui tourne 24/7, chacun dans son métier, comme Apple (design, hardware, software, retail, legal, ops). Polygone doit viser le niveau Apple — pas "un projet étudiant qui marche". |

**Ma promesse** : je prends Q2+Q4 comme cahier des charges. Q1 je le résous en te montrant un prototype, pas en te faisant choisir un slogan.

---

## 1. Le rêve tranché — Polygone en une phrase (à valider)

> **Polygone est un réseau vivant qui n'existe nulle part.**
> Chaque machine prête ce qu'elle n'utilise pas — le daemon respire avec toi. Chaque message traverse en 7 fragments et meurt — le relay ne voit rien et n'a rien à donner. Ta clé est une carte que tu échanges en main propre — le résidu social est le produit.

Si tu montres ton tel 30 secondes à un inconnu, il voit **ça** :

```
[ta clé ⬡ carte] → [envoyer "salut"] → 7 fragments qui s'allument → TTL qui tourne → ✓ livré → verite: "voici ce que ce nœud garde de toi : rien"
```

Pas un dashboard. Pas un discours PQC. **Un geste qui prouve une absence.** C'est le wedge que personne n'a.

### Ce qui le rend unique (Q2)

1.  **P2P + daemon dynamique** — `polygoned` n'est pas un utilitaire. C'est le poumon : il mesure ton CPU/RAM/GPU/bande passante toutes les 5s, calcule une allocation (GlowUpEngine), et l'applique (cgroup/nice). Quand tu bosses, il s'efface. Quand tu dors, il prête. Aucun concurrent ne fait ça en Rust PQC.
2.  **Privacy par non-existence** — Shamir 4-of-7 + TTL 30s + relay stateless + duress `--confirmer` qui détruit `identity.json + received/ + reputation.json + peers.json`. Il n'y a pas de "politique de rétention" — il n'y a **rien à retenir**. C'est l'Axiome 1 rendu exécutable (`verite`).
3.  **Et "plein d'autres choses"** — mais **une seule à la fois**, chacune une voix autonome (fugue Bach) : `msg` → `drive` → `brain` (Ollama local) → `mesh` (UDP 7642) → `hide` (SOCKS5) → `compute` (sandbox systemd + wasmi). Le piège de tes 37 projets, c'est de vouloir tout ship en même temps. Le rêve Apple, c'est **une release qui tue**, puis la suivante.

---

## 2. Les non-négociables — ce qu'on ne sacrifie JAMAIS (Q4)

Même si ça retarde la v1 de 3 mois :

1.  **Expérience > fonction** — `premier-soir` doit être un rituel de 5 min qui **fait sentir** le silence. Si l'UX est moyenne, on ne ship pas.
2.  **Fonctionnement réel** — pas de stub. Si `msg` dit "chiffré", c'est ML-KEM-1024 + AES-256-GCM + ML-DSA-65 vérifiés par test, pas une promesse. La CI doit être verte.
3.  **Honnêteté radicale** — deux threat models séparés, chaque promesse = test ou commande, le relay dit ce qu'il voit (from/to/session) noir sur blanc.
4.  **Zéro télémétrie, zéro compte, AGPL-3.0** — pas négociable, c'est l'Axiome.

Tout le reste est négociable : Windows, token POLY, browser anti-hallucination, petals distribué.

---

## 3. Le produit — à hauteur Apple

### 3.1 L'expérience (ce que l'utilisateur vit)

| Moment | Commande | Ce qu'il ressent |
|---|---|---|
| **Install** | `curl -fsSL github.com/lvs0/Polygone-Network/install.sh \| bash` | 1 commande, pas de Rust, pas de compte, identité `zar-ka-kos` générée |
| **Premier soir** | `polygone premier-soir` | Guidé : sa carte → envoie "test" → 7 fragments naissent → TTL tourne → 4/7 reconstruisent → `verite` → "rien" + carnet d'observation commité |
| **Quotidien** | `polygone` (TUI 2-tabs : Envoyer / Quitter) | Ambre sur slate, typographie qui respire, pas de spinner — l'attente est visible |
| **Vérifier** | `polygone verite` | Forensique : énumère tout ce que le nœud garde — verdict |
| **Échanger** | `polygone carte` | Joli cadre ⬡ à montrer en personne — le social EST la rétention |
| **Prêter** | `polygoned` tourne seul | Il ne le voit pas — son PC respire mieux, le réseau vit |
| **Urgence** | `polygone duress --confirmer` | Destruction irréversible, documentée, testée |

**Design system** : cyber-amber #f59e0b + slate #0f172a, JetBrains Mono + Georgia italic (1 phrase poétique = 1 footnote tech), grain, pulse 4s. Pas de glassmorphism, pas de particules.

### 3.2 Le système — comment ça respire

```
                    ┌── TON PC (plane 1) ──────────────┐
                    │  polygone (TUI/CLI)  ←→  polygoned│  boucle 5s : snapshot → GlowUp → cgroup/nice
                    │  identity.json (600)  received/  │  socket ~/.polygone/daemon.sock
                    └──────────┬────────────────────────┘
                               │ UDP 7642 (announce)
                    ┌──────────▼──────┐
                    │  LE LAN (plane 2)│  mesh : "POLYGONE v1 <node> <relay> <ram>" — zero-config
                    └──────────┬──────┘
                               │ TCP NDJSON (7 frags + KEM + sig)
                    ┌──────────▼──────┐
                    │  LE RELAY (plane 3)│  polygone-relay : 16 shards, 64 KiB, 200 env/s, HELLO_OK/DENIED
                    │  stateless, drop   │  voit from/to/session/tailles — jamais le contenu
                    └─────────────────┘
```

**Flux offline** : plaintext → ML-KEM-1024 encaps → BLAKE3 KDF ("polygone session key v1") → AES-256-GCM (nonce 96b) → Shamir 4-of-7 → wire `KEM_CT:/SENDER_PK:/FRAG:`
**Flux réseau** : 1 enveloppe KEM (signée ML-DSA-65, `name_ct` chiffré) + 7 fragments NDJSON → relay route sur `to` → Bob buffer session ≥4/7 → verify sig → reconstruct → decrypt → `received/`

### 3.3 L'état réel (2026-09-10)

- 6 crates (core/client/relay/daemon/petals/gateway), ~9 600 LOC produit, **109 tests verts** (à re-vérifier), `cargo build --release` OK, clippy clean récent (dd1c83b)
- Services live : msg/drive/brain/mesh/compute(MVP)/hide(MVP)/ghost(MVP) — petals-distribué + shell en staging
- CI GitHub : jamais verte (jamais poussée). Branche `chore/remove-v2` renomme v2→1.0.0 en attente.
- Gaps honnêtes : `time_sync` orphelin, `user_active()` = false, Windows = `compile_error!`, TOFU.

---

## 4. La company — "Apple en agents" (ta demande Q4)

Tu ne veux pas un agent qui code. Tu veux une **entreprise qui tourne en continu**, chaque rôle dans sa lane, qui fait de Polygone un produit à hauteur Apple.

### 4.1 L'orga — 13 rôles, hiérarchie Apple-inspired

```
                          ┌─ LÉVY — Founder / CEO ─┐
                          │  tranche, valide, ship  │
                          └────────────┬────────────┘
                                       │
              ┌────────────────────────┼────────────────────────┐
              │                        │                        │
     ┌────────▼────────┐    ┌─────────▼─────────┐    ┌────────▼────────┐
     │  HERMÈS — COO   │    │  JONY — CPO       │    │  WOZ — CTO      │
     │  orchestre,     │    │  expérience,      │    │  architecture,  │
     │  délègue,       │    │  TUI, design      │    │  Rust, PQC      │
     │  contrôle       │    │  system, rituel   │    │  relay, daemon  │
     └────────┬────────┘    └─────────┬─────────┘    └────────┬────────┘
              │                       │                       │
  ┌───────────┼───────────┐ ┌─────────┼──────────┐ ┌─────────┼──────────┐
  │           │           │ │         │          │ │         │          │
 CRYPTO   NETWORK   DAEMON  DESIGN   QA/CHAOS  BRAIN  SECURITY  INFRA  DOCS  GROWTH
  ML-KEM    relay    GlowUp  amber   109→200   Ollama  threat   CI/CD   LEGAL  landing
  ML-DSA    mesh     policy  slate   forensic  petals  duress   Ghost   SPEC   NLnet
  Shamir    Hide     cgroup  typo    smoke     local   sandbox  install PHILOS Proton
  BLAKE3    TCP      linux/  grain   chaos     distrib  audit   release  README payreq
                     macOS   pulse   perf      wasmi   zeroize  Docker  CHANGE pay
```

| # | Rôle | Nom de code | Mission continue | KPI |
|---|---|---|---|---|
| 1 | **CEO** | Lévy | trancher les D1-D9, dire GO/NO, valider le Premier Soir | 1 décision / semaine |
| 2 | **COO / Orchestrateur** | **Hermès** | planifie, délègue à Ruflo, contrôle, corrige, termine — 24/7 | loop sans fin, pas de "done" sans preuve |
| 3 | **CTO / Architect** | **Woz** | workspace 6 crates, `ARCHITECTURE.md` = code, pas de drift | `cargo test --workspace` vert |
| 4 | **Crypto Lead** | **Galois** | ML-KEM-1024 + ML-DSA-65 + Shamir + KDF — benchmark vs liboqs, FIPS 205 | ops/sec, tailles NIST |
| 5 | **Network Lead** | **Mesh** | relay blind (shards, HELLO, rate-limit) + mesh UDP + Hide SOCKS5 | latence, drop, `hide-smoke.sh` vert |
| 6 | **Daemon Lead** | **Glow** | `polygoned` : snapshot → GlowUp → apply (cgroup/nice/socket) + linux/macos parity | alloc mesurée, pas promise |
| 7 | **Security Lead** | **Mitnick** | threat-commodity/high-value, kill-switch, sandbox `ProtectHome`/`wasmi fuel`, duress 4-file | audit interne, `duress --confirmer` testé |
| 8 | **Design Lead** | **Jony** | DESIGN_SYSTEM (amber/slate, TUI 2-tabs, pulse, grain), landing, carte | Figma → code, 0 glassmorphism |
| 9 | **QA / Chaos Lead** | **Rabbit** | 109→200 tests, `forensic-drive.sh`, `forensic-zero-log.sh`, chaos relay, fuzz | coverage, "on voit rien" = test |
| 10 | **Infra / Release Lead** | **Ship** | CI GitHub verte, `install.sh` SHA256, 3 OS, Ghost Node Docker/Render, `daemon.sock` vivant | `install.sh \| bash` → `polygone demo` vert |
| 11 | **Brain / AI Lead** | **Petals** | `petals ask` Ollama local (zéro cloud), puis petals-distribué (shard) | `ask` <2s local |
| 12 | **Docs / Legal Lead** | **Scribe** | ECOSYSTEM.md mère, SPEC, LEGAL, PHILOSOPHY, CHANGELOG — docs gouvernent, pas suivent | 1 source de vérité |
| 13 | **Growth Lead** | **Signal** | landing `polygone.network`, payrequest.me/lvs0, dossier NLnet/Prototype Fund, approche Proton | 1er utilisateur hors toi |

**Règle Apple** : chaque rôle **possède** son domaine, **documente** ses limites, et **refuse** de shipper du brut. Hermes ne code pas — il orchestre.

### 4.2 Comment ça tourne en continu (sans te déranger)

- **Swarm Ruflo** : `hierarchical-mesh`, 15 agents max, mémoire hybrid + HNSW. Topologie :
  - `Woz + Galois + Mesh + Glow` → feature / refactor (hierarchical)
  - `Jony + Scribe` → polish landing/docs (mesh)
  - `Rabbit + Mitnick` → security audit (hierarchical)
  - `Ship` → CI/release (solo, bloquant)
- **Hermès loop** : toutes les 5 min, vérifie `git status`, `cargo test`, `cargo clippy`, `install.sh` — si vert, commit + push ; si rouge, délègue fix à l'agent compétent. Pas de "vibe done".
- **Handoff Lévy** : toi tu n'es interrompu que pour **trancher** (D1, D3, D5, GO v1). Le reste, la company tourne seule. Tu reviens, tu vois un `polygone premier-soir` qui marche — pas un rapport.

### 4.3 Ce que "à la hauteur la plus haute" veut dire concrètement

- **Code** : `#![forbid(unsafe_code)]` sur core, `cargo clippy -D warnings` vert, `cargo bench` vs liboqs, fuzz harness.
- **Expérience** : TUI utilisable en SSH 80×24, `premier-soir` en 5 min sans explication, `verite` qui liste vraiment.
- **Design** : landing `index.html` 20k digne d'Apple (ambre qui respire, pas un template), `carte` comme objet imprimable.
- **Sécurité** : pas "on dit qu'on est safe" — deux threat models + kill-switch runbook + audit externe planifié (NCC/Quarkslab).
- **Release** : `curl -fsSL .../install.sh | bash` vérifié SHA256, 3 OS, version git, binaires 4× strippés.

---

## 5. Roadmap — rêve → réel, en 4 phases (une seule à la fois)

### Phase 0 — La vérité des disques (1 semaine) **[FAIT à 80%]**
- [x] Fouille 18 racines + 30 docs (ce dossier)
- [x] ARCHITECTURE.md réécrite sur 4 crates réels — **à vérifier**
- [ ] `cargo test --workspace` + `cargo clippy -D warnings` verts sur `main` — **à prouver**
- [ ] Pousser `main` (dd1c83b) sur GitHub — **D7 pending**

### Phase 1 — L'honnêteté du transport (2 semaines) — **PROCHAIN**
- [ ] ML-DSA branché partout, `name_ct` partout, HELLO signé, relay durci — **déjà fait à 90% (Phase 1.3 vérifiée 2026-08-08) — à re-vérifier**
- [ ] `hide` SOCKS5 single-hop documenté honnête + `hide-smoke.sh` vert
- [ ] Ghost Node Docker → Render free tier vivant

### Phase 2 — La sécurité critique (2 semaines)
- [ ] Sandbox RES `ProtectHome=yes` + `InaccessiblePaths=~/.polygone` + wasmi fuel — **déjà fait Phase 2, à re-vérifier**
- [ ] Duress 4-file + runbook testé physiquement
- [ ] Zeroize documenté (pqcrypto n'expose pas de bytes mutables)

### Phase 3 — La sortie (1 semaine)
- [ ] CI GitHub verte (checksums épinglés, rust-cache, artefacts SHA256SUMS)
- [ ] `install.sh` réparé (SHA256 fail-closed, GitHub Releases, `bash -n` gate) — **vérifié itération 25, à re-prouver**
- [ ] `premier-soir` / `verite` / `carte` + landing finale → **premier utilisateur hors toi**

### Phase 4 — Produit++ & protocoles (continu, après v1)
- [ ] `petals-distribué` (shard LLM), `shell` — seulement si `STAGING.md` conditions remplies
- [ ] Port Windows (`windows.rs` → `WindowsPlatform` réel)
- [ ] FIPS 205 SLH-DSA, benchmarks liboqs, paper

**Règle** : on ne passe à Phase N+1 que si Phase N est **verte + committée + poussée**.

---

## 6. Ce que je te propose maintenant (tu tranches)

### Option A — "Fais-le" (recommandée, ton Q4)
Je lance **la company maintenant** :
1. Je crée le swarm Ruflo (13 rôles, hierarchical-mesh, mémoire hybrid)
2. Hermès loop démarre (vérif 5 min, délègue, contrôle, termine)
3. Premier chantier : **Phase 1 re-vérification** — `cargo test --workspace`, `cargo clippy`, `hide-smoke.sh`, `polygone demo`, `premier-soir` — preuves, pas promesses
4. Toi tu ne fais rien — tu reviens quand c'est vert, tu valides le Premier Soir

### Option B — "Montre-moi d'abord"
Je te fais une **démo live** en 10 min : `cargo test --workspace` + `polygone demo` + `premier-soir` + `verite` sous tes yeux, puis tu tranches.

### Option C — "Je veux trancher Q1"
Tu me dis qui est l'utilisateur 0 (pote lycée / journaliste / dev SDK) et je taille la roadmap pour lui seul.

---

## 7. Questions qui restent (rapides, 1 mot suffit)

1.  **GO pour la company 13 rôles ?** oui / non / modifie
2.  **On shippe quoi en v1.0.0 ?** msg+drive+ghost (reco) / +hide / +petals local
3.  **Budget relay :** ta poche 5€/mois OK pour 6 mois ou on dossier NLnet maintenant ?
4.  **Windows :** on assume Linux/macOS only pour v1 ou tu veux le port maintenant ?
5.  **Nom :** on garde `Polygone` (sans "v2") — OK ? (branche `chore/remove-v2` prête)

---

*Ce dossier est ton miroir. Dis "GO" et la company démarre. Dis "modifie X" et je retranche. Le rêve ne sera pas réduit — il sera **rendu réel, une phase à la fois, à hauteur Apple**.*

*— Buffy + Hermès, 2026-09-10, Halluin → Mouscron.*
