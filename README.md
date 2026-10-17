# ⬡ Polygone

**Un réseau de transit éphémère post-quantique, écrit en Rust.**

Un message est chiffré (ML-KEM-1024 + AES-256-GCM), découpé en fragments (Shamir 4-of-7), routé par un relais qui ne voit que les métadonnées de routage, puis reconstruit et déchiffré par le destinataire. **Le relais ne persiste pas les fragments et ne les met pas en attente hors ligne.** L’identité et les fichiers reçus, eux, sont stockés localement.

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

# Tunnel SOCKS5 via Polygone Hide (Phase 1)
polygone ecouter --relay 127.0.0.1:7000 --hide   # terminal 1 : exit node
polygone hide --via 127.0.0.1:7000 --sortie <node_id> --dest <clef_pk> --ecoute 127.0.0.1:9050  # terminal 2 : client SOCKS5
curl --socks5-hostname 127.0.0.1:9050 https://example.com

# Trouver les nœuds du LAN
polygone annoncer --relay 127.0.0.1:7000
polygone voisins

# IA locale (zéro cloud)
polygone petals ask --model phi4-mini:latest "ta question"
```

`polygone demo` lance une démo E2E complète en 60 secondes.

## Comment ça marche

1. **Chiffrer** — ML-KEM-1024 encapsule la clé de session, AES-256-GCM chiffre le message (nonce frais par message).
2. **Fragmenter** — Shamir 4-of-7 : sur le réseau, quatre fragments sur sept permettent de reconstruire le ciphertext; le texte clair existe en mémoire aux extrémités et n'est pas exposé au relais.
3. **Router** — le relay fait transiter les fragments sans les déchiffrer ni les persister; il voit toutefois les métadonnées de routage (`from`, `to`, session) et les tailles.
4. **Reconstruire** — 4 fragments sur 7 suffisent au destinataire pour déchiffrer. La structure `SessionKey` utilise `ZeroizeOnDrop`; cela ne garantit pas l'effacement de toutes les copies internes ni du texte clair en mémoire. Les fichiers reçus sont conservés localement jusqu'à suppression.

L’enveloppe KEM de chaque session est signée en ML-DSA-65; la signature couvre aussi le ciphertext reconstruit. Son horodatage signé est vérifié à ±300 s. Le destinataire conserve en cache les identifiants des sessions complétées tant que leur timestamp peut encore être frais; une session déjà complétée est alors rejetée si elle est rejouée.
Si le cache anti-rejeu borné est saturé, les nouvelles sessions sont refusées plutôt que d'évincer un identifiant encore valide. Ces protections limitent le rejeu dans la fenêtre vérifiée; elles ne signifient pas qu'aucune forme de rejeu n'est possible en toutes circonstances.

**État du pin de confiance :** les défauts P1/P2 initiaux du store (`replace` qui modifiait puis échouait et backup orphelin ignoré par `verite`) sont corrigés et revus par Tilleul sur le snapshot de validation (24 chemins), digest SHA-256 `33626ec120864167a107ceaa66164317e2d6cc9577cd792a671b74389c5bd0bd`. Polygone rapporte 206/206 tests, Clippy strict, format, build release, smoke CLI et forensic loopback PASS. Le test Windows `peers_store_first_launch_creates_missing_parent` est présent mais `#[cfg(windows)]` et n’a pas été compilé/exécuté; le comportement Windows reste non démontré. Modal est suspendu et aucune durabilité universelle après crash n’est établie. Les révisions documentaires ci-dessous sont postérieures au digest. Voir [`RELEASE-STATUS.md`](./RELEASE-STATUS.md).

**Limite du pairing actuel :** `Pair --qr` construit une charge utile JSON `polygone-pair` avec `node_id` et clé publique ML-KEM, mais aucun QR n’est effectivement rendu et aucun consommateur de ce JSON n’a été trouvé. La charge utile interne sérialise en outre le `node_id` en 32 caractères par double-hex, contre 16 caractères pour l’ID réseau; c’est un défaut confirmé de cette sortie interne, pas la preuve qu’un flux utilisateur opérationnel est cassé. La carte n’affiche que des préfixes. La charge utile ne contient pas la clé ML-DSA ni une preuve de liaison, et le pin ML-DSA appris par TOFU réseau ne prouve pas son lien avec la KEM. Il n’existe pas encore de commande CLI prise en charge pour comparer l’empreinte canonique complète hors bande.

## Ce que ça ne fait pas

| Pas dans v2.0.0 | Raison |
|---|---|
| Interface web | La TUI suffit |
| Compte / cloud sync | Confidentialité par défaut |
| Chiffrement au repos | Effacement par duress, pas coffre |
| Chiffrement E2E des requêtes Compute | Le corps JSON est signé mais non chiffré; le relais et les observateurs du TCP sans TLS peuvent le lire. Ne pas y envoyer de contenu sensible |
| Abonnement | AGPL-3.0, gratuit |

## État

- Le snapshot de validation (24 chemins) digest SHA-256 `33626ec120864167a107ceaa66164317e2d6cc9577cd792a671b74389c5bd0bd` a 206/206 tests, Clippy strict, fmt, build release, smoke CLI et forensic loopback PASS rapportés; Tilleul confirme le Client 134/134 et ne trouve pas de nouveau défaut TOFU. Le test Windows de premier bootstrap est présent, mais non compilé/exécuté (`#[cfg(windows)]`; MinGW échoue sur `features.h` dans `pqcrypto-mlkem`), donc la portabilité Windows reste non démontrée. Modal reste suspendu; aucun smoke inter-environnements ni test entre hôtes physiques n’a été exécuté. Les mises à jour documentaires sont postérieures au digest; aucun changement de code n’est inclus. Voir [`RELEASE-STATUS.md`](./RELEASE-STATUS.md).
- Crypto (core) → réelle et testée : ML-KEM-1024, ML-DSA-65, AES-256-GCM, BLAKE3, Shamir 4-of-7
- Services présents dans le workspace : messages, fichiers, IA locale, mesh LAN, `verite`, `premier-soir`, **`hide` (Phase 1 MVP : SOCKS5 via relay aveugle + exit node)**; Compute reste un MVP partiel : son gate grant ciblé est clos, mais le corps des requêtes/réponses n’est pas chiffré E2E et aucun smoke réseau entre environnements n’a encore été exécuté.
- En cours : `petals` distribué, `shell` — voir [`STAGING.md`](./STAGING.md)
- Audit externe : pas encore réalisé

## Documentation

- [`docs/cli.md`](./docs/cli.md) — référence de la commande
- [`THREAT_MODEL.md`](./THREAT_MODEL.md) — ce que Polygone protège, et ce qu'il ne protège pas
- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — architecture des 6 paquets du workspace (4 paquets exécutables)
- [`PHILOSOPHY.md`](./PHILOSOPHY.md) — les axiomes de design
- [`DECISIONS.md`](./DECISIONS.md) — les décisions d'architecture
- [`LEGAL.md`](./LEGAL.md) — licence AGPL-3.0, subpoena, kill-switch

## Contribuer

Voir [`CONTRIBUTING.md`](./CONTRIBUTING.md) et [`LEGAL.md`](./LEGAL.md).

---

*AGPL-3.0 · v2.0.0 · état de validation locale : [`RELEASE-STATUS.md`](./RELEASE-STATUS.md) · posture « honesty-first ».*


---

**Soutenir** — [`payrequest.me/lvs0`](https://payrequest.me/lvs0)
