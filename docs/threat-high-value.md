# THREAT MODEL — Utilisateur haute-valeur (« dissident »)

> *Scope 2 du modèle de menace Polygone. Livrable S2.*
> *Adversaire : étatique ou déterminé. SLA : 30 minutes d'adoption.*
> *Ce document ne promet pas l'impossible. Il dit précisément ce qui est
> protégé, et ce qui ne le sera jamais.*

---

## 1. Qui est l'utilisateur

Un journaliste, un militant, un lanceur d'alerte — quelqu'un dont **le fait même
de communiquer** est une information sensible, pas seulement le contenu.

Pour cet utilisateur, être vu *en train de contacter* une source peut être plus
grave que le contenu du message.

## 2. Adversaire type

| Adversaire | Capacité |
|---|---|
| État avec subpoena | Peut contraindre les opérateurs, saisir des machines |
| Accès physique prolongé | Imagerie forensique, clones de disque |
| Monitoring réseau national | Analyse de trafic à grande échelle |
| Adversaire actif | Sybil, MITM, manipulation de protocole, rubber-hose |

Cet adversaire a du temps, de l'argent et la loi de son côté. Il cible
**Polygone** spécifiquement.

## 3. Ce que Polygone protège

| Menace | Protection | Mécanisme |
|---|---|---|
| Prouver qu'un échange a existé | Non-prouvabilité par construction | Fragments éphémères (stateless, non rejouables), 4-of-7, aucun agrégat reconstituable après coup |
| Lien expéditeur ↔ destinataire | Relay aveugle, aucun routage par identité | Le relay ne voit que des fragments chiffrés, pas d'adresse résolue |
| Subpoena sur l'opérateur | Mécanisme documenté, 24 h de gel | `LEGAL.md` §4 — l'opérateur ne *peut* pas fournir plus que des fragments |
| Contenu en transit | Chiffrement bout-en-bout post-quantique | ML-KEM-1024 + AES-256-GCM + ML-DSA-65 |
| Machine saisie en état d'usage | Mode duress | `docs/kill-switch.md` — autodestruction locale : identité, fichiers reçus, scores, ancres `peers.json` (trace relationnelle) |
| Déchiffrement rétroactif | Post-quantique par défaut | ML-KEM-1024 / ML-DSA-65 (FIPS 203/204) |

## 4. Ce que Polygone ne protège PAS

> ⚠ Pour cet utilisateur, la liste des non-protections est un document de
> survie. À lire deux fois.

- ⚠ **Rubber-hose attack** — la torture, la menace directe sur vous ou vos
  proches. Aucun logiciel ne protège contre ça. Si on vous demande les clés,
  les clés existent.
- ⚠ **Reconnaissance physique** — caméra, mouchard, filature. Polygone
  n'efface pas votre présence physique.
- ⚠ **Ingénierie sociale** — quelqu'un qui vous fait *volontairement* révéler.
  L'opérateur humain est le maillon le plus faible, documenté comme tel.
- ⚠ **Disclosure humaine forcée** — *quelqu'un finit par parler* : vous, un
  contact, un proche. Polygone ne peut pas empêcher un humain de parler.
- ⚠ **Adversaire avec accès physique prolongé avant usage** — clés installées
  sur une machine compromise = clés compromises. Vérifiez la chaîne d'installation.
- ⚠ **Analyse de trafic statistique avancée** — les métadonnées (timing,
  tailles, topologie) sont réduites, pas annulées.

## 5. Coût d'adoption

- **30 minutes** : comprendre ce document + `LEGAL.md` + `docs/kill-switch.md`,
  générer l'identité, tester le mode duress.
- Aucune inscription. Aucune identité liée à un email. Aucune télémétrie.

## 6. Fabriquer la confiance (pour cet utilisateur)

- **1 audit externe indépendant** (Trail of Bits, NCC Group, Quarkslab, ANSSI)
  — **NON RÉALISÉ à ce jour**. C'est dit explicitement ici et dans `LEGAL.md` §5.
  Ne faites pas confiance à ce document : faites l'audit, ou attendez qu'il soit fait.
- Mode duress **testé physiquement** par l'opérateur avant usage réel.
- Mises à jour du binaire **signées PGP**, vérifiables.
- Le code est `#![forbid(unsafe_code)]` sur le core — aucun code non vérifié.

## 7. Vérification par vous-même (en 10 minutes)

```bash
git clone https://github.com/lvs0/Polygone-Network
cd Polygone-Network
cargo test --workspace          # 71 tests
cargo run -p polygone -- demo   # l'audit « on voit rien » est un test, pas une promesse
```

---

*Scope 2 · SLA 30 minutes · Livrable S2 (en retard assumé, livré 2026-08-06).*
*Ce document est honnête jusque dans ses limites — cf. §4.*
