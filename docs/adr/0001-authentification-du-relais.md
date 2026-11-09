# ADR-0001 — Authentifier le relais par preuve de clé, pas par identifiant déclaré

- **Statut** : accepté, implémenté
- **Date** : 2026-11-09
- **Contexte** : P0 (premier réseau authentifié), suite de S1 (`40fe6ba`)
- **Décideurs** : opérateur (Lévy), agent d'implémentation

## Contexte

Le relais acceptait `HELLO <node_id>`, une chaîne choisie par le client. N'importe
quel pair pouvait donc s'enregistrer sous l'identité d'un autre et recevoir son
trafic : l'authenticité reposait uniquement sur le destinataire final. Le relais
ne pouvait pas distinguer un pair légitime d'un usurpateur, et le premier
arrivé gagnait le slot de routage.

Une contrainte structurelle : le relais ne doit détenir **aucune** clé et ne doit
jamais lire la charge utile. Toute solution qui exige du relais qu'il vérifie
les messages (au lieu de la seule identité de connexion) est rejetée.

## Décision

1. `node_id = BLAKE3(clé publique ML-DSA-65)` tronqué à 128 bits. Le pair ne
   choisit pas son identité, il la subit : toute usurpation devient une
   incohérence détectable sans annuaire.
2. Handshake en quatre temps, versionné :
   `AUTH_INIT v1 <node_id> <sign_pk> <kem_pk>` → `CHALLENGE <kem_ct>` →
   `AUTH_RESP <signature> <token>` → `AUTH_OK` ou `AUTH_DENIED <motif>`.
3. La preuve combine deux possessions : le secret KEM (décapsulation d'un
   ciphertext frais, jeton dérivé par BLAKE3) et la clé de signature (ML-DSA-65
   sur un transcript liant version, `node_id`, les deux clés et le ciphertext).
4. Le relais refuse un slot déjà vivant (`duplicate`) au lieu d'écraser la route.
5. Le mode `HELLO` reste disponible derrière `--allow-insecure-hello`, refusé
   par défaut.

## Alternatives écartées

| Alternative | Pourquoi non |
|---|---|
| `node_id` déclaré + signature des messages seulement | Le relais ne peut rien vérifier ; le slot de routage reste volable et l'usurpateur reçoit le trafic de la victime. |
| Annuaire de clés tenu par le relais | Le relais deviendrait une autorité d'identité : un relais hostile substitue la clé KEM et lit tout. La clé du destinataire reste échangée hors bande. |
| Certificats X.509 / PKI | Dépendance à une autorité centrale, contraire au modèle sans compte ni serveur. |
| Signature de chaque fragment par le relais | Le relais lirait la charge utile : contraire à la promesse de routage aveugle. |
| Jeton seul (sans signature) | Le jeton prouve le secret KEM, pas la clé qui nomme le `node_id` — les deux moitiés de l'identité doivent être liées. |

## Conséquences

**Positives** : usurpation détectée au niveau du relais *et* du destinataire ;
slot non volable ; aucune clé au relais ; refus explicites et stables ;
version de protocole qui rend une évolution incompatible bruyante.

**Négatives, assumées** :
- le relais apprend les `node_id` de routage, la session, la taille et le
  moment des connexions — la confidentialité porte sur le contenu, pas sur le
  graphe (voir `docs/threat-msg.md`) ;
- `clef-tourner` change la clé ML-DSA, donc le `node_id` : la rotation casse
  l'ancre de confiance des pairs. Décision D2, non tranchée ;
- pas de traversée de NAT : le relais doit être joignable en entrée ;
- `AUTH_INIT` n'est pas limité en débit (une encapsulation ML-KEM par tentative),
  borné seulement par 1 024 connexions et 10 s.

## Preuves

| Propriété | Test |
|---|---|
| `node_id` non dérivé de la clé → refus | `a_node_id_not_derived_from_the_signing_key_is_refused` |
| Signature valide sans secret KEM → refus | `a_valid_signature_without_the_kem_secret_is_refused` |
| Rejeu d'`AUTH_RESP` → refus | `replaying_auth_resp_on_a_fresh_challenge_is_refused` |
| Slot vivant non volable | `a_second_connection_cannot_steal_a_live_authenticated_slot` |
| Version inconnue → refus explicite | `an_unsupported_auth_version_is_refused_explicitly` |
| Reprise après coupure | `an_abruptly_disconnected_peer_can_reconnect_with_the_same_identity` |
| Parcours réel multi-processus | `scripts/p0-two-process.sh` |
| Deux hôtes L3 distincts | `scripts/p0-two-host.sh` |
