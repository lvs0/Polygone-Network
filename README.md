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
# Installation (Linux/macOS) — une commande
curl -fsSL polygone.network/install | bash

# Ou depuis le repo
cargo build --workspace --release

# Le produit — la TUI (2 commandes : envoyer / quitter, style vim)
polygone

# Les autres commandes produit
polygone demo            # démo E2E post-quantique complète (60 s)
polygone clef            # votre clef publique (à partager)
polygone envoyer -d <clef> "message"   # chiffrer + fragmenter (ML-KEM + Shamir 4/7)
polygone recevoir wire.txt             # reconstruire + déchiffrer

# Le vrai réseau (plane 2 — relay aveugle)
polygone-relay                          # terminal 1 : le relay
polygone ecouter                        # terminal 2 : Bob écoute
polygone envoyer --via 127.0.0.1:7000 --a <node_bob> -d <clef_bob> "salut"
                                        # terminal 3 : Alice envoie
# → Bob reçoit et déchiffre avec 4/7 fragments. Le relay ne voit que du routage.

# Le Drive — envoyer un FICHIER chiffré + fragmenté (2e service livré)
polygone envoyer --via 127.0.0.1:7000 --a <node_bob> -d <clef_bob> \
    --fichier ~/documents/secret.txt
# → Bob : ~/.polygone/received/secret.txt — contenu vérifié identique

# Petals — l'IA locale (3e service livré, pilot D4) — Ollama local, zéro cloud
polygone petals status                 # modèles installés
polygone petals ask --model phi4-mini:latest "ta question"
# → le modèle répond, rien ne quitte votre machine

# Le Mesh — trouver les nœuds du LAN sans adresse en dur (Phase 4)
polygone annoncer --relay 127.0.0.1:7000   # Bob annonce son relay sur le LAN
polygone voisins                           # Alice scanne : node + relay trouvés

# Les 4 binaires du workspace v2 :
#   polygone / polygone-client   la commande produit (+ TUI, demo, msg, net)
#   polygone-relay               relay aveugle (stateless, routage)
#   polygoned                    daemon d'allocation de ressources
#   (tests)                      cargo test --workspace → 82 tests
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

- `cargo test --workspace` → ✅ (58 tests)
- `cargo build --workspace` → ✅
- Crypto core (`polygone-core`) → ✅ **complet** : ML-KEM-1024, ML-DSA-65,
  AES-256-GCM, BLAKE3 KDF, Shamir 4-of-7 — tous branchés, tous testés
- Démo E2E (`polygone-client demo`) → ✅ relay aveugle + audit « on voit rien »
- Bench handshake D2 → 📊 données enregistrées (voir [`DECISIONS.md`](./DECISIONS.md) D2)
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
| `msg`   | 🟢 **Live** — messages E2E via relay (4/7) |
| `drive` | 🟢 **Live** — fichiers E2E via relay (4/7), `~/.polygone/received/` |
| `brain` | 🟢 **Live** — IA locale (petals → Ollama, zéro cloud) |
| 5 autres| ⚪ [`STAGING.md`](./STAGING.md) |

---

## Contribution

Voir [`LEGAL.md`](./LEGAL.md) §6 + [`.well-known/security.txt`](./.well-known/security.txt)
(PGP-signed disclosure).

---

*MIT License · v2.0.0-rc1 · Hope · Posture « honesty-first ».*
