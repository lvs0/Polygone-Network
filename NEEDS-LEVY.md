# NEEDS-LEVY — décisions que l'agent ne peut pas prendre

> Chaque ligne bloque une partie du travail ou fixe une direction durable.
> L'agent peut avancer ailleurs en attendant ; il ne tranche pas à ta place.

## Bloquant avant publication

| # | Question | Options | Pourquoi ça bloque |
|---:|---|---|---|
| 1 | **Licence** : AGPL-3.0-or-later (état actuel du `Cargo.toml`) ou retour à MIT (ancienne ligne v1) ? | AGPL / MIT / autre | Une release sans licence claire est inutilisable par un partenaire |
| 2 | **Le produit est-il un seul binaire `polygone`** (GUI + CLI) ou deux produits distincts ? | 1 binaire / 2 produits | Décide l'architecture de P3 |
| 3 | **Le palier payant** peut-il exister avant que le gratuit soit prouvé ? | Non (recommandé) / Oui | Décide `VISION.md` §6 et l'ordre de la roadmap |
| 4 | **AGPL vs adoption entreprise** : acceptable, ou faut-il une licence plus permissive pour Proton/entreprises ? | Garder / changer | Impact direct sur le modèle économique |

## Direction produit

| # | Question | Hypothèse actuelle de l'Atlas |
|---:|---|---|
| 5 | Le **mobile** est-il prioritaire dans les 6 mois ? | Non (`VISION.md` §8) |
| 6 | Le **jeton POLY** est-il définitivement reporté ? | Oui, sine die |
| 7 | **Hide multi-hop** est-il un objectif, ou un mono-saut honnête suffit-il ? | Mono-saut d'abord |
| 8 | **Petals distribué** doit-il être livré avant le SDK ? | Non : P4.5 après P4.4 |
| 9 | Faut-il **supprimer** les dossiers sources après consolidation, ou les garder en archive froide ? | Garder (non destructif) |
| 10 | Le **nom** « Polygone » et la marque sont-ils figés ? | Oui, sauf indication |

## Décisions techniques (ajout 2026-11-15)

| # | Question | Options | Ce que ça bloque |
|---:|---|---|---|
| 15 | **Preuve canonique** : `polygone.evidence.v1` doit-il devenir un fichier persisté (`evidence.draft.json` + `.sha256`) ? | Oui, avec ADR / non, rester en mémoire | Une primitive existe et est testée (`crates/core/src/evidence.rs`), mais écrire une preuve sur disque est un format persisté : ADR requis (§9.6 de `project/AUTONOMOUS_EXECUTION.md`). Sans décision : aucune preuve n'est écrite. |
| 16 | **Attribution de faute** quand deux mesures divergent (`Divergent`) : producteur, demandeur, partagée, ou aucune ? | Producteur / demandeur / partagée / indéterminée | Le type `Comparison` nomme le fait (`output_differs`, `committed_hash_disagrees`) mais ne dit pas qui doit payer. Toute politique de réputation en dépend. |
| 17 | **Relais en cours de mise à jour** : la reprise après redémarrage est prouvée ; la reprise *pendant* une mise à jour binaire ne l'est pas. | Tester maintenant / plus tard | Un test de reprise après mise à jour exige de figer un binaire antérieur comme fixture, donc une décision de stockage de cette fixture. |

## Faits à confirmer

| # | Fait | Où c'est incertain |
|---:|---|---|
| 11 | Les **17 commits distants** contiennent-ils du travail à garder absolument (checksum installateur, tests Petals, portage Windows) ? | `Salvage.md` §7, `docs/RECONCILIATION.md` à produire |
| 12 | Les **3 comptes Groq / 2 comptes OpenRouter** sont-ils bien des comptes distincts, ou des clés du même compte ? | `~/.hope/providers.json` ; impact sur `hope/free` |
| 13 | Les **quotas publiés** de chaque free tier (RPM/RPD) sont-ils ceux que tu veux appliquer ? | `accounts.mjs` ne devine pas |
| 14 | Le **domaine** de déploiement public et l'hébergeur (TLS, DNS) sont-ils décidés ? | Bloque P5 et le Gateway public |

## Ce que l'agent ne fera jamais sans toi

- Changer la licence.
- Changer le nom ou la marque.
- Publier une release.
- Pousser sur `main` de `Polygone-Network`.
- Supprimer un dossier source.
- Activer un palier payant ou un jeton.
- Déclarer le support Windows/macOS.

## Comment répondre

Une ligne par numéro dans ce fichier, ou un message. L'agent met à jour la
décision et la date, puis reprend la tâche débloquée.
