# ⬡ Polygone

**Un réseau de transit éphémère post-quantique, écrit en Rust.**

Un message est chiffré (ML-KEM-1024 + AES-256-GCM), découpé en fragments (Shamir 4-of-7), routé par un relais qui ne voit que du routage, puis reconstruit et déchiffré par le destinataire. **Rien n'est persisté :** le relais ne stocke rien, les fragments meurent.

Crypto : ML-KEM-1024 (FIPS 203) · ML-DSA-65 (FIPS 204) · AES-256-GCM · BLAKE3 · Shamir 4-of-7.
Version : **v2.0.0** · Licence : **AGPL-3.0** · Pas de compte, pas de télémétrie.

---

## Installation

```bash
curl -fsSL https://github.com/lvs0/Polygone-Network/releases/latest/install.sh | bash
```

Ou depuis le code :

```bash
git clone https://github.com/lvs0/Polygone-Network.git
cd Polygone-Network
cargo build --workspace --release
```

## Démarrage rapide

```bash
# L'essentiel — la TUI (envoyer / quitter, style vim)
polygone

# Le scénario guidé : envoie un message, vois-le mourir, vérifie (5 minutes)
polygone premier-soir

# Vérifier l'absence : ce que ce nœud garde de toi
polygone verite

# La clé comme objet à échanger en personne
polygone carte

# Envoyer / recevoir, sans réseau
polygone envoyer -d <clef> "message" > wire.txt
polygone recevoir wire.txt

# Envoyer / recevoir, via le relay
polygone-relay                          # terminal 1 : le relay
polygone ecouter                        # terminal 2 : Bob écoute
polygone envoyer --via 127.0.0.1:7000 --a <node_bob> -d <clef_bob> "salut"

# Envoyer un fichier (chiffré + fragmenté)
polygone envoyer --via 127.0.0.1:7000 --a <node_bob> -d <clef_bob> --fichier ~/documents/secret.txt

# Trouver les nœuds du LAN
polygone annoncer --relay 127.0.0.1:7000
polygone voisins

# IA locale (zéro cloud)
polygone petals ask --model phi4-mini:latest "ta question"
```

`polygone demo` lance une démo E2E complète en 60 secondes.

## Comment ça marche

1. **Chiffrer** — ML-KEM-1024 encapsule la clé de session, AES-256-GCM chiffre le message (nonce frais par message).
2. **Fragmenter** — Shamir 4-of-7 : le message n'existe nulle part en entier.
3. **Router** — le relay fait transiter les fragments ; il ne les lit pas et ne les stocke pas (stateless, drop).
4. **Reconstruire** — 4 fragments sur 7 suffisent au destinataire pour déchiffrer, puis tout est oublié (`zeroize`).

Chaque message est signé ML-DSA-65 et horodaté (±300 s) : le rejeu est impossible.

## Ce que ça ne fait pas

| Pas dans v2.0.0 | Raison |
|---|---|
| Interface web | La TUI suffit |
| Compte / cloud sync | Confidentialité par défaut |
| Chiffrement au repos | Effacement par duress, pas coffre |
| Abonnement | AGPL-3.0, gratuit |

## État

- `cargo test --workspace` → **162 tests verts** (produit 52, core 52, daemon 26, relay 7, alloc 25)
- Crypto (core) → réelle et testée : ML-KEM-1024, ML-DSA-65, AES-256-GCM, BLAKE3, Shamir 4-of-7
- Services live : messages, fichiers, IA locale, mesh LAN, compute sandboxé, `verite`, `premier-soir`
- En cours : `hide` (tunnel), `petals` distribué, `shell` — voir [`STAGING.md`](./STAGING.md)
- Audit externe : pas encore réalisé

## Documentation

- [`docs/cli.md`](./docs/cli.md) — référence de la commande
- [`THREAT_MODEL.md`](./THREAT_MODEL.md) — ce que Polygone protège, et ce qu'il ne protège pas
- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — comment c'est construit (4 crates)
- [`PHILOSOPHY.md`](./PHILOSOPHY.md) — les axiomes de design
- [`DECISIONS.md`](./DECISIONS.md) — les décisions d'architecture
- [`LEGAL.md`](./LEGAL.md) — licence AGPL-3.0, subpoena, kill-switch

## Contribuer

Voir [`CONTRIBUTING.md`](./CONTRIBUTING.md) et [`LEGAL.md`](./LEGAL.md).

---

*AGPL-3.0 · v2.0.0 · 162 tests · Posture « honesty-first ».*


---

**Soutenir** — [`payrequest.me/lvs0`](https://payrequest.me/lvs0)
