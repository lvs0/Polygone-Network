# Coupure et reprise en pleine session — 20261110T215246Z (UTC)

> **Instantané daté.** Cette preuve horodatée ne décrit pas l'état courant ;
> voir [STATE.md](../../STATE.md) et [FOUNDATION-STATUS.md](../../project/FOUNDATION-STATUS.md).

> Exécuté par `scripts/p0-modal-reconnect.sh`. Journaux bruts :
> [`modal-reconnect-20261110T215246Z-a1.log`](modal-reconnect-20261110T215246Z-a1.log),
> [`modal-reconnect-20261110T215246Z-a2.log`](modal-reconnect-20261110T215246Z-a2.log) et
> [`modal-reconnect-20261110T215246Z-modal.log`](modal-reconnect-20261110T215246Z-modal.log).

## Séquence

| Étape | Ce qui a été fait | Résultat |
|---|---|---|
| 1 | A envoie `P0CUT1-20261110T215225Z-18729` via le tunnel 1 (`r501.modal.host:37009`) | livré, code client `0` |
| 2 | le tunnel 1 est fermé par le relais | écouteur de B tombé : `1` |
| 3 | B se réenregistre sur le tunnel 2 (`r496.modal.host:41359`), même HOME, même identité | réenregistré |
| 4 | A envoie `P0CUT2-20261110T215225Z-21884` via le tunnel 2, même `node_id` et même clé KEM | livré, code client `0` |

| Élément | Valeur |
|---|---|
| Identité A | `7b340990257369ef2273597350d5c097` |
| Identité B | `f437d393f2eb71827179622ec2200666` (inchangée après la coupure) |
| Ancre de confiance apprise avant la coupure | `⬡ NOUVEAU PAIR APPRIS : 7b340990257369ef2273597350d5c097 → BLAKE3-256 bd60948cc814b954cc9af481c39789d810f296862d26a4a03eeaed4e1c9173ee — vérifiez les 64 caractères avec l'expéditeur (hors-ligne) avant de lui faire confiance.` |
| Ancre réapprise après la coupure | `False` |
| boot_id de B | `98176158-a335-40de-8757-9b8de25b4a86` |
| boot_id de R | `f9dda7d1-87b8-4336-8e38-24c13bc8e4f5` |

## Résultat

- Marqueur 1 arrivé avant la coupure : **oui**.
- B s'est réenregistré **sans changer d'identité** : le `node_id`, la clé KEM et
  l'ancre de confiance de A restent valides après la coupure.
- Marqueur 2 arrivé après la coupure : **oui**.

## Ce que cette preuve n'établit pas

- La reprise si **le relais** redémarre : son état de slots est en mémoire, rien
  n'est persisté. Ici seule la liaison publique a été coupée, pas le relais.
- La coupure réelle d'un lien résidentiel (changement d'IP publique, box qui
  redémarre) : la coupure est provoquée par la fermeture du tunnel.
- La conservation des messages envoyés **pendant** la coupure : aucun n'a été
  envoyé dans l'intervalle.
- `modal.forward` reste expérimental chez Modal.

## Résumé du nœud B

```json
{
    "role": "peer",
    "b_id": "f437d393f2eb71827179622ec2200666",
    "b_kem_len": 3136,
    "addr1": "r501.modal.host:37009",
    "addr2": "r496.modal.host:41359",
    "b_listener_died_on_cut": true,
    "b_reconnected": true,
    "b_seen_marker1": true,
    "b_seen_marker2": true,
    "b_trust_anchor": "\u2b21 NOUVEAU PAIR APPRIS : 7b340990257369ef2273597350d5c097 \u2192 BLAKE3-256 bd60948cc814b954cc9af481c39789d810f296862d26a4a03eeaed4e1c9173ee \u2014 v\u00e9rifiez les 64 caract\u00e8res avec l'exp\u00e9diteur (hors-ligne) avant de lui faire confiance.",
    "b_anchor_relearned_after_cut": false,
    "listener2_alive": true,
    "kernel": "4.19.0-gvisor",
    "boot_time": "1794347557",
    "boot_id": "98176158-a335-40de-8757-9b8de25b4a86",
    "uptime_s": 2.1,
    "hostname": "modal",
    "cpu": 17
}
```

## Résumé du relais

```json
{
    "role": "relay",
    "addr1": "r501.modal.host:37009",
    "addr2": "r496.modal.host:41359",
    "cut_at_epoch": 1794347561.795,
    "relay_alive": true,
    "relay_log_tail": [],
    "kernel": "4.19.0-gvisor",
    "boot_time": "1794347552",
    "boot_id": "f9dda7d1-87b8-4336-8e38-24c13bc8e4f5",
    "uptime_s": 2.8,
    "hostname": "modal",
    "cpu": 17
}
```
