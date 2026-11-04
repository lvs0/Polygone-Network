# PLAN MAÎTRE — Polygone, gateway et agent autonome

> **Instantané daté — ne pas lire comme l'état courant.** Ce document est rédigé le 2026-11-04 (import) ; il ne décrit pas l'état vérifié aujourd'hui, qui se lit dans [`ETAT-REEL.md`](../../../ETAT-REEL.md) et [`STATE.md`](../../../STATE.md).

> NOTE DE PORTAGE — 4 novembre 2026 : plan directeur historique ; relire le statut courant dans STATE.md, VISION.md et ../STATUS-2026-11-04.md.

> Rédigé le 2 novembre 2026 après audit des anciennes versions et confrontation au code. Le corpus conversationnel a été consulté localement ; les exports bruts ne sont pas publiés.
>
> Ce document est un plan historique. VISION.md définit la vision et ETAT-REEL.md décrit l’état vérifiable du code actuel.
---

## 0. Décisions prises (et pourquoi)

| # | Décision | Raison |
|---:|---|---|
| D1 | Un seul dossier `checkout Atlas séparé`, 15 versions documentées, originaux non déplacés | Corriger 17 Go de dispersion sans risquer une perte |
| D2 | Le dépôt Polygone Network réaligné est la base de travail actuelle ; les anciens checkouts restent des sources historiques | Conserver les preuves et éviter une fusion aveugle |
| D3 | Aucun fichier Rust modifié pendant la fusion | Une fusion de code non testée serait le « faux-fix » dénoncé par les audits |
| D4 | Les apports des autres versions sont **copiés dans `salvage/`**, documentés, non branchés | Garder le bon *et* le mauvais, avec provenance et raison |
| D5 | `VISION.md` remplace les visions contradictoires | 5 documents de vision concurrents = aucune décision |
| D6 | `ETAT-REEL.md` est la seule source pour README/site | 18 promesses sur 33 étaient fausses |
| D7 | `hope-gateway` passe d'un quota par *provider* à un quota par *compte* | La demande est « multi-compte selon les règles des services et les crédits » |
| D8 | « Gratuit à vie » = **modèle local d'abord**, pas un fournisseur miracle | Aucun tiers ne garantit la gratuité à vie ; le local, si |
| D9 | L'agent autonome est un orchestrateur qui **dispatch et vérifie**, pas un cron qui rapporte | les sessions cron historiques n’ont pas produit de livrable décisif |
| D10 | GitHub : commits locaux par étape, push après vérification | La demande est explicite, la prudence aussi |

---

## 1. Ce que la vision impose au plan

Trois contraintes traversent tout le reste :

1. **La confidentialité est un critère d'architecture, pas un slogan.**
   → pas de compte obligatoire, pas de télémétrie, pas de cloud requis pour le
   P2P, menace explicitement documentée.
2. **La gratuité est une promesse politique.**
   → le palier gratuit doit exister sans carte bancaire, et le calcul local est
   la seule brique qu'on contrôle vraiment.
3. **La preuve précède la déclaration.**
   → chaque étape du plan a une sortie vérifiable, et l'agent qui l'exécute n'a
   pas le droit de cocher sans commande.

---

## 2. État des lieux consolidé

### 2.1 Le corpus

- corpus historique consulté localement ; exports bruts exclus du dépôt.
- 65 % de sessions `cron` (bruit opérationnel), 17 % Telegram, 16 % CLI.
- La vision réelle est dans les messages `user` longs des sessions Telegram/CLI.
- L'extraction n'est pas exhaustive (la base d'origine a 5 391 sessions).

### 2.2 Les 15 dossiers

Voir `01-AUDIT/INVENTAIRE.md`. Résumé :

- **1 canonique** : `Projects/Polygone-real` (Rust, 7 crates, 333 tests, 65
  fichiers non commités, divergence 6/17 commits).
- **4 sources utiles** : v1 MIT (`Projets/Polygone`), snapshot remote
  (`Polygone-Network`), v2 orpheline, documentation datée.
- **3 harnais** : `polygone-company` (18 cycles, 4 `done`), `polygone-team`
  (366 cycles), `r-labs` (Docker).
- **3 gisements de données** : moteur de mémoire, pipeline d'ingestion, graphe
  de connaissances.
- **2 archives** : démon biomimétique (mesures en dur), un SVG.
- **1 état runtime** : `~/.polygone` (clés privées en clair, ne pas committer).

### 2.3 Les trois causes racines

1. **Divergence non réconciliée** — 6 commits locaux / 17 distants depuis
   septembre. Personne ne sait quelle ligne est la vérité.
2. **Prose en avance sur le code** — 18 capacités promises sans appelant.
3. **Automatisation sans livrable** — les harnais tournent, produisent des
   rapports, et s'arrêtent sur des blocages non résolus.

---

## 3. Ce qui a été produit (l'Atlas)

```
checkout Atlas séparé/
├── README.md                  point d'entrée
├── sources historiques locales non publiées/ (non publiées)
│   ├── EXTRACT-POLYGONE.md    inventaire + méthode + limites
│   ├── VISION-EXTRAITE.md     la vision, citée et arbitrée
│   └── CHRONOLOGIE.md         avril → novembre, avec les tournants
├── 01-AUDIT/INVENTAIRE.md     les 15 versions + anomalies
├── docs/atlas/plan/PLAN-MAITRE.md     ce document
├── code produit à la racine de ce dépôt
│   ├── VISION.md              vision d'autorité (remplace les 5 autres)
│   ├── ETAT-REEL.md           table de vérité 11/18/4
│   ├── ROADMAP.md             P0 → P5, sorties vérifiables
│   ├── SALVAGE.md             registre : d'où vient quoi, gardé/rejeté
│   ├── salvage/               apports des autres versions, tracés
│   └── (workspace Rust canonique, 7 crates)
├── versions/01..15/            chaque version renommée + VERSION.md + ARBRE
├── docs/atlas/agent/                   orchestrateur, ordres, dispatch, GitHub
└── 05-GATEWAY/                 aménagement de hope-gateway
```

**Non destructif :** chaque `versions/NN/ARBRE` est un lien vers l'original.
Aucun fichier source n'a été déplacé, renommé ou supprimé.

---

## 4. La version réalignée — principe de fusion

### 4.1 Ce qu'on garde

- Tout le workspace Rust canonique (7 crates, 333 tests).
- Le daemon (snapshot, GlowUp, bail, ledger, IPC `0600`, `--isoler`).
- Les documents de vérité (`AUDIT_VERACITE_VISION`, `FEATURE_SPEC`,
  `IMPL_PLAN`, `STAGING`).
- Les smokes réels (`compute`, `drive`, `mesh`, `hide`).
- L'installateur et la CI.

### 4.2 Ce qu'on pille

| Apport | Source | Usage |
|---|---|---|
| `polygone-bench` + `REPORT.md` | `03` | Seul banc d'essai chiffré du corpus ; à porter |
| `AXIOMS.md` | `05` | Formulation d'axiomes, à corriger (3 et 7 faux) |
| `HISTOIRE.md`, `MSH_SPEC.md`, `DESIGN_PHILOSOPHY.md` | `02` | Mémoire de conception |
| `00-vision-source-2026-05-10.md` | `13` | Source primaire de la vision |
| Protocole `dispatch → preuve → état` | `11` | Modèle de l'orchestrateur |

### 4.3 Ce qu'on rejette (mais qu'on conserve)

- `tokenomics.md`, `business-model.md` : contredisent le palier gratuit.
- `THREAT_MODEL.md` de la v2 : « aucune menace non mitigée » = dangereux.
- Le code Rust v1 (Ed25519) et le workspace v2 orphelin.
- `archive/2026-07-src/` (monolithe libp2p).
- Le démon `neuron/synapse/cortex` (mesures en dur).
- Les 13 Mo de sous-produits d'outillage (`graphify-out`, `.claude-flow`…).

### 4.4 Vérification

La version réalignée doit passer `cargo test --workspace` **et** être commitée
sur une branche. Si les tests échouent, on ne « corrige » pas en désactivant :
on documente l'échec et on traite la cause.

---

## 5. Plan technique Polygone (résumé ordonné)

Détail complet dans `ROADMAP.md`. Les cinq verrous, dans l'ordre :

### P0 — Ne rien perdre
Archive restaurable, inventaire commit-par-commit, branche `realign/p0`,
correction d'`ARCHITECTURE.md`. **Rien d'autre ne commence avant.**

### P1 — Le contrat de ressources
Séparer *mesuré / réservable / contraint*. Refuser ce qu'on ne peut pas garantir.
`--isoler` par défaut sous Linux. Tester crash, veille, SIGKILL, perte réseau.
macOS : isolation ou message explicite.

**Pourquoi c'est prioritaire :** c'est la seule brique dont l'utilisateur a donné
l'ordre de départ explicite (« commence par ça proprement »), et c'est la
dépendance de Drive, Petals et Serverless.

### P2 — Deux machines physiques
Protocole versionné, test LAN réel, test WAN avec NAT, cas négatifs, latence
mesurée branchée sur le Gateway.

**Pourquoi :** sans ça, « réseau décentralisé » est une architecture, pas un
produit. C'est le trou le plus ancien du projet.

### P3 — Le premier parcours utilisateur
Installation/désinstallation propres, `polygone` guidé, appairage, envoi,
pause, diagnostics, rollback, test sur machine modeste.

**Pourquoi :** c'est le reproche le plus répété — « pas intuitif, pas
d'interface, accessible à tous ».

### P4 — Les services, dans l'ordre des dépendances
Msg → Drive → Hide mono-saut → Serverless → Petals → SDK.
Chaque service : modèle de menace + critères d'acceptation **avant** le code.

### P5 — Durcissement et publication
Audit externe, fuzzing, CI multi-OS, SBOM, releases signées, benchmarks.

---

## 6. Plan `hope-gateway` — multi-compte, crédits, gratuité

### 6.1 État actuel (mesuré)

- 25 providers sondés le 27 octobre, **10 répondent réellement** en chat :
  Groq, Mistral, NVIDIA NIM, OpenRouter, LogFare, Google AI Studio, Inception,
  OmniRoute local, Ollama Cloud, Pollinations.
- 15 providers hors jeu, avec la raison HTTP mesurée (402, 403, 404).
- 112 entrées de clés lues, **62 clés uniques**.
- Le gateway gère déjà : rotation de clés, circuit breaker, quotas
  minute/heure/jour, fallback, cache, SSE, compteurs par agent.

### 6.2 Ce qui manque, et que la demande exige

| Manque | Conséquence | Correctif |
|---|---|---|
| Quota **par provider**, pas par compte | 3 comptes Groq partagent un seul quota → on croit avoir 3× la capacité, on a 1× | `accounts.mjs` : un objet compte par clé, avec ses propres fenêtres |
| Aucune notion de **crédits** restants | Impossible de router vers le compte qui a encore du crédit | Compteur `credits` par compte, décrémenté par usage, réinitialisé par fenêtre |
| Aucune **règle de service** | Un compte peut être interdit pour un service (ex. pas de vision) | `serviceRules` par provider : `chat`, `tools`, `vision`, `audio` |
| Aucune politique **gratuit à vie** | Le routeur choisit parfois un provider payant sans le dire | Flag `freeForever` + politique `hope/free` qui n'utilise que le gratuit prouvé |
| Pas d'endpoint d'audit | On ne peut pas voir qui consomme quoi | `GET /hope/accounts` + `GET /hope/free-forever` |
| Pas de mesure de la gratuité | Les free tiers changent | `tools/free-forever-audit.mjs` re-sonde et reclassifie |

### 6.3 La vérité sur « gratuit à vie »

**Aucun fournisseur tiers ne garantit la gratuité à vie.** Les free tiers
existent, changent, et se ferment (plusieurs le sont déjà : Cerebras, Hugging
Face, Venice, DeepSeek → 402 mesurés).

Hiérarchie honnête, du plus sûr au plus fragile :

| Rang | Source | Gratuité | Limite |
|---:|---|---|---|
| 1 | **Ollama / llama.cpp local** | **Garantie** (votre matériel) | Qualité et vitesse liées à la machine |
| 2 | **Cloudflare Workers AI** | Allocation quotidienne gratuite | Compte + Account ID requis |
| 3 | **Google AI Studio** | Free tier réel | Quotas serrés, données utilisées |
| 4 | **Groq** | Free tier réel, très rapide | RPM/RPD bas |
| 5 | **Mistral** | Free tier expérimental | Conditions changeantes |
| 6 | **GitHub Models** | Gratuit avec compte GitHub | Quotas, catalogue limité |
| 7 | **OpenRouter `:free`** | Modèles gratuits | 50 req/jour/compte, disponibilité variable |
| 8 | **Pollinations** | Sans clé | Non garanti, qualité variable |
| 9 | **NVIDIA NIM** | Crédits offerts | Pas « à vie » |

**Conclusion de plan :** le seul « gratuit à vie » défendable est **le modèle
local en premier**, avec les free tiers comme *bonus mesuré*, jamais comme
fondation. C'est aussi ce qui sert la vision : pas de dépendance à un tiers.

### 6.4 Implémentation (voir `05-GATEWAY/AMENAGEMENT.md`)

1. `accounts.mjs` — registre de comptes, quotas, crédits, règles, persistance.
2. `config.mjs` — `serviceRules`, `freeForever`, `accounts` déclarés.
3. `server.mjs` — routage et réservation par compte, endpoint `/hope/accounts`.
4. `tools/free-forever-audit.mjs` — re-sonde et classe les free tiers.
5. Tests : quota par compte, épuisement, rotation, service interdit, audit.

**Garde-fou :** les tests existants doivent rester verts. Aucun secret n'est
écrit sur disque ; l'état ne contient que des mesures.

---

## 7. Plan agent Freebuff

### 7.1 Le constat

`checkout séparé Freebuff` est le dépôt complet `CodebuffAI/freebuff` (monorepo Bun/TypeScript,
workspaces `agents`, `cli`, `common`, `packages/*`, `sdk`). Il est donc possible
de faire tourner un agent **à partir du code**, et pas seulement du binaire.

### 7.2 Ce qu'on ne fait pas

- On ne réécrit pas Freebuff.
- On ne contourne pas ses conditions d'usage ni son authentification.
- On ne patche pas le réseau pour simuler un abonnement.

### 7.3 Ce qu'on fait

1. **Fork propre** du dépôt (branche `polygone-agent`), avec un `README` de fork
   qui dit exactement ce qui change.
2. **Contexte injecté** : `AGENTS.md` à la racine du workspace Polygone, qui
   pointe vers les sources privées de vision (non publiées) et `docs/atlas/audit/INVENTAIRE.md` (état historique).
3. **Ordres** : `docs/atlas/agent/ORDRE.md` — la constitution de l'agent (ce qu'il doit
   faire, ne jamais faire, comment prouver).
4. **Orchestrateur** : `docs/atlas/agent/orchestrateur.mjs` — boucle
   `découvrir → planifier → dispatcher → vérifier → committer`, avec workers
   parallèles, budget de temps, et journal d'état.
5. **Providers** : l'agent parle à `hope-gateway` (`http://127.0.0.1:8787/v1`),
   qui applique les règles de comptes et de crédits. Un seul point de sortie,
   donc un seul endroit où mesurer.

### 7.4 Pourquoi cette architecture

- Le gateway isole l'agent des fournisseurs : changer de modèle ne change pas
  l'agent.
- L'orchestrateur isole le travail des conversations : chaque tâche a une
  preuve, un commit, un état.
- Le fork isolé garantit qu'une mise à jour de Freebuff ne casse pas le travail
  Polygone.

---

## 8. Plan GitHub

| Étape | Action | Condition |
|---|---|---|
| G1 | `git init` + commit initial de `Polygone-Atlas` | Atlas complet, doc relue |
| G2 | `git init` + commit de Polygone-Network (branche initiale `realign/v1`, état historique) | Tests workspace verts |
| G3 | Vérifier la divergence 6/17 | Inventaire commit par commit |
| G4 | Push de `realign/v1` sur `lvs0/Polygone-Network` | Jamais de force-push |
| G5 | CI déclenchée et verte | `cargo fmt`, `test`, `clippy` |
| G6 | Commit de chaque incrément par l'agent | Message conventionnel + preuve |

**Interdits absolus :** `push --force`, `reset --hard`, `clean -fd`,
`rm -rf` sur un dossier source, commit de `~/.polygone`, commit de `.env`,
commit de `_work/` (conversations brutes).

---

## 9. Ce que j'ai hypothétisé (à valider ou corriger)

L'utilisateur a prévenu : il ne dit pas tout. Voici donc les hypothèses
explicites que j'ai prises, pour qu'il puisse les invalider.

| # | Hypothèse | Si c'est faux |
|---:|---|---|
| H1 | Le produit doit rester **un seul binaire** `polygone` | On sépare CLI et GUI en deux produits |
| H2 | La **gratuité** prime sur le revenu à court terme | On active le palier payant plus tôt |
| H3 | **AGPL** est acceptable (état actuel du code) | On revient à MIT avant release |
| H4 | Le **local d'abord** est la bonne réponse à « gratuit à vie » | On parie sur un free tier tiers |
| H5 | L'utilisateur veut **un** dépôt canonique, pas quinze | On garde des dépôts séparés par module |
| H6 | Les harnais `company`/`team` doivent être **remplacés** par un orchestrateur unique | On les répare séparément |
| H7 | Le mobile n'est **pas** prioritaire maintenant | On ajoute une étape mobile en P3 |
| H8 | ~~Le jeton `POLY` est **reporté**~~ → **tranché le 2026-11-03** : pas de cryptomonnaie, une unité de réciprocité dans le daemon | On rouvre la conception économique (critères dans `DECISION-POLY.md` §6) |
| H9 | L'agent doit travailler **3–5 h en autonomie** avec 1–2 checkpoints | On raccourcit les sessions |
| H10 | Le push GitHub est autorisé **après vérification**, pas avant | On reste local |

---

## 10. Risques et garde-fous

| Risque | Probabilité | Garde-fou |
|---|---|---|
| Écraser du travail non commité | Élevée si on merge | Branche + archive + inventaire commit par commit |
| Fusionner du code non testé | Élevée | Aucun `.rs` modifié pendant la fusion |
| Déclarer vert sans preuve | Élevée (historique) | `ETAT-REEL.md` + commandes obligatoires |
| Fuite de secrets | Moyenne | `_work/`, `.env`, `identity.json` exclus ; audit avant commit |
| Agent qui tourne à vide | Élevée | Sortie obligatoire : commit + preuve, sinon `blocked` |
| Free tiers qui disparaissent | Certaine | Local d'abord, audit mensuel |
| Dérive de périmètre | Élevée | `VISION.md` §8 + `ROADMAP.md` « hors périmètre » |
| Conflit de licence | Moyenne | Décision propriétaire documentée |

---

## 11. Les 72 prochaines heures (concret)

1. **Vérifier** `cargo test --workspace` sur la version réalignée.
2. **Archiver** le dépôt canonique (P0.1) et **commiter** `realign/p0`.
3. **Inventorier** les 6 + 17 commits divergents (P0.2) — aucun merge.
4. **Corriger** `ARCHITECTURE.md` et le README (P0.4).
5. **Implémenter** `accounts.mjs` + endpoint `/hope/accounts` (D7).
6. **Lancer** `free-forever-audit` et figer la politique `hope/free`.
7. **Écrire** les ordres de l'agent et faire un premier cycle à blanc.
8. **Pousser** `Polygone-Atlas` et `realign/v1` (G4/G5).

---

## 12. Ce que « fini » veut dire pour ce plan

Ce plan est fini quand :

- l'Atlas existe, est versionné et publiable ;
- la version réalignée compile et passe ses tests ;
- `hope-gateway` sait dire, pour chaque requête, **quel compte** a servi,
  **combien de crédits** restent et **si c'est gratuit** ;
- l'agent a des ordres écrits, un orchestrateur, et une définition de la preuve ;
- GitHub contient tout cela, sans secret et sans force-push ;
- et la prochaine action est évidente pour quiconque ouvre `README.md`.

Pas avant.

---

## 13. Livré et vérifié au 2 novembre 2026

| Livrable | État | Preuve |
|---|---|---|
| `Polygone-Atlas` | publié | [lvs0/Polygone-Atlas](https://github.com/lvs0/Polygone-Atlas), 44 fichiers |
| 15 versions documentées | fait | `versions/*/VERSION.md`, originaux intacts |
| Version réalignée | publiée | `realign/v1` sur `Polygone-Network`, **348 tests verts** |
| `VISION.md`, `ETAT-REEL.md`, `ROADMAP.md`, `SALVAGE.md` | fait | dans le dépôt Polygone Network actuel |
| `hope-gateway` multi-compte | publié | [lvs0/hope-gateway](https://github.com/lvs0/hope-gateway), **44 tests verts** |
| Audit gratuité | fait | `reports/free-forever-2026-11-02.json` |
| Orchestrateur | fonctionnel | `--status`, `--discover`, `--once` (dry-run) |
| Ordres de l'agent | fait | `docs/atlas/agent/ORDRE.md` |

### Ce qui n'est pas encore fait, et qui ne doit pas être présenté comme fait

- Le fork Freebuff n'est ni patché ni lancé : procédure écrite seulement.
- La divergence Git n'est pas réconciliée.
- Aucun test entre deux machines physiques.
- Les 18 capacités non câblées restent non câblées.
- L'agent n'a pas encore exécuté une seule tâche réelle (mode dry-run).