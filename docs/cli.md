# cli.md — Référence de la commande `polygone`

> **Instantané daté — ne pas lire comme l'état courant.** Sa date de rédaction n'est pas indiquée ; il ne décrit pas l'état vérifié aujourd'hui, qui se lit dans [`ETAT-REEL.md`](../ETAT-REEL.md) et [`STATE.md`](../STATE.md).

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

### `polygone premier-soir [--ttl <s>]`
Le scénario guidé de 5 minutes — la promesse « Le message meurt. Regarde. »
comme expérience : carte → 7 fragments naissent → TTL réel qui tourne
(défaut 30 s) → 4/7 reconstruisent → `verite` prouve que rien ne reste →
carnet d'observation à commiter. `--ttl` réduit l'attente (tests, démo).

### `polygone verite`
Forensique locale : énumère TOUT ce que ce nœud garde (identité, ancres
`peers.json`, fichiers `received/`, scores), le classe (à moi / à toi /
rien), puis rend le verdict — « voici ce que j'ai de toi : rien » quand il
n'y a aucune donnée de message sur disque. La confiance devient une
interaction, pas une lecture de README.

### `polygone carte`
La clé comme objet social : l'identité (pseudo, empreinte ML-KEM,
empreinte ML-DSA, adresse ⬡) affichée en carte encadrée — à montrer et
échanger en personne. C'est le résidu social du Premier Soir.

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
  dans `~/.polygone/received/`). Le relay borne chaque trame NDJSON à 64 Kio :
  les fichiers dont un fragment sérialisé dépasse cette limite sont refusés
  avant toute connexion/envoi. Pour les gros blocs, utilisez le service direct
  `polygone bloc` / `polygone node`.
- `--stdin` : lit le message depuis stdin — rien n'apparaît dans
  l'historique shell. À préférer pour les messages sensibles
  (cf. `docs/kill-switch.md`). Exemple :
  `printf 'secret' | polygone envoyer -d <clef> --stdin`
  Symétrique de `polygone recevoir -`. Mutuellement exclusif avec
  `--fichier`.

### `polygone recevoir [fichier | -]`
Reconstruit (≥ 4/7) et déchiffre le format filaire (fichier ou stdin).

### `polygone ecouter [--relay <addr>] [--annoncer] [--compute]`
Écoute en continu les messages/fichiers via le relay.
- `--annoncer` : annonce aussi le nœud + relay sur le LAN (mesh).
- `--compute` : agit en nœud fantôme RES — accorde les requêtes.

### `polygone doctor`

Le diagnostic honnête : ce que la machine **mesure**, ce qu'elle peut
**réserver**, ce que l'OS **applique** — et rien d'autre.

```
⬡ Polygone doctor
  Daemon : joignable · palier balanced · séquence 42

  Capacité RAM
    Mesurée   : 15.50 Gio
    Réservable: 7.75 Gio
    Contrainte: 0.00 Gio

  Isolation OS (ce processus)
    Cgroup    : /user.slice/user-1000.slice/session-2.scope (scope Polygone : non)
    MemoryMax : illimitée (memory.max = max)
    CPUQuota  : illimitée (cpu.max = max)

  Conclusion : licence à respecter — aucune limite OS confirmée
```

- **Contrainte** vient du ledger du daemon. `0` veut dire « engagement
  logiciel », pas « quota appliqué » : le daemon ne pose qu'une priorité
  `nice` et le publie tel quel. Une `Capacity` absente ou incohérente fait
  échouer la commande plutôt que d'afficher des zéros rassurants.
- **Isolation OS** est lue dans `/proc/self/cgroup`, puis `memory.max` et
  `cpu.max` du processus qui lance `doctor` — pas une promesse sur le daemon.
  Un fichier illisible donne « Non mesurée », jamais une valeur inventée ;
  `max` donne « illimitée », jamais `0.00 Gio`.
- Pour qu'un nœud soit réellement plafonné : `polygone node servir --isoler`
  (scope `systemd --user`, cgroup v2), ci-dessous.

Codes de sortie : `0` rapport produit ; `1` daemon indisponible, `Capacity`
absente/incohérente.

### `polygone node servir [--ecoute <addr>] [--ram-gb <Gio>] [--coeurs <n>] [--isoler]`
Démarre le service de blocs pair-à-pair **après** obtention d'un bail auprès
du daemon local `polygoned`. Valeurs par défaut : `0.0.0.0:7643`, `0.25` Gio,
`1` cœur. Le port n'est ouvert qu'après l'accord du daemon.
- `--ecoute` : adresse TCP ; `0.0.0.0` rend le service accessible sur les
  interfaces réseau, sous réserve du pare-feu et du réseau local.
- `--ram-gb` et `--coeurs` : ressources demandées au daemon. Sans `--isoler`,
  ce sont des engagements que le consommateur s'auto-impose, pas des quotas OS.
- `--isoler` (Linux uniquement) : relance le processus dans un scope systemd
  utilisateur dédié sous `app.slice`, avec `MemoryMax` égal à la RAM demandée,
  `CPUQuota` égal au nombre de cœurs × 100 % et `TasksMax=64`. Le client vérifie
  ensuite le chemin `/proc/self/cgroup`, `memory.max` et `cpu.max` avant d'ouvrir
  le port ou de demander le bail. Le scope est supprimé à l'arrêt. Si systemd ou
  cgroup v2 ne sont pas disponibles, la commande échoue sans démarrer le nœud.
- Ce scope est séparé de `session-*.scope` : la session graphique n'est pas
  plafonnée. La limite mémoire couvre le processus entier, pas seulement les
  blocs servis ; choisir une RAM trop basse peut faire terminer le processus.
- Ctrl+C arrête le service et rend explicitement le bail. Si le daemon disparaît
  ou signale un retrait de ressources, le nœud ferme son écoute.
- Une fois l'hystérésis atteinte, `shrinking` reste visible pendant toute la
  contraction ; le nœud rend alors le bail. Les nouvelles demandes sont refusées
  avec `policy_shrinking` jusqu'à ce que la cible de ressources soit atteinte.

**Limite importante :** sans `--isoler`, le plafond de transferts simultanés est
auto-imposé par le processus et aucune limite CPU/RAM OS n'est appliquée. Avec
`--isoler` sous Linux, systemd/cgroup v2 impose bien `MemoryMax` et `CPUQuota`
au processus. Ce service ne transforme pas la RAM distante en RAM locale.
macOS n'a pas encore de mécanisme d'isolation équivalent branché.

### `polygone pause [--duree <30m|2h>]`

Suspend le service pendant une durée **bornée** (10 min à 4 h) : arrête le
processus qui sert (le bail meurt avec sa connexion — le daemon ne conserve
rien), puis écrit une intention horodatée dans `~/.polygone/pause.json`.

```
⬡ Pause enregistrée : 30 min, jusqu'à 1793771400 (epoch)
  Bail : service pid 4242 arrêté (SIGTERM), bail rendu
  Intention : /home/moi/.polygone/pause.json
```

- Une durée hors bornes (`9m`, `5h`, `30s`) est refusée : ce n'est pas une pause.
- Aucun service vivant : la pause est quand même enregistrée, et la ligne le dit.
- **Ce n'est pas un état système** : la pause ne survit pas à un redémarrage tant
  que CLI.8 n'existe pas. C'est écrit sur disque pour être relu, pas pour être
  garanti après reboot.

### `polygone reprendre`

Consomme l'intention et redemande un bail au daemon.

- Aucune intention : refus nommé (`aucune pause en cours`).
- Intention expirée : refus nommé (`pause expirée depuis N s`) — aucun bail
  n'est demandé, et l'intention est consommée par ce refus.
- Bail refusé par le daemon : l'intention **reste** en place, pour réessayer
  sans perdre la durée restante.
- Bail obtenu : l'allocation est rendue aussitôt et la ligne le dit — le service
  lui-même se relance avec `node servir`, qui rouvre sa prise d'écoute.

### `polygone desinstaller [--confirmer]`

Énumère **tout** ce que la désinstallation toucherait, puis ne supprime qu'avec
`--confirmer`.

```
⬡ Désinstallation — 6 chemin(s) existant(s) sur 12 énuméré(s)
  • identity.json — fichier (21550 o)
  • ledger.json — fichier (425 o)
  • blocks/ — dossier (12 entrée(s))
  · received/ — absent
  • ~/.local/bin/polygone
⬡ 9 chemin(s) supprimé(s). Retirez la ligne PATH de votre rc shell…
```

- Sans `--confirmer` : exit 1, `confirmation requise`, **aucune écriture**.
- Racines connues uniquement : `~/.polygone`, `~/.config/polygone`,
  `~/.local/share/polygone`, et les binaires `~/.local/bin/polygone*`. Un chemin
  hors de là est refusé, jamais supprimé.
- Un daemon qui répond encore fait échouer la commande : supprimer son socket
  pendant qu'il tourne le ferait recréer une arborescence à moitié.
- La ligne `PATH` du rc shell n'est **pas** retirée automatiquement : elle
  appartient à votre configuration, la sortie le rappelle.

### `polygone node status`
Affiche l'état local du service, l'âge réel du bail, les compteurs et une
découverte mesh mesurée à l'instant. Un état périmé est explicitement marqué.

### `polygone voisins [--duree <s>]`
Scanne le LAN (UDP 7642) et liste les nœuds Polygone + leurs relays.
Les annonces UDP ne sont pas authentifiées : elles servent à découvrir des
points de contact, jamais à prouver une identité ni à garantir la RAM annoncée.
`bash scripts/mesh-smoke.sh` teste l'annonce, la réponse au PING et la redécouverte
du nœud local sur le LAN.

**Interfaces utilisées.** Le scan et l'annonce n'emploient jamais `0.0.0.0` :
un socket est ouvert par interface autorisée, lié à l'adresse de cette interface,
et le datagramme part vers son broadcast dirigé. Le loopback, les adresses
link-local (`169.254.0.0/16`) et les interfaces VPN ou de conteneur (`tun*`,
`tap*`, `wg*`, `tailscale*`, `docker*`, `br-*`, `veth*`, `virbr*`, `ve-*`, `zt*`,
`utun*`, `ppp*`) sont toujours exclus : une annonce ne doit pas partir sur le
réseau du café ni sur le VPN (MESH.1). Sans interface autorisée, la commande
échoue en le disant — elle n'annonce rien par défaut.
`POLYGONE_MESH_IFACE=wlp3s0` (ou `eth0,wlp3s0`) restreint la liste. La variable
ne peut que **retirer** des interfaces : nommer `tailscale0` ne le réactive pas.

### `polygone annoncer --relay <addr>`
Annonce le nœud + relay sur le LAN, répond aux PING de découverte, avec la même
sélection d'interfaces que `voisins` (`POLYGONE_MESH_IFACE` pour restreindre).

### `polygone compute [--emprunter <node> --via <relay>] [--executer "<cmd>" --emprunter <node> --via <relay>]`
- Sans option : RAM libre locale + nœuds fantômes du LAN (RES).
- `--emprunter <node> --via <relay>` : demande du compute au fantôme,
  vérifie le grant signé et renvoie un code de sortie non nul en cas de refus.
- `--executer "<cmd>" --emprunter <node> --via <relay>` : envoie la tâche
  au fantôme, qui l'exécute DANS sa sandbox (MemoryMax 256 Mo,
  CPUQuota 50 %, NoNewPrivileges, ProtectSystem=strict, PrivateTmp,
  PrivateNetwork et autres restrictions systemd) et renvoie la sortie via
  le relay. Un code de sortie distant non nul reste un échec signé : il ne
  peut pas être affiché comme une réussite ni augmenter la réputation.
- `bash scripts/compute-smoke.sh` : test d'intégration local avec un relay,
  un nœud prêteur et un emprunteur isolés dans des HOME temporaires ; vérifie
  réussite, échec distant, réputation des deux côtés et ancrage TOFU.
- `--wasm <fichier.wasm> --emprunter <node> --via <relay>` : envoie un
  module WASM (compilé `--target wasm32-wasi`) qui tourne dans le sandbox
  wasmi du fantôme ; la sortie revient via le relay.

### `polygone petals <status|models|ask>`
IA locale via Ollama (`POLYGONE_OLLAMA_URL`, défaut 127.0.0.1:11434).
- `status` : modèles installés. `models` : liste brute. `ask <q>` : génère.

### `polygone serverless [--commande <cmd>] [--seuil <n>] [--parts <n>] <charge utile>`
Prépare une exécution en quorum (`docs/SERVERLESS-SPEC.md`).
- Découpe la charge utile en parts Shamir (défaut 4-of-7) pour le transit,
  puis **vérifie localement** que 4 parts sur 7 la reconstituent.
- Affiche l'identifiant de la demande, son hash, la politique et les parts.

**Honnêteté** : rien n'est envoyé. Le dispatch vers les nœuds fantômes
(`ecouter --compute`) n'est pas branché ; la commande s'arrête après la
vérification locale et le dit à l'écran.

### `polygone clef`
Votre clef publique ML-KEM-1024 (hex) — ce qu'on partage pour recevoir.

### `polygone id`
Identifiant de nœud (16 premiers hex de la clef publique).

### `polygone duress [--confirmer]`
Mode duress (Axiome 5) : détruit l'identité + les fichiers reçus.
`--confirmer` requis — irréversible. L'identité se régénère au
prochain lancement.

### `polygone hide [--via <relay>] [--sortie <node>] [--dest <clef>] [--ecoute <addr>]`
### `polygone hide --circuit <node:clef,node:clef,...>`
Proxy SOCKS5 local (défaut 127.0.0.1:9050) qui route le trafic TCP
à travers le relay aveugle vers un nœud de sortie Polygone.
- `--via <relay:port>` : relay aveugle (défaut 127.0.0.1:7000).
- `--sortie <node_id>` : identifiant du nœud de sortie (16 hex).
- `--dest <clef>` : clé publique ML-KEM-1024 du nœud de sortie (hex).
- `--ecoute <addr>` : adresse d'écoute SOCKS5 (défaut 127.0.0.1:9050).
- `--circuit` : mode planification (dry-run). Valide 1 à 3 sauts, refuse les
  doublons et les clés mal formées, affiche l'ordre de chiffrement, puis
  **s'arrête sans ouvrir de proxy**.

Le nœud de sortie se lance avec `polygone ecouter --hide`.

**Honnêteté** : le relay ne voit que métadonnées + tailles (chiffré E2E).
Le nœud de sortie voit la destination réelle (comme un exit Tor).
Single hop MVP — relay + exit node ensemble peuvent corréler.
Pas de padding, pas de multi-hop — documenté, pas promis.

## Variables d'environnement

| Variable | Effet |
|---|---|
| `POLYGONE_OLLAMA_URL` | URL de l'Ollama local (défaut `http://127.0.0.1:11434`) |
| `POLYGONE_MESH_IFACE` | Restreint les annonces mesh à ces interfaces (ex. `wlp3s0`, `eth0,wlp3s0`) ; ne peut que retirer, jamais réactiver une interface exclue |
| `POLYGONE_GATEWAY_ADDR` | Adresse de la passerelle (défaut `127.0.0.1:8088`) |
| `POLYGONE_SITE_DIR` | Répertoire servi par la passerelle (défaut `web`) |
| `POLYGONE_INSTALL_DIR` | Répertoire d'installation (installateur) |
| `HOME` | Base de `~/.polygone` (identité, fichiers reçus, blocs et état du nœud) |
| `POLYGONE_DAEMON_SOCKET` | Socket IPC du daemon local (défaut `~/.polygone/daemon.sock`) |
| `POLYGONE_LEDGER_PATH` | Chemin du ledger de réservations du daemon |

## Fichiers

| Fichier | Rôle |
|---|---|
| `~/.polygone/identity.json` | Identité (clés + pseudo), chmod 600 |
| `~/.polygone/peers.json` | Ancre de confiance : `node_id → empreinte ML-DSA` (TOFU), chmod 600 |

Les empreintes TOFU affichées sont BLAKE3-256 sur les octets ML-DSA décodés (64 caractères hexadécimaux), à comparer hors-ligne.
| `~/.polygone/received/` | Fichiers reçus via le relay |
| `~/.polygone/blocks/` | Blocs adressés par hash, servis par `polygone node servir` |
| `~/.polygone/node-state.json` | État local et compteurs du service de blocs |
| `~/.polygone/ledger.json` | Réservations persistées par le daemon |
| `~/.config/polygone/daemon.toml` | Config du daemon `polygoned` |

---

*Référence CLI · v2.0.0-rc2 · « On voit rien. Et c'est comme ça que ça devrait être. »*
