# ⬡ Polygone

> **L'information n'existe pas. Elle traverse.**

Post-quantum ephemeral transit network.
**ML-KEM-1024** · **ML-DSA-65** · **AES-256-GCM** · **Shamir 4-of-7** · **BLAKE3**.

**Version : v2.0.0-rc1** · Posture `honesty-first` · MIT License.
Pas de token. Pas de télémétrie. Pas d'investisseurs.

---

## Qu'est-ce que ce produit fait (vraiment)

Deux choses :

1. **Envoyer un message** sans que personne d'autre ne le sache traversé.
2. **Envoyer un fichier** sans que personne d'autre ne le sache traversé.

C'est tout. v2.0.0-rc1 livre ces deux choses, point.

---

## Qu'est-ce que ce produit ne fait PAS

| Pas dans v2.0.0-rc1 | Pourquoi |
|----------------------|----------|
| Browser GUI | La TUI suffit. Pas d'ambition UX. |
| IA locale / Petals | Pas dans scope. Voir [`STAGING.md`](./STAGING.md). |
| Tor replacement | Polygone-hide pas livré. Voir [`STAGING.md`](./STAGING.md). |
| Cloud sync | Privacy-by-default. |
| Compte utilisateur | Privacy-by-default. |
| Subscription / token | MIT License, $0, forever. |

---

## Quickstart

```bash
cargo build --release
./target/release/polygone
```

TUI actuelle : 4 onglets (Phase 3). Cible 2 onglets au v2.0.0-final lorsque **D1** GO — voir [`DECISIONS.md`](./DECISIONS.md).

Pas de YAML. Pas de `config.toml`. Pas de provider à choisir.

---

## Lisez ceci en premier

1. [`PHILOSOPHY.md`](./PHILOSOPHY.md) — les 5 axiomes. Poétique **et** technique.
2. [`THREAT_MODEL.md`](./THREAT_MODEL.md) — ce que Polygone protège, ce qu'il ne protège PAS.
3. [`LEGAL.md`](./LEGAL.md) — subpoena, kill-switch, pas de garantie.
4. [`COUNCIL_DECISIONS.md`](./COUNCIL_DECISIONS.md) — pourquoi chaque choix existe.
5. [`DESIGN_SYSTEM.md`](./DESIGN_SYSTEM.md) — pourquoi l'ambre, pourquoi le suspense.

---

## Statut honnête

- `cargo test` → ✅
- `cargo build` → ✅
- Audit externe → **NON RÉALISÉ** (cf. `LEGAL.md` §5)
- Production-grade P2P → ⚠️ wired in, transport simulé
- Documentation complète → 🟡 en cours (S2 livrable threat model)

---

## Pas de tagline sans footnote

> *« L'information n'existe pas. Elle traverse. »*

Signifie littéralement : aucun fragment reconstructible sans réunion de
4-of-7 fragments Shamir pendant le TTL. C'est une promesse **de design**,
pas une déclaration métaphysique.

Cf. [`PHILOSOPHY.md`](./PHILOSOPHY.md) Axiome 1.

---

## Statut par service

| Service | Statut |
|---------|--------|
| `msg`   | 🟢 **Live** |
| `drive` | 🟢 **Live** |
| 6 autres| ⚪ [`STAGING.md`](./STAGING.md) |

---

## Contribution

Voir [`LEGAL.md`](./LEGAL.md) §6 + [`.well-known/security.txt`](./.well-known/security.txt)
(PGP-signed disclosure).

---

*MIT License · v2.0.0-rc1 · Conseil des Sages 2026-06-29 · Posture « honesty-first ».*
