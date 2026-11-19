# PRODUCT-READINESS — le socle est-il prêt pour une application ?

> Date : **2026-11-13**, HEAD `f6382e6` + exécuteur local et reçu signé.
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
| Calcul / tâches | **PARTIEL** | file réelle (créer, lire, annuler, expirer), exécution locale `echo`/`blake3` et résultat signé vérifiable ; aucun placement multi-nœuds, aucun historique |
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
9. Créer, lire et annuler une tâche via `/api/v1/tasks`, et lire son résultat
   signé lorsque `executor_attached` vaut `true` ; sans fichier de tâches, la
   file reste réelle mais la tâche reste `published`.

Chaque valeur ci-dessus a une source réelle et un contrat documenté.

---

## 3. Blocages par écran

| Écran | Blocage | Tâche qui débloque |
|---|---|---|
| Overview | aucun blocage majeur | — |
| Network | pas de routage live exposé ; `voisins` non exposé par API | `F1-API-01` (domaine `network`) |
| My Node | réservations détaillées non listées ; écriture config sans API | `F1-API-01` (domaines `resources`, `settings`) |
| Compute | exécution locale et résultat signé disponibles ; placement multi-nœuds et progression en temps réel absents | `F1-API-01`, `F1-EVT-01` |
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
| Tests de contrat par domaine | partiel : contrat des tâches testé (13 tests de magasin), autres domaines à écrire |
| Preuve multi-machine | absente — bloque toute promesse réseau publique |
| CI réelle | absente — bloque une release crédible |

---

## 5. Ce qu'il ne faut pas faire

- Construire les six écrans et les remplir de valeurs simulées.
- Exposer la gateway sur `0.0.0.0` sans TLS et sans authentification.
- Annoncer « calcul distribué » avant un placement multi-nœuds réel.
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
5. F1-TASK-02/03 (fait)     → exécuteur local, reprise et résultat signé
6. UI Compute + Activity    → file et résultat maintenant ; progression après 7
7. F1-EVT-01                → remplacer le polling par des événements
```

---

## 7. Niveau de confiance

| Affirmation | Confiance |
|---|---|
| Les données locales affichées existent et sont mesurées | haute |
| Une application de contrôle local est réalisable sans réécrire le cœur | haute |
| Une application de calcul distribué est réalisable sans nouveau moteur | **moyenne** : modèle, file, exécution locale signée et vérification existent ; le placement distribué reste à écrire |
| Le réseau fonctionne entre deux machines | **non vérifié** |
| Le socle est publiable en l'état | **non** : CI, secrets finaux et multi-machine manquent |

---

## 8. Conclusion

Le socle est suffisamment propre pour **commencer** une application de contrôle
local sans dette majeure : identité, ressources, réservations, diagnostic et
gateway sont réels et testés. Une application de calcul local est désormais
possible : tâche, exécution `echo`/`blake3`, reprise et résultat signé vérifiable.
Il manque encore le placement multi-nœuds, les événements et l'historique pour
une application de calcul distribué. La prochaine étape technique est
`F1-API-01`, puis `F1-OBS-01`.
