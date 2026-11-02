# Phase 0 — RAPPORT DE FRANCHISSEMENT

Date : 2026-10-12
Dépôt : `/home/lvs0/Projects/Polygone-real`
HEAD : `060c11d` (branche `main`) — **219 fichiers modifiés/non suivis, non commités**
Opérateur : LVS · Agent : CORVUS (deepseek-v4.1-flash via logfare)

---

## Critère de fin (manifeste)

> « un octet traverse un câble entre deux machines réelles.
> Critère de fin : sha256 reçu = sha256 émis, sur une machine différente. »

**ATTEINT.**

```
sha256 émis par B (Modal, us-east-1) : a05c78f943a536d567816ad6316bdbce9ad469f92973a32e40f3dad016fa572f
sha256 reçu par A (ThinkPad, Arch)   : a05c78f943a536d567816ad6316bdbce9ad469f92973a32e40f3dad016fa572f
taille                                : 1048576 octets
hash BLAKE3                           : 25a3560e61c9b749142d0229fda764f881bd93595ded8d100c6caf22c26ef023
```

Sortie brute du client côté A :

```
⬡ bloc récupéré chez 100.26.248.115:44427 — 1048576 octets, hash vérifié
  25a3560e61c9b749142d0229fda764f881bd93595ded8d100c6caf22c26ef023  (/tmp/p0modal/home-a/.polygone/blocks)
```

## Topologie

- **A** — ThinkPad X250, Arch Linux, IP LAN `192.168.1.104`.
  Binaire `target/release/polygone` (release, glibc 2.39).
- **B** — conteneur Modal, image `debian:trixie-slim` + Python 3.12.
  App `polygone-phase0`, fonction `node_b`, 1 CPU / 512 Mio.
  Tunnel TCP brut : `r449.modal.host:44427` (IP résolue `100.26.248.115`).
- **Protocole** — TCP brut, ligne + octets (`crates/client/src/blocks.rs:21-23`) :
  `GET <hash>` → `HAVE <len>\n<octets>` ou `MISS`.

## Étapes du test falsifiable (§7 de docs/ARCHITECTURE_RESEAU.md)

| # | Étape | Statut |
|---|---|---|
| 1 | B stocke un bloc et affiche son hash | ✅ `polygone bloc mettre` → `25a3560e…` |
| 2 | A demande ce hash par le réseau, octets identiques | ✅ **sha256 identique** (LAN local + Internet via Modal) |
| 3 | B est tué ; la même demande échoue | ✅ `Connection refused (os error 111)`, port fermé |
| 4 | Bloc altéré côté serveur ; A refuse | ✅ B répond `MISS`, A : « bloc absent chez le pair », rien rangé |

### Détail étapes 3 et 4 (exécutées en local, deux `HOME` distincts)

**Étape 3 — le bloc venait bien de B, pas du disque local de A.**

```
--- 3a. avant kill : A obtient (doit réussir) ---
⬡ bloc récupéré chez 127.0.0.1:7651 — 1048576 octets, hash vérifié
--- 3b. on tue B ---
port 7651 : FERMÉ
--- 3c. après kill : A redemande ---
Error: Connection refused (os error 111)
```

**Étape 4 — un bloc altéré est refusé.**

Un seul octet inversé sur 1 048 576 (`d[len//2] ^= 0xFF`) :

```
sha256 avant : a05c78f943a536d567816ad6316bdbce9ad469f92973a32e40f3dad016fa572f
sha256 après : ddc234ec9c7079e6b297e43ae2e95a0b84870218c16a46442eb9e1657494aa1b
```

Le nom du fichier dans le magasin reste `…25a3560e…` (inchangé), mais le contenu ne
correspond plus. B re-hache à la lecture, constate l'incohérence et répond `MISS` :

```
--- A demande le bloc altéré ---
Error: bloc absent chez le pair (MISS)
--- A a-t-il rangé le bloc corrompu ? ---
(vide = refusé, rien rangé)
```

Sans les étapes 3 et 4, on aurait prouvé un `cp` (docs/ARCHITECTURE_RESEAU.md:137).
Ce n'est pas un `cp`.

## Ce qui est PROUVÉ

- Transport TCP réel entre deux machines physiquement distinctes, à travers Internet.
- Le bloc vient bien de B : dossiers `HOME` distincts, adresse IP publique de Modal.
- Intégrité BLAKE3 vérifiée à la réception (le client le dit : « hash vérifié »).
- Validation préalable en local (deux `HOME`, deux processus) avant le saut au cloud —
  même protocole, même hash.

## Ce qui N'EST PAS prouvé — sans enjoliver

- **Le mesh LAN.** `mesh.rs` (UDP broadcast `255.255.255.255`) n'a pas tourné.
  Modal est du cloud : il n'y a pas de LAN. Le manifeste l'autorise :
  « Si B est indisponible : louer un VPS. Le mesh ne sera pas prouvé (LAN-only),
  mais le transport TCP, oui. »
- **`node servir` et le bail.** Non exercés ici. `bloc servir` sert sans bail ;
  c'est `node servir` qui exige un bail du daemon (`2b-13`). Phase 4.
- **Découverte automatique de pairs.** A a reçu l'adresse de B manuellement.
  Aucun pair n'a été découvert par broadcast.

Les quatre étapes du §7 sont toutes exécutées (2 en local, 2 via Internet),
mais 3 et 4 l'ont été en local — pas à travers Internet. Sur Modal, B est en
lecture seule depuis A : on ne peut ni le tuer proprement ni corrompre son
disque à distance. La combinaison (transport réel + falsifiabilité) est
couverte, mais en deux lieux, pas un seul.

## Limites réelles trouvées dans le code

### 1. `parse_peer` refuse les noms DNS

`crates/client/src/main.rs:302-310` :

```rust
full.parse::<SocketAddr>()
    .map_err(|e| anyhow::anyhow!("adresse de pair illisible « {chez} » : {e}"))
```

`SocketAddr` n'accepte qu'une IP littérale. `r449.modal.host:44427` est refusé :
« invalid socket address syntax ». Contourné en résolvant en IP côté A.
**À corriger** : `ToSocketAddrs` (résolution système) plutôt que `SocketAddr::parse`.

### 2. Le hash BLAKE3 d'un bloc rangé demande de reconstruire le chemin

Le magasin range `blocks/<2 premiers hex>/<hash complet>`. Mon script Modal a lu
`a3560e…` au lieu de `25a3560e…` en prenant `p.name` à la racine.
**À corriger** : un helper public `blocks::hash_of_path(&Path) -> Option<String>`
plutôt que de refaire la convention à chaque appelant.

## Décisions prises seul

- **Base d'image** : `debian:trixie-slim` et non `debian_slim`. Mesuré :
  le binaire exige `GLIBC_2.39`, bookworm n'a que 2.36.
- **Tunnel non chiffré** : `modal.forward(7643, unencrypted=True)` → TCP brut.
  Le protocole Polygone n'est pas HTTP ; `@modal.web_server` ne convient pas.
- **Bloc de test** : 1 Mio déterministe (`sha256(seed ‖ compteur)`), identique
  des deux côtés, pour que la vérification soit indépendante du protocole.

## Blocages

- Aucun. Phase 0 franchie.
- Le nœud Modal tourne encore (900 s) ; il s'éteint seul.

## Ce qui nécessite l'utilisateur

Rien pour cette phase. Les quatre décisions ouvertes du manifeste restent ouvertes
(second nœud, petals vs consommateur neuf, périmètre public, module tiers).

## Fichiers créés

- `phase0/modal_node.py` — nœud B sur Modal (nouveau, hors dépôt suivi)
- `phase0/run_phase0.sh` — script du test §7 côté A (nouveau)
