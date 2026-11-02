# Lettre à l'EFF — Projet Polygone

**Date** : 2026-09-02
**Objet** : Projet Polygone — Réseau P2P post-quantique pour la liberté d'expression et la vie privée

---

Chers collègues,

Le projet **Polygone** (https://github.com/lvs0/Polygone-Network) est un réseau pair-à-pair post-quantique conçu pour protéger la liberté d'expression et la vie privée numérique, sans dépendre d'une autorité centrale.

**Pourquoi Polygone intéresse l'EFF** :
- **Chiffrement fort par défaut** : Post-quantique (ML-KEM-1024, ML-DSA-65, AES-256-GCM) — résistant aux ordinateurs quantiques futurs.
- **Pas de métadonnées centrales** : Pas de serveur central, pas de journal de connexions, pas de graphe social.
- **Relay aveugle** : Le nœud de transit ne peut ni lire, ni modifier, ni tracer le contenu.
- **Code ouvert, licence AGPL-3.0** : Auditable, modifiable, forkable. Aucune porte dérobée.

**Menaces adressées** :
- Surveillance de masse (PRISM, Upstream, XKeyscore)
- Censure d'État (DPI, blocage IP/DNS)
- Répression des lanceurs d'alerte / journalistes
- Harvest now, decrypt later (quantum)

**Limites actuelles (transparence totale)** :
- **Single-hop Hide uniquement** : Pas encore de multi-hop (onion routing) — documenté dans `docs/HIDE-SPEC.md`.
- **Pas d'audit tiers** : Code ouvert mais non audité professionnellement.
- **Pas de financement EFF / ONG** : Développement bénévole.

**Ce que nous demandons à l'EFF** :
1. **Revue de posture** : Confirmation que l'architecture respecte les principes "Security by Design" de l'EFF.
2. **Visibilité** : Mention dans Surveillance Self-Defense ou liste d'outils recommandés (si pertinent).
3. **Soutien juridique** : Conseils sur la posture AGPL-3.0 face aux lois de déchiffrement obligatoire (ex: LOPSI, Cloud Act, Investigatory Powers Act).
4. **Bounty / Audit** : Aide à monter un programme de bounty ou audit financé.

**Ressources** :
- Code : https://github.com/lvs0/Polygone-Network
- Docs : `docs/threat-commodity.md`, `docs/threat-high-value.md`, `docs/kill-switch.md`
- Licence : AGPL-3.0 (`LEGAL.md`)
- Contact : `eff@polygone.network` (PGP disponible)

Nous partageons vos valeurs : *Privacy is a right, not a feature.* Polygone est notre contribution technique à cette bataille.

Avec respect,

**Lévy Verpoort Scherpereel** — Auteur principal, Polygone
`polygone.network` | `eff@polygone.network`

---

*Pièces jointes : `docs/threat-commodity.md`, `docs/threat-high-value.md`, `docs/kill-switch.md`, `LEGAL.md`*