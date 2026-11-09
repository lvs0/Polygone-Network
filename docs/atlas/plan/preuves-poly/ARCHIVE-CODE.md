# POLY — ce que le code archivé faisait réellement

> **Instantané daté — ne pas lire comme l'état courant.** Ce document est rédigé le 2026-11-04 (import) ; il ne décrit pas l'état vérifié aujourd'hui, qui se lit dans [`ETAT-REEL.md`](../../../../ETAT-REEL.md) et [`STATE.md`](../../../../STATE.md).

> NOTE DE PORTAGE — 4 novembre 2026 : extraits historiques conservés comme preuve d’archive ; ne pas recopier sans audit.

> Source : `Projects/Polygone-real/archive/2026-07-src/` (monolithe mis de côté,
> jamais compilé après la purge). Chaque affirmation ci-dessous vient d'un
> `fichier:ligne` réel. Aucune extrapolation.

---

## 1. Modèle comptable

La spec d'origine est citée dans l'en-tête de `economy/mod.rs` :

> « Économie POLY : Solde disponible en jetons POLY (système de comptabilité
> local stocké de manière sécurisée dans `~/.polygone/poly.toml` **sans recours
> à une blockchain**). Affichage de la consommation courante exprimée en temps
> réel (ex : 0.1 POLY / min). »

| Propriété | Valeur réelle (code) |
|---|---|
| Support | fichier TOML local `~/.polygone/poly.toml` |
| Blockchain | **aucune** — c'est écrit dans le doc-comment, pas une omission |
| Écriture | atomique (`.toml.tmp` puis `rename`), `economy/mod.rs:99-116` |
| Solde initial | **100.0 POLY** offerts par défaut (`:63-65`) |
| Consommation | `0.1 POLY/min` **× nombre de services actifs** (`RATE_PER_MIN`, `:21`) |
| Gain prêt CPU | `10 POLY / cœur-heure` (`:24`) |
| Gain prêt RAM | `5 POLY / Go-heure` (`:27`) |
| Conversion RAM | le gain divise les Mo par 1024 → traite `amount` comme des **Mo** |
| Découvert | interdit : `record_renting_expense` plafonne au solde (`:186`) |

Le fichier porte aussi des compteurs cumulés (`total_lending_earned`,
`total_renting_spent`, contrats complétés) et un `booted_at_ms`.

**Conclusion partielle : POLY n'a jamais été conçu comme une cryptomonnaie.**
C'était une unité de compte interne, à un seul côté, dans un fichier texte
sans signature ni réplication.

---

## 2. Qui appelait quoi — le point décisif

| Fonction | Appelant réel | Verdict |
|---|---|---|
| `Ticker::load()` | `tui/app.rs:212`, `compute/daemon.rs:181` | branché |
| `Ticker::tick()` (drain) | `tui/app.rs:318`, `tui/app.rs:457`, `compute/daemon.rs:403` | branché |
| `Ticker::set_active()` | `tui/app.rs:281`, `:335`, `daemon.rs:395,399,422` | branché |
| `Ticker::snapshot()` | `tui/views.rs:215,491,623,624`, `daemon.rs:439` | branché (affichage) |
| `Ticker::record_lending_income()` | **uniquement les tests** (`economy/mod.rs:335`) | **orphelin** |
| `Ticker::record_renting_expense()` | **uniquement les tests** (`:346`, `:355`) | **orphelin** |
| `Ticker::calculate_lending_income()` | **uniquement les tests** (`:364`, `:367`) | **orphelin** |
| `Ticker::lending_summary()` | **uniquement les tests** (`:374`) | **orphelin** |
| `Ledger::default()` | `load_from` quand le fichier manque | branché |
| `WorkVoucher::new()` / `verify()` | **aucun appelant, aucun test** | **code mort** |
| `KarmaStore::load_from_file()` / `total_units()` | **aucun appelant, aucun test** | **code mort** |
| `ResourceScheduler::enqueue_request()` | **aucun appelant** | **code mort** |
| `ResourceScheduler::schedule()` | `compute/daemon.rs:378` | branché… sur une file vide |
| `simulate_consumption()` | **aucun appelant** | **code mort** |

### La conséquence en cascade

`enqueue_request()` n'étant jamais appelé, `pending_queue` reste vide pour
toujours. Donc :

1. `schedule()` (`daemon.rs:378`) itère sur une file vide → retourne `vec![]` ;
2. aucune `ResourceAllocation` n'est jamais créée ;
3. `active_allocations().len()` vaut **toujours 0** (`daemon.rs:394`) ;
4. `set_active(0)` → `rate_per_min = 0.1 × 0 = 0` ;
5. `tick()` draine `0` → le solde **reste figé à 100.00** ;
6. `total_poly_earned` n'est incrémenté que dans `expire_allocations()`, qui
   parcourt une liste d'allocations vide → **reste 0.0**.

Le seul chiffre vivant était donc une constante de départ. Un utilisateur
ouvrant la TUI voyait `100.00 POLY` et `0.00/min` — le solde ne bougeait que
si `active_services` devenait non nul, ce que rien dans le produit ne pouvait
produire.

### Deux comptabilités jamais réconciliées

`ComputeStatus` (`daemon.rs:447-451`) expose côte à côte deux sources :

- `poly_earned` ← `lending_stats.total_poly_earned` (compteur du scheduler) ;
- `poly_balance` ← `poly_snapshot.balance` (ledger TOML).

Le scheduler crédite `total_poly_earned` (`lending.rs:354`) mais **n'appelle
jamais** `record_lending_income()`. Les deux moitiés ne se parlent pas : gagner
du POLY aurait fait monter un compteur d'affichage sans jamais augmenter le
solde.

---

## 3. Le `WorkVoucher` — la primitive prometteuse, restée morte

`crypto/karma.rs:9-63`. Structure : `worker_id`, `requester_id`, `units`,
`issued_at`, `signature`.

| Question | Réponse |
|---|---|
| Qui signe ? | le **demandeur** (`requester_id`), avec sa clé ML-DSA-65 |
| Donc qu'est-ce que ça prouve ? | que le demandeur reconnaît avoir reçu N unités de travail |
| Qu'est-ce que ça ne prouve pas ? | que le travail a été fait, ni son coût, ni la solvabilité du signataire |
| Qui vérifie ? | `verify(public_key)` — correct, mais **personne ne l'appelle** |
| Persistance | `KarmaStore` en bincode, jamais instancié |
| Réputation | le champ `reputation` du protocole est un `u32` fourni par le pair lui-même, non vérifié |

C'est une **reconnaissance de dette signée**, pas une preuve de travail. Le nom
`WorkVoucher` était optimiste : la signature atteste d'un accord, pas d'une
exécution. Rien n'empêche un demandeur de ne jamais émettre de voucher, ni un
worker d'en fabriquer avec une clé complice.

---

## 4. Le contrat de prêt (`compute/lending.rs`)

Cycle de vie réel : `enqueue_request` → `schedule` → `ResourceAllocation`
(actif) → `expire_allocations` (fin) → `poly_earned()` calculé sur
`poly_offer × heures écoulées`.

| Garantie | Réalité |
|---|---|
| Paiement à la fin | **aucun** — le calcul existe, le débit/crédit n'est jamais déclenché |
| Non-paiement | **non traité** : pas de séquestre, pas de créance, pas de pénalité |
| Triche sur l'usage | `consumed_mb` et `cpu_seconds_used` ne sont remplis que par `simulate_consumption()`, qui n'est jamais appelé |
| Preuve d'exécution | aucune — le protocole prévoit `PaymentProof`, jamais émis |
| Protection de l'hôte | `ResourceLimits` réels (fractions RAM/CPU, plancher de réserve, `lending_ok` si machine inactive) |
| Plafond de simultanéité | `MAX_CONCURRENT_TASKS = 4` |
| Priorités | 4 niveaux, tri décroissant à la planification |
| GPU / stockage | **explicitement non supportés** (`_ => false`, `lending.rs:333`) |

Le protocole (`compute/protocol.rs`) est complet et propre — 10 types de
messages, bincode, validateurs — mais **aucun message n'est jamais transporté** :
`ComputeMessage::to_bytes/from_bytes` n'ont d'appelant que dans les tests.
Les 11 tests du module testent la sérialisation, pas une négociation.

---

## 5. Verdict technique

**Réellement fonctionnel :**

- La comptabilité locale : lecture/écriture atomique d'un TOML, arithmétique de
  drain et de gain, tests unitaires présents et justes sur les taux
  (`4 cœurs × 1 h = 40 POLY`).
- Les `ResourceLimits` : calcul de ce qu'on peut prêter à partir de métriques
  réelles, avec plancher de sécurité.
- Le protocole comme **spécification** : les types décrivent honnêtement une
  négociation complète.

**Simulation :**

- `poly_offer` / `poly_earned()` : le prix y est un champ saisi par l'appelant,
  sans marché, sans contrepartie, sans règlement.
- `reputation` : auto-déclarée dans `CapabilityAnnounce`.

**Pur affichage :**

- Tout le panneau POLY de la TUI (`tui/views.rs:623`) : solde et débit/minute.
- `poly_earned` / `poly_spent` dans `ComputeStatus` : zéro structurel.

**Le point le plus dur :** le ledger était « branché » et néanmoins inerte,
parce que la seule porte d'entrée de la demande de ressources
(`enqueue_request`) n'était ouverte par personne. Une économie dont un seul
côté est câblé ne produit pas des chiffres faux — elle produit des chiffres
constants, ce qui est plus difficile à détecter.

**Ce qui mérite d'être repris, et sous quelle condition :** le `WorkVoucher`
signé est la bonne primitive — c'est le seul objet du corpus qui pourrait
fonder un échange vérifiable entre deux inconnus. Mais il faudrait qu'il soit
signé par **les deux** parties (le worker atteste l'exécution, le demandeur
atteste la réception), et qu'un ledger crédite sur présentation d'une paire de
signatures. Autrement dit : ce n'est pas POLY tel qu'il existait qu'il faut
ressusciter, c'est le reçu.