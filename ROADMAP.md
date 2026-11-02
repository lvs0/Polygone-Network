# ROADMAP — de l'état réel au produit montrable

> Pas de calendrier fictif. Chaque étape a une **sortie vérifiable**. Une étape
> ne commence pas avant que la précédente ait sa preuve. Les identifiants
> (`P0.1`) servent à l'orchestrateur et aux commits.

---

## P0 — Sauver et réconcilier (ne rien perdre)

| ID | Tâche | Sortie / preuve |
|---|---|---|
| P0.1 | Archive restaurable du dépôt canonique (`.git`, fichiers suivis, non suivis, `target/` exclu) | `tar -tzf` + `cmp` sur 3 fichiers critiques = PASS |
| P0.2 | Inventaire des 6 commits locaux et 17 distants, patch par patch | Tableau `SALVAGE` : retenu / rejeté / à discuter, aucun merge |
| P0.3 | Branche `realign/p0` avec l'état actuel commité | `git status` propre sur la branche |
| P0.4 | Corriger `ARCHITECTURE.md` (4 → 7 crates) et retirer les affirmations fausses | `ETAT-REEL.md` et README cohérents |

**Bloquant :** P0.1–P0.3. Rien d'autre ne commence sans archive et branche.

## P1 — Contrat de ressources honnête

| ID | Tâche | Sortie / preuve |
|---|---|---|
| P1.1 | Séparer capacité mesurée / réservable / réellement contrainte dans le type et l'API | Types distincts, test unitaire |
| P1.2 | Refuser une réservation non garantissable | Test : demande > plafond → refus explicite |
| P1.3 | Brancher une source d'activité utilisateur fiable ou garder `Unknown` avec message UI | Test + doc |
| P1.4 | Rendre `--isoler` le défaut Linux quand systemd est disponible | Test réel : scope créé, limites vérifiées, scope déchargé |
| P1.5 | Tests crash / perte réseau / veille / SIGKILL | 4 tests d'intégration |
| P1.6 | MacOS : soit isolation, soit message « non isolé » explicite | Doc + UI |

**Sortie :** une charge de test est limitée **par l'OS**, le propriétaire garde
la main, la restitution est prouvée.

## P2 — Premier réseau entre deux machines physiques

| ID | Tâche | Sortie / preuve |
|---|---|---|
| P2.1 | Protocole versionné : identité, auth pairs, transport, découverte | Document + tests de compatibilité |
| P2.2 | Test LAN réel, deux machines, pare-feu | Journal horodaté + transfert comparé |
| P2.3 | Test WAN, deux réseaux distincts, NAT | Journal + latence mesurée |
| P2.4 | Cas négatifs : perte, reconnexion, pair malveillant, replay, trame trop grande | 6 tests d'intégration |
| P2.5 | Mesure de latence réelle branchée sur le statut Gateway | `latency_ms` non-null en production |

**Sortie :** un E2E reproductible entre deux hôtes, sans dépendre d'un seul
processus local. C'est la preuve qui manque depuis le début.

## P3 — Premier parcours utilisateur

| ID | Tâche | Sortie / preuve |
|---|---|---|
| P3.1 | Installation propre Linux (script + paquet), désinstallation propre | VM neuve, `install` → `uninstall` sans résidu |
| P3.2 | `polygone` : création d'identité, appairage, envoi/réception, pause | Session enregistrée, non-technicien |
| P3.3 | Interface de référence simple (paramètres de contribution compréhensibles) | Captures + test manuel |
| P3.4 | Diagnostics actionnables (dernière preuve de fonctionnement) | Écran + doc |
| P3.5 | Mises à jour signées + rollback | Test de downgrade |
| P3.6 | Machine modeste + réseau domestique réel | Rapport de test |

**Sortie :** une personne non technique installe, connecte deux appareils,
échange un message et contrôle sa contribution.

## P4 — Services, dans l'ordre des dépendances

| ID | Service | Condition d'entrée | Sortie |
|---|---|---|---|
| P4.1 | **Msg** | P2 + P3 | Message éphémère fiable, TTL réel |
| P4.2 | **Drive / blocs** | P4.1 | Quotas, réplication, disponibilité mesurée |
| P4.3 | **Hide mono-saut** | P2 | Tunnel robuste, limites documentées |
| P4.4 | **Serverless** | P1 + P2 | Dispatch réel, reçus signés, quorum appelé |
| P4.5 | **Petals** | P1 + P2 | Pipeline 2 workers, inférence de référence mesurée |
| P4.6 | **SDK / bridges** | API stable | Client externe qui fonctionne |

**Règle :** chaque service a son modèle de menace et ses critères d'acceptation
**avant** la première ligne de code.

## P5 — Durcissement et publication

| ID | Tâche | Sortie |
|---|---|---|
| P5.1 | Audit de sécurité externe | Rapport public |
| P5.2 | Fuzzing parsers + tests de protocole | Corpus + zéro crash |
| P5.3 | CI Linux/macOS/Windows + builds reproductibles | Matrice verte |
| P5.4 | SBOM, provenance, signatures de release | Artefacts vérifiables |
| P5.5 | Benchmarks avec matériel et conditions | `polygone-bench` branché |
| P5.6 | Documentation utilisateur + threat model + divulgation | Dossier de release |

---

## Ce qui n'entre pas dans la roadmap

Interface neuronale, IA biomimétique, OS Polygone, token `POLY`, marketplace,
téléphone-nœud, Hide multi-hop, support Windows déclaré, « mieux que Tor ».
Ces sujets restent dans `VISION.md` §8 comme **hors périmètre jusqu'à décision**.

## Ordre de priorité en cas de conflit

1. Ne rien perdre (P0).
2. Ne pas mentir (P0.4, ETAT-REEL).
3. Prouver les ressources (P1).
4. Prouver le réseau (P2).
5. Prouver l'usage (P3).
6. Seulement ensuite, ajouter (P4).
