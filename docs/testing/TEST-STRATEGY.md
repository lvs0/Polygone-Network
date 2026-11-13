# TEST-STRATEGY — niveaux de preuve et portes de qualité

> Date : **2026-11-09**, HEAD `e2936a3`. Ce document dit **ce que chaque test
> prouve**, et surtout ce qu'il ne prouve pas. Une preuve locale n'est jamais
> une preuve multi-machine ; un mock n'est jamais une preuve réelle.

---

## 1. Niveaux de preuve

| Niveau | Nom | Ce qui tourne | Ce que cela prouve | Ce que cela ne prouve pas |
|---|---|---|---|---|
| L0 | Unitaire | une fonction, sans E/S | règles, invariants, erreurs, bornes | intégration, réseau, persistance réelle |
| L1 | Intégration en processus | plusieurs modules, fichiers temporaires | protocole, persistance, migration, permissions | deux machines, pannes réseau |
| L2 | Multi-processus local | relais + deux pairs + daemon sur loopback | sockets réels, handshake, routage, signatures | deux hôtes, NAT, pare-feu, latence réelle |
| L3 | Multi-hôte physique | deux machines distinctes | réseau réel, découverte, reconnexion | Internet, NAT, adversaire actif |
| L4 | Charge / résilience | pannes injectées, volume borné | comportement sous saturation | sécurité, correction fonctionnelle |
| L5 | Audit externe | tiers | inconnues non couvertes par l'équipe | — (non réalisé) |

**Règle de lecture** : un résultat L0/L1/L2 ne doit jamais être écrit « réseau
validé ». Le mot « multi-machine » est réservé au niveau L3.

---

## 2. Portes de qualité locales

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node scripts/check-docs.mjs --tests <N>
node scripts/check-ci-honesty.mjs
node --test docs/atlas/agent/tests/backlog.test.mjs
cargo test -p polygone-client --test no_tracked_secrets
```

`make check` exécute fmt + clippy + tests + docs. `make smoke` exécute le
parcours réel relais + deux pairs. `make migrate-proof` rejoue la migration
`peers.json` v1 → v2.

---

## 3. Répartition actuelle des tests

| Suite | Nombre | Niveau dominant |
|---|---:|---|
| `polygone-core` (lib) | 32 | L0 |
| `polygone-client` (lib) | 65 | L0/L1 |
| `polygone` (bin) | 127 | L0/L1/L2 |
| `polygone-relay` | 28 | L1/L2 |
| `polygone-gateway` (lib + bin + serve) | 47 | L1/L2 |
| `polygone-petals` | 19 | L0/L1 |
| `polygone-serverless` | 17 | L0 |
| `polygoned` (lib + bin) | 127 | L0/L1/L2 |
| tests d'intégration client (`doc_index`, `no_tracked_secrets`) | 3 | L1 |
| doc-tests | 1 | L0 |
| **Total** | **465** | |

Aucun test de niveau L3 n'existe. Les scripts `p0-two-host.sh` et
`docs/P0-DEUX-MACHINES.md` préparent ce niveau mais n'ont pas été exécutés.

---

## 4. Ce qui est testé, par domaine

| Domaine | Preuves existantes | Trous |
|---|---|---|
| Cryptographie | tailles exactes, mauvaise clé, 35 combinaisons Shamir | canaux auxiliaires, fuzzing des parseurs |
| Identité | persistance, permissions, rotation, révocation | restauration depuis sauvegarde |
| Messages | aller-retour, signature, anti-rejeu, usurpation `from` | perte réseau, réordonnancement |
| Relais | handshake, slot, limites, `from` spoofé | charge, saturation, multi-hôte |
| Ledger | réserve/libère/renouvelle/expire, redémarrage, atomicité, quarantaine | concurrence extrême, disque plein |
| Ressources | mesure Linux/macOS, fail-closed, capacité publiée | GPU réel, macOS runner |
| Daemon | tick, publication unique, pas d'écriture cgroup | longue durée, signaux |
| Gateway | appairage, heartbeat, ping/pong, socket réel | TLS, exposition publique |
| Exécution | shell, WASM, timeout, unité arrêtée | évasion d'un adversaire déterminé |
| Blocs | hash, put/get, serveur local | deux hôtes |
| Migration | `peers.json` v1 → v2, inter-versions | autres formats persistés |
| Installation | fixture, désinstallation sans résidu | VM neuve, paquets système |

---

## 5. Tests négatifs exigés

Un test positif sans contre-épreuve ne prouve pas qu'un refus fonctionne. Les
contre-épreuves suivantes existent et doivent rester :

- mauvaise clé KEM → échec de déchiffrement ;
- signature absente/invalide → rejet ;
- `from` différent de l'identité → trame jetée ;
- rejeu → `REPLAY.*` explicite ;
- pong sans ping / rejoué → 409 ;
- heartbeat mal signé → 403, `last_seen` inchangé, aucun fichier écrit ;
- allocation au-delà de la marge → refus nommé ;
- renouvellement d'un bail échu → refus ;
- ledger illisible → quarantaine, pas de résurrection ;
- secret suivi par git → test en échec nommant le fichier ;
- document daté sans bandeau → `check-docs` en échec.

---

## 6. Résilience

Scénarios couverts : SIGKILL du consommateur, SIGKILL du daemon, expiration de
bail, redémarrage, perte de bail côté nœud, inactivité de connexion, panic d'un
handler, fichier de pairs corrompu, ledger corrompu.

Scénarios non couverts : coupure réseau pendant un transfert, réordonnancement,
duplication massive, disque plein pendant une écriture, horloge qui saute,
réveil de veille, changement d'interface réseau.

---

## 7. Charge

Aucun test de charge n'a été exécuté. Toute campagne future doit :

1. fixer des limites basses et mesurables ;
2. surveiller CPU, mémoire et disque de la machine de test ;
3. s'arrêter avant saturation du poste ;
4. rapporter la configuration matérielle et la date.

Ne pas lancer plusieurs compilations ou campagnes en parallèle sur cette machine
(4 cœurs, 7,4 Gio de RAM, disque à 93 %).

---

## 8. CI

Le workflow `.github/workflows/ci.yml` prévoit :

- `test` sur Ubuntu et macOS : fmt, clippy, tests, build release ;
- `smoke` sur Ubuntu : syntaxe des scripts, installateur hors ligne, honnêteté
  de la matrice, absence de secret suivi, parcours P0 réel, smokes produit ;
- `windows-canary` non bloquant, échec attendu et documenté.

**Limite** : ce workflow n'a jamais été exécuté sur cet arbre. La structure est
vérifiée localement, pas le runner.

---

## 9. Secrets et dépendances

- `crates/client/tests/no_tracked_secrets.rs` refuse les chemins sensibles
  suivis par git.
- `~/bin/gitleaks` 8.30.1 a produit une preuve datée ; elle doit être rejouée
  sur l'arbre final avant toute publication.
- Aucun audit de dépendances (`cargo audit`, SBOM) n'a été exécuté.

---

## 10. Règles d'ajout de test

1. Un correctif de bug ajoute un test qui échoue avant et passe après.
2. Un test de refus doit nommer le refus, pas seulement constater l'absence.
3. Un test qui n'a lu aucun fichier ou n'a exécuté aucun cas doit échouer
   explicitement (garde anti-faux-négatif).
4. Un test de persistance écrit dans un répertoire temporaire isolé.
5. Aucun test ne dépend de l'ordre d'exécution ni d'un état global non réinitialisé.
