# Mission P0 — rapport de preuves

> Exécution du 2026-11-09. Dépôt `lvs0/Polygone-Network`, branche
> `p0/first-secure-network`, bâtie sur S1 (`40fe6ba`).
> Une seule machine physique disponible : la démonstration à deux machines
> **n'a pas été exécutée**. Ce rapport ne le prétend pas.

## 1. Résultat en une ligne

Deux nœuds aux identités distinctes échangent un message authentifié à travers
le relais, dans **trois processus OS séparés** (relais + émetteur + récepteur),
avec refus explicite d'une identité non prouvée. Le saut « deux machines » reste
à faire, avec sa procédure écrite.

## 2. Ce qui a été corrigé (Phase 2)

| Défaut | Correction | Preuve |
|---|---|---|
| Le client pouvait rester **suspendu indéfiniment** sur un relais qui accepte la connexion puis se tait : `TcpStream::connect` réussissait, la première lecture n'avait aucune borne | Délai global de 10 s sur le handshake (`RELAY_HANDSHAKE_TIMEOUT`), erreur explicite nommant l'adresse et la durée | `a_silent_relay_fails_the_handshake_within_the_deadline` |
| Aucun test de **reconnexion** après coupure brutale : on ignorait si le slot de routage était libéré ou restait mort | Test de reprise : coupure sans goodbye, réenregistrement avec la même identité, puis refus d'une troisième connexion (slot réellement vivant) | `an_abruptly_disconnected_peer_can_reconnect_with_the_same_identity` |
| En-tête du relais affirmant qu'il « ne voit jamais l'identité de l'expéditeur ni du destinataire » — **faux** : il authentifie et route sur les `node_id` | En-tête réécrit : ce que le relais voit (node ids, session, tailles, horaires) et ne voit pas (charge utile, clés) | `docs/PROTOCOL-RELAY-AUTH.md` §1 |
| Aide de `polygone id` annonçant un identifiant « dérivé de la clé KEM » — faux depuis S1 (`node_id = BLAKE3(sign_pk)`) | Aide corrigée | `crates/client/src/main.rs` |
| Cible `[[bench]] inference_bench` déclarée alors que `crates/petals/benches/` n'existe pas : `cargo fmt --all`, `clippy --all-targets` et tout build de cibles échouaient | Déclaration morte retirée, raison consignée dans le `Cargo.toml` | `cargo fmt --all --check` = 0, `clippy --workspace --all-targets` = 0 |

Aucune modification n'a été faite au protocole lui-même : le handshake de S1
était correct sur le fond. Les défauts corrigés portent sur les délais, la
reprise, et des déclarations fausses.

## 3. Preuves exécutées

### 3.1 Parcours réel, trois processus (`scripts/p0-two-process.sh`)

```text
== 1. relais (processus 1) sur 127.0.0.1:34633 ==
== 2. identités distinctes (deux HOME séparés) ==
A   = 93c0f794700ebe473d13c9cc7fe6cbdf
B   = 8b8939b458aedcd4442694010a95a90d
KEM = af896d92115e77f7… (3136 caractères hex)
== 3. B écoute (processus 2) ==
== 4. A envoie via le relais (processus 3) ==
⬡ message envoyé via relay 127.0.0.1:34633 → 8b8939b458aedcd4442694010a95a90d
  session b874e3606ea5ab32 · 7 fragments + KEM_CT (4 suffisent pour lire)
== 5. livraison authentifiée chez B ==
message reçu, ancre de confiance apprise pour 93c0f794700ebe473d13c9cc7fe6cbdf
== 6. une identité non prouvée est refusée (HELLO hérité) ==
refus explicite : HELLO_DENIED auth_required
== 7. aucune clé privée dans les journaux ==
aucun secret dans les trois journaux
== PASS ==
```

Ce que cela prouve : handshake complet entre processus distincts, livraison
réelle par le relais, ancre de confiance apprise côté B, refus d'un `HELLO` non
authentifié, absence de secret dans les journaux.
Ce que cela ne prouve pas : le comportement sur deux machines (voir §5).

### 3.2 Porte complète (2026-11-09)

| Commande | Résultat |
|---|---|
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0, aucune régression |
| `cargo test --workspace` | **450 passés / 0 échec** |
| `cargo test -p polygone-relay auth_tests` | 8 passés / 0 échec |
| `./scripts/p0-two-process.sh` | PASS |

Les 450 tests incluent les deux tests ajoutés ici (`silent_relay`,
`an_abruptly_disconnected_peer_can_reconnect…`) ; les 448 autres étaient déjà
verts avant cette mission.

## 4. Ce qui protège quoi

| Menace | Mécanisme | Test |
|---|---|---|
| Se faire passer pour un pair au relais | `node_id = BLAKE3(sign_pk)` + signature ML-DSA-65 du transcript | `a_node_id_not_derived_from_the_signing_key_is_refused` |
| S'enregistrer sans posséder la clé KEM | challenge `ML-KEM-1024` frais, jeton `BLAKE3::derive_key` | `a_valid_signature_without_the_kem_secret_is_refused` |
| Rejouer une preuve de handshake | ciphertext neuf par connexion, transcript liant le ciphertext | `replaying_auth_resp_on_a_fresh_challenge_is_refused` |
| Voler le slot d'un pair vivant | refus `duplicate`, ack explicite, aucune écriture « dernier arrivé gagne » | `a_second_connection_cannot_steal_a_live_authenticated_slot` |
| Usurper `from` dans un message | destinataire : `BLAKE3(signer) == from` + ancre de confiance | `a_forged_from_is_rejected_even_with_an_empty_trust_cache` |
| Rejouer un message | `ts` signé, fenêtre ±300 s, cache de sessions terminées | `tests.rs` (SEC.3) |

## 5. Non vérifié, et pourquoi

1. **Deux machines physiques.** Une seule machine disponible. La procédure
   complète, avec contrôles négatifs et tableau de preuve, est dans
   [`P0-DEUX-MACHINES.md`](P0-DEUX-MACHINES.md). Tant que ce tableau est vide,
   la définition de terminé de la mission n'est **pas** satisfaite sur ce point.
2. **Traversée de NAT.** Non implémentée. Le relais doit être joignable en
   entrée ; deux pairs derrière des NAT symétriques ne peuvent pas se parler.
3. **Version de protocole.** `AUTH_INIT` n'en porte aucune : une évolution
   incompatible sera un refus, pas une négociation.
4. **Rotation de clé.** `clef-tourner` change la clé ML-DSA, donc le `node_id`
   et l'ancre de confiance des pairs ; aucune propagation n'existe. C'est la
   décision D2, toujours non tranchée.
5. **Débit d'`AUTH_INIT`.** Aucune limite dédiée ; une encapsulation ML-KEM par
   tentative. Borné par 1 024 connexions et 10 s, pas empêché.
6. **Confidentialité pendant le traitement.** Le destinataire déchiffre pour
   lire : le message est en clair dans sa mémoire. Aucune propriété de calcul
   confidentiel n'est revendiquée, ici ni ailleurs.

## 6. Référence Akash (Phase 5)

Deux mécanismes retenus, sans importer l'architecture :

- **Artefacts identifiés par le hash de leur contenu** (ADR-002) : appliqué ici
  en liant `node_id` au hash de la clé de signature plutôt qu'à un identifiant
  déclaré — c'était déjà le cas depuis S1, et c'est ce qui rend l'usurpation
  détectable sans annuaire.
- **Test de reprise après montée de version** : Akash exécute un scénario de
  récupération en CI. Ici, la reprise testée est celle de la connexion après
  coupure brutale ; la reprise après migration de format reste à écrire (AK-02).

Rejeté : escrow/enchères/jetons (contraire à la gratuité), isolation K8s/kata
(contraire au modèle sans serveur). Ces rejets étaient déjà consignés dans
`POLYGONE_UNIFICATION` / `AKASH_ENGINEERING_BENCHMARK.md` et ne sont pas
rouverts ici.
