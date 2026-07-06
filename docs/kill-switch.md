# kill-switch.md — Mode Duress

> *Document public. Pas de détail d'implémentation.*
> *Référence : `LEGAL.md` §5 — associé à `Conseil des Sages 2026-06-29` (Mitnick).*
> *Date : 2026-06-29.*

---

## 1. Qu'est-ce que c'est ?

Le **mode duress** est une fonctionnalité de Polygone qui déclenche la destruction
**irréversible** de toutes les clés locales et fragments stockés sur la machine
lorsqu'un **signal matériel spécifique** est reçu.

Il ne s'agit pas d'un mode de chiffrement renforcé. Il s'agit d'une autodestruction.

## 2. Pourquoi ?

> **« Le risque n'est pas la cryptographie. C'est la machine. »**
> — Kevin Mitnick, Comité 2 Conseil des Sages 2026-06-29

Quand un opérateur Polygone est contraint physiquement
(perquisition, interrogatoire, douane, key cop à l'aéroport),
la cryptographie ne sert à rien si la machine est saisie en marche, ouverte.
Le mode duress est la réponse : détruire avant la saisie.

## 3. Configuration

Polygone peut être configuré pour reconnaître un déclencheur parmi :

| Type | Description | Indication |
|------|-------------|-----------|
| **Capteur matériel USB** | Watchdog spécifique branché en permanence | Détection à chaud, sans contact visuel |
| **Séquence d'interaction clavier** | Combinaison secrète reconnue par le module TUI | Action humaine volontaire, réversible avant déclenchement |
| **Bouton panique physique** | GPIO configuré sur le hardware | Action explicite, intentionnelle |

Aucun de ces modes n'est activé par défaut. L'activation est explicite
dans `~/.config/polygone/state.json` (ou autre fichier de config à venir).

## 4. Implémentation — volontairement non détaillée ici

**Ce document ne contient pas les détails d'implémentation.**

L'implémentation vit dans `src/crypto/kill_switch.rs` et `src/tui/app.rs`.
Elle n'est volontairement pas détaillée ici pour empêcher la rétro-ingénierie
par un adversaire qui lirait la doc publique.

Un audit par tierce partie est requis pour ce module avant v1.
Aucun audit externe n'a encore été réalisé à ce jour.
(Posture « Anti-Bill-Gates » — `Conseil des Sages` S9)

## 5. Audit

Le code du mode duress est revu en cours de développement par le mainteneur principal.
**Aucun audit externe indépendant n'a été réalisé.**

Candidats d'audit (à activer Phase 8+) :

- Trail of Bits
- NCC Group
- Quarkslab (FR)
- ANSSI (FR, gratuit si le projet est jugé d'intérêt public)

## 6. Responsabilité

L'activation du mode duress est **irréversible**.

L'opérateur assume :

- La perte complète des clés locales (impossibilité de récupération après-geste).
- La perte des fragments stockés localement (Drive).
- L'impossibilité de prouver l'usage passé aux autorités *a posteriori*.

Le mode duress est documenté dans `LEGAL.md` §5 et accepté par l'opérateur
lors de l'activation explicite.

## 7. Hors-scope

Ce document ne couvre **pas** :

- Les modes d'effacement sécurisé du disque dur (effacement ATA, NVMe sanitize).
- Le chiffrement intégral du disque (LUKS, FileVault, BitLocker) — supposé actif par l'opérateur.
- La génération aléatoire forte (BLAKE3 + DRBG) — déjà traitée dans `ECOSYSTEM.md` §6.

---

*Fin de `kill-switch.md`.*
*MIT License, voir `LICENSE`.*
