# ⬡ Polygone

**Un réseau de transit éphémère post-quantique, écrit en Rust.**

Un message est chiffré (ML-KEM-1024 + AES-256-GCM), découpé en fragments (Shamir 4-of-7), routé par un relais qui ne voit que du routage, puis reconstruit et déchiffré par le destinataire. **Rien n'est persisté :** le relais ne stocke rien, les fragments meurent.

Crypto : ML-KEM-1024 (FIPS 203) · ML-DSA-65 (FIPS 204) · AES-256-GCM · BLAKE3 · Shamir 4-of-7.
Version : **v2.0.0** · Licence : **AGPL-3.0** · Pas de compte, pas de télémétrie.

---

## Installation

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/lvs0/Polygone-Network/main/install.sh | bash
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

# Tunnel SOCKS5 via Polygone Hide (Phase 1)
polygone ecouter --relay 127.0.0.1:7000 --hide   # terminal 1 : exit node
polygone hide --via 127.0.0.1:7000 --sortie <node_id> --dest <clef_pk> --ecoute 127.0.0.1:9050  # terminal 2 : client SOCKS5
curl --socks5-hostname 127.0.0.1:9050 https://example.com

# Partager volontairement le magasin de blocs sur le LAN
polygoned                                  # terminal 1 : daemon local
polygone node servir --isoler           # terminal 2 : bail + cgroup OS dédié (Linux/systemd)
polygone node status                      # terminal 3 : état, bail et compteurs
# Ctrl+C dans le terminal du nœud rend le bail

# Trouver les nœuds du LAN
polygone annoncer --relay 127.0.0.1:7000
polygone voisins

# IA locale (zéro cloud)
polygone petals ask --model phi4-mini:latest "ta question"

# Serverless — préparer une exécution en quorum (vérifie les parts, n'envoie rien)
polygone serverless --commande "printf" "bonjour"

# Platform — site + API HTTP locale sur 127.0.0.1:8088
polygone-gateway
# Le binaire installé retrouve automatiquement les ressources sous share/polygone/web.
# Les binds publics sont refusés tant que TLS et l'authentification mutuelle ne sont pas prêts.
```

`polygone demo` lance une démo E2E complète en 60 secondes.

## Comment ça marche

1. **Chiffrer** — ML-KEM-1024 encapsule la clé de session, AES-256-GCM chiffre le message (nonce frais par message).
2. **Fragmenter** — Shamir 4-of-7 : le message n'existe nulle part en entier.
3. **Router** — le relay fait transiter les fragments ; il ne les lit pas et ne les stocke pas (stateless, drop).
4. **Reconstruire** — 4 fragments sur 7 suffisent au destinataire pour déchiffrer, puis tout est oublié (`zeroize`).

Chaque message est signé ML-DSA-65 et horodaté (±300 s) : le rejeu est impossible.

## Ce que ça ne fait pas

| Pas dans le produit aujourd'hui | Raison |
|---|---|
| Compte / cloud sync | Confidentialité par défaut |
| Chiffrement au repos | Effacement par duress, pas coffre |
| Abonnement | AGPL-3.0, gratuit |
| Multi-hop Hide | planification de circuit seulement, transport single-hop |
| Distribution des poids Petals | planner de pipeline seulement, pas de transfert de poids |

## État

- `cargo test --workspace` — la suite fait foi, elle n'est pas recopiée ici
- Crypto (core) → réelle et testée : ML-KEM-1024, ML-DSA-65, AES-256-GCM, BLAKE3, Shamir 4-of-7
- Services live : messages, fichiers, IA locale, mesh LAN, service de blocs sous bail (`polygone node`), compute sandboxé, `verite`, `premier-soir`
- **`hide`** → Phase 1 livrée : SOCKS5 via relay aveugle + exit node, single-hop. La planification de circuit multi-hop existe, le transport multi-hop non.
- **`serverless`** → partiel : crate `polygone-serverless` (Shamir 4-of-7 pour le transit, quorum de ≥ 4 nœuds distincts sur le hash de sortie) et commande CLI. Le dispatch sur le relay n'est pas branché.
- **`petals`** → local livré (`polygone petals` → Ollama, zéro cloud) ; planner distribué disponible, distribution des poids absente.
- **Platform** → binaire `polygone-gateway` distribué avec `web/` (site statique, API JSON) et PWA installable ; écoute loopback uniquement, sans TLS.
- Le service de blocs demande un bail et s'arrête sur retrait de politique. Sous Linux, `polygone node servir --isoler` utilise un scope systemd/cgroup v2 distinct avec `MemoryMax` et `CPUQuota` vérifiés ; sans cette option, le plafond reste auto-imposé. macOS n'a pas encore d'isolation OS équivalente branchée.
- En cours : `shell` — voir [`STAGING.md`](./STAGING.md)
- Audit externe : pas encore réalisé

## Documentation

- [`docs/cli.md`](./docs/cli.md) — référence de la commande
- [`docs/SERVERLESS-SPEC.md`](./docs/SERVERLESS-SPEC.md) — serverless : quorum, transit Shamir, non-objectifs
- [`THREAT_MODEL.md`](./THREAT_MODEL.md) — ce que Polygone protège, et ce qu'il ne protège pas
- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — comment c'est construit (6 bibliothèques + le démon)
- [`PHILOSOPHY.md`](./PHILOSOPHY.md) — les axiomes de design
- [`DECISIONS.md`](./DECISIONS.md) — les décisions d'architecture
- [`LEGAL.md`](./LEGAL.md) — licence AGPL-3.0, subpoena, kill-switch

## Contribuer

Voir [`CONTRIBUTING.md`](./CONTRIBUTING.md) et [`LEGAL.md`](./LEGAL.md).

---

*AGPL-3.0 · v2.0.0 · Posture « honesty-first ».*


---

**Soutenir** — [`payrequest.me/lvs0`](https://payrequest.me/lvs0)
