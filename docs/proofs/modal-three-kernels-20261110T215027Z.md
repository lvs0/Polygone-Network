# Preuve réseau à trois noyaux — 20261110T215027Z (UTC)

> **Instantané daté.** Cette preuve horodatée ne décrit pas l'état courant ;
> voir [STATE.md](../../STATE.md) et [FOUNDATION-STATUS.md](../../project/FOUNDATION-STATUS.md).

> Exécutée par `scripts/p0-modal-three-kernels.sh`. Journaux bruts :
> [`modal-three-kernels-20261110T215027Z-a.log`](modal-three-kernels-20261110T215027Z-a.log) et
> [`modal-three-kernels-20261110T215027Z-modal.log`](modal-three-kernels-20261110T215027Z-modal.log).

## Montage

| Rôle | Hôte | Noyau | boot_id | Démarrage |
|---|---|---|---|---|
| A (expéditeur) | machine locale | `Linux 7.2.5-3-omarchy` | `96607c82-9b01-45bf-95a9-8cf1ebce989b` | `1794231716` |
| R (relais) | conteneur Modal n°1 | `4.19.0-gvisor` | `1acb281c-a76b-40bb-8fab-eef71dc298fd` | `1794347415` |
| B (destinataire) | conteneur Modal n°2 | `4.19.0-gvisor` | `99d41715-af70-465c-934b-1a79583819e1` | `1794347418` |

| Élément | Valeur |
|---|---|
| Tunnel public du relais | `r480.modal.host:37609` (TCP brut) |
| Identité A | `3ee6b900cb1fa6b4734df45976c93745` |
| Identité B | `396a6bd8c814425b98bcdc7ab21545b7` |
| Clé KEM de B | 3136 caractères hex |
| Marqueur | `P0THREE-20261110T215009Z-23184` |
| Code de retour du client A | `0` |
| Durée côté B (enregistrement → marqueur) | 1.805 s |

## Résultat

- Marqueur retrouvé chez B : **oui**, via R, chaque saut traversant le WAN.
- B a appris l'ancre de confiance de A : `⬡ NOUVEAU PAIR APPRIS : 3ee6b900cb1fa6b4734df45976c93745 → BLAKE3-256 89e939b6989c9d84d8716b3384ae1916dfc10755c08a105d76bfaf7664904b53 — vérifiez les 64 caractères avec l'expéditeur (hors-ligne) avant de lui faire confiance.`.
- Trois `boot_id` distincts : ce sont trois noyaux, pas trois namespaces.
- R et B sont deux conteneurs Modal séparés : le relais route sans lire la
  charge utile, et B ne partage pas le noyau du relais.

## Résumé du relais

```json
{
    "role": "relay",
    "addr": "r480.modal.host:37609",
    "relay_alive": true,
    "relay_log_tail": [],
    "kernel": "4.19.0-gvisor",
    "boot_time": "1794347415",
    "uptime_s": 1.3,
    "hostname": "modal",
    "local_ip": "127.0.0.1",
    "boot_id": "1acb281c-a76b-40bb-8fab-eef71dc298fd",
    "cpu": 17
}
```

## Résumé du nœud B

```json
{
    "role": "peer",
    "marker": "P0THREE-20261110T215009Z-23184",
    "b_id": "396a6bd8c814425b98bcdc7ab21545b7",
    "b_kem_len": 3136,
    "b_seen_marker": true,
    "b_trust_anchor": "\u2b21 NOUVEAU PAIR APPRIS : 3ee6b900cb1fa6b4734df45976c93745 \u2192 BLAKE3-256 89e939b6989c9d84d8716b3384ae1916dfc10755c08a105d76bfaf7664904b53 \u2014 v\u00e9rifiez les 64 caract\u00e8res avec l'exp\u00e9diteur (hors-ligne) avant de lui faire confiance.",
    "elapsed_s": 1.805,
    "relay_addr": "r480.modal.host:37609",
    "listener_alive": true,
    "kernel": "4.19.0-gvisor",
    "boot_time": "1794347418",
    "uptime_s": 1.3,
    "hostname": "modal",
    "local_ip": "127.0.0.1",
    "boot_id": "99d41715-af70-465c-934b-1a79583819e1",
    "cpu": 17
}
```

## Ce que cette preuve n'établit pas

- Traversée de NAT résidentiel : A est le seul hôte hors centre de données, et
  aucune box n'est dans le chemin.
- Résistance à la perte de paquets prolongée, au changement d'adresse, à la
  reconnexion inter-hôtes.
- Anonymat : R voit les deux `node_id`, la session, les tailles, les horaires et
  les deux adresses publiques.
- R et B reposent sur la même technologie de sandbox (gVisor) : deux instances
  distinctes, pas deux matériels distincts. Les deux conteneurs rapportent
  `127.0.0.1` comme adresse locale, ce qui rend l'IP inutilisable comme preuve
  de séparation — d'où le `boot_id`.
- `modal.forward` est marqué expérimental par Modal ; l'API peut changer.
