# ⬡ GOVERNANCE.md — Polygone Company OS (13 rôles SUPERPAW)

> **Source** : `DOSSIER-POLYGONE-DREAM.md` §4 — Q4 verbatim : *« un système d'agent qui représente le nombre de salariés d'une grosse entreprise comme Apple pour qu'il travaille ensemble en continu chacun dans leurs rôles et qui réalise Polygone qui doit être à la hauteur la plus haute. »*
> **Parent** : `SPEC.md` v1 §1-§5 · `ECOSYSTEM.md` (mère) · `ARCHITECTURE.md` · `PHILOSOPHY.md` · `DECISIONS.md` (D1-D9)
> **Statut** : v1.0 — 2026-09-10 · Scribe/Jony · AGPL-3.0
> **Implémentation** : `.claude/company/` + `scripts/company-loop.sh` + Ruflo `hierarchical-mesh` 15 agents · tmux `polygone-loop`

---

## 0. Le cap — ce que "Apple" veut dire ici

Pas un logo. Une **barre de qualité**. Si ce n'est pas à la hauteur, on ne ship pas (`DOSSIER-POLYGONE-DREAM.md` §4.3) :

- **Code** = lisible en un après-midi, `forbid(unsafe_code)` sur core, `clippy -D warnings` vert, bench vs liboqs, fuzz harness — pas de stub qui prétend marcher.
- **Expérience** = `premier-soir` en 5 min sans explication, TUI 80×24 SSH, `verite` qui liste vraiment, `carte` imprimable — pas un dashboard qui lag.
- **Design** = ambre qui respire (`#f59e0b` / `#0f172a`), pas un template. Une seule promesse par écran (Jobs).
- **Honnêteté** = deux threat models séparés, chaque promesse = test ou commande, le relay dit ce qu'il voit noir sur blanc.
- **Release** = `curl -fsSL .../install.sh | bash` vérifié SHA256, 3 OS, binaires strippés.

---

## 1. L'orga — 13 rôles, hiérarchie Apple-inspired

```
                        ┌─ LÉVY — Founder / CEO ─┐
                        │  tranche, valide, ship  │
                        │  1 décision / semaine    │
                        └────────────┬────────────┘
                                     │
            ┌────────────────────────┼────────────────────────┐
            │                        │                        │
   ┌────────▼────────┐    ┌─────────▼─────────┐    ┌────────▼────────┐
   │  HERMÈS — COO   │    │  JONY — CPO       │    │  WOZ — CTO      │
   │  orchestre,     │    │  expérience,      │    │  architecture,  │
   │  délègue,       │    │  TUI, design,     │    │  Rust, PQC,     │
   │  contrôle,      │    │  rituel, landing  │    │  relay, daemon  │
   │  24/7 loop      │    │  carte, verite    │    │  workspace      │
   └────────┬────────┘    └─────────┬─────────┘    └────────┬────────┘
            │                       │                       │
┌───────────┼───────────┐ ┌─────────┼──────────┐ ┌─────────┼──────────┐
│           │           │ │         │          │ │         │          │
CRYPTO   NETWORK   DAEMON DESIGN   QA/CHAOS  BRAIN  SECURITY  INFRA  DOCS  GROWTH
Galois    Mesh     Glow   Jony   Rabbit   Petals  Mitnick  Ship   Scribe Signal
```

| # | Rôle | Nom de code | Propriétaire (lane) | Mission continue | KPI hebdo | Swarm lane |
|---|---|---|---|---|---|---|
| 1 | **CEO** | **Lévy** | toi | trancher D1-D9, dire GO/NO, valider le Premier Soir | 1 décision | — |
| 2 | **COO / Orchestrateur** | **Hermès** | `loop-master` · `scripts/company-loop.sh` | planifie, délègue à Ruflo, contrôle, corrige, termine — 24/7, 0 "done" sans preuve | loop sans fin, drift = alerte | controller |
| 3 | **CTO / Architect** | **Woz** | `ARCHITECTURE.md` · workspace 6 crates | `ARCHITECTURE.md` = code, zéro drift, `SPEC.md` v1 respecté | `cargo test --workspace` vert | hierarchical |
| 4 | **Crypto Lead** | **Galois** | `crates/core` (kem, shamir, sign, symmetric) | ML-KEM-1024 + ML-DSA-65 + Shamir 4/7 + BLAKE3 KDF, bench vs liboqs, FIPS 205 | ops/sec, tailles NIST, `demo` vert | hierarchical |
| 5 | **Network Lead** | **Mesh** | `crates/relay` + `crates/client/net.rs` + `mesh.rs` | relay blind (16 shards, HELLO_OK/DENIED, 64 KiB, 200 env/s) + mesh UDP 7642 + Hide SOCKS5 | `forensic-zero-log` + `hide-smoke` verts | hierarchical |
| 6 | **Daemon Lead** | **Glow** | `daemon/` (system, policy/glow_up, socket) | `polygoned` snapshot → GlowUp → cgroup/nice/socket + linux/macos parity | `polygoned doctor/status/dry-run` verts, `daemon.sock` vivant | hierarchical |
| 7 | **Security Lead** | **Mitnick** | `docs/threat-*.md` + `duress.rs` + `exec.rs` | threat commodity/high-value, kill-switch, sandbox `ProtectHome`+`wasmi fuel`, duress 4-file | audit interne, `duress --confirmer` 4→0 | hierarchical |
| 8 | **Design Lead** | **Jony** | `DESIGN_SYSTEM.md` + `index.html` + `tui.rs` | amber #f59e0b / slate #0f172a, TUI 2-tabs, pulse 4s, grain, carte | Figma→code, 0 glassmorphism, landing 20k | mesh |
| 9 | **QA / Chaos Lead** | **Rabbit** | `scripts/*.sh` + `crates/*/tests` | 109→200 tests, `forensic-drive.sh`, `forensic-zero-log.sh`, chaos relay, fuzz, `smoke-commands` | coverage, "on voit rien" = test | hierarchical |
| 10 | **Infra / Release Lead** | **Ship** | `.github/workflows` + `install.sh` + `docs/deployment/` | CI verte, `install.sh` SHA256 fail-closed, 3 OS, Ghost Docker/Render | `install.sh \| bash` → `demo` vert | solo bloquant |
| 11 | **Brain / AI Lead** | **Petals** | `crates/client/petals.rs` | `petals ask` Ollama local (zéro cloud), puis distribué shard (Phase 8+) | `ask` <2s local, `petals status` vert | mesh |
| 12 | **Docs / Legal Lead** | **Scribe** | `ECOSYSTEM.md` + `SPEC.md` + `LEGAL.md` + `PHILOSOPHY.md` | ECOSYSTEM mère, SPEC, LEGAL, PHILOSOPHY, CHANGELOG — docs gouvernent, pas suivent | 1 source de vérité, `spec-audit` vert | mesh |
| 13 | **Growth Lead** | **Signal** | `index.html` + `docs/BUDGET.md` + `docs/STRATEGIE.md` | landing `polygone.network`, payrequest.me/lvs0, dossier NLnet/Prototype Fund, approche Proton | 1er utilisateur hors toi | mesh |

**Règle Apple** : chaque rôle **possède** son domaine, **documente** ses limites, et **refuse** de shipper du brut. Hermès ne code pas — il orchestre (`AGENTS.md`).

> SUPERPAW = acronyme d'orchestration : **S**ignal · **U**X (Jony) · **P**etals · **E**ngine (Woz) · **R**abbit · **P**olicy (Glow) · **A**rch (Scribe) · **W**arden (Mitnick) — 13 rôles, une company. Le nom est un clin d'œil, l'orga est réelle.

---

## 2. RACI — une phase à la fois

On ne passe à Phase N+1 que si N est **verte + committée + poussée** (`SPEC.md` §5).

| Phase | Responsible | Accountable | Consulted | Informed |
|---|---|---|---|---|
| **0 Vérité disques** | Woz + Scribe | Hermès | tous | Lévy |
| **1 Transport honnête** | Galois + Mesh | Woz | Mitnick + Rabbit | Lévy |
| **2 Sécurité critique** | Mitnick + Glow | Galois | Rabbit + Ship | Lévy |
| **3 La sortie** | Ship + Rabbit | Hermès | Jony + Scribe + Signal | Lévy |
| **4 Produit++** | Jony + Petals | Scribe | Woz + Signal | Lévy |

Toute décision binaire → `DECISIONS.md` → Lévy tranche (D1-D9). Pas d'interprétation flottante.

---

## 3. Cadence — comment ça tourne sans te déranger

### 3.1 Le loop Hermès (COO) — `scripts/company-loop.sh` en tmux `polygone-loop`

```
loop:
  git status              → si drift >0, alerte
  cargo test --workspace  → si rouge, délègue fix à Woz/Galois/Mesh/Glow
  cargo clippy -D warnings --all-targets → si rouge, fix
  cargo fmt --check       → si rouge, fmt
  bash scripts/smoke-commands.sh      → si rouge, Rabbit
  bash scripts/forensic-zero-log.sh   → si rouge, Mitnick
  bash scripts/hide-smoke.sh          → si rouge, Mesh
  bash scripts/forensic-drive.sh      → si rouge, Rabbit
  si tout vert → commit + push (D7 GO)
  sleep 300
```

Pas de "vibe done". Une preuve ou rien (`AGENTS.md` règle 2 : ne jamais croire un "done" sans preuve).

### 3.2 Le swarm Ruflo — `hierarchical-mesh`, 15 max, hybrid + HNSW

```bash
npx ruflo swarm init --topology hierarchical-mesh --max-agents 15 --strategy specialized
```

- **Topologie** : `hierarchical-mesh` — anti-drift (hierarchical) + créatif (mesh).
- **Mémoire** : `hybrid` (SQLite + AgentDB), HNSW enabled, neural enabled.
- **Stratégie** : `specialized` — chaque agent a une lane, pas de recouvrement.

| Lane | Topo | Agents | Quand |
|---|---|---|---|
| feature / refactor | hierarchical | Woz + Galois + Mesh + Glow → coder → tester → reviewer | 3+ fichiers, nouvelle feature, cross-module |
| polish | mesh | Jony + Scribe | landing / docs / TUI |
| security audit | hierarchical | Mitnick + Rabbit | sandbox / duress / threat |
| release | solo | Ship | CI / install.sh / Docker — bloquant |
| brain | mesh | Petals | petals gateway |

**Règles Ruflo** (`AGENTS.md`) : délégation si non-trivial · contrôle par inspection fichiers/tests · 3-tier routing (1=WASM codemod, 2=Haiku, 3=Sonnet/Opus) · mémoire `hybrid`.

### 3.3 Le handoff Lévy (CEO)

Tu n'es interrompu que pour **trancher** :

- D1 TUI 2-tabs : **GO** (fait 2026-08-12) · D2 bench ≤400µs : GO/NO · D3 lettres État : GO/NO
- D5 relay public : **GO** (5€/mois assumé, fait) · D7 push main : **GO** (fait) · D9 time_sync : **ARCHIVER** (fait)
- **GO v1.0.0** : toi seul la déclares (`FINISHED` dans le loop)

Le reste, la company tourne seule. Tu reviens, tu tapes `polygone premier-soir`, tu vois que ça marche — pas un rapport (`DOSSIER-POLYGONE-DREAM.md` §4.2).

---

## 4. États & rituels

| État | Sens |
|---|---|
| **114 tests verts + clippy + fmt + smoke + forensic + hide = vert réel** | C'est la vérité du dépôt — le reste est du discours |
| **Company verte** | Chaque lane a livré sa preuve dans `docs/` ou `target/release/` |
| **Push main** | D7 — la CI doit être verte sur GitHub, pas seulement en local |

**Rituels** :
- **Journal** : chaque lane écrit dans `docs/SESSION-REPORT-*.md` (1 par session).
- **Decision** : toute décision binaire → `DECISIONS.md` → Lévy tranche.
- **Preuve** : chaque "done" cite `cargo test` / `bash scripts/*.sh` / `polygone demo` — sinon c'est du vent.

---

## 5. L'infra du loop (technique)

- **Loop** : tmux `polygone-loop` (persists), `scripts/company-loop.sh` (bash, 5 min, trap).
- **Swarm** : `npx ruflo swarm init --topology hierarchical-mesh --max-agents 15 --strategy specialized`.
- **Memory** : `.swarm/memory.db` (hybrid), HNSW, neural.
- **Config** : `.claude/company/manifest.json` + `.claude/settings.json` (hooks pre/post edit, session-restore).
- **Router LLM** : `127.0.0.1:8765/v1` (anthropic_proxy, `deepseek-v4-flash-0731` + `glm-5.2` — ne pas redémarrer).
- **Transcend** : `/mnt/transcend` (932G, NTFS → ext4 à migrer) pour `polygone-archive` + `lvs-backup`. Le loop n'écrit jamais de tar >1G sans vérifier `df -h`.

---

## 6. Gouvernance & licence

- **Licence** : AGPL-3.0 — pas de fork proprio. Pas de télémétrie, pas de compte, pas de token.
- **Gouvernance** : `ECOSYSTEM.md` > `SPEC.md` > `ARCHITECTURE.md` > ce fichier. Si contradiction, l'ordre prime.
- **Transparence** : chaque lane documente ses limites (`ARCHITECTURE.md` §11 Known gaps, `docs/threat-*.md`). L'honnêteté est une feature.

---

*Tu voulais Apple. Voilà l'OS. Maintenant on l'allume. — Hermès × Scribe, 2026-09-10.*
