# kill-switch.md — Mode Duress

> *Document public. Pas de détail d'implémentation.*
> *Référence : `LEGAL.md` §5 — associé à `Conseil des Sages 2026-06-29` (Mitnick).*
> *Version : 1.0 (2026-08-06) — ajout du runbook opérateur.*

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

## 4. Implémentation — le code réel

Le kill-switch **existe dans le produit** depuis v2.0.0-rc2 :

```bash
polygone duress                # affiche le plan + refuse (confirmation requise)
polygone duress --confirmer    # détruit l'identité + les fichiers reçus
```

Ce que la commande détruit, réellement :

- `~/.polygone/identity.json` — clés ML-KEM-1024 + ML-DSA-65 (chmod 600)
- `~/.polygone/received/` — fichiers reçus via le relay
- `~/.polygone/reputation.json` — état RES local (trace des sessions)
- `~/.polygone/peers.json` — ancres de confiance TOFU (la trace de *qui*
  on a contacté : node_id → empreinte ML-DSA)

Ce qu'elle ne détruit **pas** (et pourquoi) :

- Les fragments chez les destinataires et les backups hors-ligne —
  mais sans vos clés, ils deviennent **définitivement illisibles**. C'est
  le point du mode duress.
- Les journaux shell (ex. `~/.bash_history`) — c'est l'état de
  l'utilisateur, pas de l'application. Pour les messages sensibles,
  préférez la TUI (`polygone`) ou le pipe stdin au passage d'arguments
  en clair.
- L'implémentation reste volontairement simple et lisible : le signal
  explicite `--confirmer` évite le déclenchement accidentel.

Les détails d'implémentation du capteur matériel / bouton panique
(`src/crypto/kill_switch.rs` historiquement) restent non détaillés ici
pour empêcher la rétro-ingénierie par un adversaire qui lirait la doc
publique.

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

## 8. Runbook opérateur (v1.0)

> Procédure à lire AVANT d'en avoir besoin. La lire pendant la panique ne sert à rien.

### Avant (préparation — 10 minutes, une seule fois)

1. **Activez le chiffrement intégral du disque** (LUKS/FileVault/BitLocker).
   Le mode duress détruit les clés, il n'efface pas le disque.
2. **Choisissez un déclencheur** (§3) et activez-le explicitement dans
   `~/.config/polygone/state.json`.
3. **Entraînez-vous** : déclenchez le mode duress sur une machine de test,
   vérifiez que la destruction est réelle, reconstruisez l'identité.
4. **Préparez la régénération** : vos contacts ont vos clés publiques.
   Une identité détruite = une nouvelle identité à redistribuer. Décidez
   du canal de redistribution (hors-ligne de préférence).
5. **Testez le runbook complet** une fois par trimestre.

### Pendant (le déclencheur a sonné)

1. **Ne négociez pas avec la machine** : le signal est parti, la destruction
   est en cours. Ne tentez pas de « sauver » une clé — c'est le but.
2. **Ne mentez pas sur ce qui vient d'arriver** : « le système s'est
   autodétruit » est vrai et vérifiable. C'est plus solide qu'un mensonge.
3. **Gardez votre récit simple et répétable** — identique à chaque question.
4. Si la destruction a échoué (machine déjà verrouillée, pas de signal) :
   considérez la machine comme **compromise**, pas comme protégée.

### Après (récupération)

1. Vérifiez que la destruction a bien eu lieu (fichiers de clés absents).
2. Régénérez une identité : `polygone` au premier lancement.
3. Redistribuez votre nouvelle clé publique via le canal prévu.
4. Faites le bilan : qu'est-ce qui a été perdu, qu'est-ce qui doit changer
   (déclencheur, canal, procédure) ?
5. Documentez l'incident dans votre journal — sans capturer ce qui ne doit
   pas l'être.

### Limites (à relire après l'incident)

- Le mode duress n'efface **pas** les backups hors-ligne, les copies cloud,
  les fragments chez les destinataires. Détruire les clés locales rend ces
  copies **définitivement illisibles** — c'est le point.
- Un adversaire qui a déjà copié les clés (malware persistant) contourne
  la destruction. Cf. `docs/threat-high-value.md` §4.

---

*Fin de `kill-switch.md`.*
*AGPL-3.0, voir `LICENSE`.*
