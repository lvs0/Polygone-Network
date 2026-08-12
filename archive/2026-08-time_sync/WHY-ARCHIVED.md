# time_sync — ARCHIVÉ 2026-08-12

> **Décision** : D9 (DECISIONS.md) — tranchée par Lévy.

## Pourquoi archiver

- **1 019 LOC** dans `crates/core/src/time_sync/` (engine, filter, protocol, types)
- **0 consommateur** : aucun import dans `daemon` ni `client`
- Code mort = surface d'attaque + coût maintenance
- La feature « sync inter-nœuds » n'est pas dans le périmètre avant la sortie

## Contenu archivé

```
archive/2026-08-time_sync/
├── engine.rs          — NTP-like clock sync engine (CBOR)
├── filter.rs          — filtre de jitter
├── protocol.rs        — handshake time_sync
└── types.rs           — TimeSyncMessage, ClockOffset
```

## Ré-introduction

Ré-introduire avec la feature « synchronisation d'horloge entre nœuds »
(Phase 8+). À ce moment-là :
1. Récupérer `archive/2026-08-time_sync/`
2. Créer `crates/time_sync/` (sibling, pas dans core)
3. Intégration réseau + tests
4. Décision de protocole explicite (CBOR vs autre)

## Référence

- ARCHITECTURE.md §11 (avant archivage) : « time_sync engine with no consumers — Decision: wire or archive »
- DECISIONS.md D9 : « archiver, ré-introduire avec le feature »
