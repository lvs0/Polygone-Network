# PLAN ÉCOSYSTÈME — finir Polygone, puis chaque brique autour

> NOTE DE PORTAGE — 4 novembre 2026 : plan multi-dépôts établi le 3 novembre 2026 ; Gateway et Freebuff restent séparés et leur intégration n’est pas prouvée.

> Rédigé le 2026-11-03. Complément de [`PLAN-FINITION.md`](PLAN-FINITION.md) :
> ce document-ci ne traite **que** ce qui n'est pas le cœur de Polygone.
> Même règle : pas de calendrier, une preuve nommée par tâche, une tâche n'est
> finie que quand sa preuve a été exécutée et citée.

---

## 1. Portée — ce qui est dans le plan, et ce qui n'y est pas

Six briques. Les cinq premières forment l'écosystème de Polygone ; la sixième est
le dépôt qui les raconte.

| # | Brique | Où | État vérifié le 2026-11-03 |
|---:|---|---|---|
| A | **Polygone** (produit) | racine du dépôt Polygone Network | 11 capacités vraies, 18 fausses · détail dans `PLAN-FINITION.md` |
| B | **hope-gateway** (modèles) | `checkout séparé hope-gateway` | *meilleur état du lot* : `state.json` persisté atomiquement (`server.mjs:336-371`, mode 600, 6 Ko sur disque), 4 fichiers de tests + un mock partagé, 12 rapports de mesure — **mais aucune CI** |
| C | **Agent** (orchestrateur + fork Freebuff) | `docs/atlas/agent/`, `checkout séparé Freebuff` | boucle exécutable, zéro dépendance, `STOP`, timeouts — jamais encore branchée sur le gateway |
| D | **Corpus** (ta vision, l'extract) | sources locales non publiées, `docs/atlas/audit/` | 3 documents de synthèse + inventaire ; l'original est archivé, pas indexé |
| E | **15 versions** | `versions/01…15` | un `VERSION.md` par dossier — non trié, non fusionné |
| F | **Atlas** (ce dépôt) | racine | structure et plan posés, rien de publié |

**Hors périmètre, volontairement :** `home-tv`, `airplay-server`, `stremio-server`
et les autres projets du home. Ce sont des produits distincts ; les mêler ici
donnerait un plan qui ne finit rien. Ils restent dans `[chemin local]` et n'attendent
rien de ce document.

**Déjà tranché, ne pas rouvrir :** POLY comme cryptomonnaie est **refusé**
(`DECISION-POLY.md`, preuves dans `docs/atlas/plan/preuves-poly/`). L'unité de
réciprocité est `RECIP.1` dans le backlog, sans transfert ni convertibilité.

---

## 2. Ce que « écosystème fini » veut dire

Sept phrases, chacune avec sa preuve. Les six premières lignes de `PLAN-FINITION.md`
§1 (F1…F6) restent valables pour le produit ; voici les trois qui concernent le
reste.

| # | Phrase | Preuve acceptée |
|---:|---|---|
| E1 | L'agent améliore Polygone **sans supervision** pendant 4 h, et chaque tâche close a une preuve citée | journal de session : N tâches, N preuves, 0 tâche close sans sortie de commande |
| E2 | Aucun modèle n'est joint sans passer par `hope-gateway`, et la consommation est attribuée au bon compte | journal du gateway : 100 % des requêtes avec un `account_label`, 0 sortie directe |
| E3 | Le corpus de ta vision est **lisible par l'agent** et ne peut pas être contredit par un document du dépôt sans qu'un test échoue | test de cohérence vert + rouge quand on casse une phrase de la vision |

E1 est la seule qui compte vraiment : tant que l'agent n'est pas autonome et
honnête, chaque session coûte ta présence.

---

## 3. Brique C — l'agent *(épique `AGT`)*

C'est la brique qui produit tout le reste ; elle passe donc avant la finition de
Polygone dans l'ordre d'exécution, pas après.

### AGT.1 — Vérifier que le backlog est réellement lu, dans le bon ordre
- **Constat.** `backlog.json` vient d'être réécrit : 30 tâches, identifiants
  d'épiques (`DAEMON.1`), `blocked_by` sur 3 d'entre elles. `orchestrateur.mjs:157-159`
  trie bien sur `phase` puis `priority`, mais **ne lit jamais `blocked_by`** :
  `DAEMON.10`, `DRIVE.1` et `RECIP.1` sont donc atteignables, et `DRIVE.1`
  arriverait 3ᵉ de la phase P4. Le champ existe dans le fichier, il n'a aucun effet.
- **Travail.** Lire la tâche par ordre de `priority` **à l'intérieur** de la
  phase, ignorer toute tâche `blocked_by` non satisfaite, et refuser de démarrer
  si le fichier ne parse pas — un backlog illisible ne doit pas ressembler à un
  backlog vide.
- **Preuve.** `node orchestrateur.mjs --once` trois fois de suite : la tâche
  proposée est `DOC.1`, puis `UI.5`, puis `UI.1` ; et avec `--task RECIP.1`, le
  refus nomme le bloqueur.
- **Refus.** Ne pas ajouter de « priorité auto » par heuristique. L'ordre est
  écrit, pas deviné.

### AGT.2 — Interdire qu'une tâche bloquée soit comptée comme en cours
- **Constat.** Le backlog porte 3 tâches `blocked_by` (D2, D3 + BLOCKS.9/D8).
  Rien n'empêche aujourd'hui la boucle de les prendre et de tourner dans le vide.
- **Travail.** État `blocked` distinct, sortie du compteur d'avancement, et
  remontée dans `STATE.md` sous « attend une décision ».
- **Preuve.** la mesure historique du 3 novembre listait 4 bloquées et 26 ouvertes ; la mesure du 4 novembre est dans `../STATUS-2026-11-04.md` et la commande `node docs/atlas/agent/orchestrateur.mjs --status` ; le compteur
  d'avancement ne les inclut pas.
- **Refus.** Ne jamais débloquer une tâche pour faire avancer la boucle.

### AGT.3 — Un commit par tâche, avec la preuve dans le message
- **Constat.** `--commit` existe ; rien ne vérifie que le message cite la preuve.
- **Travail.** Exiger un trailer `Preuve:` dans le message, extrait de la sortie
  du check. Refuser le commit s'il manque.
- **Preuve.** Commit accepté avec trailer ; commit refusé sans, dans le même
  journal.
- **Refus.** Pas de `git commit --amend` ni de `push --force` automatique.

### AGT.4 — Discipline de branches : jamais sur `main` en direct
- **Constat.** La boucle commite là où le dépôt est. `ETAT-REEL.md` §8 documente
  déjà une divergence local/distant ; une boucle qui commite sur `main` la
  fabrique en continu.
- **Travail.** Une branche par tâche (`task/<id>`), `main` intact, fusion
  seulement après preuve verte.
- **Preuve.** `git branch --show-current` dans le journal de chaque tâche ;
  `main` inchangé après 3 tâches.
- **Refus.** Pas de merge automatique : la fusion reste une action humaine.

### AGT.5 — Fan-out de sous-agents, avec un fichier par agent et zéro recouvrement
- **Constat.** Les six épiques ont été écrits en parallèle et **trois constats
  se sont révélés faux** ; deux workers ont aussi touché des sujets voisins sans
  se voir. Le fan-out marche, à condition que le coordinateur relise.
- **Travail.** Découpage par fichiers possédés, écrit dans le brief ; refus de
  lancer deux workers sur le même fichier ; relecture obligatoire de l'ensemble
  avant d'accepter le lot.
- **Preuve.** Un lot de 3 workers : rapport listant les fichiers de chacun, aucun
  doublon, et relecture citée.
- **Refus.** Pas de fan-out sur un fichier unique, jamais.

### AGT.6 — Brancher l'agent sur `hope-gateway`, exclusivement
- **Constat.** `FREEBUFF-FORK.md` décrit la cible (`http://127.0.0.1:8787/v1`,
  alias `hope/code`, `hope/free`) ; rien n'est branché. Un agent qui utilise une
  clé directe contourne la comptabilité des comptes.
- **Travail.** Provider OpenAI-compatible pointant sur le gateway ; garde-fou qui
  refuse un `base_url` non-loopback dans la config de l'agent.
- **Preuve.** `grep` de la config : une seule base URL, loopback ; et le journal
  du gateway montre les requêtes correspondantes.
- **Refus.** Aucun contournement de quota, aucune clé dans le fork.

### AGT.7 — Fork Freebuff sur branche dédiée, sans toucher l'amont
- **Constat.** Le dépôt est en local (`checkout séparé Freebuff`). `FREEBUFF-FORK.md` §3 nomme
  la branche `polygone-agent` ; elle n'existe pas encore.
- **Travail.** Créer la branche, y ajouter `AGENTS.md` (pointeur vers
  `ORDRE.md`, `VISION.md`, `ETAT-REEL.md`) et `.polygone-agent/README.md`.
  Ne pas modifier `agents/`, `cli/`, `common/` sans nécessité démontrée.
- **Preuve.** `git log --oneline main..polygone-agent` : uniquement des ajouts de
  documentation ; l'amont est intact.
- **Refus.** Pas de patch réseau, pas de simulation d'abonnement.

### AGT.8 — Session de 4 h sans intervention, mesurée
- **Constat.** `--loop --hours 4` existe, jamais exécuté en conditions réelles.
- **Travail.** Lancer une session, journaliser chaque tâche, arrêter proprement
  (ou sur `STOP`), produire un compte rendu machine.
- **Preuve.** Journal horodaté de 4 h : tâches tentées, preuves citées, refus,
  arrêt. Un échec honnête est un résultat acceptable.
- **Refus.** Pas de tâche marquée finie « parce que le temps est écoulé ».

### AGT.9 — Reprise après arrêt brutal
- **Constat.** Si le processus meurt, la tâche en cours reste-t-elle `in_progress`
  pour toujours ? Non vérifié.
- **Travail.** Bail de tâche avec expiration (comme `DAEMON.1`, même leçon), et
  reprise propre au démarrage.
- **Preuve.** `SIGKILL` en pleine tâche, redémarrage : la tâche redevient
  disponible, pas « en cours ».
- **Refus.** Ne pas réexécuter une tâche dont la preuve était déjà verte.

### AGT.10 — Ce que l'agent ne peut pas faire, écrit noir sur blanc
- **Constat.** `ORDRE.md` §1 pose cinq interdits ; il manque la liste mécanique
  des chemins et commandes refusés par la boucle elle-même.
- **Travail.** Refus en code (pas seulement en prose) : `~/.polygone/identity.json`,
  `.env`, `git push --force`, `reset --hard`, `clean -fd`, `rm -rf` sur source.
- **Preuve.** Un test par interdit : la commande est refusée et le refus est
  journalisé.
- **Refus.** Un interdit dans un document seul n'est pas un interdit.

---

## 4. Brique B — hope-gateway *(épique `GWX`)*

Le gateway est la brique la plus saine : persistance atomique, quotas par compte,
tests. Les tâches ci-dessous ne le réparent pas, elles le rendent **prouvable
dans le temps**.

### GWX.1 — Exécuter la suite et figer le résultat
- **Constat.** `package.json` définit `test` (`node --test tests/*.test.mjs`)
  sur 4 fichiers (`accounts`, `agent-isolation`, `gateway`,
  `provider-capabilities`) plus un mock partagé. Aucune exécution récente n'est
  consignée dans le dépôt : `reports/` ne contient que des mesures de providers,
  pas de résultat de suite.
- **Travail.** Lancer la suite, consigner le nombre de tests et la date dans
  `LIMITS.md`, corriger ce qui échoue.
- **Preuve.** Sortie brute de `npm test` citée avec sa date et son code de sortie.
- **Refus.** Ne pas marquer `skip` un test pour verdir la suite.

### GWX.2 — Ajouter une CI (le seul vrai trou)
- **Constat.** `ls .github` → **absent**. Aucun test ne tourne ailleurs que sur
  cette machine, et `be08086` (multi-compte) n'a aucune barrière.
- **Travail.** Workflow : `node --test` sur Node 22, plus un job qui vérifie que
  `state.json` reste lisible après aller-retour (sérialisation → restauration).
- **Preuve.** CI verte sur un commit ; et rouge sur un commit qui casse
  `restoreAccounts` (test négatif cité).
- **Refus.** Pas de CI qui ne teste que le lint.

### GWX.3 — Rendre l'audit « gratuit à vie » récurrent, pas daté
- **Constat.** `reports/free-forever-2026-11-02.json` est une **photo**. Un
  free-tier change sans prévenir ; la promesse vieillit mal.
- **Travail.** Exécuter `tools/free-forever-audit.mjs` à date fixe, comparer avec
  le rapport précédent, et faire échouer si un provider `freeForever: true` a
  perdu sa gratuité ou ses limites.
- **Preuve.** Deux rapports successifs et le diff commenté ; un provider retiré
  de la liste `freeForever` par la mesure, pas à la main.
- **Refus.** Ne pas continuer à annoncer « gratuit à vie » pour un provider qui
  n'a pas été re-mesuré dans la fenêtre définie.

### GWX.4 — Injection de panne par provider
- **Constat.** Le repli existe (`quotaBlockReason`, cooldowns, `pickAccount`),
  l'épuisement réel d'un provider n'est pas un scénario de test connu.
- **Travail.** Pour chaque provider configuré : 429, 500, timeout, réponse
  malformée. Vérifier le passage au suivant **et** le retour au premier après
  cooldown.
- **Preuve.** Un test par mode d'échec, avec le nom du provider suivant utilisé.
- **Refus.** Pas de repli silencieux vers un provider plus cher sans le dire dans
  la réponse.

### GWX.5 — Séparer identité et autorisation
- **Constat.** `server.mjs:194-199` : le bearer sert à **identifier** l'agent
  (`auth:<sha256 tronqué>`), pas à l'autoriser. Le gateway est donc ouvert à qui
  atteint le port.
- **Travail.** Décider (voir D10) : loopback strict, ou jeton obligatoire. Dans
  les deux cas, refuser explicitement plutôt que servir.
- **Preuve.** Requête sans jeton depuis une interface non-loopback : refus avec
  code, journalisé.
- **Refus.** Ne pas laisser un port ouvert « parce que c'est local ».

### GWX.6 — Un seul document de vérité pour les limites
- **Constat.** `LIMITS.md` dépasse 600 lignes, avec des mesures datées de
  plusieurs semaines, à côté de `catalog/*.json` et des règles d'`accounts.mjs`.
  Trois sources pour un même chiffre, c'est la même maladie que Polygone.
- **Travail.** Un test qui vérifie que chaque limite publiée dans `LIMITS.md`
  existe dans la config, et inversement.
- **Preuve.** Test vert ; rouge quand on modifie une limite dans un seul endroit.
- **Refus.** Pas de « environ » pour un chiffre qui a une source.

### GWX.7 — Attribuer la consommation par agent, pas seulement par compte
- **Constat.** L'identité d'agent est calculée mais son usage n'est pas agrégé
  par agent dans l'état persisté.
- **Travail.** Compteur par agent, exposé en lecture, borné.
- **Preuve.** Deux agents simulés, deux compteurs distincts après redémarrage.
- **Refus.** Pas de journal de prompts : le compteur suffit, le contenu reste
  hors du disque.

---

## 5. Brique D — le corpus *(épique `SRC`)*

Ta vision est la matière première de l'agent. Elle est aujourd'hui **résumée**
mais pas **exploitable** : l'agent lit `VISION-EXTRAITE.md` et ne peut pas
remonter à la phrase d'origine.

### SRC.1 — Geler le corpus et son empreinte
- **Constat.** sources locales non publiées contient trois synthèses ; l'extract d'origine est
  archivé ailleurs. Rien ne prouve que la synthèse n'a pas dérivé.
- **Travail.** Empreinte SHA-256 de chaque source et de chaque synthèse,
  consignée ; refus d'écrire dans sources locales non publiées sans mise à jour de l'empreinte.
- **Preuve.** Fichier d'empreintes, et une modification refusée par le contrôle.
- **Refus.** Ne jamais éditer une synthèse « pour la clarté » : on ajoute.

### SRC.2 — Index citable : chaque phrase de vision a un identifiant
- **Constat.** `VISION-EXTRAITE.md` est lisible par un humain, pas citable par un
  agent. Un worker ne peut pas dire « je m'appuie sur V-12 ».
- **Travail.** Identifiants stables (`V-01`, `V-02`…) sur les affirmations
  structurantes, avec la source d'origine en regard.
- **Preuve.** Un worker cite `V-xx` dans un brief, et la phrase se retrouve en une
  commande (`grep V-xx`).
- **Refus.** Pas d'identifiant sur une phrase qui n'est pas dans le corpus.

### SRC.3 — Table de correspondance vision ↔ code
- **Constat.** L'écart central du projet est là : la vision promet, le code fait
  autre chose. `ETAT-REEL.md` mesure le code ; rien ne mesure l'écart.
- **Travail.** Pour chaque `V-xx` : l'état réel (vrai / faux / hors périmètre) et
  le fichier de code concerné.
- **Preuve.** Tableau complet ; chaque « vrai » a un test nommé en face.
- **Refus.** Pas de « partiel » : soit c'est vrai, soit ce ne l'est pas.

### SRC.4 — L'agent lit la vision avant de coder
- **Constat.** `ORDRE.md` §3 dit « LIRE ORDRE.md → ETAT-REEL → BACKLOG » : la
  vision n'est pas dans la boucle.
- **Travail.** Ajouter la lecture de `SRC.2` et `SRC.3` à l'étape 1, et refuser
  une tâche qui contredit une `V-xx` sans décision écrite.
- **Preuve.** Le brief d'une tâche contient les `V-xx` applicables ; un brief qui
  les contredit est refusé.
- **Refus.** Ne pas résumer la vision dans le brief : la citer.

### SRC.5 — Ce qui est hors périmètre, gardé hors périmètre
- **Constat.** La vision contient des choses que le plan refuse (monnaie,
  téléphone-nœud, « mieux que Tor », OS Polygone). Sans liste, un agent les
  recodera un jour.
- **Travail.** Liste `V-xx → hors périmètre, raison`, référencée par `ORDRE.md`
  interdit n°5.
- **Preuve.** Une demande hors périmètre produit un refus qui cite la ligne.
- **Refus.** On ne « profite pas de l'occasion » pour l'implémenter.

---

## 6. Brique E — les 15 versions *(épique `VER`)*

### VER.1 — Trier chaque version en trois catégories
- **Constat.** 15 dossiers, un `VERSION.md` chacun, aucun tri. Certaines versions
  contiennent du code, d'autres seulement des documents.
- **Travail.** Trois catégories : **source de vérité** (à réconcilier), **idée
  non reprise** (à extraire vers le corpus), **abandonnée** (archivée, raison
  écrite). Une ligne par version dans `01-AUDIT/INVENTAIRE.md`.
- **Preuve.** Les 15 lignes existent ; aucune version sans catégorie.
- **Refus.** Ne supprimer aucun dossier, même abandonné.

### VER.2 — Extraire les idées non reprises avant d'oublier
- **Constat.** C'est exactement ce qui s'est perdu entre 2024 et 2026 : les
  meilleures idées vivent dans des versions mortes.
- **Travail.** Pour chaque version « idée non reprise » : quelle idée, où elle
  est, pourquoi elle n'a pas survécu, et si elle revient dans le plan.
- **Preuve.** Un paragraphe par idée, avec le chemin du fichier d'origine.
- **Refus.** Ne pas ressusciter une idée sans la mettre dans `SRC.5` ou backlog.

### VER.3 — Vérifier que rien de vivant ne dépend d'une version morte
- **Constat.** `RECONCILIATION.md` a déjà établi que le dépôt distant contient du
  travail non repris. Le risque symétrique existe.
- **Travail.** Chercher dans `racine du dépôt Polygone Network` les références à des chemins des
  versions ; vérifier chaque référence.
- **Preuve.** Liste des références, et zéro référence pendante.
- **Refus.** Pas de copier-coller d'une version morte « au cas où ».

---

## 7. Brique F — l'Atlas lui-même *(épique `ATL`)*

### ATL.1 — Un point d'entrée qui dit la vérité en 30 secondes
- **Constat.** `README.md` existe et décrit la structure. Il ne dit pas où en est
  le produit aujourd'hui.
- **Travail.** En tête du README : les trois compteurs de `PLAN-FINITION.md` §7
  (vraies / fausses / portes franchies), avec la date.
- **Preuve.** Les chiffres du README égaux à ceux de `ETAT-REEL.md`, vérifié par
  le test de `DOC.1`.
- **Refus.** Pas de « bientôt disponible ».

### ATL.2 — Un index navigable des 15 versions et des 132 tâches
- **Constat.** Deux index manquent : versions (VER.1) et tâches (le backlog en
  couvre 30 sur 132).
- **Travail.** `01-AUDIT/INVENTAIRE.md` tient l'index des versions ;
  `docs/atlas/plan/INDEX-TACHES.md` tient la liste complète, avec l'état de chacune.
- **Preuve.** Index généré, et aucune tâche d'épique absente.
- **Refus.** Pas d'index maintenu à la main quand il peut être généré.

### ATL.3 — Rendre l'Atlas publiable sans exposer de secret
- **Constat.** L'Atlas agrège des traces de sessions et des documents ; certains
  pourraient contenir des chemins, des identités ou des extraits sensibles.
- **Travail.** Passe de détection (même outil que `SEC.8`), liste des fichiers
  exclus, et une règle de publication.
- **Preuve.** Scan sans résultat sensible, ou fichier retiré et raison écrite.
- **Refus.** Ne pas publier « en retirant après ».

### ATL.4 — Le dépôt publié et le dépôt local ne divergent plus
- **Constat.** `ETAT-REEL.md` §8 documente une divergence Git bloquante entre
  local et distant pour Polygone ; l'Atlas n'a pas encore ce problème, il
  l'aura dès qu'il sera publié.
- **Travail.** Un contrôle qui compare `HEAD` local et distant, et refuse une
  session si l'écart dépasse un commit non poussé.
- **Preuve.** Contrôle vert, puis rouge sur un commit non poussé ajouté exprès.
- **Refus.** Pas de `push --force` pour « réaligner ».

---

## 8. Ordre global — six vagues

Une vague n'est pas une durée : c'est un ensemble de preuves à obtenir. On ne
passe pas à la suivante tant qu'une preuve de la vague est rouge.

| Vague | Polygone (A) | Gateway (B) | Agent (C) | Corpus / versions (D, E) |
|---|---|---|---|---|
| **W0** | `DOC.1`, `UI.5` (fermer G0) | `GWX.1` (mesurer avant de toucher) | `AGT.1`, `AGT.2` | `SRC.1` (geler) |
| **W1** | `DAEMON.1` → `DAEMON.3` → `DAEMON.13` | `GWX.2` (CI) | `AGT.3`, `AGT.4` | `VER.1` |
| **W2** | `BLOCKS.9`, `MSG.6`, `MESH.1`, `SEC.3/4` | `GWX.4` (pannes) | `AGT.5`, `AGT.10` | `SRC.2`, `VER.2` |
| **W3** | `CLI.1`, `CLI.2`, `CLI.5`, `INST.2`, `INST.9`, `UI.1/2/4` | `GWX.3` (gratuité récurrente), `GWX.6` | `AGT.6`, `AGT.7` | `SRC.3` |
| **W4** | G4 : `SRV`, `PETALS.2/3`, `DRIVE.1→` , `HIDE.3` | `GWX.5`, `GWX.7` | `AGT.8` (4 h réelles) | `SRC.4`, `VER.3` |
| **W5** | G5 : `CI.*`, `SEC.7/8/9`, `INST.*`, `DOC.*` | fermeture (CI + audit daté) | `AGT.9` | `SRC.5`, `ATL.1→4` |

**Pourquoi l'agent en W0 et pas à la fin :** sans `AGT.1` et `AGT.2`, tout le
travail des vagues suivantes est lancé par une boucle qui ne sait pas lire son
propre backlog — et le premier symptôme serait une tâche bloquée exécutée en
boucle. C'est le seul investissement qui se rembourse à chaque session.

**Pourquoi `GWX.1` avant `GWX.2` :** une CI qui fige une suite déjà rouge
n'apprend rien. On mesure d'abord, on automatise ensuite.

### Règles d'exécution

1. **Trois couloirs, un par dépôt** (`racine du dépôt Polygone Network`, `hope-gateway`,
   `docs/atlas/agent/`). Jamais deux agents dans le même dépôt en parallèle — c'est la
   règle 4 de `PLAN-FINITION.md` §6, et elle vaut aussi entre briques.
2. **Deux tâches maximum en cours** par couloir. La troisième signifie que la
   première est mal découpée.
3. **Une preuve rouge arrête la vague**, pas seulement la tâche.
4. **Le corpus ne se modifie pas** pendant une vague de code : `SRC.x` et les
   tâches de code ne partagent aucun fichier, mais une vision qui bouge change
   le sens de ce qu'on code.
5. **Chaque fin de vague met à jour trois fichiers** : `STATE.md`,
   `docs/atlas/agent/backlog.json` (tâches closes retirées, suivantes promues) et
   `01-AUDIT/INVENTAIRE.md` (versions reclassées).

---

## 9. Décisions qui ne m'appartiennent pas *(suite de D1…D9)*

Deux nouvelles, propres à l'écosystème.

| # | Question | Bloque | Conséquence si on ne tranche pas |
|---:|---|---|---|
| **D10** | **Le gateway écoute-t-il sur loopback seul, ou avec jeton sur une interface ?** | `GWX.5` | aujourd'hui il identifie sans autoriser : tout processus local peut consommer tes comptes |
| **D11** | **Le fork Freebuff est-il publié, ou reste-t-il privé ?** | `AGT.7`, `ATL.3` | un fork publié engage la licence de l'amont et rend ta config visible ; un fork privé ne peut pas être partagé |

D10 est la seule urgente : elle coûte une phrase et ferme un port.

---

## 10. Ce que ce plan ne fait pas

- Il ne promet **aucune date**, ici non plus. 29 tâches (132 pour Polygone, 29
  ici) sont un inventaire, pas un engagement.
- Il ne planifie **pas** les autres projets du home (`home-tv`,
  `airplay-server`, `stremio-server`).
- Il ne rouvre **pas** POLY comme monnaie.
- Il ne promet **pas** que l'agent sera bon : `AGT.8` peut échouer, et ce serait
  un résultat utile.
- Il ne remplace ni `PLAN-FINITION.md` (cœur Polygone) ni `ROADMAP.md`
  (référence longitudinale `P0`…`P5`).