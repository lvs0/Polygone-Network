# Redémarrage du relais en pleine session — 20261110T215622Z (UTC)

> **Instantané daté.** Cette preuve horodatée ne décrit pas l'état courant ;
> voir [STATE.md](../../STATE.md) et [FOUNDATION-STATUS.md](../../project/FOUNDATION-STATUS.md).

> Exécuté par `scripts/p0-modal-relay-restart.sh`. Le tunnel public ne change
> pas : seul le processus du relais est remplacé.

## Séquence

| Étape | Action | Résultat |
|---|---|---|
| 1 | A envoie `P0RS1-20261110T215602Z-5412` à travers R1 (pid 4) | livré, code `0` |
| 2 | R1 tué (SIGKILL), R2 démarre sur le même port (pid 6) | `restart_ok=true` |
| 3 | A envoie une sonde `P0RSPROBE-20261110T215602Z-21018` avant tout réenregistrement | code `0` — `⬡ message envoyé via relay r502.modal.host:44245 → 8d83455dcf45c3ac8693b60df9f8ee81   session 73273de17ce0e0fc · 7 fragments + KEM_CT (4 suffisent pour lire) ` |
| 4 | B se réenregistre sur la même adresse, même identité | `1` |
| 5 | A envoie `P0RS2-20261110T215602Z-2840` | livré, code `0` |

## Ce qui survit, mesuré

| Élément | Avant | Après redémarrage |
|---|---|---|
| Processus d'écoute de B | vivant | tombé : `1` (code `0`) |
| Slot de B chez le relais | présent | perdu : R2 ne connaît aucun pair au démarrage |
| Sonde envoyée après redémarrage | — | code `0`, livrée à B (B s'était réenregistré avant l'envoi) |
| Reconnexion automatique de B | — | `0` |
| Identité de B (`node_id`) | `8d83455dcf45c3ac8693b60df9f8ee81` | inchangée |
| Ancre de confiance de A | apprise | conservée, réapprise : `0` |
| Adresse publique du relais | `r502.modal.host:44245` | identique (le tunnel n'a pas bougé) |

## Conclusion

Le relais ne persiste aucun état : un redémarrage efface les enregistrements,
et le pair doit se réenregistrer. Ce qui survit est local au pair — identité et
ancre de confiance — donc le réenregistrement se fait sans nouvel appairage.
La reconnexion automatique n'existe pas : c'est au pair (ou à son superviseur)
de la déclencher.

## Deux observations à ne pas confondre

- La sonde a été émise **après** le réenregistrement de B : une course entre le
  redémarrage et la boucle du pair. Elle a donc été livrée ; elle ne prouve pas ce
  que fait le relais quand on écrit à un pair qu'il ne connaît pas.
- L'écouteur de B sort avec le **code 0** quand le relais disparaît : un superviseur
  qui ne relance que sur code non nul ne le relancera jamais.

## Ce que cette preuve n'établit pas

- Aucun envoi n'a été fait vers un pair absent du relais pour mesurer un code
  d'erreur typé au-delà de la sonde ci-dessus.
- Le redémarrage du relais **et** le changement d'adresse simultanés.
- La persistance éventuelle d'un état de relais sur disque : le code actuel n'en
  déclare aucun, mais seuls les journaux de cette exécution sont cités.

## Résumé du nœud B

```json
{
    "role": "peer",
    "b_id": "8d83455dcf45c3ac8693b60df9f8ee81",
    "b_kem_len": 3136,
    "addr": "r502.modal.host:44245",
    "b_listener_died_on_restart": true,
    "b_listener_exit_code": 0,
    "b_auto_reconnected": false,
    "b_reregistered_same_addr": true,
    "b_anchor_relearned": false,
    "b_seen_marker1": true,
    "b_seen_marker2": true,
    "b_trust_anchor": "\u2b21 NOUVEAU PAIR APPRIS : b5ef3e2a3db900165b5cd1fb75603e34 \u2192 BLAKE3-256 4dc7f8dbf36a0da2590a0982b674862863e95eec6df595c1a397c87b0d152d1c \u2014 v\u00e9rifiez les 64 caract\u00e8res avec l'exp\u00e9diteur (hors-ligne) avant de lui faire confiance.",
    "listener2_alive": true,
    "kernel": "4.19.0-gvisor",
    "boot_time": "1794347772",
    "boot_id": "3bac42b6-8691-46c9-8690-7dbcb39a7675",
    "uptime_s": 1.3,
    "hostname": "modal",
    "cpu": 17
}
```

## Résumé du relais

```json
{
    "role": "relay",
    "kernel": "4.19.0-gvisor",
    "boot_time": "1794347769",
    "boot_id": "735c64c3-3385-4b2e-be5e-d3a9b69f8f11",
    "uptime_s": 1.2,
    "hostname": "modal",
    "cpu": 17,
    "relay1_pid": 4,
    "addr": "r502.modal.host:44245",
    "relay1_killed_at_epoch": 1794347777.471,
    "relay2_pid": 6,
    "restart_ok": true,
    "relay2_alive": true,
    "relay1_log_tail": [],
    "relay2_log_tail": []
}
```
