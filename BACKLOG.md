# BACKLOG — travail restant, par priorité

> Source machine : `../04-AGENT/backlog.json` (lu par l'orchestrateur).
> Ce fichier est la vue humaine. Les deux doivent rester cohérents.
> Une tâche cochée ici doit avoir une preuve exécutée dans `STATE.md`.

## P0 — Ne rien perdre

- [ ] **P0.1** Archive restaurable du dépôt canonique
  - Preuve : `tar -tzf` OK + `cmp` sur 3 fichiers critiques = PASS
- [ ] **P0.2** Inventaire des 6 commits locaux et 17 distants
  - Preuve : `docs/RECONCILIATION.md` avec hash + raison de chaque décision
- [ ] **P0.3** Corriger la documentation factuellement fausse
  - Preuve : aucune affirmation contredite par `Cargo.toml` ou `ETAT-REEL.md`

## P1 — Ressources prouvées

- [ ] **P1.1** Séparer capacité mesurée / réservable / contrainte
  - Preuve : test de refus d'une demande supérieure au plafond
- [ ] **P1.2** `--isoler` par défaut sous Linux
  - Preuve : scope réel, `memory.max` et `cpu.max` vérifiés, scope déchargé
- [ ] **P1.3** Tests crash, veille, perte réseau, SIGKILL
  - Preuve : 4 tests exécutés, sortie citée

## P2 — Deux machines réelles

- [ ] **P2.1** Protocole de pair versionné et authentifié
- [ ] **P2.2** Échange réel LAN entre deux hôtes physiques

## P3 — Parcours utilisateur

- [ ] **P3.1** Installation et désinstallation propres (Linux)
- [ ] **P3.2** Parcours guidé : identité, appairage, message, pause

## P4 — Services (bloqué jusqu'à P1 + P2)

- [ ] Msg fiable avec TTL réel
- [ ] Drive : quotas, réplication, disponibilité mesurée
- [ ] Hide mono-saut durci
- [ ] Serverless : dispatch réel, reçus signés, quorum appelé
- [ ] Petals : pipeline 2 workers, inférence de référence
- [ ] SDK / bridges

## P5 — Durcissement

- [ ] Audit externe, fuzzing, CI multi-OS, SBOM, release signée

---

## Découvertes automatiques (TODO/FIXME réels)

Relevées par `orchestrateur.mjs --discover` le 2026-11-02 :

| Fichier | Ligne | Contenu |
|---|---:|---|
| `crates/petals/src/backends/llamacpp.rs` | 14 | Add candle model, tokenizer, etc. |
| `crates/petals/src/backends/llamacpp.rs` | 20 | Initialize candle model from GGUF |
| `crates/petals/src/benchmarks.rs` | 153 | implement memory measurement |
| `crates/petals/src/benchmarks.rs` | 183 | detect actual device |
| `crates/petals/src/daemon.rs` | 163 | (à préciser) |

Ces éléments sont dans Petals, donc **P4** : ils ne se traitent pas avant P1/P2.

## Tests marqués ignorés / conditionnels

À auditer, pas à supprimer : `crates/client/src/main.rs`,
`crates/gateway/src/main.rs`, `daemon/src/resources/linux.rs`,
`daemon/src/resources/macos.rs`. Vérifier s'il s'agit d'un `#[cfg]` légitime
(plateforme) ou d'un test désactivé pour faire passer la CI.
