# CONTRIBUTING.md — Comment contribuer à Polygone

> *Projet AGPL-3.0 · Posture « honesty-first » · Zéro télémétrie, zéro compte, zéro cloud.*

---

## Principe directeur

**Le silence est le produit.** (Axiome 4, `PHILOSOPHY.md`)

On ne promet pas ce qu'on ne tient pas. On documente les limites noir sur blanc. On livre ce qu'on dit.

---

## Avant de commencer

1. **Lis** : `PHILOSOPHY.md`, `THREAT_MODEL.md`, `LEGAL.md`, `ARCHITECTURE.md`
2. **Build & test local** :
   ```bash
   cargo test --workspace
   cargo clippy --workspace --all-targets -- -D warnings
   ./scripts/smoke-commands.sh
   ```
   Tout doit être vert avant de proposer un changement.

---

## Comment contribuer

### Issues

- Bugs : reproductibles, avec `cargo test` output + logs si daemon
- Features : lisez `STAGING.md` d'abord — beaucoup sont archivées volontairement
- Security : email direct `relay-lvs0@protonmail.com` (pas d'issue publique)

### Pull Requests

1. **Une PR = un changement cohérent** (pas de "et aussi j'ai corrigé ci")
2. **Tests** : ajoutez des tests pour le comportement nouveau
3. **Docs** : mettez à jour `README.md`, `docs/cli.md`, `CHANGELOG.md` si visible utilisateur
4. **Clippy** : `cargo clippy --workspace --all-targets -- -D warnings` doit passer
5. **Style** : Rust 2021 edition, `rustfmt` standard, fichiers < 500 lignes si possible

### Architecture (4 crates)

| Crate | Rôle | Ne pas casser |
|-------|------|---------------|
| `crates/core` | Crypto pure (ML-KEM, ML-DSA, AES-GCM, BLAKE3, Shamir) | API publique, tests crypto |
| `crates/client` | Produit `polygone` (TUI + CLI + hide + petals + compute) | Commandes utilisateur, UX |
| `crates/relay` | Relay aveugle (stateless, drop) | Routage, pas de stockage |
| `daemon` | `polygoned` — allocation ressources + policy GlowUp | Config, allocation, status |

### Règles non-négociables

- **Zéro secret** : pas de clés en dur, pas de télémétrie, pas de phone-home
- **Zéro compte** : l'utilisateur n'a pas d'identité centrale
- **AGPL-3.0** : toute modif distribuée = source dispo
- **Honnêteté** : si une limite existe, elle est dans `THREAT_MODEL.md`

---

## Workflow de release

1. Version dans `Cargo.toml` (workspace.package.version)
2. `CHANGELOG.md` mis à jour (format : Keep a Changelog)
3. Tag git `vX.Y.Z`
4. Binaires release → GitHub Releases (install.sh les récupère)
5. `README.md` version badge mis à jour

---

## Contact

- Mainteneur : Lévy Verpoort Scherpereel `<relay-lvs0@protonmail.com>`
- Soutien : [`payrequest.me/lvs0`](https://payrequest.me/lvs0)
- Repo : `https://github.com/lvs0/Polygone-Network`

---

*AGPL-3.0 · « L'information n'existe pas. Elle traverse. »*