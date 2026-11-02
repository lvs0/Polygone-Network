# POLYGONE-SPEC-AUDIT.md — Delta between spec v1.0.0 and current code

> Generated 2026-06-05 from `POLYGONE-SPEC-1.0.0.txt` (the master
> founding document, addressed to Hermès).
>
> Legend: ✅ done · 🟡 partial · ❌ missing · ⚠️ drift

## §1 Vision philosophique
- ✅ Zero-trust / zero-knowledge
- ✅ No central cloud
- 🟡 Cross-platform — Linux compile, macOS/Windows untested
- ⚠️ "Une seule commande globale" — currently 5 binaries (`polygone`,
  `polygone-computer`, `polygone-server`, `polygone-ctl`,
  `polygone-serve-live`). Spec wants a single `polygone` entrypoint
  that dispatches to sub-services.

## §2 Fondations cryptographiques
- ✅ ML-KEM-1024 (`crypto/kem.rs`)
- ✅ AES-256-GCM (`crypto/symmetric.rs`)
- ✅ Shamir 4-of-7 (`crypto/shamir.rs`)
- ✅ TTL 30s on relay (`server/mod.rs`)
- ✅ `#![forbid(unsafe_code)]` on `main.rs`
- 🟡 `#![forbid(unsafe_code)]` on `lib.rs` — verify
- 🟡 ChaCha20-Poly1305 fallback — not implemented, AES-256-GCM only

## §3 Architecture workspace
- ❌ **Single crate** — spec wants a Cargo workspace with 6 crates:
  - `common/` — types partagés
  - `app/` — binaire unifié
  - `polygone-msg/`
  - `polygone-drive/`
  - `polygone-hide/`
  - `polygone-mesh/`
  - `polygone-brain/`
- 🟡 Current modules (in `src/`) approximate the spec's intent:
  - `crypto/` ≈ part of `common/`
  - `network/` ≈ part of `common/`
  - `protocol/` ≈ part of `msg/`
  - `services/` + `computer/` = registry pattern
  - `tui/` = TUI master
  - `web/` = web admin
- ⚠️ Services (`msg`, `drive`, `hide`, `mesh`, `brain`) are *not*
  separate crates — they are types in `services/mod.rs` and haven't
  been implemented yet (only a `Stub`/`DemoService` exists).

## §4 TUI maître — 4 onglets
- ✅ 4 onglets (app.rs, views.rs) — Accueil / Favoris / Services / Paramètres
- ✅ Navigation flèches + touches 1-4
- ❌ **Solde POLY** (économie locale, `~/.polygone/poly.toml`, 0.1 POLY/min)
- ❌ **Identité écosystème** (pseudo + NodeId crypto, displayed in Accueil)
- ❌ **Touche [R] Refresh** + indicateur "Dernière MAJ il y a Xs"
- ❌ Pas de polling continu — currently may have implicit refresh
- ❌ **Raccourcis [P/R/U/Q]** dans Accueil
- ❌ **Persistance visuelle POLY** dans onglet Services
- ❌ **Statut Drive** (10GB/∞) + raccourci web admin dans Paramètres
- 🟡 Ports 8080 / 9050 — defaults not enforced

## §5 Installateur Genius
- 🟡 `polygone-install` exists (1378 lines) — TUI installer
- ❌ Détection archi/OS automatique
- ❌ Choix langue + pseudo avec génération crypto par défaut
- ❌ Sélection modules interactive
- ❌ Optimisation système hôte (CPU governor, ZRAM, BFQ)
- ❌ Connectivité mesh (Wi-Fi mDNS, Bluetooth)

## §6 Fonctionnalités avancées
- ❌ **Drive streaming média** à la volée
- ❌ **Liens publics éphémères** (24h expiration)
- ❌ **Mesh load balancing** intelligent
- ❌ **Mesh fragmentation de tâches** sur la grappe

## §7 Feuille de route 5 phases
| Phase | Status |
|-------|--------|
| **1** Consolidation core/workspace | ❌ 0% (single crate) |
| **2** TUI maître complet | 🟡 40% (4 onglets, manque POLY/identité/refresh) |
| **3** P2P (libp2p + Kademlia) + Drive web | 🟡 30% (libp2p deps, drive web stub) |
| **4** Mesh mDNS + Bluetooth | ❌ 0% |
| **5** Brain (IA locale quantifiée) + Petals | ❌ 0% |

## Order of attack (proposed)

1. **Phase 1** : Refactor single-crate → workspace (common, app,
   msg, drive, hide, mesh, brain). Foundation for everything else.
2. **Phase 2** : TUI conformant — POLY, identité, touche [R],
   raccourcis [P/R/U/Q], persistance visuelle, statuts drive.
3. **Phase 3** : polgone-msg (Kademlia DHT), polygone-drive
   (streaming + liens éphémères), server relay polish.
4. **Phase 4** : polygone-mesh (mDNS Wi-Fi), load balancer.
5. **Phase 5** : polygone-brain (Notch + Petals fallback).

## Risks

- Workspace refactor breaks 60 tests. Git commit exists as safety
  net (commit `69293a4`).
- Hardware: Bluetooth requires hardware. Mesh Wi-Fi requires
  mDNS-capable network.
- Brain requires Notch model integration (already exists in
  `~/Projets/Notch/`).
