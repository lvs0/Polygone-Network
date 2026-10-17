# Polygone-Network — pile et formats réellement utilisés

> État observé dans l’arbre local le 16 octobre 2026, `HEAD` `49f7db9`. Le dépôt contient déjà des modifications locales non commitées; ce document ne décrit pas une release publiée et ne constitue pas une nouvelle validation de sécurité.

## Périmètre produit

- **Polygone-Network** est la couche de transport, d’identité et de chiffrement.
- **Polygone Compute** est le point d’intégration applicatif.
- **HYPER** reste une roadmap distincte d’orchestration des ressources. HYPER et Compute ne sont pas fusionnés avec Polygone-Network.

## Résumé de la pile active

| Couche | Implémentation constatée | Rôle et limites |
|---|---|---|
| Client | Rust 2021, binaire `polygone`, workspace Cargo | Identité locale, chiffrement, fragmentation, CLI et consommation du relay. Runtime asynchrone Tokio. |
| Relay | Rust/Tokio, binaire `polygone-relay` | Serveur TCP, port par défaut `7000`; bind sur `0.0.0.0`. Enregistre les nœuds connectés, route vers les nœuds en ligne et abandonne les enveloppes destinées aux nœuds hors ligne. |
| Transport du message/fichier | TCP vers le relay + JSON délimité par LF (NDJSON) | Flux ordonné : `HELLO`, accusé d’enregistrement, puis enveloppes JSON. Le TCP n’est pas chiffré par TLS dans ce chemin; le contenu utile est chiffré de bout en bout par Polygone. |
| Découverte de proximité | UDP broadcast, port `7642` | Annonces et recherche de pairs sur le LAN (`POLYGONE v1 ...`). Ce canal ne transporte pas les messages ni les fichiers. Il est distinct du transfert TCP par relay. |
| Sérialisation du chemin réseau | `serde_json` | Les structures `Vec<u8>` deviennent des tableaux JSON d’octets (`[12,34,...]`), pas du base64 ni un flux binaire compact. |
| Identité de nœud | Identité locale persistée en JSON | `node_id` = les 16 premiers caractères hexadécimaux de la clé publique ML-KEM. L’identité contient également des clés privées encodées en hex; ne jamais partager `identity.json`. |

Les dépendances et commentaires anciens ne constituent pas tous le protocole en production. En particulier, la connexion actuelle du client est `TcpStream` Tokio; ce parcours ne met en œuvre ni libp2p, ni QUIC, ni TLS. `serde_cbor` figure parmi des dépendances du workspace, mais le chemin client↔relay décrit ici sérialise avec `serde_json`. Le type `polygone_core::Envelope` est un modèle séparé/ancien; le relay et le client utilisent `NetEnvelope` dans `crates/client/src/net/mod.rs`.

## Échange réseau réellement observé

1. Le client ouvre une connexion TCP vers `relay:port`.
2. Il écrit `HELLO <node_id>\n` et attend `HELLO_OK\n`. Un identifiant déjà enregistré reçoit `HELLO_DENIED\n`; le client refuse alors de poursuivre.
3. Le client émet une enveloppe par ligne JSON. Le relay accepte les enveloppes dont `kind` vaut `fragment`, vérifie que `from` correspond au `HELLO`, route selon `to` et transmet la ligne JSON telle quelle au pair connecté.
4. Le relay garde en mémoire sa table des connexions, pas une file de messages persistante. Si le destinataire n’est pas connecté, le fragment est abandonné. La connexion d’écoute doit donc être active avant l’envoi.

En-tête de message réseau schématique (les tableaux d’octets sont abrégés) :

```json
{"kind":"fragment","from":"<node-id>","to":"<node-id>","session":"<16 caractères hex>","seq":0,"type":"kem","idx":0,"threshold":4,"total":7,"payload":[...],"sig":[...],"signer":"<clé ML-DSA-65 en hex>","name_ct":[...],"ts":1792160000}
```

- Champs communs : `kind`, `from`, `to`, `session`, `seq`, `type`, `idx`, `threshold`, `total`, `payload`.
- Enveloppe KEM (`type: "kem"`) : `payload` porte le ciphertext ML-KEM; `sig`, `signer` et `ts` portent la signature ML-DSA-65, la clé publique de vérification en hex et l’horodatage signé. `name_ct` est présent pour un transfert de fichier.
- Enveloppe de partage (`type: "frag"`) : `payload` porte un partage Shamir; `idx` est compris entre 1 et 7. Un partage avant l’enveloppe KEM n’est pas accepté par le récepteur.
- `session` est généré à partir de 8 octets aléatoires, encodés en hex. `seq` est informatif sur le chemin message; l’anti-rejeu repose sur la fraîcheur signée et le cache des sessions terminées, pas sur une séquence stricte.
- Le relay impose une limite de 64 Kio par ligne et un plafond de débit par connexion; il inspecte les champs de routage et ne déchiffre pas les contenus. Il voit toutefois les identifiants/routages et des métadonnées de session : le transport TCP clair n’est pas un tunnel TLS.

## Identité, confiance et chiffrement

- Première utilisation : génération d’une paire ML-KEM-1024 et d’une paire ML-DSA-65. Elles sont stockées dans `$HOME/.polygone/identity.json` sous forme JSON/hex; le programme tente d’appliquer `chmod 600` sur Unix. Les clés privées restent locales, mais le fichier n’est pas un coffre chiffré au repos.
- À partager pour recevoir : la clé publique ML-KEM affichée par `polygone clef` et le `node_id` affiché par `polygone id`. Ne jamais transmettre les champs privés ni le fichier d’identité.
- Le message est encapsulé pour le destinataire par **ML-KEM-1024** (FIPS 203); le secret partagé de 32 octets dérive une clé de session de 32 octets par BLAKE3 avec le domaine `polygone session key v1`.
- **AES-256-GCM** chiffre le contenu avec un nonce aléatoire de 12 octets et un tag d’authentification de 16 octets. La forme binaire passée à Shamir est `[nonce 12 octets][ciphertext + tag]`.
- Cette séquence chiffrée est répartie en **7 parts Shamir, seuil 4**. Un envoi de message comporte une enveloppe KEM et jusqu’à sept enveloppes de partage; le destinataire reconstruit avec au moins 4 parts.
- Le KEM envelope est signé en **ML-DSA-65** (FIPS 204). La signature couvre le domaine `polygone-net-v2\0`, la session, `from`, `to`, l’horodatage, le ciphertext KEM et le ciphertext symétrique reconstruit. Le récepteur n’accepte l’horodatage que dans une fenêtre de ±300 secondes.
- Au premier message valide, TOFU enregistre automatiquement la clé de signature d’un pair dans `$HOME/.polygone/peers.json`; le CLI ne bloque pas sur une confirmation manuelle. Les scripts ci-dessous comparent seulement l’empreinte de la clé publique ML-DSA déjà apprise. Ils ne lient pas cette clé à la KEM publique échangée séparément, ne constituent pas une procédure complète de pairing prise en charge et ne doivent pas autoriser l’envoi de contenu sensible.
- Les défauts P1/P2 TOFU (`replace` qui modifie puis échoue et backup orphelin ignoré par `verite`) sont corrigés et revus par Tilleul sur le snapshot de validation des 24 chemins, SHA `33626ec120864167a107ceaa66164317e2d6cc9577cd792a671b74389c5bd0bd`; Polygone rapporte 206/206 workspace, Clippy, fmt, release, smoke CLI et forensic loopback PASS. Le test Windows `peers_store_first_launch_creates_missing_parent` est présent mais n’a pas été compilé/exécuté (`#[cfg(windows)]`; échec MinGW `features.h` dans `pqcrypto-mlkem`), donc Windows reste non démontré. Modal est suspendu, sans ressource créée; aucun transfert entre hôtes physiques ou environnements n’a été exécuté. Les mises à jour documentaires ci-dessous sont postérieures au digest. Voir `RELEASE-STATUS.md`; ne pas conclure à une durabilité universelle.

### Comparaison partielle de la clé ML-DSA — ne valide pas le bundle complet

Le CLI affiche seulement un préfixe de 16 caractères hexadécimaux pour l’empreinte ML-DSA (`polygone carte` et l’avis de nouveau pair). À titre de vérification technique partielle, calculer localement SHA-256 de la **clé publique ML-DSA seulement** sur A :

```bash
python3 - "$HOME/.polygone/identity.json" <<'PY'
import hashlib, json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    identity = json.load(f)
print(hashlib.sha256(bytes.fromhex(identity["sign_pk_hex"])).hexdigest())
PY
```

Après que B a reçu un premier message d’essai d’A et que `peers.json` contient l’association TOFU, calculer localement l’empreinte de la clé publique enregistrée pour le `node_id` d’A :

```bash
python3 - "$HOME/.polygone/peers.json" "<node_id-A>" <<'PY'
import hashlib, json, sys
with open(sys.argv[1], encoding="utf-8") as f:
    peers = json.load(f)
print(hashlib.sha256(bytes.fromhex(peers[sys.argv[2]])).hexdigest())
PY
```

Comparer les deux empreintes SHA-256 par un canal indépendant. Ne jamais transmettre `identity.json`, son contenu, ni une clé privée; les commandes ci-dessus n’affichent que l’empreinte de signature. TOFU est automatique au premier contact et n’attend pas cette comparaison; utiliser uniquement un fichier synthétique pour ce premier échange et interrompre l’essai si les empreintes diffèrent. Cette concordance ne valide pas l’identité KEM+ML-DSA complète; aucune commande ou procédure de bundle canonique n’est actuellement prise en charge. Ne pas envoyer de contenu sensible sur la base de cette vérification partielle.

## Format Drive réellement émis

Le transfert réseau de fichier découpe le contenu en blocs de **12 Kio**. Chaque bloc est chiffré et transporté dans sa propre session signée (un KEM + sept parts). Le plaintext de chaque bloc commence par :

```text
"PGDRIVE1" | transfer_id[16] | seq[u32 little-endian] |
 total[u32 little-endian] | filename_len[u32 little-endian] |
 filename[UTF-8] | data
```

Le nom est aussi placé dans `name_ct`, chiffré avec la clé de session de l’enveloppe KEM; le relay ne le lit pas. La limite d’entrée réseau annoncée par le code est de 256 Mio. Le récepteur ne crée le fichier que lorsque tous les blocs cohérents sont présents, dans `$HOME/.polygone/received/`; le client affiche un BLAKE3 du fichier reçu. Le script de smoke fabrique désormais un fichier synthétique de **3 blocs** (36 864 octets), puis compare le hash d’envoi au hash reçu fourni par l’opérateur; il ne revendique pas avoir effectué de test physique.

## TTL et risque restant — inchangés

Le TTL d’inactivité des sessions partielles est actuellement **300 secondes** (`SESSION_TTL_SECS`). La purge périodique est effectuée toutes les 30 secondes. Cette tâche n’a modifié ni le TTL ni le comportement du récepteur. Le risque pré-authentification déjà consigné reste à signaler : l’occupation potentielle d’un slot peut atteindre **environ 15 min 30 s** selon la fenêtre d’horodatage et le renouvellement par fragments distincts. Aucune durée absolue non renouvelable ni politique de mitigation n’est choisie ici; ce choix reste à L‑VS.

## Smoke test minimal à deux hôtes

**État : préparé, non exécuté entre deux hôtes physiques.** Les tests loopback consignés dans `RELEASE-STATUS.md` ne remplacent pas cette validation. Ce scénario emploie le relay sur l’hôte B et ne teste pas la découverte UDP; il vérifie un transfert chiffré réel A→B sur TCP. Le gate Modal de ce cycle, s’il est exécuté, teste seulement deux environnements distincts et ne doit pas être présenté comme preuve physique. Les deux machines doivent utiliser le même snapshot de code/binaire et avoir des horloges à moins de 300 secondes l’une de l’autre.

Utiliser un `HOME` de test dédié sur chaque hôte, différent du `HOME` habituel et distinct entre A et B. Le conserver pendant tout l’essai et dans tous les terminaux de cet hôte afin que `identity.json` et `peers.json` restent cohérents. Ne pas utiliser de répertoire temporaire supprimé à la fermeture.

### Hôte B — destinataire et relay

Dans le dépôt, construire si nécessaire :

```bash
cargo build --release --bin polygone --bin polygone-relay
```

Dans chacun des terminaux de B (relay et écoute), initialiser le même `HOME` dédié :

```bash
export POLYGONE_SMOKE_HOME="${POLYGONE_SMOKE_HOME:-$HOME/.polygone-two-host-b}"
mkdir -p "$POLYGONE_SMOKE_HOME"
export HOME="$POLYGONE_SMOKE_HOME"
```

Récupérer les valeurs publiques (les commandes créent l’identité au premier lancement) :

```bash
target/release/polygone id
target/release/polygone clef
```

Partager avec A le `node_id` et **la première ligne** de `clef` (clé ML-KEM publique). Ne pas partager `identity.json`. Dans le pare-feu de B, **autoriser uniquement l’IP source de A vers TCP/7000** (le relay écoute sur `0.0.0.0`; ne pas exposer le port plus largement). Dans un terminal B, démarrer le relay :

```bash
bash scripts/smoke-two-hosts.sh relay 7000
```

Dans un second terminal B, démarrer une seule instance d’écoute, qui se connecte au relay local :

```bash
bash scripts/smoke-two-hosts.sh listen 127.0.0.1:7000
```

Laisser ce processus actif. Sa bannière affiche le `node_id`; vérifier qu’il correspond à celui communiqué.

### Hôte A — envoi

Depuis le même dépôt/binaire, lancer (remplacer les trois valeurs) :

```bash
export POLYGONE_SMOKE_HOME="${POLYGONE_SMOKE_HOME:-$HOME/.polygone-two-host-a}"
mkdir -p "$POLYGONE_SMOKE_HOME"
export HOME="$POLYGONE_SMOKE_HOME"
bash scripts/smoke-two-hosts.sh send <IP-ou-nom-de-B>:7000 <node_id-de-B> <clé-ML-KEM-publique-de-B>
```

Le script fabrique un fichier-témoin unique de trois blocs de 12 Kio, l’envoie via `polygone envoyer --fichier ... --via ... --a ...`, demande le BLAKE3 affiché par B et compare les deux hashes; il efface ensuite sa copie locale temporaire. L’attente d’envoi est bornée par `POLYGONE_SEND_TIMEOUT_SECONDS` (défaut 180 s, plage 1–3600 s; GNU coreutils `timeout` requis). Exemple pour fixer 300 secondes :

```bash
POLYGONE_SEND_TIMEOUT_SECONDS=300 bash scripts/smoke-two-hosts.sh send <IP-ou-nom-de-B>:7000 <node_id-de-B> <clé-ML-KEM-publique-de-B>
```

En cas d’expiration, le script arrête l’envoi par SIGTERM puis SIGKILL au besoin; pour arrêter aussi l’écouteur ou le relay, faire Ctrl-C dans le terminal correspondant.

### Critères de réussite

- A affiche l’envoi terminé et un `BLAKE3 <hash>`.
- B affiche la réception du fichier dans `$HOME/.polygone/received/` du HOME de test isolé, avec le même hash BLAKE3 et une taille non nulle.
- Après le premier message synthétique, les empreintes SHA-256 ML-DSA de cette vérification partielle concordent; ne partager ni `identity.json` ni les clés privées. Cela ne valide pas la liaison entre la clé de signature et la KEM du pair.
- Un échec de connexion/timeout, l’absence de réception ou un hash différent signifie **FAIL**. L’affichage d’envoi côté A seul n’est pas un accusé de réception applicatif. Le script vérifie trois blocs, pas un transfert de 256 Mio.

Ce smoke ne prouve ni une disponibilité en présence de perte réseau, ni la sécurité contre saturation, ni le comportement multi-hop; il ne modifie pas les TTL. Pour un essai réel, noter séparément les OS, versions/binaires, adresses utilisées (sans publier de secrets), résultat, hashes et heure, et mettre à jour le statut uniquement après observation sur le second hôte.
