# Redémarrage du relais en pleine session — 20261110T215706Z (UTC)

> **Instantané daté.** Cette preuve horodatée ne décrit pas l'état courant ;
> voir [STATE.md](../../STATE.md) et [FOUNDATION-STATUS.md](../../project/FOUNDATION-STATUS.md).

> Exécuté par `scripts/p0-modal-relay-restart.sh`. Le tunnel public ne change
> pas : seul le processus du relais est remplacé.

## Séquence

| Étape | Action | Résultat |
|---|---|---|
| 1 | A envoie `P0RS1-20261110T215649Z-8568` à travers R1 (pid 4) | livré, code `0` |
| 2 | R1 tué (SIGKILL), R2 démarre sur le même port (pid 6) | `restart_ok=true` |
| 3 | A envoie une sonde `P0RSPROBE-20261110T215649Z-28326` avant tout réenregistrement | code `0` — `⬡ message envoyé via relay r475.modal.host:33685 → 5dd5402002d4b522545b549b8a27f32b   session 07f4000a9c82811f · 7 fragments + KEM_CT (4 suffisent pour lire) ` |
| 4 | B se réenregistre sur la même adresse, même identité | `1` |
| 5 | A envoie `P0RS2-20261110T215649Z-18562` | livré, code `0` |

## Ce qui survit, mesuré

| Élément | Avant | Après redémarrage |
|---|---|---|
| Processus d'écoute de B | vivant | tombé : `1` (code `0`) |
| Slot de B chez le relais | présent | perdu : R2 ne connaît aucun pair au démarrage |
| Sonde envoyée après redémarrage | — | code `0`, livrée à B (B s'était réenregistré avant l'envoi) |
| Reconnexion automatique de B | — | `0` |
| Identité de B (`node_id`) | `5dd5402002d4b522545b549b8a27f32b` | inchangée |
| Ancre de confiance de A | apprise | conservée, réapprise : `0` |
| Adresse publique du relais | `r475.modal.host:33685` | identique (le tunnel n'a pas bougé) |

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
    "b_id": "5dd5402002d4b522545b549b8a27f32b",
    "b_kem_len": 3136,
    "addr": "r475.modal.host:33685",
    "b_listener_died_on_restart": true,
    "b_listener_exit_code": 0,
    "b_auto_reconnected": false,
    "b_reregistered_same_addr": true,
    "b_anchor_relearned": false,
    "b_seen_marker1": true,
    "b_seen_marker2": true,
    "b_trust_anchor": "\u2b21 NOUVEAU PAIR APPRIS : 8370b9abb0a9a36704637649cc4a6845 \u2192 BLAKE3-256 0e267e4e35920a6267cd8bb4c2e7eb1238472e881d2416c879d87f0541ebb6bf \u2014 v\u00e9rifiez les 64 caract\u00e8res avec l'exp\u00e9diteur (hors-ligne) avant de lui faire confiance.",
    "listener2_alive": true,
    "kernel": "4.19.0-gvisor",
    "boot_time": "1794347818",
    "boot_id": "b3e9d0bc-1220-4b4d-83a7-6e2d0b66a514",
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
    "boot_time": "1794347815",
    "boot_id": "c58908d3-2cf5-4304-b2d6-1d939db85490",
    "uptime_s": 1.4,
    "hostname": "modal",
    "cpu": 17,
    "relay1_pid": 4,
    "addr": "r475.modal.host:33685",
    "relay1_killed_at_epoch": 1794347821.375,
    "relay2_pid": 6,
    "restart_ok": true,
    "relay2_alive": true,
    "relay1_log_tail": [],
    "relay2_log_tail": []
}
```
