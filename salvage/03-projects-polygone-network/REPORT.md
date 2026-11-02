# Polygone — Rapport fiabilité / rapidité / statistiques

- Crate `polygone-core` v2.0.0
- Profil : complet (release)
- Horodatage : 1790604502 (unix)
- ⬡ Polygone · ML-KEM-1024 + ML-DSA-65 + AES-256-GCM · Shamir 4-of-7

## 1. Invariants des primitives

- ✅ ML-KEM-1024 : clé publique et chiffré = 1568 octets
- ✅ ML-KEM-1024 : clé secrète = 3168 octets
- ✅ ML-DSA-65 : clé publique = 1952 octets
- ✅ ML-DSA-65 : signature = 3309 octets
- ✅ encapsulation et signature font l'aller-retour

## 2. Rapidité


### Latence par opération

| Opération | n | moyenne | écart-type | p50 | p95 | p99 |
|---|---|---|---|---|---|---|
| Génération d'identité (ML-KEM + ML-DSA) | 20 | 145.9 µs | 12.1 µs | 140.8 µs | 174.5 µs | 181.5 µs |
| ML-KEM-1024 encapsulate | 200 | 47.4 µs | 4.4 µs | 47.2 µs | 51.4 µs | 67.3 µs |
| ML-KEM-1024 decapsulate | 200 | 49.8 µs | 10.0 µs | 53.1 µs | 58.2 µs | 72.1 µs |
| ML-DSA-65 sign | 200 | 259.9 µs | 169.9 µs | 208.9 µs | 596.6 µs | 812.8 µs |
| ML-DSA-65 verify | 200 | 73.9 µs | 13.7 µs | 68.2 µs | 108.8 µs | 113.9 µs |
| seal (message 256 o → 7 fragments) | 200 | 311.0 µs | 125.4 µs | 275.5 µs | 544.9 µs | 764.6 µs |
| open (4 fragments → clair) | 200 | 134.1 µs | 6.8 µs | 131.4 µs | 142.8 µs | 155.7 µs |

### Débit symétrique et partage de secret

| Opération | taille | moyenne | débit |
|---|---|---|---|
| AES-256-GCM chiffrement | 1.0 Mio | 1.919 ms | 521.1 Mio/s |
| AES-256-GCM déchiffrement | 1.0 Mio | 1.811 ms | 552.3 Mio/s |
| Shamir split 4-of-7 | 256 Kio | 20.243 ms | 12.4 Mio/s |
| Shamir reconstruct 4-of-7 | 256 Kio | 2.423 ms | 103.2 Mio/s |

### Latence de bout en bout (seal → open)

- moyenne 480.5 µs · p50 425.7 µs · p95 780.7 µs · p99 979.9 µs
- débit applicatif ~2081 messages/s (1 message = 256 o)

## 3. Fiabilité


### Aller-retour

- ✅ 500/500 messages reconstruits à l'identique (4 fragments sur 7)

### Tolérance à la perte de fragments

- ✅ 35/35 sous-ensembles de 4 fragments reconstruisent
- ✅ 35/35 sous-ensembles de 3 fragments sont refusés

### Détection d'altération

- ✅ 500/500 fragments corrompus détectés

### Usurpation et mauvais destinataire

- ✅ 200/200 signatures usurpées rejetées
- ✅ 200/200 déchiffrements par un tiers refusés

### Éphémérité (TTL)

- ✅ un ensemble expiré est refusé
- ✅ un ensemble frais est accepté

## 4. Statistiques


### Unicité

| Test | n | doublons | verdict |
|---|---|---|---|
| Clés KEM générées distinctes | 500 | 0 | ✅ |
| message_id, nonces et kem_ct distincts (même message) | 2000 | 0 | ✅ |

### Uniformité du partage de secret

- Octets observés : 28672 (7 parts × 4096 octets)
- Chi-deux (ddl 255) : 246.7
- p approximatif : 0.635
- Verdict : compatible avec une distribution uniforme (aucun biais détectable)

### Surcoût sur le fil

| Grandeur | Octets |
|---|---|
| Message en clair | 64 |
| Charge utile chiffrée (Σ fragments) | 560 |
| Sur le fil (Σ fragments sérialisés, métadonnées répliquées) | 65947 |
| Facteur d'expansion | 1030.4× |

Note : la clé KEM (1568 o), la clé publique de signature (1952 o) et la
signature (3309 o) sont répliquées sur chaque fragment. C'est le prix de
l'absence de tout état partagé côté relais.

## 5. Synthèse

- Tailles : KEM pk 1568 o · KEM ct 1568 o · sig pk 1952 o · signature 3309 o
- seal() ~311.0 µs
- open() ~134.1 µs
- bout en bout ~480.5 µs
- Invariants de fiabilité violés : 0

**Verdict global : ✅ tous les invariants tiennent.**
