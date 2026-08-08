# ⬡ Polygone

> **« Le message meurt. Regarde. »**

Un réseau de transit éphémère post-quantique. Un message traversé, vu,
**mort** — avec la preuve que rien ne reste.
**ML-KEM-1024** · **ML-DSA-65** · **AES-256-GCM** · **Shamir 4-of-7** · **BLAKE3**.

**Version : v2.0.0-rc2** · Posture `honesty-first` · AGPL-3.0.
Pas de token. Pas de télémétrie. Pas d'investisseurs.

---

## Une seule promesse, une seule expérience

Pas « chiffré » (Signal le fait), pas « éphémère » (tout le monde le dit) —
**la mort visible du message, avec preuve post-quantique**. L'effacement est
une expérience, pas une propriété :

```bash
polygone premier-soir     # 5 minutes : envoyez, voyez mourir, vérifiez
```

Le scénario guidé : votre carte → 7 fragments naissent → le TTL tourne sous
vos yeux (réellement) → 4/7 reconstruisent → `verite` prouve que rien ne
reste → un carnet d'observation à commiter. C'est le premier utilisateur :
**vous**, ce soir.

Deux commandes font le reste de la promesse, toujours vérifiables :

```bash
polygone verite           # forensique locale : « voici ce que j'ai de toi : rien »
polygone carte            # la clé comme objet social — à échanger en personne
```

Règle produit++ : **toute promesse de ce README est soit un test CI, soit une
commande `polygone *` que vous pouvez lancer vous-même.** Pas de troisième
voie.

---

## Quickstart

```bash
# Installation (Linux/macOS)
curl -fsSL https://github.com/lvs0/Polygone-Network/releases/latest/install.sh | bash

# Ou depuis le repo
cargo build --workspace --release

# Le produit — la TUI (2 commandes : envoyer / quitter, style vim)
polygone

# Le Premier Soir — la promesse, en 5 minutes
polygone premier-soir

# Vérifier l'absence — ce que ce nœud garde de vous
polygone verite

# La clé comme objet social
polygone carte

# Les commandes de transport
polygone demo            # démo E2E post-quantique complète (60 s)
polygone envoyer -d <clef> "message"   # chiffrer + fragmenter (ML-KEM + Shamir 4/7)
polygone recevoir wire.txt             # reconstruire + déchiffrer

# Le vrai réseau (plane 2 — relay)
polygone-relay                          # terminal 1 : le relay
polygone ecouter                        # terminal 2 : Bob écoute
polygone envoyer --via 127.0.0.1:7000 --a <node_bob> -d <clef_bob> "salut"
                                        # terminal 3 : Alice envoie
# → Bob reçoit et déchiffre avec 4/7 fragments. Le relay route, il ne lit pas le contenu.

# Le Drive — un FICHIER chiffré + fragmenté (2e service livré)
polygone envoyer --via 127.0.0.1:7000 --a <node_bob> -d <clef_bob> \
    --fichier ~/documents/secret.txt
# → Bob : ~/.polygone/received/secret.txt — contenu vérifié identique

# Le Mesh — trouver les nœuds du LAN sans adresse en dur
polygone annoncer --relay 127.0.0.1:7000   # Bob annonce son relay sur le LAN
polygone voisins                           # Alice scanne : node + relay trouvés

# L'IA locale (zéro cloud)
polygone petals status                 # modèles installés
polygone petals ask --model phi4-mini:latest "ta question"

# Les 4 binaires du workspace :
#   polygone                 la commande produit (+ TUI, demo, msg, net)
#   polygone-client          alias de build du même binaire
#   polygone-relay           relay (stateless, routage)
#   polygoned                daemon d'allocation de ressources
```

Pas de YAML. Pas de `config.toml`. Pas de provider à choisir.

---

## Qu'est-ce que ce produit fait (vraiment)

Deux choses, et leur négatif :

1. **Envoyer un message** que le relay ne peut pas lire — chiffré ML-KEM-1024
   + AES-256-GCM, fragmenté Shamir 4-of-7, signé ML-DSA-65, rejoué
   impossible (horodatage signé ±300 s).
2. **Envoyer un fichier** que personne d'autre ne peut lire — même chemin, nom
   chiffré hors-bande (le relay voit des octets opaques).
3. **Rien ne reste.** Les fragments vivent en mémoire (TTL 30 s) et meurent.
   4/7 reconstruisent, puis oublient (`zeroize`). `polygone verite` l'énumère.

**Honnêteté d'architecture (lue dans le code, pas dans le rêve) :** le relay
voit les *métadonnées* de routage (`from`, `to`, `session`, tailles) parce
qu'il route dessus — et rien d'autre. Le modèle de menace est documenté dans
[`docs/threat-commodity.md`](./docs/threat-commodity.md) et
[`docs/threat-high-value.md`](./docs/threat-high-value.md).

**Honnêteté de confiance :** chaque pair a une ancre réelle —
`~/.polygone/peers.json` (TOFU : empreinte ML-DSA apprise au premier contact
vérifié, affichée à l'écoute pour vérification hors-ligne, clé différente
pour un pair connu = rejet). « C'est bien Alice » est signé **et** ancré.

---

## Qu'est-ce que ce produit ne fait PAS

| Pas dans v2.0.0-rc2 | Pourquoi |
|----------------------|----------|
| Browser GUI | La TUI suffit. Pas d'ambition UX. |
| Tor replacement | Polygone-hide pas livré. Voir [`STAGING.md`](./STAGING.md). |
| Cloud sync | Privacy-by-default. |
| Compte utilisateur | Privacy-by-default. |
| Subscription / token | AGPL-3.0, $0, forever. |
| Chiffrement au repos | Décision : effacement par duress, pas coffre. Voir [`DECISIONS.md`](./DECISIONS.md). |

---

## Lisez ceci en premier

1. [`PHILOSOPHY.md`](./PHILOSOPHY.md) — les 5 axiomes. Poétique **et** technique.
2. [`THREAT_MODEL.md`](./THREAT_MODEL.md) — ce que Polygone protège, ce qu'il ne protège PAS.
3. [`ARCHITECTURE.md`](./ARCHITECTURE.md) — l'architecture **réelle** (4 crates).
4. [`STAGING.md`](./STAGING.md) — services archivés + conditions de retour.
5. [`DECISIONS.md`](./DECISIONS.md) — les décisions binaires, dont D5 (relay public assumé).

### Documentation produit

| Doc | Contenu |
|---|---|
| [`docs/cli.md`](./docs/cli.md) | Référence complète de la commande `polygone` |
| [`docs/PREMIER-SOIR.md`](./docs/PREMIER-SOIR.md) | 🌙 Le protocole de sortie — premier test avec de vraies personnes |
| [`docs/BUDGET.md`](./docs/BUDGET.md) | 💶 La soutenabilité du relay — €/mois, noir sur blanc |
| [`docs/STRATEGIE.md`](./docs/STRATEGIE.md) | Les 3 angles, le pitch, le modèle économique |
| [`docs/config.md`](./docs/config.md) | Fichiers de configuration |
| [`docs/threat-commodity.md`](./docs/threat-commodity.md) | Menace — utilisateur quotidien |
| [`docs/threat-high-value.md`](./docs/threat-high-value.md) | Menace — dissident |
| [`docs/kill-switch.md`](./docs/kill-switch.md) | Mode duress + runbook opérateur |
| [`LEGAL.md`](./LEGAL.md) | Subpoena, kill-switch, licence AGPL-3.0 |

---

## Statut honnête

- `cargo test --workspace` → ✅ **108 tests uniques** (produit 46, core 34, relay 7, daemon 21)
- `cargo fmt --check` → ✅ propre
- Crypto core (`polygone-core`) → ✅ **réelle et testée** : ML-KEM-1024, ML-DSA-65,
  AES-256-GCM, BLAKE3 KDF, Shamir 4-of-7 — tailles exactes vérifiées par tests
- **Signature réseau ML-DSA-65** → ✅ **branchée et vérifiée** (Phase 1 + contre-attaque Phase 4) :
  chaque message est signé, vérifié, fail-closed ; ancrage de confiance `peers.json`
- **Rejeu** → ✅ impossible : horodatage signé, fenêtre ±300 s, cache anti-rejeu
- **Relay** → ✅ durci : HELLO authentifié par possession, ack `HELLO_OK`/`HELLO_DENIED`,
  64 KiB/ligne, rate-limit, table shardée, plafond 1024 connexions
- **Sandbox RES** → ✅ bornée : systemd durci + fuel metering WASM + sortie plafonnée
- Démo E2E (`polygone demo`) → ✅ in-process
- Audit externe → **NON RÉALISÉ** (cf. `LEGAL.md` §5 — l'Axiome 6 attendra)
- **Premier Soir (utilisateur réel)** → ⬜ **LE SEUL CHIFFRE QUI COMPTE** — à faire.
  Quand ce sera fait : « testé par N personnes le <date> ».

---

## Pas de tagline sans footnote

> *« Le message meurt. Regarde. »*

Signifie littéralement : aucun message ne réside en aucun nœud après son
TTL. Les fragments sont chiffrés, répartis 4-of-7, reconstruits puis
oubliés (`zeroize`). C'est une promesse **de design**, pas une déclaration
métaphysique — et c'est une **commande** : `polygone premier-soir`.

> *« L'information n'existe pas. Elle traverse. »* — cf. [`PHILOSOPHY.md`](./PHILOSOPHY.md) Axiome 1.

---

## Statut par service

| Service | Statut |
|---------|--------|
| `msg`   | 🟢 **Live** — messages E2E via relay (4/7, signés, anti-rejeu) |
| `drive` | 🟢 **Live** — fichiers E2E via relay (4/7), `~/.polygone/received/` |
| `brain` | 🟢 **Live** — IA locale (petals → Ollama, zéro cloud) |
| `mesh`  | 🟢 **Live** — découverte LAN |
| `compute` | 🟢 **Live (MVP)** — prêt + exécution sandboxée (shell + WASM, authentifié) |
| `verite` | 🟢 **Live** — forensique locale, « voici ce que j'ai de toi : rien » |
| `premier-soir` | 🟢 **Live** — le scénario guidé (la promesse, en 5 min) |
| `carte` | 🟢 **Live** — la clé comme objet social |
| `hide`, `petals-distribué`, `shell` | ⚪ [`STAGING.md`](./STAGING.md) |

---

## Soutenabilité (noir sur blanc)

Le relay public est un bien commun : gratuit pour les utilisateurs, payé par
un budget assumé. Voir [`docs/BUDGET.md`](./docs/BUDGET.md) — coût €/mois,
sources (grants NLnet/Prototype Fund, dons), et la règle : **si le budget ne
tient plus, le relay s'arrête en le disant — pas en silence.**

---

## Contribution

Voir [`LEGAL.md`](./LEGAL.md) §6 + [`.well-known/security.txt`](./.well-known/security.txt).

---

*AGPL-3.0 · v2.0.0-rc2 · Hope · Posture « honesty-first » · « Le message meurt. Regarde. »*
