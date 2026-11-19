# PRODUCT-READINESS — le socle est-il prêt pour une application ?

> Date : **2026-11-13**, HEAD `f6382e6` + exécuteur local et reçu signé.
> Verdict fondé sur `project/FOUNDATION-STATUS.md` et
> `product/UI-API-MAPPING.md`. Aucune capacité n'est supposée disponible sans
> API et sans preuve.
>
> **Mise à jour 2026-11-19 — prototype `prototype-complet` CONSTRUIT.**
> Les domaines `identity`, `resources`, `settings` (lecture), `usage` (ledger
> actif + refus) et `events` (F1-EVT-01) sont livrés, contractés
> (`API-CONTRACTS.md` §2.4) et testés (14 tests de contrat `local_api.rs` +
> tests unitaires `local.rs`/`events.rs` + test ledger daemon). Les écrans
> Console étendu, Compute et Activité sont construits dessus (`UI-API-MAPPING`
> §9) et vérifiés en vivant sur daemon + gateway réels (tâches `echo`/`blake3`
> signées, événements, relais 503 si daemon absent, refus `policy_shrinking`
> relayés). Le verdict « PAS PRÊT pour l'activité » est levé ; restent bloqués
> l'historique de consommation, le placement multi-nœuds et les écritures.

---

## 1. Verdict

**PRÊT pour une application de lecture et de contrôle local — prototype
construit et vérifié le 2026-11-19. PAS PRÊT pour une application de calcul
distribué ou d'historique de consommation.**

| Type d'application | Prêt ? | Justification |
|---|---|---|
| Tableau de bord local (état, ressources, identité) | **OUI — construit** | `node.html` + `node-local.js` sur `/identity`, `/daemon`, `/resources`, `/settings` |
| Gestion du partage (profil, pause, reprise) | **lecture via API, écriture via CLI** | `/settings` lecture seule livrée ; écriture = F1-SEC-02 |
| Réseau (pairs appairés, latence) | **PARTIEL** | nœuds appairés seulement ; pas de graphe live |
| Calcul / tâches | **PARTIEL — écran construit** | `compute.html` : file réelle, exécution locale `echo`/`blake3`, résultat signé ; aucun placement multi-nœuds |
| Activité / consommation | **OUI pour le flux et le ledger actif — construit** | `activity.html` : événements bornés (F1-EVT-01) + réservations actives + refus ; l'historique de consommation n'est pas enregistré |
| Paramètres avancés | **lecture OUI, écriture NON** | `/settings` livrée ; pas de révocation par API |
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
| Overview | aucun blocage majeur — **construit** | — |
| Network | pas de routage live exposé ; `voisins` non exposé par API | `F1-API-01` (domaine `network`), mesures inter-hôtes |
| My Node | réservations actives **listées** (`/usage`) ; écriture config sans API | `F1-SEC-02` (écriture) |
| Compute | exécution locale et résultat signé **affichés** ; placement multi-nœuds absent ; progression couverte par les événements de cycle de vie | `F1-API-01` (placement), mesures inter-hôtes |
| Activity | événements et ledger actif **affichés** ; historique de consommation non enregistré ; journal de diagnostic non structuré | `F1-USG` (à ouvrir), `F1-OBS-01`, `F1-MET-01` |
| Settings | lecture **affichée** ; pas d'API d'écriture, pas de révocation par API | `F1-SEC-02` |

---

## 4. Prérequis avant toute interface graphique

| Prérequis | État |
|---|---|
| Contrats d'API stables pour les domaines affichés | **fait pour les domaines locaux** (`API-CONTRACTS.md` §2.4, `beta`) ; `network`/`tasks` distribués encore partiels |
| Distinction mesuré / réservable / contraint | fait |
| Valeurs absentes représentées par `null`, jamais `0` | fait côté gateway **et UI** (`node-local.js`, `activity.js`, `compute.js`) |
| Authentification locale de l'UI | absente : loopback seulement |
| Gestion des erreurs avec code + action | **fait** : codes stables (`daemon_absent`, `identity_absent`, `config_invalid`, `ledger_unsupported`…) + hint, affichés par l'UI |
| Tests de contrat par domaine | **fait pour les domaines locaux** : 14 tests `local_api.rs` (parité relais sur vrai daemon, secrets, absence, gap, 501) + unitaires `local.rs`/`events.rs` + ledger daemon |
| Preuve multi-machine | absente — bloque toute promesse réseau publique |
| CI réelle | **existe** (`.github/workflows/ci.yml`, verte sur `main` depuis la réconciliation du 2026-11-19) |

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
1. F1-API-01 (lecture)      → FAIT 2026-11-19 (identity, resources, settings, usage)
2. UI Overview + My Node    → FAIT 2026-11-19 (node.html + node-local.js)
3. F1-OBS-01                → diagnostic exportable, sans secret (à faire)
4. F1-TASK-01 (fait)        → modèle, file et API de tâches
5. F1-TASK-02/03 (fait)     → exécuteur local, reprise et résultat signé
6. UI Compute + Activity    → FAIT 2026-11-19 (compute.html, activity.html)
7. F1-EVT-01                → FAIT 2026-11-19 (flux borné à curseur, 13 kinds)
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

**Mise à jour 2026-11-19** : le prototype de contrôle local est CONSTRUIT.
`F1-API-01` (lecture des domaines locaux) et `F1-EVT-01` sont livrés et testés ;
les écrans Console, Compute et Activité existent et consomment les contrats.
La prochaine étape technique est `F1-OBS-01` (diagnostic exportable), puis
`F1-SEC-02` (écritures bornées) et le placement multi-nœuds (mesures
inter-hôtes requises). L'historique de consommation reste un ticket à ouvrir
(`F1-USG`) : le ledger n'enregistre pas les baux clos, et aucune UI ne doit
prétendre le contraire.
