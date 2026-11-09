# Protocole d'authentification du relais (P0)

> État vérifié le 2026-11-09 sur `p0/first-secure-network`, bâti sur S1 (`40fe6ba`).
> Ce document décrit **le code tel qu'il est**, pas une intention. Chaque
> propriété renvoie au test qui la prouve.

## 1. Ce que le relais fait, et ne fait pas

Le relais est un **routeur aveugle** : il reçoit des enveloppes NDJSON, lit
`kind`, `from`, `to`, `session`, et réachemine la ligne **verbatim** au pair
destinataire. Il ne stocke aucun fragment, ne déchiffre rien, n'a aucune clé.

Il **voit** donc : les `node_id` de routage, l'identifiant de session, la taille
des enveloppes et le moment des connexions. Il **ne voit pas** : la charge utile
(chiffrée), le texte clair, les clés privées. Toute documentation qui affirme
que le relais ignore l'identité des pairs est fausse : il route sur ces
identités et les authentifie.

## 2. Identité d'un nœud

```
node_id = hex(BLAKE3(ML-DSA-65 public key))[0..32]      (128 bits, 32 caractères)
```

- Dérivée **uniquement** de la clé de signature : un pair ne peut pas choisir
  son `node_id`, il le subit.
- Le `node_id` ne contient **pas** la clé KEM. Le lien clé de signature ↔ clé
  KEM est prouvé à chaque connexion, il n'est pas persisté (voir §6).
- La clé KEM du destinataire s'échange **hors bande** (`polygone clef`). Le
  relais ne sert jamais d'annuaire de clés : c'est ce qui empêche un relais
  hostile de substituer une clé et de lire le trafic.

## 3. Trame et handshake

Transport : TCP, une ligne = une trame, fin de ligne `\n`, plafond 64 KiB.
Encodage hexadécimal minuscule pour toutes les valeurs binaires.

```text
client → relais : AUTH_INIT <node_id> <sign_pk_hex> <kem_pk_hex>\n
relais → client : CHALLENGE <kem_ct_hex>\n
client → relais : AUTH_RESP <signature_hex> <token_hex>\n
relais → client : AUTH_OK\n
                | AUTH_DENIED <motif>\n
```

Puis, sur la même connexion :

```text
client → relais : {"kind":"fragment","from":…,"to":…,"session":…,"seq":…,"payload":[…]}\n
relais → pair   : la même ligne, verbatim
```

### 3.1 Contenu de la preuve

| Élément | Calcul | Vérifié par |
|---|---|---|
| `node_id` | `BLAKE3(sign_pk)` tronqué à 128 bits | relais, à `AUTH_INIT` |
| `kem_ct` | encapsulation ML-KEM-1024 vers `kem_pk`, fraîche à chaque connexion | relais (génère), client (décapsule) |
| `token` | `BLAKE3::derive_key("polygone-relay-auth-v1", shared_secret)` | relais, comparaison à temps constant |
| `signature` | ML-DSA-65 sur `relay_auth_transcript(node_id, sign_pk, kem_pk, kem_ct)` | relais |

La signature couvre le `node_id`, **les deux clés publiques** et le ciphertext :
un attaquant ne peut pas réutiliser une preuve valide en changeant une clé ou en
rejouant un challenge d'une autre connexion.

### 3.2 Motifs de refus (stables, exploitables)

`malformed`, `identity_mismatch`, `bad_kem_key`, `bad_sign_key`,
`bad_signature`, `invalid_token`, `duplicate`, `auth_required` (HELLO hérité).

`duplicate` signifie qu'une connexion vivante possède déjà ce `node_id` :
le relais ne fait **jamais** de « dernier arrivé gagne ».

### 3.3 HELLO hérité

`HELLO <node_id>` n'est accepté que si le relais a été lancé avec
`--allow-insecure-hello`. Par défaut il répond `HELLO_DENIED auth_required` et
ferme. Ce mode existe pour l'interopérabilité de tests, jamais pour un
déploiement.

## 4. Propriétés prouvées, et par quel test

| Propriété | Preuve |
|---|---|
| Un pair doit prouver sa clé de signature | `strict_relay_refuses_legacy_hello`, `a_node_id_not_derived_from_the_signing_key_is_refused` |
| Un message non authentifié est refusé | `strict_relay_refuses_legacy_hello` |
| Une identité usurpée est rejetée (relais) | `a_node_id_not_derived_from_the_signing_key_is_refused`, `refuse_from_spoofing_through_relay` |
| Une identité usurpée est rejetée (destinataire) | `a_forged_from_is_rejected_even_with_an_empty_trust_cache` (`Refusal::IdentityMismatch`) |
| Preuve de possession de la clé KEM | `a_valid_signature_without_the_kem_secret_is_refused` |
| Pas de rejeu du handshake | `replaying_auth_resp_on_a_fresh_challenge_is_refused` |
| Pas de vol de slot vivant | `a_second_connection_cannot_steal_a_live_authenticated_slot` |
| Reprise après déconnexion brutale | `an_abruptly_disconnected_peer_can_reconnect_with_the_same_identity` |
| Relais muet → erreur bornée, pas de blocage | `a_silent_relay_fails_the_handshake_within_the_deadline` |
| Parcours réel à processus indépendants | `scripts/p0-two-process.sh` |

## 5. Anti-rejeu des messages (au-delà du handshake)

Le handshake empêche de **s'enregistrer** sans clé. Le contenu est protégé
ailleurs, côté destinataire (`crates/client/src/net/mod.rs`) :

- chaque session porte `ts` (unix) et une signature ML-DSA-65 sur
  `canonical_bytes(session, from, to, ts, kem_ct, ciphertext)` ;
- le destinataire refuse `|now − ts| > 300 s` (`Refusal::StaleTimestamp`) ;
- il refuse un `from` qui n'est pas `BLAKE3(signer)` (`IdentityMismatch`) ;
- il refuse une clé de signature qui contredit l'ancre de confiance apprise
  (`UntrustedKey`) — le TOFU n'accepte donc rien d'un `from` forgé ;
- les sessions terminées sont mémorisées (LRU borné) pour refuser un renvoi
  à l'intérieur de la fenêtre.

## 6. Limites assumées

1. **Aucune version de protocole** sur `AUTH_INIT`. Une évolution incompatible
   sera un refus, pas une négociation. À ajouter avant toute publication large.
2. **Rotation de clé = changement de `node_id`** : `clef-tourner` renouvelle la
   clé ML-DSA, donc l'identité de routage et l'ancre de confiance des pairs.
   La rotation est prouvée localement, mais aucun message ne propage la nouvelle
   identité aux pairs. C'est la question D2, non tranchée.
3. **Le relais apprend qui parle à qui** (voir §1). La confidentialité porte sur
   le contenu, pas sur le graphe social. `docs/threat-msg.md` en donne la
   lecture honnête ; l'anonymat de routage n'est pas revendiqué.
4. **Pas de limite de débit sur `AUTH_INIT`** : chaque tentative coûte une
   encapsulation ML-KEM. Le plafond de 1 024 connexions et le délai de 10 s
   bornent l'impact, sans l'empêcher.
5. **Chiffrement en transit ≠ confidentialité pendant le traitement.** Le
   destinataire déchiffre pour lire : à ce moment, le message est en clair dans
   sa mémoire. Aucune propriété de calcul confidentiel n'est revendiquée.
