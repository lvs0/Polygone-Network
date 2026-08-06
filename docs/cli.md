# cli.md — Référence de la commande `polygone`

> *Documentation exhaustive de la commande produit v2.0.0-rc2.*

---

## Sans argument — la TUI (D1)

```text
polygone
```

Lance la TUI : écran d'accueil (identité, uptime, services, crypto),
puis tout se passe derrière `:` (style vim, Axiome 2) :

| Commande | Rôle |
|---|---|
| `:envoyer` | chiffrer + fragmenter un message (ML-KEM-1024 → Shamir 4/7) |
| `:recevoir` | reconstruire + déchiffrer (≥ 4 fragments) |
| `:voisins` | scanner le LAN (mesh, Phase 4) |
| `:compute` | ressources locales + nœuds fantômes (RES) |
| `:executer <tâche>` | exécution sandboxée sur un fantôme du LAN |
| `:ia <question>` | l'IA locale répond (petals → Ollama, zéro cloud) |
| `:demo` | démo E2E — relay aveugle + audit « on voit rien » |
| `:clef` | votre clef publique ML-KEM-1024 |
| `:statut` | rafraîchir l'affichage |
| `:quitter` | sortir proprement |

Échap annule la commande en cours. Ctrl-C quitte.

## Sous-commandes

### `polygone test`
Self-test cryptographique réel — 7/7 assertions (ML-KEM, AES-GCM,
BLAKE3 KDF, Shamir 4/7 + 3/7, ML-DSA sign/verify + tamper).
Exit 0 uniquement si tout est vert.

### `polygone demo`
La démo E2E complète : Alice → relay aveugle → Bob, avec audit
(« on voit rien ») + simulation d'adversaire (3/7 et 7/7 sans clé).

### `polygone envoyer [--dest <clef>] [--via <relay> --a <node>] [--fichier <path>] <message>`
- Sans options : auto-démo (clef fraîche), imprime le format filaire
  (`KEM_CT`/`SENDER_PK`/`FRAG`) sur stdout.
- `--dest <hex>` : chiffre pour la clef donnée (ML-KEM-1024).
- `--via <relay:port> --a <node_id> -d <clef>` : envoie à travers le
  relay aveugle vers le nœud destinataire.
- `--a <node_id> -d <clef>` (sans `--via`) : trouve le relay du
  destinataire sur le LAN (mesh) — zéro configuration.
- `--fichier <path>` : envoie un fichier (le destinataire le reçoit
  dans `~/.polygone/received/`).

### `polygone recevoir [fichier | -]`
Reconstruit (≥ 4/7) et déchiffre le format filaire (fichier ou stdin).

### `polygone ecouter [--relay <addr>] [--annoncer] [--compute]`
Écoute en continu les messages/fichiers via le relay.
- `--annoncer` : annonce aussi le nœud + relay sur le LAN (mesh).
- `--compute` : agit en nœud fantôme RES — accorde les requêtes.

### `polygone voisins [--duree <s>]`
Scanne le LAN (UDP 7642) et liste les nœuds Polygone + leurs relays.

### `polygone annoncer --relay <addr>`
Annonce le nœud + relay sur le LAN, répond aux PING de découverte.

### `polygone compute [--emprunter <node> --via <relay>] [--executer "<cmd>" --emprunter <node> --via <relay>]`
- Sans option : RAM libre locale + nœuds fantômes du LAN (RES).
- `--emprunter <node> --via <relay>` : demande du compute au fantôme,
  affiche le grant reçu.
- `--executer "<cmd>" --emprunter <node> --via <relay>` : envoie la tâche
  au fantôme, qui l'exécute DANS sa sandbox (MemoryMax 256 Mo,
  NoNewPrivileges, ProtectSystem=strict, PrivateTmp, PrivateNetwork,
  CPU 50 %) et renvoie la sortie via le relay.
- `--wasm <fichier.wasm> --emprunter <node> --via <relay>` : envoie un
  module WASM (compilé `--target wasm32-wasi`) qui tourne dans le sandbox
  wasmi du fantôme ; la sortie revient via le relay.

### `polygone petals <status|models|ask>`
IA locale via Ollama (`POLYGONE_OLLAMA_URL`, défaut 127.0.0.1:11434).
- `status` : modèles installés. `models` : liste brute. `ask <q>` : génère.

### `polygone clef`
Votre clef publique ML-KEM-1024 (hex) — ce qu'on partage pour recevoir.

### `polygone id`
Identifiant de nœud (16 premiers hex de la clef publique).

### `polygone duress [--confirmer]`
Mode duress (Axiome 5) : détruit l'identité + les fichiers reçus.
`--confirmer` requis — irréversible. L'identité se régénère au
prochain lancement.

## Variables d'environnement

| Variable | Effet |
|---|---|
| `POLYGONE_OLLAMA_URL` | URL de l'Ollama local (défaut `http://127.0.0.1:11434`) |
| `POLYGONE_INSTALL_DIR` | Répertoire d'installation (installateur) |
| `HOME` | Base de `~/.polygone` (identité, fichiers reçus) |

## Fichiers

| Fichier | Rôle |
|---|---|
| `~/.polygone/identity.json` | Identité (clés + pseudo), chmod 600 |
| `~/.polygone/received/` | Fichiers reçus via le relay |
| `~/.config/polygone/daemon.toml` | Config du daemon `polygoned` |

---

*Référence CLI · v2.0.0-rc2 · « On voit rien. Et c'est comme ça que ça devrait être. »*
