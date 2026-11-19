# P0 — validation sur deux machines physiques

> Procédure prête, **non exécutée** : une seule machine était disponible le
> 2026-11-09. Ce document est le protocole à rejouer tel quel ; il se termine par
> le tableau de preuves à remplir. Tant que ce tableau est vide, la mission P0
> n'est **pas** validée sur deux machines — les tests locaux ne le remplacent pas.

## 1. Ce que le test doit établir

1. A et B ont des identités distinctes, chacune dérivée de sa clé ML-DSA-65.
2. A s'authentifie auprès du relais (`AUTH_OK`), B aussi.
3. Un message de A arrive chez B, déchiffré, avec une ancre de confiance apprise.
4. Un tiers qui prétend être B est refusé (`HELLO_DENIED`, `AUTH_DENIED duplicate`).
5. Aucune clé privée n'apparaît dans les journaux.

## 2. Prérequis

| Élément | Exigence |
|---|---|
| Binaires | `polygone` et `polygone-relay`, même commit sur les deux machines |
| Réseau | les deux machines joignent le relais ; port TCP 7000 ouvert sur la machine qui héberge le relais |
| Horloges | écart < 300 s (la signature de message porte un `ts`) |
| Identités | un `HOME` par pair, jamais partagé |

Le relais peut tourner sur une troisième machine ou sur B. Il ne détient aucune
clé ; le placer sur une machine tierce est le cas le plus intéressant.

## 3. Préparation (une fois par machine)

```bash
git switch p0/first-secure-network
cargo build --release -p polygone-client -p polygone-relay
install -m 0755 target/release/polygone target/release/polygone-relay ~/.local/bin/
```

## 4. Parcours

Sur la machine **R** (relais), ou sur B :

```bash
polygone-relay --port 7000          # journalise INFO ; aucune clé requise
```

Sur **B** (récepteur) :

```bash
polygone id                          # → NODE_B (32 hex)
polygone clef                        # → CLEF_B (clé ML-KEM-1024, hex)
polygone ecouter --relay <IP_R>:7000
```

Transmettre `NODE_B` et `CLEF_B` à A **par un canal que vous contrôlez**
(message en personne, signal, papier). Ce transfert hors bande est la propriété
de sécurité : si la clé vient du relais, un relais hostile peut lire le message.

Sur **A** (émetteur) :

```bash
polygone id                          # → NODE_A, doit différer de NODE_B
polygone envoyer --via <IP_R>:7000 --a NODE_B -d CLEF_B "P0 deux machines $(date -Is)"
```

## 5. Ce qui doit s'afficher

Chez B, dans les deux secondes :

```text
⬡ message reçu (session <id> · 4/7 fragments)
P0 deux machines 2026-…
  ⬡ NOUVEAU PAIR APPRIS : <NODE_A> → BLAKE3-256 <empreinte> — vérifiez les 64 caractères…
```

L'empreinte affichée doit être comparée **de vive voix** avec celle qu'affiche A.
Une empreinte qui diffère est une attaque ou une erreur d'identité : arrêter.

## 6. Contrôles négatifs (à faire dans la même session)

```bash
# 1. usurpation au niveau du relais : HELLO nu, sans preuve
printf 'HELLO %s\n' "$NODE_B" | timeout 3 nc <IP_R> 7000
# attendu : HELLO_DENIED auth_required

# 2. double enregistrement : relancer `ecouter` sur B pendant que B écoute
polygone ecouter --relay <IP_R>:7000
# attendu chez le second : AUTH_DENIED duplicate (le premier garde son slot)

# 3. rejeu : rejouer la ligne AUTH_RESP capturée sur une nouvelle connexion
#    (tcpdump côté A) → AUTH_DENIED invalid_token
```

## 7. Journal de preuve à remplir

| Champ | Machine A | Machine B |
|---|---|---|
| Nom d'hôte | | |
| `uname -srm` | | |
| Commit (`git rev-parse --short HEAD`) | | |
| `node_id` | | |
| Empreinte BLAKE3-256 de la clé de signature | | |
| Adresse du relais utilisée | | |
| Horodatage de réception | | |
| Message reçu (extrait) | — | |

| Contrôle | Résultat observé | PASS/FAIL |
|---|---|---|
| `AUTH_OK` sur A | | |
| `AUTH_OK` sur B | | |
| message livré et lisible | | |
| empreintes identiques de vive voix | | |
| `HELLO` nu refusé | | |
| double enregistrement refusé | | |
| `AUTH_RESP` rejoué refusé | | |
| aucune clé privée dans les journaux | | |

## 8. Conditions d'échec connues

- **NAT symétrique / CGNAT** : le relais doit être joignable en entrée ; un pair
  derrière un NAT non traversable ne peut pas s'enregistrer. Aucune traversée de
  NAT n'est implémentée.
- **Horloge dérivée > 300 s** : `StaleTimestamp`, message refusé (comportement
  voulu, mais l'erreur doit être lue).
- **Deux pairs avec le même `HOME`** : même identité, `AUTH_DENIED duplicate` sur
  le second. C'est un test d'erreur, pas un déploiement.
- **Relais muet** : `relais … muet : aucune réponse au handshake après 10000 ms`
  (délai ajouté en P0), au lieu d'un blocage indéfini.

## 9. Volet Compute — protocole à rejouer (2026-11-14)

**Statut : toujours non exécuté.** Le second hôte Linux avec shell manque. Les
commandes ci-dessous sont prêtes ; le tableau §7 doit être complété avant toute
revendication de validation physique.

### 9.1 Préparation

Sur **R** (relais, éventuellement B) :

```bash
polygone-relay --port 7000
```

Sur **B** (ghost) :

```bash
polygone id                          # → NODE_B
polygone ecouter --relay <IP_R>:7000 --compute
```

Sur **A** (demandeur) :

```bash
polygone voisins --duree 3           # doit voir NODE_B et son relay
polygone compute --emprunter NODE_B --via <IP_R>:7000 \
  --executer 'printf p0-compute-ok'
```

La sortie attendue contient `p0-compute-ok` et :

```text
vérification : producer-signed — signature de l'auteur, pas preuve de justesse
(option L1 : relancer avec --verifier-local pour recalculer localement et
comparer ; une divergence conserve producer-signed et fait échouer la commande)
```

### 9.2 Confidentialité observée sur le relais

Sur **R**, capturez le TCP sans le modifier :

```bash
sudo tcpdump -A -s0 -i any 'tcp port 7000' -w /tmp/p0-compute.pcap
```

Le fichier peut montrer `from`, `to`, `session`, `type`, les tailles et les
ciphertexts. Il ne doit contenir **ni** `p0-compute-ok`, **ni** la chaîne de la
tâche, **ni** le résultat. Une occurrence est un échec P0 immédiat.

### 9.3 Contrôles négatifs

| Contrôle | Attendu |
|---|---|
| Relais rejoue la trame `req` | aucun second grant, aucun second calcul |
| A envoie un `req` en clair | ghost ne l'exécute pas, aucun grant exploitable |
| A usurpe `NODE_B` avec une autre clé | `verify_req` refuse, aucune exécution |
| Coupure réseau pendant l'exécution | B conserve son état ; A ne reçoit pas de faux succès |
| Redémarrage de B | B réécoute ; les sessions partielles ne sont pas livrées |
| Révocation : `polygone clef-tourner` sur B | l'ancien `node_id`/empreinte n'est plus accepté ; A doit réappairer |

### 9.4 Critères de réussite

- [ ] `NODE_A` ≠ `NODE_B`, identités dérivées des clés ML-DSA.
- [ ] `kem_offer` signé par B, clé KEM liée à `NODE_B`.
- [ ] tâche exécutée dans la sandbox, sortie non vide, `verified`/niveau L0 affiché.
- [ ] capture relais sans tâche ni résultat en clair.
- [ ] rejeu refusé, requête en clair refusée.
- [ ] coupure et redémarrage sans faux grant ni double exécution.
- [ ] révocation effective après rotation.
- [ ] aucune clé privée dans les journaux.

Tant que ces cases ne sont pas cochées sur **deux hôtes distincts**, Compute
reste **non validé physiquement** et ne doit pas être annoncé comme prêt.
