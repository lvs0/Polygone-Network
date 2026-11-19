# Aménagement de `hope-gateway` — comptes, crédits, gratuité

> NOTE DE PORTAGE — 4 novembre 2026 : rapport d’un checkout externe hope-gateway mesuré le 2 novembre 2026. Ce code n’est pas contenu dans Polygone Network et aucune intégration runtime n’est revendiquée.

> Appliqué le 2 novembre 2026 dans `checkout séparé hope-gateway`.
> Point de sortie unique des agents : `http://127.0.0.1:8787/v1`.

---

## 1. Le défaut corrigé

Avant, **toutes les clés d'un provider partageaient un seul bucket de quota**.
Le commentaire du code le justifiait : « rotating API keys must never multiply
the account quota ». C'est vrai quand les clés appartiennent au même compte.
C'est faux quand elles appartiennent à des comptes distincts — et c'était
impossible à distinguer.

Résultat : soit on sous-utilisait (3 comptes Groq traités comme 1), soit on
sur-vendait (si on avait additionné naïvement).

## 2. La règle retenue

> **Une clé n'est pas un compte.**

- Sans `accounts` déclarés : les clés forment **un seul compte partagé**
  (`label: "default"`), qui hérite des limites du provider. Comportement
  historique, le plus sûr.
- Avec `accounts` déclarés : chaque compte porte ses **propres** clés, limites,
  crédits et règles de service.

## 3. Ce qui a été ajouté

| Fichier | Rôle |
|---|---|
| `accounts.mjs` | Registre de comptes : quotas, crédits, règles, cooldowns, sérialisation |
| `config.mjs` | `FREE_FOREVER`, politique `hope/free`, passage des comptes déclarés |
| `server.mjs` | Réservation par compte, rotation de clés par compte, endpoints |
| `tests/accounts.test.mjs` | 12 tests : partage, multi-compte, crédits, services, secrets |
| `tools/free-forever-audit.mjs` | Inventaire et mesure des free tiers |

### 3.1 Modèle de compte

```jsonc
{
  "label": "groq-1",              // identifiant public, jamais la clé
  "key": "gsk_…",                 // ou "keys": ["…","…"]
  "plan": "free",                 // free | paid | trial | unknown
  "freeForever": true,
  "services": ["chat", "tools"],  // null = tous
  "limits": { "rpm": 30, "rph": 0, "rpd": 1000, "tpm": 6000, "tpd": 0 },
  "credits": {
    "remaining": 200000,          // budget consommable
    "initial": 200000,            // pour la remise à zéro
    "unit": "tokens",             // "tokens" | "requests"
    "reset": "utc-day",           // never | utc-day | utc-month
    "costPer1kTokens": 1          // optionnel
  }
}
```

### 3.2 Déclaration dans `~/.hope/providers.json`

```jsonc
{
  "providers": {
    "groq": {
      "keys": ["gsk_a", "gsk_b"],           // compte partagé (défaut)
      "limits": { "rpm": 30 }
    },
    "openrouter": {
      "accounts": [
        {
          "label": "or-1",
          "key": "sk-or-a",
          "plan": "free",
          "freeForever": true,
          "limits": { "rpd": 50 },
          "credits": { "remaining": 50, "initial": 50, "unit": "requests", "reset": "utc-day" }
        },
        {
          "label": "or-2",
          "key": "sk-or-b",
          "plan": "free",
          "freeForever": true,
          "limits": { "rpd": 50 },
          "credits": { "remaining": 50, "initial": 50, "unit": "requests", "reset": "utc-day" }
        }
      ]
    }
  }
}
```

Deux comptes OpenRouter = deux fois 50 requêtes/jour, **parce que ce sont deux
comptes réels**. Deux clés du même compte ne donnent toujours que 50.

## 4. Comportement à l'exécution

1. **Routage par alias.** `hope/auto`, `hope/fast`, `hope/quality`, `hope/long`,
   `hope/code`, et le nouveau **`hope/free`**.
2. **Réservation.** `reserveQuota(provider, tokens, service)` vérifie d'abord le
   plafond agrégé du provider, puis choisit un compte disponible et réserve
   dessus. La rotation répartit la charge.
3. **Clé.** `nextKey` prend la clé du compte réservé, en tournant sur ses clés
   et en évitant celles en cooldown.
4. **Échec.** `401` refroidit la clé ; `402/403/429` refroidissent **le compte**
   (c'est un problème de plan, de crédits ou de quota, pas de clé).
5. **Persistance.** L'usage (fenêtres, crédits restants, cooldowns) est écrit
   dans `~/.hope/state.json` — **jamais les clés**. Un redémarrage ne remet donc
   pas les quotas à zéro.

## 5. Endpoints

| Endpoint | Réponse |
|---|---|
| `GET /hope/accounts` | Comptes par provider : plan, crédits restants, usage 1 m/1 h/24 h, cooldown, dernière erreur |
| `GET /hope/free-forever` | Providers `freeForever`, preuve déclarée, comptes associés |
| `GET /admin/status` | Ajoute `accounts_usable`, `accounts_total`, `free_forever`, et le détail par compte |

Garde : si `HOPE_GATEWAY_KEY` est défini, ces routes exigent le même
`Authorization: Bearer …` que `/admin`.

## 6. La vérité sur « gratuit à vie »

**Aucun fournisseur tiers ne garantit la gratuité à vie.**

Le probe du 27 octobre 2026 a mesuré 25 providers : 10 répondent. Plusieurs free
tiers sont **déjà fermés** (Cerebras, Hugging Face, Venice, DeepSeek → `402`).

| Rang | Source | Gratuité | Limite réelle |
|---:|---|---|---|
| 1 | **Ollama / llama.cpp local** | **Garantie** (votre matériel) | Qualité et vitesse liées à la machine |
| 2 | Google AI Studio | Free tier mesuré (343 ms) | Quotas serrés, données utilisées |
| 3 | Groq | Free tier mesuré (72 ms) | RPM/RPD bas, par compte |
| 4 | Mistral | Free tier expérimental (206 ms) | Conditions changeantes |
| 5 | OpenRouter `:free` | Mesuré (119 ms) | 50 req/jour/compte |
| 6 | Pollinations | Aucune clé (174 ms) | Aucune SLA, qualité variable |
| 7 | Cloudflare Workers AI | Allocation quotidienne | Account ID requis, non sondé |
| — | NVIDIA NIM, Inception, LogFare | Crédits offerts | Pas « à vie » |

**Décision :** `hope/free` ne contient que les providers marqués `freeForever` et
mesurés. Le seul « à vie » défendable reste **le modèle local en premier** —
ce qui sert aussi la vision : pas de dépendance à un tiers.

## 7. Vérification exécutée

```bash
cd checkout séparé hope-gateway
node --check server.mjs && node --check config.mjs && node --check accounts.mjs
npm test                      # 32 tests historiques + 12 nouveaux = 44
node tools/free-forever-audit.mjs
```

Résultats au 2 novembre 2026 : **44/44 tests verts**, audit produit 7 providers
classés, rapport écrit dans `reports/free-forever-2026-11-02.json`.

## 8. Ce qui reste à faire

1. **Compléter `~/.hope/providers.json`** avec les comptes réels (libellés,
   plans, limites publiées). Le gateway ne devine pas les quotas.
2. **Lancer `--live`** une fois par mois pour reclassifier les free tiers.
3. **Déclarer les règles de service** quand un compte ne doit pas servir la
   vision ou l'audio.
4. **Ne jamais** écrire une clé dans un rapport, un log ou un commit.
