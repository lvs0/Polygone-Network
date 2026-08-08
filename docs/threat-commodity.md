# THREAT MODEL — Utilisateur quotidien (« commodity »)

> *Scope 1 du modèle de menace Polygone. Livrable S2.*
> *Adversaire : opportuniste ou curieux. SLA : 5 minutes d'adoption.*
> *« On voit rien. Et c'est comme ça que ça devrait être. »*

---

## 1. Qui est l'utilisateur

Quelqu'un qui veut que **ses messages et ses fichiers ne soient pas lisibles
par les intermédiaires** — sans devenir un expert en opsec.

C'est l'utilisateur de Signal, mais qui n'a pas envie de faire confiance à un
serveur central, et qui veut une protection qui survivra aux ordinateurs
quantiques.

## 2. Adversaire type

| Adversaire | Capacité |
|---|---|
| FAI curieux | Voit le trafic, pas le contenu |
| GAFA (Google, Apple, Meta, Microsoft, Amazon) | Données des apps, métadonnées cloud |
| Colocataire / Wi-Fi partagé / VPN tiers | Peut observer le réseau local |
| Malware opportuniste | Vol de fichiers locaux, keylogging de masse |
| Collecteur « harvest now, decrypt later » | Stocke le trafic chiffré pour plus tard |

Aucun de ces adversaires ne cible **Polygone** spécifiquement. Ils exploitent
des faiblesses génériques à grande échelle.

## 3. Ce que Polygone protège

| Menace | Protection | Mécanisme |
|---|---|---|
| Lecture du message en transit | Chiffrement bout-en-bout | AES-256-GCM, clé dérivée d'un secret ML-KEM-1024 |
| Déchiffrement futur par ordinateur quantique | KEM post-quantique | ML-KEM-1024 (FIPS 203) |
| Reconstruction d'un message capturé | Fragmentation à seuil | Shamir 4-of-7 — 3 fragments = zéro information |
| Persistance du message sur les nœuds | Éphémérité | TTL ≤ 30 s, fragments non consolidés désintégrés |
| Usurpation de l'expéditeur | Signature | ML-DSA-65 (FIPS 204), détachée, vérifiable — ✅ **branché au chemin réseau** (Phase 1) : chaque message est signé, la signature est vérifiée avant déchiffrement, une clé connue = pas d'usurpation possible |
| Fuite du nom de fichier | Nom hors-bande | Le nom du fichier est chiffré par la clé de session (`name_ct`) — le relay ne voit que des octets opaques |

## 4. Ce que Polygone ne protège PAS

> ⚠ Écrit en toutes lettres. C'est la partie la plus importante de ce document.

- ⚠ **Keylogger sur votre machine** — si l'adversaire lit ce que vous tapez
  *avant* le chiffrement, rien ne peut l'en empêcher.
- ⚠ **Malware persistant déjà installé** — il voit les clés quand vous les
  utilisez.
- ⚠ **Coercition physique / vol de machine** — voir `docs/kill-switch.md`
  (mode duress) pour ce qu'on *peut* faire, qui est limité.
- ⚠ **Disclosure humaine forcée** — personne ne peut vous empêcher de parler.
- ⚠ **Métadonnées réseau** — le *timing*, les *tailles* et le *routage*
  restent observables au niveau réseau. Réduits, pas annulés.

## 5. Coût d'adoption

```
curl -fsSL polygone.network/install | bash
polygone
```

5 minutes. Une identité est générée au premier lancement. Aucune inscription,
aucune adresse email, aucune télémétrie.

## 6. Fabriquer la confiance (pour cet utilisateur)

- `cargo test --workspace` reproduit l'état en 2 minutes (109 tests).
- Le code source est conçu pour être lisible en un après-midi.
- `polygone demo` montre le pipeline complet — y compris l'audit du relay :
  « on voit rien » n'est pas une promesse, c'est un test.

---

*Scope 1 · SLA 5 minutes · Livrable S2 (en retard assumé, livré 2026-08-06).*
*Pas de surpromesse : ce que ce modèle ne couvre pas est listé au §4.*
