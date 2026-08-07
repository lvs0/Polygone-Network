# ⬡ Polygone

> **L'information n'existe pas. Elle traverse.**

Post-quantum ephemeral transit network.
**ML-KEM-1024** · **ML-DSA-65** · **AES-256-GCM** · **Shamir 4-of-7** · **BLAKE3**.

**Version : v2.0.0-rc2** · Posture `honesty-first` · AGPL-3.0.
Pas de token. Pas de télémétrie. Pas d'investisseurs.

---

## Qu'est-ce que ce produit fait (vraiment)

Deux choses :

1. **Envoyer un message** sans que personne d'autre ne puisse le lire.
2. **Envoyer un fichier** sans que personne d'autre ne puisse le lire.

C'est tout. v2.0.0-rc2 livre ces deux choses, en local et à travers le relay.

**Honnêteté d'architecture (lue dans le code, pas dans le rêve) :** le relay
voit les *métadonnées* de routage (`from`, `to`, `session`, tailles) parce
qu'il route dessus. Il ne voit **jamais** le contenu — chiffré ML-KEM-1024 +
AES-256-GCM. Le modèle de menace est documenté dans
[`docs/threat-commodity.md`](./docs/threat-commodity.md) et
[`docs/threat-high-value.md`](./docs/threat-high-value.md).

---

## Qu'est-ce que ce produit ne fait PAS

| Pas dans v2.0.0-rc2 | Pourquoi |
|----------------------|----------|
| Browser GUI | La TUI suffit. Pas d'ambition UX. |
| Tor replacement | Polygone-hide pas livré. Voir [`STAGING.md`](./STAGING.md). |
| Cloud sync | Privacy-by-default. |
| Compte utilisateur | Privacy-by-default. |
| Subscription / token | AGPL-3.0, $0, forever. |

---

## Quickstart

```bash
# Installation (Linux/macOS)
curl -fsSL https://github.com/lvs0/Polygone-Network/releases/latest/install.sh | bash

# Ou depuis le repo
cargo build --workspace --release

# Le produit — la TUI (2 commandes : envoyer / quitter, style vim)
polygone

# Les autres commandes produit
polygone demo            # démo E2E post-quantique complète (60 s)
polygone clef            # votre clef publique (à partager)
polygone envoyer -d <clef> "message"   # chiffrer + fragmenter (ML-KEM + Shamir 4/7)
polygone recevoir wire.txt             # reconstruire + déchiffrer

# Le vrai réseau (plane 2 — relay)
polygone-relay                          # terminal 1 : le relay
polygone ecouter                        # terminal 2 : Bob écoute
polygone envoyer --via 127.0.0.1:7000 --a <node_bob> -d <clef_bob> "salut"
                                        # terminal 3 : Alice envoie
# → Bob reçoit et déchiffre avec 4/7 fragments. Le relay route, il ne lit pas le contenu.

# Le Drive — envoyer un FICHIER chiffré + fragmenté (2e service livré)
polygone envoyer --via 127.0.0.1:7000 --a <node_bob> -d <clef_bob> \
    --fichier ~/documents/secret.txt
# → Bob : ~/.polygone/received/secret.txt — contenu vérifié identique

# Petals — l'IA locale (pilot D4) — Ollama local, zéro cloud
polygone petals status                 # modèles installés
polygone petals ask --model phi4-mini:latest "ta question"

# Le Mesh — trouver les nœuds du LAN sans adresse en dur (Phase 4)
polygone annoncer --relay 127.0.0.1:7000   # Bob annonce son relay sur le LAN
polygone voisins                           # Alice scanne : node + relay trouvés

# Les 4 binaires du workspace v2 :
#   polygone / polygone-client   la commande produit (+ TUI, demo, msg, net)
#   polygone-relay               relay (stateless, routage)
#   polygoned                    daemon d'allocation de ressources
#   (tests)                      cargo test --workspace → 89 tests
```

Pas de YAML. Pas de `config.toml`. Pas de provider à choisir.

---

## Lisez ceci en premier

1. [`PHILOSOPHY.md`](./PHILOSOPHY.md) — les 5 axiomes. Poétique **et** technique.
2. [`THREAT_MODEL.md`](./THREAT_MODEL.md) — ce que Polygone protège, ce qu'il ne protège PAS.
3. [`ARCHITECTURE.md`](./ARCHITECTURE.md) — l'architecture **réelle** (4 crates).
4. [`STAGING.md`](./STAGING.md) — services archivés + conditions de retour.
5. [`DECISIONS.md`](./DECISIONS.md) — les décisions binaires.

### Documentation produit

| Doc | Contenu |
|---|---|
| [`docs/cli.md`](./docs/cli.md) | Référence complète de la commande `polygone` |
| [`docs/STRATEGIE.md`](./docs/STRATEGIE.md) | Les 3 angles, le pitch, le modèle économique |
| [`docs/config.md`](./docs/config.md) | Fichiers de configuration |
| [`docs/threat-commodity.md`](./docs/threat-commodity.md) | Menace — utilisateur quotidien |
| [`docs/threat-high-value.md`](./docs/threat-high-value.md) | Menace — dissident |
| [`docs/PREMIER-SOIR.md`](./docs/PREMIER-SOIR.md) | 🌙 Le protocole de sortie — premier test avec de vraies personnes |
| [`docs/kill-switch.md`](./docs/kill-switch.md) | Mode duress + runbook opérateur |
| [`LEGAL.md`](./LEGAL.md) | Subpoena, kill-switch, licence AGPL-3.0 |

---

## Statut honnête

- `cargo test --workspace` → ✅ **89 tests** (client 30, core 34, relay 4, daemon 21)
- `cargo build --workspace` → ✅
- Crypto core (`polygone-core`) → ✅ **réelle et testée** : ML-KEM-1024, ML-DSA-65,
  AES-256-GCM, BLAKE3 KDF, Shamir 4-of-7 — tailles exactes vérifiées par tests
- Démo E2E (`polygone-client demo`) → ✅ in-process
- **Signature réseau ML-DSA-65** → ⚠️ générée, **pas encore branchée** au chemin
  réseau (Phase 1 en cours — voir `ARCHITECTURE.md` §11)
- Audit externe → **NON RÉALISÉ** (cf. `LEGAL.md` §5 — l'Axiome 6 attendra)
- CI GitHub → réparée, à valider (les 51 commits post-rc2 n'ont jamais vu la CI)

---

## Pas de tagline sans footnote

> *« L'information n'existe pas. Elle traverse. »*

Signifie littéralement : aucun message ne réside en aucun nœud après son
TTL. Les fragments sont chiffrés, répartis 4-of-7, reconstruits puis
oubliés (`zeroize`). C'est une promesse **de design**, pas une déclaration
métaphysique. Cf. [`PHILOSOPHY.md`](./PHILOSOPHY.md) Axiome 1.

---

## Statut par service

| Service | Statut |
|---------|--------|
| `msg`   | 🟢 **Live** — messages E2E via relay (4/7) |
| `drive` | 🟢 **Live** — fichiers E2E via relay (4/7), `~/.polygone/received/` |
| `brain` | 🟢 **Live** — IA locale (petals → Ollama, zéro cloud) |
| `mesh`  | 🟢 **Live** — découverte LAN (Phase 4) |
| `compute` | 🟢 **Live (MVP)** — prêt + exécution sandboxée (shell + WASM) |
| `hide`, `petals-distribué`, `shell` | ⚪ [`STAGING.md`](./STAGING.md) |

---

## Contribution

Voir [`LEGAL.md`](./LEGAL.md) §6 + [`.well-known/security.txt`](./.well-known/security.txt).

---

*AGPL-3.0 · v2.0.0-rc2 · Hope · Posture « honesty-first ».*
