# ⬡ POLYGONE — Company OS (Apple en agents)

> **Source** : `DOSSIER-POLYGONE-DREAM.md` §4 — ta demande Q4 : "un système d'agent qui représente le nombre de salariés d'une grosse entreprise comme Apple pour qu'il travaille ensemble en continue chacun dans leurs rôles et qui réalise Polygone à la hauteur la plus haute."
> **Statut** : v1.0 — org chart + RACI + KPIs + cadence. Implémentation : `.claude/company/` + `scripts/company-loop.sh` + Ruflo `hierarchical-mesh` 15 agents.
> **Licence** : AGPL-3.0.

---

## 0. Le cap — ce que "Apple" veut dire ici

Pas un logo. Une **barre de qualité** :

- **Code** = lisible en un après-midi, `forbid(unsafe_code)` sur le core, `clippy -D warnings` vert, bench vs liboqs, fuzz harness — pas de stub qui prétend marcher.
- **Expérience** = `premier-soir` en 5 min sans explication, TUI en SSH 80×24, `verite` qui liste vraiment, `carte` imprimable — pas un dashboard qui lag.
- **Design** = ambre qui respire, pas un template. Une seule promesse par écran (Jobs).
- **Honnêteté** = deux threat models séparés, chaque promesse = test ou commande, le relay dit ce qu'il voit noir sur blanc.
- **Release** = `curl -fsSL .../install.sh | bash` vérifié SHA256, 3 OS, binaires strippés.

**Si ce n'est pas à la hauteur, on ne ship pas.**

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
ML-KEM    relay   GlowUp  amber  114→200  Ollama  threat  CI/CD  LEGAL  landing
ML-DSA    mesh    policy  slate  forensic petals  duress  Ghost  SPEC   NLnet
Shamir    Hide    cgroup  grain  smoke    local   sandbox install PHILOS Proton
```

| # | Rôle | Nom de code | Propriétaire | Mission continue (lane) | KPI (hebdo) | Swan lane |
|---|---|---|---|---|---|---|
| 1 | **CEO** | Lévy | toi | trancher D1-D9, dire GO/NO, valider le Premier Soir | 1 décision | — |
| 2 | **COO** | **Hermès** | loop-master.md | planifie, délègue à Ruflo, contrôle, corrige, termine — 24/7 | loop sans fin, 0 "done" sans preuve | controller |
| 3 | **CTO** | **Woz** | ARCHITECTURE.md | workspace 6 crates, `ARCHITECTURE.md` = code, zéro drift | `cargo test --workspace` vert | hierarchical |
| 4 | **Crypto** | **Galois** | crates/core | ML-KEM-1024 + ML-DSA-65 + Shamir 4-of-7 + BLAKE3 KDF, bench vs liboqs, FIPS 205 | ops/sec, tailles NIST, `demo` vert | hierarchical |
| 5 | **Network** | **Mesh** | crates/relay + net.rs | relay blind (16 shards, HELLO_OK/DENIED, 64 KiB, 200 env/s) + mesh UDP 7642 + Hide SOCKS5 | `forensic-zero-log` + `hide-smoke` verts | hierarchical |
| 6 | **Daemon** | **Glow** | daemon/ | `polygoned` snapshot → GlowUp → cgroup/nice/socket + linux/macos parity | `polygoned doctor/status/dry-run` verts, `daemon.sock` vivant | hierarchical |
| 7 | **Security** | **Mitnick** | threat-*.md + duress.rs | threat commodity/high-value, kill-switch, sandbox `ProtectHome`+`wasmi fuel`, duress 4-file | audit interne, `duress --confirmer` 4→0 | hierarchical |
| 8 | **Design** | **Jony** | DESIGN_SYSTEM.md + index.html | amber #f59e0b / slate #0f172a, TUI 2-tabs, pulse 4s, grain, carte | Figma→code, 0 glassmorphism, landing 20k | mesh |
| 9 | **QA/Chaos** | **Rabbit** | scripts/*.sh | 114→200 tests, `forensic-drive.sh`, `forensic-zero-log.sh`, chaos relay, fuzz, `smoke-commands` | coverage, "on voit rien" = test | hierarchical |
| 10 | **Infra/Release** | **Ship** | .github/workflows + install.sh | CI verte, `install.sh` SHA256, 3 OS, Ghost Docker/Render, `daemon.sock` | `install.sh \| bash` → `demo` vert | solo bloquant |
| 11 | **Brain/AI** | **Petals** | crates/petals | `petals ask` Ollama local (zéro cloud), puis distribué shard | `ask` <2s local, `petals status` vert | mesh |
| 12 | **Docs/Legal** | **Scribe** | ECOSYSTEM.md + SPEC + LEGAL | ECOSYSTEM mère, SPEC, LEGAL, PHILOSOPHY, CHANGELOG — docs gouvernent | 1 source de vérité, `spec-audit` vert | mesh |
| 13 | **Growth** | **Signal** | index.html + BUDGET.md | landing `polygone.network`, payrequest.me/lvs0, dossier NLnet/Prototype Fund, Proton | 1er utilisateur hors toi | mesh |

**Règle Apple** : chaque rôle **possède** son domaine, **documente** ses limites, et **refuse** de shipper du brut. Hermes ne code pas — il orchestre.

---

## 2. RACI — qui fait quoi (une phase à la fois)

| Phase | Responsible | Accountable | Consulted | Informed |
|---|---|---|---|---|
| **0 Vérité disques** | Woz + Scribe | Hermès | tous | Lévy |
| **1 Transport honnête** | Galois + Mesh | Woz | Mitnick + Rabbit | Lévy |
| **2 Sécurité critique** | Mitnick + Glow | Galois | Rabbit + Ship | Lévy |
| **3 La sortie** | Ship + Rabbit | Hermès | Jony + Scribe + Signal | Lévy |
| **4 Produit++** | Jony + Petals | Scribe | Woz + Signal | Lévy |

Une seule phase à la fois. On ne passe à N+1 que si N est **verte + committée + poussée**.

---

## 3. Cadence — comment ça tourne en continu (sans te déranger)

### 3.1 Le loop Hermès (COO)

`scripts/company-loop.sh` — toutes les 5 min, en tmux `polygone-loop` :

```
loop:
  git status          → si drift >0, alerte
  cargo test --workspace → si rouge, délègue fix à Woz/Galois/Mesh/Glow
  cargo clippy -D warnings --all-targets → si rouge, fix
  cargo fmt --check   → si rouge, fmt
  bash scripts/smoke-commands.sh → si rouge, délègue à Rabbit
  bash scripts/forensic-zero-log.sh → si rouge, Mitnick
  bash scripts/hide-smoke.sh        → si rouge, Mesh
  si tout vert → commit + push (D7)
  sleep 300
```

Pas de "vibe done". Une preuve ou rien.

### 3.2 Le swarm Ruflo (hierarchical-mesh, 15 max, hybrid + HNSW)

- **Topologie** : `hierarchical-mesh` — anti-drift (hierarchical) + créatif (mesh) — 15 agents max.
- **Mémoire** : `hybrid` (SQLite + AgentDB), HNSW enabled, neural enabled.
- **Stratégie** : `specialized` — chaque agent a une lane, pas de recouvrement.

| Lane | Topo | Agents | Quand |
|---|---|---|---|
| feature / refactor | hierarchical | Woz + Galois + Mesh + Glow → coder → tester → reviewer | 3+ fichiers, nouvelle feature, cross-module |
| polish | mesh | Jony + Scribe | landing / docs / TUI |
| security audit | hierarchical | Mitnick + Rabbit | sandbox / duress / threat |
| release | solo | Ship | CI / install.sh / Docker — bloquant |
| brain | mesh | Petals | petals gateway |

**Règles Ruflo** (AGENTS.md) :

- Toute tâche non triviale → délégation.
- Ne jamais croire un "done" sans preuve — inspecter fichiers, tests, erreurs, comportement réel.
- 3-tier routing : 1 = codemod WASM, 2 = Haiku, 3 = Sonnet/Opus.

### 3.3 Le handoff Lévy (CEO)

Tu n'es interrompu que pour **trancher** :

- D1 TUI 2-tabs : GO (fait) / NO — **fait**
- D3 lettres État : GO / NO
- D5 relay public : GO (5€/mois assumé) / NO
- D7 push main : GO
- GO v1.0.0 : **toi seul la déclares** (`FINISHED` dans le loop)

Le reste, la company tourne seule. Tu reviens, tu tapes `polygone premier-soir`, tu vois que ça marche.

---

## 4. États & rituels

| État | Sens |
|---|---|
| **114 tests verts + clippy + fmt + smoke + forensic + hide** = vert réel — c'est la vérité du dépôt |
| **Company verte** = chaque lane a livré sa preuve dans `docs/` ou `target/release/` |
| **Push main** = D7 — la CI doit être verte sur GitHub, pas seulement en local |

**Rituels** :

- **Journal** : chaque lane écrit son avancement dans `docs/SESSION-REPORT-*.md` (1 par session).
- **Decision** : toute décision binaire → `DECISIONS.md` → Lévy tranche.
- **Preuve** : chaque "done" cite `cargo test` / `bash scripts/*.sh` / `polygone demo` — sinon c'est du vent.

---

## 5. L'infra du loop (technique)

- **Loop** : tmux `polygone-loop` (persists), `scripts/company-loop.sh` (bash, 5 min, trap).
- **Swarm** : `npx ruflo swarm init --topology hierarchical-mesh --max-agents 15 --strategy specialized`.
- **Memory** : `.swarm/memory.db` (hybrid), HNSW.
- **Config** : `.claude/company/manifest.json` + `.claude/settings.json` (hooks pre/post edit, session-restore).
- **Transcend** : `/mnt/transcend` (932G, NTFS → ext4 à migrer) pour `polygone-archive` + `lvs-backup`. Le loop n'écrit jamais de tar >1G sans vérifier `df -h`.

---

*Tu voulais Apple. Voilà l'OS. Maintenant on l'allume.*
