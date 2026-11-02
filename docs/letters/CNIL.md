# Lettre à la CNIL — Projet Polygone

**Date** : 2026-09-02
**Objet** : Notification d'un traitement de données personnelles minimaliste — Projet Polygone

---

Madame, Monsieur,

Le projet **Polygone** (https://github.com/lvs0/Polygone-Network) est un réseau pair-à-pair post-quantique conçu pour l'échange de messages éphémères et de fichiers sans persistance centrale.

**Nature du traitement** :
- Aucune collecte d'identifiants directs (nom, email, IP) n'est effectuée par le protocole lui-même.
- Les nœuds échangent des identités opaques (`NodeId` ML-DSA-65) et des clés éphémères ML-KEM-1024.
- Les messages sont fragmentés (Shamir 4-of-7), chiffrés (AES-256-GCM), et expirent (TTL).
- Le relay est *aveugle* : il ne voit que des enveloppes chiffrées sans pouvoir les déchiffrer.

**Base légale** : Intérêt légitime (Art. 6.1.f RGPD) — sécurité des communications, protection de la vie privée par conception.

**Durée de conservation** : Aucune. Les données transitent et expirent. Aucune base de données centrale.

**Droits des personnes** : Le protocole ne permet pas l'exercice des droits d'accès/rectification/effacement sur le réseau lui-même (données chiffrées, pas de contrôle central). L'utilisateur garde le contrôle local de ses clés.

**Mesures de sécurité** :
- Chiffrement post-quantique (ML-KEM-1024, ML-DSA-65, AES-256-GCM)
- Fragmentation Shamir 4-of-7
- Relay aveugle (zero-knowledge)
- Kill-switch documenté (`docs/kill-switch.md`)

**Sous-traitance** : Aucune. Le réseau est auto-hébergé par les pairs.

**Transferts hors UE** : Possibles (réseau P2P mondial), mais données chiffrées de bout en bout.

**Délégué à la protection des données** : Non désigné (pas de traitement centralisé).

Nous restons à votre disposition pour tout complément d'information.

Cordialement,

**Lévy Verpoort Scherpereel** — Auteur principal, Polygone
`polygone.network` | `lvs0@protonmail.com`

---

*Pièces jointes : `docs/kill-switch.md`, `docs/threat-commodity.md`, `LEGAL.md`*