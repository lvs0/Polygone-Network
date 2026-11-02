# Plan — le bloc adressé par hash (préparation de 2b-12b)

> Décidé en 2b-11a (`docs/ARCHITECTURE_RESEAU.md:105-120`) : **le magasin de blocs adressé par hash,
> servi par un pair, vérifié par le hash**.
> **Le test se fait sur DEUX MACHINES.** Deux processus prouvent que le code marche ; deux machines
> prouvent que le RÉSEAU marche — la seule chose que ce projet n'a jamais démontrée. Sur une seule
> machine la variante locale est un test **PLUS FAIBLE** : elle ne doit pas être présentée comme la preuve.
> Aucun code ici : le §4 ne contient que des commandes shell.

## 1. Ce qui existe, et ce qui est réutilisable tel quel

1. **Hash + attestation — `crates/core/src/receipt.rs`.** `Receipt` (`:13-19`) = sender ‖ receiver ‖ ts ‖
   `payload_hash` ‖ signature, signé ML-DSA-65 (`sign :33`, `verify :41`). **`hash_payload` (`:54-58`) est
   du BLAKE3 réel** : c'est LUI qu'on réutilise tel quel. **Mais** `Receipt` et `hash_payload` n'ont
   **aucun appelant** dans `crates/` + `daemon/src/` (grep vide) et `receipt` n'est même pas réexporté
   (`crates/core/src/lib.rs:12-19`). Un receipt **peut** servir d'attestation de service — en signant le
   `payload_hash` du bloc qu'on vient de servir — mais **il ne le fait pas encore**.
2. **Découverte de pairs — `crates/client/src/mesh.rs`.** UDP **broadcast**, `MESH_PORT = 7642` (`:19`),
   **LAN seulement** (`255.255.255.255`, `:53`), annonce `POLYGONE v1 <node_id> <relay> [<ram_mb>]`
   (`:23-26`) ; `discover` (`:45`) répond au `PING` (`:52`). **Réutilisable tel quel**, et `Peer.addr`
   (`:35`) donne l'IP **source** du pair — donc l'hôte pour un service direct. **Il compile** :
   `cargo check -p polygone-client` → RC=0. CLI déjà présente : `polygone voisins --duree`
   (`main.rs:109-113`), `polygone annoncer --relay` (`main.rs:115-120`).
3. **Stockage — `crates/client/src/drive.rs`.** Écrit dans `~/.polygone/received/` (`received_dir :19-26`),
   nomme par `dir.join(filename)` (`:31`). **Ce qui manque** pour qu'un nom soit un hash : le nom vient de
   l'**émetteur** (`net/mod.rs:1008-1010`, `sanitize_name`), pas du contenu. Il faut que le **receveur**
   nomme après vérification — **l'inverse** du chemin actuel. `drive.rs` est de plus **mort** :
   `mod drive;` (`main.rs:15`) mais **aucun appelant de `DriveFile`** ; la réception réelle passe par
   `net/mod.rs:1025-1031`.
4. **Relais — `crates/relay/src/relay.rs`.** Ne relaie **que** `kind == "fragment"` (`:175`), une **ligne**
   à la fois (`read_line_capped :61`), plafonnée à **64 Kio** (`MAX_LINE :39`), et le corps est décodé par
   `serde_json::from_slice` (`:166`) — un octet y coûte un nombre décimal. **Il ne peut donc PAS porter un
   bloc** : il transporte des *lignes*, pas des blocs.
5. **Ledger — `daemon/src/socket.rs`.** `Ledger` (`:451`) ne connaît que `reserved_ram_bytes` (`:456`) et
   `reserved_cpu_cores` (`:459`), réservés par `reserve` (`:483`) et libérés par `release` (`:500`,
   idempotent), `grants` en mémoire (`:453`). **Réponse : NON, il ne peut pas compter d'octets de stockage
   aujourd'hui** — aucun champ, aucune unité. Le premier bloc servi ne passera pas par lui (§5).
6. **Un type « bloc » quelque part ?** `grep -rniE "struct (Block|Chunk|Blob)" crates/ daemon/src/` →
   **RC=1, rien**. `Fragment` (`crates/core/src/envelope.rs:34`) est un **fragment de Shamir**, rôle et
   champs différents. **Non : le type n'existe pas.**

## 2. Le format de bloc — décidé

- **Contenu** : les octets bruts, **rien d'autre**. Pas de nom, pas de date, pas d'auteur : c'est du
  contexte, et le contexte ne se hache pas.
- **Clé = `hash(contenu)`**, BLAKE3-256 sur le contenu **seul**, **jamais** `hash(nom de fichier)` : deux
  fichiers identiques sous deux noms doivent donner le même bloc, sinon le cache ne déduplique rien.
- **Taille maximale : 1 Mio.** Au-delà de 64 Kio (`relay.rs:39`) un transport par ligne est impossible ;
  1 Mio tient dans une requête TCP directe et reste un bloc de cache utile.
- **Chemin sur disque** : `~/.polygone/blocks/<2 hex>/<62 hex>` — dérivé du hash, donc jamais transmis :
  le receveur le recalcule. Deux niveaux pour ne pas entasser 10 000 fichiers dans un dossier.
- **Deux blocs identiques** : le second **écrase** le premier (mêmes octets, même hash, même fichier).
  L'écriture doit donc être **atomique** (écrire à côté, renommer) : un bloc à demi écrit serait un bloc
  invalide sous un nom valide.
- **Vérification côté receveur** : **avant** l'écriture, en mémoire, sur les octets reçus. Si le hash
  diffère → **le bloc est supprimé, jamais gardé**, et l'échec est signalé avec les deux hashs. Garder un
  bloc qu'on sait faux, c'est empoisonner le cache.
- **Ce qui n'est PAS dans le bloc** : expiration, version du calculateur, auteur, signature. Ces champs
  vivront dans l'index et l'attestation (§3) : les mettre dans le bloc changerait sa clé.

## 3. Le protocole de service — minimal

- **On DEMANDE, on n'annonce pas.** Un pair veut un hash, il le **demande** à un pair découvert
  (`mesh.rs:45`). Annoncer exigerait de savoir qui détient quoi — un index distribué, bien plus gros que
  le premier bloc.
- **Deux messages, une connexion** : `GET <hex64>\n` → `HAVE <len>\n<octets bruts>` ou `MISS\n`. **Pas de
  session, pas de handshake, pas de reprise.** Le demandeur essaie un pair à la fois, avec un **délai de
  5 s** ; à l'expiration, le pair est déclaré mort et le suivant (`mesh.rs:35`) est essayé.
- **S'il n'a pas le bloc** : il répond `MISS`, et il le dit. **S'il est lent** : délai dépassé, on ne
  l'attend pas. **S'il ment** : impossible de mentir sur le contenu, le demandeur recalcule le hash (§2) ;
  mentir sur `HAVE` en envoyant autre chose est un **rejet au hash**, pas une corruption.
- **Ce n'est PAS le protocole v1 du daemon.** Le v1 (`daemon/src/socket.rs`) est entre **un consommateur
  et SON daemon**, en NDJSON sur `~/.polygone/daemon.sock`, avec versions et ledger. Celui-ci est entre
  **PAIRS**, en TCP direct, sans état. **Deux protocoles, deux rôles : ne pas les confondre ni les fondre.**

## 4. Le test falsifiable — DEUX MACHINES, même LAN

> Commandes visant l'outil de l'incrément suivant (`polygone bloc …`) ; les étapes 0, 3 et 4 utilisent
> l'existant. **A** = serveuse, **B** = demandeuse. Sur une seule machine : test **plus faible**.
> Note : `b3sum` est absent de la machine et `b2sum` n'est **pas** BLAKE3 — la vérification ne peut donc
> venir que de l'outil lui-même. C'est cohérent avec le §2 : c'est le **receveur** qui vérifie, pas un script.

**0 — Les deux machines se voient.** A : `polygone annoncer --relay 127.0.0.1:7000`. B :
`polygone voisins --duree 3` → **attendu** : A apparaît avec son IP. *Échec* : le LAN filtre le broadcast
(`mesh.rs:53`) — sans découverte, rien à tester.

**1 — A produit un bloc.** A : `head -c 262144 /dev/urandom > /tmp/bloc.bin` puis
`polygone bloc mettre /tmp/bloc.bin` → **attendu** : `hash = <64 hex>`, et le fichier est sous
`~/.polygone/blocks/<2>/<62>`. B note le hash. A ne partage alors **aucun dossier** avec B. *Échec* :
hash absent, ou fichier ailleurs qu'au chemin dérivé.

**2 — B le demande par le réseau.** B : `polygone bloc obtenir <hash> --chez <ip-de-A>` → **attendu** :
`262144 octets — hash OK`, fichier écrit sous `~/.polygone/blocks/…`, et le `sha256sum` **égal** à celui
que A imprime sur son original. *Échec* : « hash invalide » → le transport a menti, ou B avait déjà un
bloc corrompu.

**3 — A est tuée ; B ne reçoit plus rien.** B : `rm -rf ~/.polygone/blocks`. A : `kill -9` du serveur.
B relance la même demande → **attendu** : `injoignable` en **≤ 5 s**. *Échec* : B obtient les octets →
**ils étaient locaux**, le réseau n'a rien prouvé — c'est exactement ce que cette étape existe pour tuer.

**4 — Un faux serveur renvoie un octet modifié.** A (ou n'importe qui sur le LAN) :
`cp /tmp/bloc.bin /tmp/faux.bin`,
`printf '\x00' | dd of=/tmp/faux.bin bs=1 seek=100 conv=notrunc`,
`socat TCP-LISTEN:9001,reuseaddr,fork SYSTEM:'printf "HAVE %s\n" "$(stat -c%s /tmp/faux.bin)"; cat /tmp/faux.bin'`.
B : `polygone bloc obtenir <hash> --chez <ip-de-A> --port 9001` → **attendu** : `bloc invalide` avec les
deux hashs, et **aucun fichier écrit**. *Échec* : B sert le bloc → la vérification était décorative.
*Contrôle* : le même faux serveur alimenté par `/tmp/bloc.bin` (non modifié) doit, lui, être **accepté** —
sinon le refus venait du faux serveur, pas de l'octet.

## 5. Ce qu'on ne fait pas maintenant

- **Ledger de stockage** : `daemon/src/socket.rs:453` est un `HashMap` en mémoire ; le rendre durable et le mettre d'accord avec un octet durable attend qu'un bloc ait été servi une fois.
- **Fragmentation (Shamir)** : `Fragment` (`crates/core/src/envelope.rs:34`) fragmente pour la confidentialité, pas pour la disponibilité ; un bloc se réplique.
- **NAT traversal** : le mesh est **LAN seulement** (`mesh.rs:53`) ; deux machines du même réseau suffisent à prouver la boucle.
- **DHT** : un index distribué suppose qu'il y ait **plusieurs** blocs et **plusieurs** détenteurs.
- **Chiffrement du bloc** : le hash **est** la vérification ; chiffrer empêcherait de servir un pair qui n'a pas la clé.

## Découvertes inattendues

**La primitive demandée existe déjà — à moitié.** `crates/core/src/envelope.rs:44` déclare
`Fragment.content_hash` avec le commentaire « **BLAKE3 hash of the full ciphertext** », mais le **seul**
code qui le remplit est `Fragment::dummy_hash` (`:51-57`), un pliage XOR, et son propre commentaire dit
« *Dummy content hash for development — real impl uses blake3* » (`:50`) ; il est appelé au test `:163`.
**Un champ dont la documentation dit BLAKE3 et dont le code fait du XOR** : ne pas le prendre pour la
primitive attendue. Le vrai BLAKE3 est `receipt.rs:54`.

**Deux autres écarts, à ne pas confondre avec du travail fait.** (1) `Receipt` est du code mort : zéro
appelant, pas réexporté (`crates/core/src/lib.rs:12-19`) — l'attestation de service est **à faire**.
(2) `drive.rs` est un module mort (`main.rs:15`) doublé par `net/mod.rs:1025` : **deux** `received_dir`,
et celle qui reçoit vraiment n'est pas celle du Drive.
