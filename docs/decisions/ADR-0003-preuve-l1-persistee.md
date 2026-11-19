# ADR-0003 — Preuve L1 persistée : `polygone.evidence.v1` sur disque

- **Statut** : acceptée (2026-11-19, mandat opérateur « finir la production en toute autonomie »)
- **Autorité** : §9.6 de `project/AUTONOMOUS_EXECUTION.md` — un format persisté exige un ADR.
- **Preuve** : `crates/client/src/net/mod.rs` (`persist_l1_evidence`) ;
  tests `net::tests::l1_evidence_*` ; primitive `crates/core/src/evidence.rs`.

## Contexte

`--verifier-local` (L1) recalcule une tâche déterministe dans le même sandbox et compare les
sorties **en mémoire** (`annotate_local_recomputation`). La comparaison disparaît avec le
processus : deux opérateurs ne peuvent pas confronter leurs mesures après coup, et une
divergence n'est pas auditable au-delà de l'affichage. La primitive `polygone.evidence.v1`
(session 10) donne une forme canonique comparable octet à octet, mais n'était câblée à aucun
binaire. Il manquait la décision de **persistance** : où, sous quelle forme, avec quelles
garanties.

## Décision

Chaque vérification L1 — **accord comme divergence** — émet un fichier de preuve :

- **Emplacement** : `~/.polygone/evidence-<measured_at>-<digest8>.json`
  (`measured_at` en secondes Unix, `digest8` = 8 premiers hex du BLAKE3 des octets canoniques).
  Les fichiers vivent à la racine du répertoire privé `.polygone` (0700) : le helper
  d'écriture privée ne durcit que ce répertoire, pas un sous-répertoire — un préfixe
  `evidence-` plutôt qu'un dossier `evidence/`.
- **Contenu** : JSON canonique `polygone.evidence.v1`, `deny_unknown_fields`, ordre des champs
  fixé par la structure (`EvidenceV1::canonical_json`).
- **Sémantique des engagements** :
  - `committed_blake3` = digest de la sortie du grant **signé par le producteur**, reçu avant
    le recalcul : c'est l'engagement antérieur ;
  - `observed_blake3` = digest de la sortie **recalculée localement** : c'est l'observation.
- **Sujet** : `<ghost_node>:<blake3(commande)[..16]>` — lie la preuve au producteur et à la
  tâche exacte. Deux commandes différentes ne se comparent pas (`SubjectDiffers`).
- **Provenance** : `method = local_recompute`, `producer` = node_id du **demandeur** (celui qui
  mesure, jamais le fantôme), `host` = hostname lu dans `/proc/sys/kernel/hostname` si disponible.
- **Écriture** : atomique (tmp + rename + sync), fichier **0600**, répertoire privé créé par le
  helper existant `identity::write_private_atomic` — aucun nouveau mécanisme d'E/S.
- **Refus** (`ok: false`) : aucune preuve — un refus n'a rien à recalculer.
- **Rotation** : aucune en v1. Un fichier par vérification ; la croissance est bornée par
  l'usage réel de `--verifier-local` (opt-in). Toute rotation future exigera un ADR.
- **Transport** : aucun champ de ce format n'entre dans une enveloppe réseau ; le grant affiché
  n'est pas modifié (le chemin est imprimé sur stdout, pas ajouté au JSON signé).
  `docs/architecture/API-CONTRACTS.md` n'est pas impacté.

## Conséquences

- Une divergence L1 devient un artefact durable : `compare()` entre deux fichiers dit
  `Match` / `Divergent` / `Inconclusive` avec une raison typée, sans chaîne libre.
- Le niveau de preuve reste L1 : deux exécutions identiques ne protègent pas d'un bug partagé.
  L'attribution de faute (producteur / demandeur / partagée) reste une décision humaine, hors
  format.
- Un échec d'écriture n'invalide pas le résultat vérifié : il est signalé (`⚠ preuve L1 non
  persistée`) et la commande continue — le disque ne devient pas un point de panne du compute.
- La primitive `core::evidence` reste pure (aucune E/S) ; toute l'E/S vit dans le client.

## Alternatives écartées

- **Base SQLite** : un schéma de plus à migrer pour un artefact append-only ; le fichier canonique
  se compare avec `diff`/`blake3` sans outil.
- **Preuve dans le grant JSON** : le grant est signé par le producteur ; y ajouter la mesure du
  demandeur mélangerait deux auteurs dans un même objet signé.
- **Ne persister que les divergences** : un accord non persisté ne peut pas servir de référence
  à une comparaison L2 ultérieure.
