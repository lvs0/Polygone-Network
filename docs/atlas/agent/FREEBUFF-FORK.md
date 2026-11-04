# Faire tourner un agent basé sur Freebuff

> NOTE DE PORTAGE — 4 novembre 2026 : procédure de raccordement d’un fork Freebuff séparé ; aucun chemin privé, jeton ou configuration d’identification n’est livré.

> Le dépôt source est présent en local : `checkout séparé Freebuff` (fork de
> `github.com/CodebuffAI/freebuff`, monorepo Bun/TypeScript : `agents`, `cli`,
> `common`, `packages/agent-runtime`, `packages/llm-providers`, `sdk`).
> Il existe aussi `checkout local Freebuff OFM` avec un `bridge/` et un `node_modules/`.

---

## 1. Ce qu'on veut

Un agent qui :

1. tourne **depuis le code** du dépôt, pas depuis un binaire opaque ;
2. lit la vision et l'état réel de Polygone (ce dépôt Polygone Network) ;
3. passe **exclusivement** par `hope-gateway` pour les modèles ;
4. exécute l'orchestrateur (`orchestrateur.mjs`) avec des workers parallèles ;
5. commite sur GitHub après vérification, jamais avant.

## 2. Ce qu'on ne fait pas

- Pas de contournement de l'authentification, des quotas ou des conditions
  d'usage de Freebuff.
- Pas de patch du réseau pour simuler un abonnement.
- Pas de modification du dépôt upstream sans branche dédiée.
- Pas de secrets dans le fork (clés, tokens, identités).

## 3. Fork propre

```bash
cd /path/to/freebuff
git remote -v                     # doit montrer CodebuffAI/freebuff
git switch -c polygone-agent
```

Ajouter à la racine du fork :

- `AGENTS.md` → pointe vers une synthèse de vision non publiée,
  `VISION.md`, `ETAT-REEL.md`, `docs/atlas/agent/ORDRE.md`.
- `.polygone-agent/README.md` → ce qui a changé dans ce fork, et pourquoi.

Ne **pas** modifier `agents/`, `cli/` ni `common/` dans un premier temps : le
contexte et les ordres suffisent. Un patch du code ne se justifie que si une
capacité manque réellement (par exemple un provider OpenAI-compatible pointant
vers `http://127.0.0.1:8787/v1`).

## 4. Brancher le modèle sur le gateway

Freebuff utilise des providers configurables. La cible est :

```
Base URL : http://127.0.0.1:8787/v1
API key  : $HOPE_GATEWAY_KEY (ou vide si le gateway tourne en loopback sans clé)
Modèles  : hope/code, hope/long, hope/free, hope/quality
```

Avantages :

- un seul point de sortie, donc un seul endroit où mesurer la consommation ;
- les comptes, crédits et quotas sont gérés par `hope-gateway` (voir
  `05-GATEWAY/AMENAGEMENT.md`) ;
- changer de fournisseur ne change pas l'agent ;
- `hope/free` garantit qu'aucun payant n'est utilisé silencieusement.

Si Freebuff exige un provider déclaré dans son propre format, ajouter une entrée
`openai-compatible` pointant sur le gateway. **Ne pas** y mettre de clé de
fournisseur : la seule clé est celle du gateway.

## 5. Brancher l'orchestrateur

Dans `docs/atlas/agent/dispatch.json` :

```jsonc
{
  "agentCommand": [
    "bun", "run", "--cwd", "/path/to/freebuff",
    "cli", "--prompt-file", "{prompt}"
  ]
}
```

Adapter les arguments à l'interface réelle du CLI (vérifier `bun run --cwd cli dev`
et l'aide du binaire avant de figer la commande). Les jetons `{prompt}`,
`{repo}` et `{task}` sont substitués par l'orchestrateur.

Puis :

```bash
cd <racine-du-dépôt>
node docs/atlas/agent/orchestrateur.mjs --once --execute        # une tâche, sans commit
node docs/atlas/agent/orchestrateur.mjs --loop --hours 4 --execute --commit
```

## 6. Ordre de démarrage recommandé

1. **Valider le gateway seul** : `curl` sur `/v1/models`, puis un petit chat
   `hope/free`. Si ça échoue, l'agent échouera aussi.
2. **Un dry-run** : `node docs/atlas/agent/orchestrateur.mjs --once` → lire le brief produit.
3. **Un worker unique** sur une tâche à faible risque (`DOC.2`,
   documentation), avec `--execute` mais **sans** `--commit`.
4. **Lire le diff**. C'est l'étape que tous les harnais précédents ont sautée.
5. **Activer `--commit`** seulement après un diff propre.
6. **Passer au parallélisme** (`maxParallel: 3`) uniquement quand un worker
   seul produit des diffs cohérents.

## 7. Contexte à injecter (résumé)

L'agent doit savoir, sans le deviner :

- Polygone est un **produit de confidentialité**, pas une démo technique.
- Le code canonique est à la racine de ce dépôt (7 membres Rust).
- 18 promesses sur 33 sont fausses ou non câblées : `ETAT-REEL.md` est la vérité.
- P0 → P5 est l'ordre. On ne saute pas.
- Un `✅` sans commande est un mensonge.
- Aucun secret n'est lu, écrit ou commité.
- GitHub : commit après vérification, jamais de force-push.

## 8. Limite honnête

Ce document décrit une **intégration à faire**, pas une intégration déjà
vérifiée. Le fork n'a pas été patché ni lancé ici : cela demanderait de lire
l'interface réelle du CLI Freebuff, d'écrire la configuration du provider, et de
prouver un premier tour de boucle. Ce qui est **livré et vérifié** aujourd'hui :
le gateway multi-compte (44 tests verts) et l'orchestrateur (dry-run, découverte,
vérification, commit optionnel). Le reste est la prochaine étape, pas un acquis.
