# config.md — Configuration de Polygone

> *État réel du produit v2.0.0-rc2. Ce document décrit ce qui existe
> aujourd'hui, pas une architecture rêvée.*

---

## 1. Fichiers de configuration

| Fichier | Rôle | Créé par |
|---|---|---|
| `~/.polygone/identity.json` | Identité : pseudo + clés ML-KEM-1024 + ML-DSA-65 | `polygone` (premier lancement), chmod 600 |
| `~/.config/polygone/daemon.toml` | Allocation de ressources (polygoned) | `polygoned` ou l'utilisateur |
| `~/.config/polygone/config.json` | Restes v1 — **inertes** : aucun code ne les lit (vérifié 2026-08-08) | v1 |
| `~/.config/polygone/services.json` | Restes v1 — **inertes** : aucun code ne les lit (vérifié 2026-08-08) | v1 |

## 2. `~/.polygone/identity.json`

```json
{
  "pseudo": "vox-kali-ren",
  "kem_pk_hex": "…1568 octets hex…",
  "kem_sk_hex": "…3168 octets hex…",
  "sign_pk_hex": "…1952 octets hex…",
  "sign_sk_hex": "…4032 octets hex…"
}
```

- `kem_pk_hex` est **public** : c'est la clef que vous partagez
  (`polygone clef`). Elle sert aussi d'identifiant de nœud sur le réseau
  (les 16 premiers caractères hex = node id).
- `kem_sk_hex`, `sign_sk_hex` sont **secrets** : ils ne quittent jamais
  la machine. Fichier créé avec les permissions `600`.
- La génération est automatique au premier lancement. La suppression du
  fichier régénère une identité neuve (irréversible).

## 3. `~/.config/polygone/daemon.toml`

Configuration du daemon d'allocation (`polygoned`), écrite par
`polygoned --gen-config` (le daemon lit aussi le format legacy
`[tier] tier = "…"` des versions antérieures) :

```toml
tier = "Balanced"          # Eco | Balanced | Performance | Max | Custom
cpu_affinity_mode = "Auto"
memory_limit_enabled = true
bandwidth_shaping = true
gpu_allocation_enabled = true
service_integration = true

[behavior]
grow_step_pct = 10
shrink_step_pct = 5
shrink_hysteresis_ticks = 5
throttle_on_user_activity = true
tick_interval_secs = 5

[safety]
min_free_ram_gb = 4.0
min_free_cpu_cores = 1
min_free_vram_mb = 512
max_cpu_percent = 85
```

Les champs absents d'une config partielle retombent sur les defaults
produit (planchers de sécurité réels, toggles activés) — jamais des zéros
silencieux.

## 4. Variables d'environnement

| Variable | Effet |
|---|---|
| `POLYGONE_INSTALL_DIR` | Répertoire d'installation (installateur) |
| `HOME` | Base des chemins `~/.polygone` et `~/.config/polygone` |

## 5. Adresses réseau par défaut

| Service | Défaut | Rôle |
|---|---|---|
| Relay | `127.0.0.1:7000` | Routage aveugle des fragments (`polygone envoyer --via`, `polygone ecouter`) |
| Interface locale (projeté) | `:8080` | Dashboard web (non livré en rc2) |
| Proxy anonyme (projeté) | `:9050` | polygone-hide (archivé, cf. STAGING.md) |

## 6. Principes

- **Zéro télémétrie** : aucun fichier de config n'est transmis, jamais.
- **Zéro secret dans le code** : les clés vont dans `~/.polygone/`, jamais
  dans le dépôt.
- **Privacy-by-default** : une seule commande génère tout ce qu'il faut.

## 7. Fichiers legacy v1 inertes

`~/.config/polygone/config.json` et `services.json` sont des restes de la
v1. **Aucun code du workspace ne les lit** (vérifié 2026-08-08) — ils
peuvent être supprimés sans risque ; le daemon actuel n'utilise que
`daemon.toml`.

---

*Documentation de configuration — v2.0.0-rc2 · « On voit rien. Et c'est comme ça que ça devrait être. »*
