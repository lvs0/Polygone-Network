# PRODUCT-READINESS — le socle est-il prêt pour une application ?

> Date : **2026-11-09**, HEAD `e2936a3` + file de tâches (modèle, API, persistance).
> Verdict fondé sur `project/FOUNDATION-STATUS.md` et
> `product/UI-API-MAPPING.md`. Aucune capacité n'est supposée disponible sans
> API et sans preuve.

---

## 1. Verdict

**PARTIELLEMENT PRÊT — pour une application de lecture et de contrôle local.
PAS PRÊT pour une application de calcul distribué, d'historique ou d'activité.**

| Type d'application | Prêt ? | Justification |
|---|---|---|
| Tableau de bord local (état, ressources, identité) | **OUI, avec réserves** | données réelles via daemon + gateway |
| Gestion du partage (profil, pause, reprise) | **OUI via CLI**, non via API | aucune API d'écriture de configuration |
| Réseau (pairs appairés, latence) | **PARTIEL** | nœuds appairés seulement ; pas de graphe live |
| Calcul / tâches | **PARTIEL** | file de tâches réelle (créer, lire, annuler, expirer) ; aucun exécuteur, aucun résultat, aucun historique |
| Activité / consommation | **NON** | aucun événement, aucune métrique exportée |
| Paramètres avancés | **NON** | pas d'API settings, pas de révocation par API |
| Application mobile / distante | **NON** | gateway loopback sans TLS, pensée pour l'hôte local |

---

## 2. Ce qu'une première application peut faire dès maintenant

1. Afficher `daemon_version`, `platform`, `uptime_secs`, `tier`.
2. Afficher les plafonds du palier et l'état `shrinking`.
3. Afficher `ram_capacity` et `storage_capacity` en distinguant mesuré,
   réservable et contraint.
4. Afficher les refus d'allocation par motif.
5. Afficher les nœuds appairés, leur `last_seen`, leur latence si mesurée.
6. Afficher l'identité publique et le `node_id`.
7. Déclencher un diagnostic et afficher sa sortie.
8. Proposer pause / reprise et changement de profil **via la CLI**, ou attendre
   une API d'écriture.
9. Créer, lire et annuler une tâche via `/api/v1/tasks`, en sachant
   qu'`executor_attached` vaut `false` : la file est réelle, l'exécution non.

Chaque valeur ci-dessus a une source réelle et un contrat documenté.

---

## 3. Blocages par écran

| Écran | Blocage | Tâche qui débloque |
|---|---|---|
| Overview | aucun blocage majeur | — |
| Network | pas de routage live exposé ; `voisins` non exposé par API | `F1-API-01` (domaine `network`) |
| My Node | réservations détaillées non listées ; écriture config sans API | `F1-API-01` (domaines `resources`, `settings`) |
| Compute | file disponible ; aucun exécuteur ni résultat vérifié | `F1-TASK-02`, `F1-TASK-03` |
| Activity | aucun événement ni métrique | `F1-EVT-01`, `F1-OBS-01`, `F1-MET-01` |
| Settings | pas d'API d'écriture, pas de révocation par API | `F1-API-01`, `F1-SEC-02` |

---

## 4. Prérequis avant toute interface graphique

| Prérequis | État |
|---|---|
| Contrats d'API stables pour les domaines affichés | partiel (`API-CONTRACTS.md`) |
| Distinction mesuré / réservable / contraint | fait |
| Valeurs absentes représentées par `null`, jamais `0` | fait côté gateway ; à respecter côté UI |
| Authentification locale de l'UI | absente : loopback seulement |
| Gestion des erreurs avec code + action | fait côté CLI ; à exposer |
| Tests de contrat par domaine | partiel : contrat des tâches testé (10 tests), autres domaines à écrire |
| Preuve multi-machine | absente — bloque toute promesse réseau publique |
| CI réelle | absente — bloque une release crédible |

---

## 5. Ce qu'il ne faut pas faire

- Construire les six écrans et les remplir de valeurs simulées.
- Exposer la gateway sur `0.0.0.0` sans TLS et sans authentification.
- Annoncer « calcul distribué » avant `F1-TASK-03`.
- Annoncer « réseau validé » avant `F1-PROOF-01`.
- Ajouter un bus d'événements complexe avant d'avoir un contrat de tâche.
- Écrire une UI qui lit les fichiers internes (`identity.json`, `ledger.json`)
  au lieu des contrats.

---

## 6. Chemin le plus court vers une première application utile

```
1. F1-API-01 (lecture)      → exposer proprement les données déjà réelles
2. UI Overview + My Node    → construire sur ces contrats
3. F1-OBS-01                → diagnostic exportable, sans secret
4. F1-TASK-01 (fait)        → modèle, file et API de tâches
5. F1-TASK-02/03            → exécuteur, reprise et résultat vérifié
6. UI Compute + Activity    → file maintenant ; progression/résultat après 5
7. F1-EVT-01                → remplacer le polling par des événements
```

---

## 7. Niveau de confiance

| Affirmation | Confiance |
|---|---|
| Les données locales affichées existent et sont mesurées | haute |
| Une application de contrôle local est réalisable sans réécrire le cœur | haute |
| Une application de calcul distribué est réalisable sans nouveau moteur | **moyenne** : modèle et file existent ; l'exécuteur distribué reste à écrire |
| Le réseau fonctionne entre deux machines | **non vérifié** |
| Le socle est publiable en l'état | **non** : CI, secrets finaux et multi-machine manquent |

---

## 8. Conclusion

Le socle est suffisamment propre pour **commencer** une application de contrôle
local sans dette majeure : identité, ressources, réservations, diagnostic et
gateway sont réels et testés. Il n'est pas suffisant pour une application de
calcul distribué : le modèle et la file de tâches existent désormais, mais
l'exécuteur, la reprise, les événements et la vérification de bout en bout des
résultats manquent. La prochaine étape technique reste `F1-RES-03`, puis
`F1-PROTO-01`, puis le branchement de l'exécuteur (`F1-TASK-02`).
