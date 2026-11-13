# THREAT-MODEL — modèle de menace du socle Polygone

> Date : **2026-11-09**, HEAD `e2936a3`. Auto-évaluation fondée sur le code et
> les tests existants. **Aucun audit externe n'a été réalisé.** Ce document
> décrit des protections vérifiées et des risques résiduels ; il ne certifie pas
> que le système est sûr.
>
> Complète, sans les remplacer : `THREAT_MODEL.md` (racine),
> `docs/THREAT-MODEL-SERVICES.md`, `docs/threat-commodity.md`,
> `docs/threat-high-value.md`, `docs/threat-msg.md`,
> `docs/PROTOCOL-RELAY-AUTH.md`.

---

## 1. Actifs à protéger

| Actif | Pourquoi | Où il vit |
|---|---|---|
| clé privée ML-KEM | déchiffre tous les messages destinés au nœud | `~/.polygone/identity.json` |
| clé privée ML-DSA | signe et authentifie l'identité | `~/.polygone/identity.json` |
| messages et fichiers reçus | contenu privé | `~/.polygone/received/` |
| contenu des blocs | données utilisateur | `~/.polygone/blocks/` |
| ancres de confiance (`peers.json`) | empêchent l'usurpation | `~/.polygone/peers.json` |
| réservations de ressources | empêchent la surallocation | `~/.polygone/ledger.json` |
| capacité de la machine hôte | abus de calcul, minage, pivot | CPU/RAM/disque/réseau |
| disponibilité du daemon et du relais | service | processus |
| métadonnées de communication | vie privée | relais, journaux |

---

## 2. Adversaires envisagés

| Adversaire | Capacité | Objectif |
|---|---|---|
| pair passif | reçoit des enveloppes | lire, corréler, cartographier |
| pair actif malveillant | envoie des trames forgées, rejoue, usurpe | injecter, tromper, dénier |
| opérateur de relais | voit les métadonnées, peut couper | analyse de trafic, censure |
| processus local malveillant (même UID) | lit les fichiers, parle au socket | vol de clés, faux états |
| tâche exécutée | code non fiable sur l'hôte | évasion, accès aux données, pivot |
| attaquant réseau | MITM, perte, rejeu, lenteur | interception, déni |
| utilisateur maladroit | mauvaises permissions, sauvegardes | fuite involontaire |
| fournisseur de dépendance compromis | code dans la chaîne de build | exécution arbitraire |

**Hors périmètre actuel** : État capable de casser la cryptographie
post-quantique, adversaire avec accès physique et disque non chiffré,
compromission simultanée de la majorité des pairs, attaques par canal auxiliaire
sur l'implémentation `pqcrypto`.

---

## 3. Frontières de confiance

```
[ navigateur ] --HTTP loopback--> [ gateway ] --fichier--> [ nodes.json ]
[ CLI ] --socket Unix 0600--> [ polygoned ] --fichier--> [ ledger.json ]
[ CLI ] --TCP--> [ relais ] --TCP--> [ pair ]
[ tâche ] --systemd/WASM--> [ hôte ]
```

Chaque frontière est un point où une donnée non fiable entre. Les sections
suivantes listent les menaces par frontière.

---

## 4. Menaces et protections

### 4.1 Identité et cryptographie

| Menace | Protection | Preuve | Résiduel |
|---|---|---|---|
| Vol de clé privée | permissions 0600, répertoire 0700, écriture atomique | tests SEC.5, `identity.rs` | disque non chiffré |
| Usurpation d'un pair | signature ML-DSA-65, TOFU ancré | `net/tests.rs`, `peers.json` | premier contact accepté |
| Rejeu d'un message | horodatage signé, fenêtre ±300 s, cache de session | `SEC.3`, relais réel | horloges très dérivées |
| Rotation abusive | nouvelle clé signée par l'ancienne, ancienne révoquée | `FV-SEC-05`, 3 tests | rotation = changement de `node_id` |
| Collision de `node_id` | dérivé de BLAKE3(clé de signature), 128 bits | `core/src/sign.rs` | annuaire non signé |
| Mauvaise clé KEM | déchiffrement échoue, test explicite | `crypto/kem.rs` | — |

### 4.2 Relais

| Menace | Protection | Preuve | Résiduel |
|---|---|---|---|
| Lecture du contenu | charges utiles chiffrées de bout en bout | `threat-msg.md`, audit SEC.10 | métadonnées visibles |
| Usurpation de `from` | `from` comparé à l'identité authentifiée | test du relais réel | — |
| Vol de slot | `HELLO_DENIED`/`AUTH_DENIED`, pas de last-writer-wins | `relay/auth_tests.rs` | — |
| Déni de service | 64 Kio, 200/s/connexion, 100/s/identité, 1 024 connexions, timeouts | tests du relais | saturation réseau en amont |
| Rejeu de handshake | nonce challenge, signature | `auth_tests.rs` | — |
| Analyse de trafic | contenu et nom chiffrés | `relay-metadata-audit.mjs` | graphe, tailles, cadence, IP |
| Perte de message | aucune file hors ligne | comportement assumé | message perdu si pair absent |

### 4.3 Socket daemon

| Menace | Protection | Preuve | Résiduel |
|---|---|---|---|
| Accès d'un autre utilisateur | socket 0600, répertoire 0700 | tests de permissions | même UID = accès complet |
| Trame malformée | validation taille/JSON/version/type | tests `decode_*` | — |
| Épuisement de connexions | 16 connexions, 60 s d'inactivité | tests serveur | un client local peut occuper les slots |
| Saturation CPU | 50 req/s/connexion | test de limitation | 16 connexions × 50/s |
| Famine du ledger | écriture atomique, verrou court | tests ledger | E/S disque sous verrou |
| État mensonger | `runtime_state` pur, mesure absente = 0 | `test_tick_is_the_only_writer` | — |

### 4.4 Gateway HTTP

| Menace | Protection | Preuve | Résiduel |
|---|---|---|---|
| Heartbeat forgé | signature contre clé épinglée | `heartbeat_requires_valid_signature` | vol de la clé de signature |
| Substitution d'identité | clé non fournie dans le corps | `authenticate` | — |
| Rejeu de heartbeat | fenêtre ±300 s | test associé | — |
| Pong rejoué | challenge à usage unique, TTL 30 s | `pong_replay_is_refused` | — |
| Fuite par CORS | CORS `*` | — | une page web locale peut lire l'API |
| Écoute publique | loopback par défaut | configuration | un opérateur peut l'exposer |
| Jeton d'appairage volé | 256 bits, TTL 300 s, usage unique | tests gateway | pas de TLS |

### 4.5 Exécution de tâches

| Menace | Protection | Preuve | Résiduel |
|---|---|---|---|
| Évasion de la tâche | `systemd-run --user` + `ProtectHome`, `PrivateNetwork`, `NoNewPrivileges`, `ProtectSystem=strict` | tests `exec`, smoke compute | même UID : pas de frontière cryptographique |
| Boucle infinie WASM | carburant borné | test de timeout | — |
| Unité orpheline | nom unique + `systemctl stop` | test | — |
| Accès à l'identité | `InaccessiblePaths=~/.polygone` | configuration systemd | shell hors systemd ? |
| Deux exécutions concurrentes | plafond 2 | code | — |

### 4.6 Données au repos et journaux

| Menace | Protection | Preuve | Résiduel |
|---|---|---|---|
| Lecture par un autre utilisateur | 0600/0700 | tests SEC.5 | root, sauvegarde, disque volé |
| Secret dans les journaux | pas de clé ni de token journalisé ; Gitleaks | preuve datée | non rejoué dans ce lot |
| Secret dans git | test `no_tracked_secrets`, hook, CI | test négatif | `git add -f` reste possible |
| Résidus après désinstallation | énumération + confirmation | tests `uninstall` | `--purge` explicite |

### 4.7 Chaîne de livraison

| Menace | Protection | Preuve | Résiduel |
|---|---|---|---|
| Dépendance compromise | `Cargo.lock`, versions épinglées | `Cargo.lock` | pas d'audit de vulnérabilités |
| Binaire falsifié | SHA256SUMS vérifié par l'installateur | `install.sh`, `test-installer.sh` | pas de signature détachée |
| Publication non autorisée | permissions CI en lecture | `.github/workflows/ci.yml` | à vérifier côté runner |

---

## 5. Risques résiduels majeurs

1. **Métadonnées** : le relais voit qui parle à qui, quand, et la taille. Le nom
   de fichier et le contenu sont chiffrés. Ce n'est pas un système d'anonymat.
2. **TOFU** : le premier contact est accepté. Une attaque active au premier
   échange est possible sans vérification hors bande de l'empreinte.
3. **Isolation des tâches** : le shell partage l'UID de l'utilisateur. Les
   protections systemd réduisent les accidents, pas un adversaire déterminé.
4. **Disque non chiffré** : l'identité et les reçus sont en clair, protégés par
   les permissions seulement.
5. **Gateway sans TLS** : un processus local peut lire le trafic loopback.
6. **Dépendances non auditées** : aucun `cargo audit`/SBOM dans ce lot.
7. **Pas de preuve multi-machine** : les protections réseau n'ont été éprouvées
   que sur loopback.

---

## 6. Ce qui n'est pas revendiqué

- Anonymat fort ou résistance à un adversaire global.
- Résistance Sybil (le coût de création d'identité est mesuré à ~2,85 ms).
- Confidentialité du graphe de communication.
- Isolation cryptographique des tâches.
- Disponibilité d'un message si le destinataire est hors ligne.
- Sécurité prouvée formellement.

---

## 7. Déclencheurs de révision

Réviser ce modèle dès que :

- un test multi-machine est exécuté (les hypothèses réseau changent) ;
- le protocole est versionné ou une nouvelle capacité est ajoutée ;
- une API produit expose de nouveaux états ;
- le chiffrement au repos est introduit ;
- une dépendance majeure est mise à jour ;
- un audit externe est réalisé ;
- une faille est divulguée (`SECURITY.md`).

---

## 8. Divulgation

Canal : `SECURITY.md` et `.well-known/security.txt`. Aucune récompense
financière n'est promise (`LEGAL.md`). Ne pas publier de secret dans un rapport
de bug ; joindre une reproduction minimale et la version.
