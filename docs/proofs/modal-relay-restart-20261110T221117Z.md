# Redémarrage du relais en pleine session — 20261110T221117Z (UTC)

> **Instantané daté.** Cette preuve horodatée ne décrit pas l'état courant ;
> voir [STATE.md](../../STATE.md) et [FOUNDATION-STATUS.md](../../project/FOUNDATION-STATUS.md).

> Exécuté par `scripts/p0-modal-relay-restart.sh`. Le tunnel public ne change
> pas : seul le processus du relais est remplacé.

## Séquence

| Étape | Action | Résultat |
|---|---|---|
| 1 | A envoie `P0RS1-20261110T221016Z-10821` à travers R1 (pid 4) | livré, code `0` |
| 2 | R1 tué (SIGKILL), R2 démarre sur le même port (pid 6) | `restart_ok=true` |
| 3 | B détecte la fermeture de R1, reste vivant, puis se réauthentifie auprès de R2 sur la même adresse | `b_listener_died=false`, `b_auto_reconnected=true` |
| 4 | A envoie la sonde `P0RSPROBE-20261110T221016Z-29245` après le redémarrage ; le journal B la reçoit après la ligne de reconnexion | livré, code `0` |
| 5 | Le banc arrête ensuite volontairement le premier écouteur et lance un nouveau processus avec le même HOME ; A envoie `P0RS2-20261110T221016Z-23584` | réenregistrement même identité, message livré, code `0` |

## Ce qui survit, mesuré

| Élément | Avant | Après redémarrage |
|---|---|---|
| Processus d'écoute de B | vivant | toujours vivant (`exit=None`) |
| Slot de B chez le relais | présent sur R1 | absent au démarrage de R2, puis recréé par l'authentification automatique de B |
| Sonde envoyée après redémarrage | — | code `0`, reçue par le même processus B après sa reconnexion |
| Reconnexion automatique de B | non disponible avant le correctif | `1` — confirmée par le log de reconnexion et la réception de la sonde |
| Identité de B (`node_id`) | `08ff52ff2baf36117262d853a7437f40` | inchangée |
| Ancre de confiance connue par B | apprise avant la coupure | conservée dans le même HOME ; `b_anchor_relearned=false` |
| Adresse publique du relais | `r479.modal.host:34803` | identique (le tunnel n'a pas bougé) |

## Conclusion

R2 démarre sans les slots de R1 : le relais reste volontairement stateless et
ne persiste pas son registre de pairs. **Le client corrigé, lui, ne sort plus** :
il détecte la coupure, attend avec un backoff progressif, ouvre une nouvelle
connexion, refait le handshake authentifié avec la même identité et reprend
l'écoute. La sonde est visible dans les logs de B après le log `reconnexion au
relay réussie` : elle a donc été reçue par le processus qui a survécu au
redémarrage, sans relancement manuel.

Le banc tue ensuite volontairement ce processus et démarre un second processus
avec le même HOME pour vérifier séparément la persistance locale de l'identité
et de l'ancre. `b_reregistered_same_addr=true` concerne ce second démarrage
manuel ; il ne constitue pas la preuve de la reconnexion automatique.

## Ce que cette preuve distingue

- La reconnexion automatique du premier processus B est établie par
  `b_listener_died_on_restart=false`, `b_auto_reconnected=true`, et la réception
  de la sonde dans `B_LINES1_TAIL` après la ligne de reconnexion.
- La persistance du relais n'est **pas** établie et n'est pas implémentée ici :
  R2 a perdu ses slots et B les a recréés en se réauthentifiant.
- Le marqueur 2 est reçu par un nouveau processus lancé volontairement par le
  banc, avec le même HOME ; il vérifie la conservation locale de l'identité et
  de l'ancre, pas la continuité du premier processus.

## Ce que cette preuve n'établit pas

- Aucun envoi n'a été fait vers un pair absent du relais pour mesurer un code
  d'erreur typé au-delà de la sonde ci-dessus.
- Le redémarrage du relais **et** le changement d'adresse simultanés.
- La livraison ou la mise en file d'un message émis pendant l'indisponibilité :
  la sonde est envoyée après la reconnexion, pas pendant la coupure.
- La persistance d'un état de relais sur disque : le relais reste sans stockage
  des slots, et cette preuve ne lui en ajoute pas.
- Un pair derrière un NAT résidentiel ou deux machines physiques distinctes.

## Résumé du nœud B

```json
{
    "role": "peer",
    "b_id": "08ff52ff2baf36117262d853a7437f40",
    "b_kem_len": 3136,
    "addr": "r479.modal.host:34803",
    "b_listener_died_on_restart": false,
    "b_listener_exit_code": null,
    "b_auto_reconnected": true,
    "b_reregistered_same_addr": true,
    "b_anchor_relearned": false,
    "b_seen_marker1": true,
    "b_seen_marker2": true,
    "b_trust_anchor": "\u2b21 NOUVEAU PAIR APPRIS : cf705240c3bee01218dafdc891a89c6e \u2192 BLAKE3-256 9b192b14e70b9e53727910ca15befc8d6259a185d7aaf4ff15bc74cac95a925a \u2014 v\u00e9rifiez les 64 caract\u00e8res avec l'exp\u00e9diteur (hors-ligne) avant de lui faire confiance.",
    "listener2_alive": true,
    "kernel": "4.19.0-gvisor",
    "boot_time": "1794348655",
    "boot_id": "043a0b7a-dad2-499e-9e56-324bacda90da",
    "uptime_s": 1.1,
    "hostname": "modal",
    "cpu": 17
}
```

## Résumé du relais

```json
{
    "role": "relay",
    "kernel": "4.19.0-gvisor",
    "boot_time": "1794348653",
    "boot_id": "892ed363-0fa3-490e-ad15-717ab12616cd",
    "uptime_s": 1.4,
    "hostname": "modal",
    "cpu": 17,
    "relay1_pid": 4,
    "addr": "r479.modal.host:34803",
    "relay1_killed_at_epoch": 1794348658.624,
    "relay2_pid": 6,
    "restart_ok": true,
    "relay2_alive": true,
    "relay1_log_tail": [],
    "relay2_log_tail": []
}
```
