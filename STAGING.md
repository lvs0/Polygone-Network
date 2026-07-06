# STAGING.md — Services parked (pas dans MVP)

> *Archive des 6 services initialement prévus dans `ECOSYSTEM.md` qui ne sont pas livrés dans v2.0.0-rc1.*
> *Source : Conseil des Sages 2026-06-29 — Musk (couper 90%) + Gödel (documenter pour la postérité).*

---

## Vue d'ensemble

| Service | Statut | Livré en v2.0.0-rc1 ? |
|---------|--------|----------------------|
| **msg**    | 🟢 Live | ✅ |
| **drive**  | 🟢 Live | ✅ |
| `compute`  | ⚪ Staging | ❌ |
| `hide`     | ⚪ Staging | ❌ |
| `mesh`     | ⚪ Staging | ❌ |
| `brain`    | ⚪ Staging | ❌ |
| `petals`   | ⚪ Staging | ❌ |
| `shell`    | ⚪ Staging | ❌ |

**2 livrés · 6 archivés** (de 8 à 2, soit 75% de coupe).

---

## Service : `compute` (Polygone Compute)

**Concept.** Lend/borrow compute pour distributed work. Idle detection → allocation → exécution sandboxée.
**Pourquoi parked.** Pool de compute adversaries ⊇ mesh-only adversary. Pas de sandbox robuste en v1. Reputation system inexistant.
**Ré-introduction Phase 8+.** Sandbox secure (WASMtime, gVisor-level isolation) + reputation + adversary budget.

---

## Service : `hide` (Polygone Hide)

**Concept.** SOCKS5 + HTTPS proxy à travers le mesh. Multi-hop routing.
**Pourquoi parked.** Tor existe. Polygone-hide doit prouver un *plus*. Pas d'audit Tor-level disponible. Multihop introduit du délai non maîtrisé.
**Ré-introduction Phase 8+.** 1 audit externe minimum (Trail of Bits) + traffic fingerprinting resistance + doc honnête vs Tor tradeoffs.

---

## Service : `mesh` (Polygone Mesh local)

**Concept.** mDNS + BLE + Wi-Fi Direct discovery. Cluster LAN sans internet.
**Pourquoi parked.** La couche `p2p` actuelle (libp2p) couvre déjà mDNS + Kademlia. BLE/Wi-Fi Direct = autre protocole, orthogonal. Pas de valeur ajoutée démontrable.
**Ré-introduction.** Si un cas concret émerge (école sans internet, zone disaster).

---

## Service : `brain` (Polygone Brain)

**Concept.** Local LLM gateway (Notch / Ollama / llama.cpp / Petals).
**Pourquoi parked.** Polygone = transit verbatim. LLM = generation verbatim. Deux missions orthogonales. Le brouillage tactique du brain avec msg/drive serait confus.
**Ré-introduction Phase 8+.** Si `petals` n'est jamais livré, brain pourrait être ré-évalué en isolation.

---

## Service : `petals` (Distributed LLM)

**Concept.** Peer shard model + parallel inference (BitTorrent-style).
**Pourquoi parked.** Compromet la compatibilité « zéro contact cloud ». Weights quantization standardization absente. Pas de demande marché définie.
**Ré-introduction Phase 8+.** Si demande explicite post-MVP.

---

## Service : `shell` (Polygone Shell)

**Concept.** SSH-like remote shell via the mesh.
**Pourquoi parked.** SSH existe. Remote shell = augmentation surface d'attaque énorme. Mitnick peut pivoter. Pas de besoin urgent.
**Ré-introduction.** ⚠️ Doit être tabouillé par design (one-time token, no persistence, no auth replay). Probablement jamais.

---

## Conventions

- `Phase 8+` = explicit postponed. Ré-évalué en revue annuelle (pas en revue mensuelle).
- `Phase ∞` = pas une feature Polygone ; elle appartient à un autre produit.

## Pourquoi cette coupe est honnête

`PHILOSOPHY.md` Axiome 4 : « Le silence est le produit, pas le silence marketing. »

L'inverse du silence marketing = promettre 8 services pour en livrer 2. Coupe Musk = promettre 2 services et en livrer 2. C'est ce que fait v2.0.0-rc1.

---

*Hérite de : `ECOSYSTEM.md` (original 8 services), Conseil des Sages 2026-06-29 (Musk + Gödel).*
*MIT License · Pas de monétisation.*
