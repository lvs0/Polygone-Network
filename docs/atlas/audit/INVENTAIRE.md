# Inventaire des 15 versions historiques de Polygone

> **Instantané daté — ne pas lire comme l'état courant.** Ce document est rédigé le 2026-11-04 (import) ; il ne décrit pas l'état vérifié aujourd'hui, qui se lit dans [`ETAT-REEL.md`](../../../ETAT-REEL.md) et [`STATE.md`](../../../STATE.md).

> Synthèse d'audit portée le 4 novembre 2026. Les noms de versions sont
> conservés pour la traçabilité ; les chemins locaux, tailles privées et
> artefacts de données ne sont pas publiés ici.

| # | Version / famille | Nature constatée | Traitement recommandé |
|---:|---|---|---|
| 01 | Polygone v1 agents | Démon biomimétique Rust/Python | Archive, distincte du produit réseau |
| 02 | Projets Polygone | Ancienne ligne produit Rust | Source historique |
| 03 | Polygone Network snapshot | Snapshot sans historique Git | Source de comparaison |
| 04 | Polygone-real | Ancien checkout canonique Rust | Source historique ; ce dépôt est le checkout de travail |
| 05 | Polygone v2 | Tentative v2 indépendante | Source utile, aucune fusion sans preuve |
| 06 | Polygone v2 brand | Ressource de marque isolée | Archive |
| 07 | Polygone résumé | Reprise documentaire et boucles d'agents | Source de méthodes |
| 08 | Polygone recovery | Moteur de mémoire / archéologie | Données privées, ne pas copier |
| 09 | Polygone projects recovery | Pipeline d'ingestion par lots | Source de méthodes à auditer |
| 10 | Polygone company | Harnais d'entreprise autonome | Modèle d'organisation |
| 11 | Polygone team | Orchestrateur de dispatch et preuves | Modèle à adapter et tester |
| 12 | Polygone knowledge graph | Graphe de connaissances SQLite | Données privées, ne pas copier |
| 13 | Documentation Polygone | Sources historiques de vision | Synthèse importée, sources brutes exclues |
| 14 | R-Labs Polygone | Recette Docker de nœud | Harnais de test à réévaluer |
| 15 | État Polygone | État runtime, identité et ledger | Données sensibles, ne jamais committer |

## Ce que la dispersion révélait

Les anciennes versions contenaient plusieurs lignes de code concurrentes,
des documents décrivant des états différents et des harnais d'autonomie ayant
produit des rapports sans toujours livrer des fonctionnalités intégrées.

Règles retenues :
1. Le code de ce dépôt, ses tests et ses appelants définissent l'état réel.
2. VISION.md définit la vision produit actuelle.
3. Les plans décrivent le travail restant ; ils ne prouvent pas son exécution.
4. Les anciennes implémentations sont des sources d'idées, pas des dépendances
   à recopier automatiquement.

## Éléments exclus des commits

Conversations brutes, exports de chat, corpus extraits, bases SQLite personnelles,
identités, clés privées, ledger runtime, fichiers .env, target/, node_modules/,
.git/, caches, journaux locaux et tout état contenant des données de compte.
