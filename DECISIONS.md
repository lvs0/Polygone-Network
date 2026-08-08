# DECISIONS.md — Les 3 blocages en attente de Lévy

> *Le Conseil des Sages produit des recommandations. Lévy décide.*
> *Ce document liste explicitement les 3 décisions qui bloquent le calendrier.*

---

## D1 — Refonte TUI 2 onglets (P-V1)

### Question
Refonte complète du TUI de `Menu + Dashboard + Favoris + Settings` vers **2 onglets** : `Envoyer` et `Quitter` (réglages derrière `:`).

### Options
- **GO** : S5–S7 alloués. Version 3 menu archivé (tag `phase3-archive`).
- **NO** : Phase 3 menu conservé. Pas de refonte.

### Effet
- **GO** — alignement Council (Jobs + Musk). Risque : sur-simplification tue l'utilité (Mitnick peut quand même pivoter l'attaque).
- **NO** — calendrier conservateur. Le Menu reste un produit acceptable.

### Quand trancher
**Avant fin S1** (29 juin → 6 juillet 2026).

### Statut
🟡 **PENDING**.

---

## D2 — Bench perf handshake Sybil-resistance (P-A2 / P-S2)

### Question
`proof_of_key` ML-DSA-65 sur `(PeerID || nonce)`. Bench doit être **≤ 200 µs**.

### Options
- **OK** — phase 4.3 install 1-clic publique procède.
- **KO** — ré-investigation ; possible retour à ML-DSA-87 sur handshake local-LAN.

### Effet
- **OK** — produit shippé, P2 Sybil-resistance active sur réseau public.
- **KO** — 1 sem. retard + ré-architecture keypath.

### Quand trancher
**Fin S2** (≈ 13 juillet 2026).

### Statut
🟡 **PENDING — bench data recorded (2026-08-06)**.

`cargo bench -p polygone-core` (release, AVX2 actif) sur la machine de dev :

```
keygen  : ~92 µs
sign    : ~265 µs   ← goulot
verify  : ~79 µs
total   : ~270–344 µs (selon charge) — gate 200 µs NON atteint
capacity: ~2900 handshakes/sec/cœur
```

**Lecture honnête** : le gate ≤ 200 µs est dépassé d'environ 1,4–1,7× sur ce
matériel. La capacité résultante (~2900 auths/sec/cœur) reste largement
suffisante pour des sessions éphémères — ce n'est pas un bloqueur produit.
L'option « retour à ML-DSA-87 » du D2 n'améliore PAS la perf (clé 3840 B,
signature 5667 B, plus lent) ; elle aggraverait le gate. Recommandation :
garder ML-DSA-65 et réviser la cible à ≤ 400 µs, ou valider empiriquement
~2900/sec comme suffisant. Décision finale : à Lévy.

---

## D3 — Lettre État (CNIL / ANSSI / EFF)

### Question
Envoyer 3 lettres ouvertes aux régulateurs sur la posture privacy de Polygone.

### Options
- **OK** — 3 accusés reçus avant S8 fin (≈ 24 août 2026).
- **KO** — ré-écriture + 2ᵉ tentative.

### Effet
- **OK** — posture « Anticipation État » assumée. Pas attaquable par *silence = suspicion*.
- **KO** — escalade Nuit de l'État. Rédaction plus politique.

### Quand trancher
**Pendant S2** (rédaction), **accusé attendu S4-S5**.

### Statut
🟡 **PENDING**.

---

## Convention

- Chaque décision est **binaire** GO/NO-GO.
- Lévy tranche explicitement. Pas d'interprétation flottante.
- Une fois tranchée pour ce cycle de release, irréversible jusqu'au cycle suivant.

---

*Hérite de : `POLYGONE_ROADMAP_v2.md` S1-S8, Conseil des Sages 2026-06-29.*

---

## D4 — Créer Polygone-Protocols sibling (Lévy-blocking)

**Déclencheur** : `~/Par contre tu dois faire.md` (29/06/2026) — la coupe 8→2 du Conseil v1 est infirmée. Lévy défend un **écosystème de petits protocoles** (Petals, Daemon, Browser, RES, Tor+++) bâti sur un Core stable.

**Décision proposée** :
| Champ | Valeur |
|-------|--------|
| **Action** | Créer `/home/l-vs/Projets/Polygone-Protocols/` à côté de `Polygone-Final/`. Chaque sous-dossier = 1 protocole avec son propre README, THREAT_MODEL, LEGAL-check, mais SANS dépendance monolithique sur Core. |
| **Effort** | 1 personne × 1 sem. (squelette + manifesto + 1 protocole-pilote = Petals) |
| **Fichiers à toucher** | `/Projets/Polygone-Protocols/{README.md, AXIOMS.md, petas/SPEC.md, petals/LEGAL.md-check, etc.}` |
| **Owner** | Lévy |
| **Acceptance** | (1) `cargo check` n'est PAS exécuté sur le sibling (pure docs + spec au MVP) ; (2) `pet als/SPEC.md` passe le test Wozniak (lisible par ingénieur extérieur en <30 min) ; (3) Le manifeste des axiomes du sibling est cohérent avec `PHILOSOPHY.md` du Core (Axiome 1 conservé, Axiome 4 inversé). |
| **Dépendance** | D1/D2/D3 levées ou skewées ; le sibling est indépendant du Core sur le plan compilation. |
| **Risque** | Dispersion cognitive — Lévy est seul. Mitigation : limiter le sibling à **1 seul** protocole-pilote (Petals) au MVP. |
| **Hard block** | **D4 — attendant GO de Lévy.** |

**Justification** :
- Conseil v1 (Axiome 4 « coupe 90% ») → erreur (bâti sur Jobs qui aurait été Hibernate par Bach, Orwell et Gödel — voir COUNCIL_V2_RECONSIDERED.md §3.3).
- Conseil v2 (Bach T2) → « ne pas couper, transposer dans une autre octave ».
- Socrate T2 → « la transparence structurelle exige que les pouvoirs soient distribués ».
- Hitchock T2 → « le suspense est meilleur que la promesse ; un sibling sans suspense = app à pre-ship, sans attente ». OK en pratique un sibling *peut* avoir du suspense si on documente clairement ce qui arrive après.

**Statut** : ⏳ PENDING. Lévy à trancher avant de lancer Polygone-Protocols.

---

## D5 — Topologie v2.0.0-final : relay public assumé (tranchée 2026-08-07)

**Déclencheur** : le produit++ a vérifié dans le code que la promesse
« le relay ne voit rien » était fausse au niveau métadonnées (il route
sur `to`, voit from/to/session/tailles, et recevait même les noms de
fichiers). Le slogan ne pouvait pas rester l'étendard d'une promesse
que le code ne tient pas.

**Décision proposée** :
| Champ | Valeur |
|-------|--------|
| **Action** | Garder le relay public, documenter noir sur blanc ce qu'il voit (`docs/threat-*.md`), nom de fichier hors-bande (`name_ct` chiffré — fait), HELLO + limites + from==hello (fait), ML-DSA branché pour prouver l'expéditeur (fait). |
| **Non-choix** | Mesh LAN-only pour la finale (refusé : casse la messagerie longue distance) ; relay auto-hébergé seulement (refusé : barrière d'adoption). |
| **Promesse réelle** | « Le relay ne lit jamais le contenu. Il voit les métadonnées de routage, réduites et documentées. » |
| **Owner** | Lévy |
| **Acceptance** | (1) `name_ct` chiffré testé (le relay ne voit pas le nom) ; (2) `from` != HELLO → drop testé ; (3) le README et ECOSYSTEM ne mentent plus sur le relay. |
| **Risque** | Un opérateur de relay corrèle qui parle à qui. Mitigation : pseudonymes de session, hors-bande, et `known_peers` (Phase 4 contacts) pour l'authenticité. |

**Justification** :
- Le contenu est illisible sans la clé ML-KEM (IND-CCA2) — la promesse
  centrale tient.
- Les métadonnées sont le prix du routage ; les réduire (noms hors-bande)
  et les documenter est plus honnête que de les nier.
- C'est la première décision binaire du document qui est **réellement
  exécutée dans le code** le jour même.

**Statut** : ✅ **TRANCHÉE le 2026-08-07** — exécutée en Phase 1 du plan produit++.

---

## D6 — Garde Axiome 4 : le produit, pas la rigueur (tranchée 2026-08-08)

**Déclencheur** : en rendant la CI honnête (cran Phase 3.1, commit
`a6ea4e4`), le gate « `wc -l crates/client/src/*.rs` ≤ 5000 » était
**ROUGE** (5 560 lignes) — jamais exécuté depuis sa création (« les 51
commits post-rc2 n'ont jamais vu la CI »). Dont 1 012 lignes de tests
réseau dans `net.rs`.

**Décision proposée** :
| Champ | Valeur |
|-------|--------|
| **Action** | `net.rs` (2 041 l, dont 1 012 de tests) → `net/mod.rs` + `net/tests.rs` (pattern standard Rust pour les gros modules de tests). L'invariant documenté dans PHILOSOPHY.md Axiome 4 reste **littéralement inchangé** et passe : 3 519. |
| **Lecture** | La garde mesure le PRODUIT (fichiers top-level `src/`), pas la rigueur (modules de tests en sous-répertoire). C'est l'intention de l'axiome : « le produit reste petit. Pas 11 000 » — en contraste avec le monolithe 11 356 LOC archivé. |
| **Non-choix** | Couper 560 lignes de produit (aucune graisse : mesh/petals/reputation/product sont des fonctionnalités livrées) ; éditer la commande documentée de l'axiome (fragile, et réécrirait la philosophie). |
| **Risque** | Le glob ne mesure plus les sous-modules (`net/mod.rs` invisible). Accepté : 11 fichiers top-level restent mesurés et le contraste « pas 11 000 » tient. |
| **Acceptance** | (1) `wc -l crates/client/src/*.rs \| tail -1` → 3 519 ≤ 5000 ; (2) `cargo test --workspace` 108/108 en parallèle ; (3) `clippy --all --all-targets -- -D warnings` → exit 0. |

**Statut** : ✅ **TRANCHÉE le 2026-08-08** — exécutée dans le commit
`a6ea4e4` (Phase 3.1).

---

## D7 — Pousser main sur GitHub pour la première CI réelle (Lévy-blocking)

**Déclencheur** : la Phase 3.1 est exécutée — tous les gates de la CI
sont prouvés verts localement (clippy `-D warnings` exit 0, tests
parallèles 108/108, self-test crypto 7/7, axiomes, forensic zero-log
exit 0). Le repo a `origin = https://github.com/lvs0/Polygone-Network.git`
mais **aucun upstream** : `main` n'a jamais été poussé ; la CI n'a jamais
tourné sur les 53 commits post-rc2.

**MISE À JOUR (2026-08-08, NO-PROGRESS CHECK — la réalité du remote)** :
le remote N'EST PAS vide. `git ls-remote` public + `git cat-file`
révèlent : `main` distant = `09be6e05` (« chore: remove unused wasmi
imports in exec.rs ») — un commit **v1 qui est un ANCÊTRE du HEAD
local**. Le local est **38 commits en avance** ; un `git push origin
main` normal ferait un **FAST-FORWARD propre** : aucune perte, aucun
force-push, l'histoire v1 reste dans le graphe (le local la contient
déjà : v1.0.0 → legacy archivé → rewrite v2). Les tags distants
v0.1.0/v0.2.0/v1.0.0 restent ; les 2 PR fermées d'avril 2026
(« analyser-tous-les-fichiers-repo », « produit-grand-public-final »)
sont de l'historique v1. **La formulation « jamais poussé / aucun
upstream » était inexacte** : le v2 n'a jamais été poussé, mais le
remote porte le commit v1 partagé. D7 se réduit donc à une action
simple + une décision d'exposition, pas une stratégie de convergence.

**Question** : Pousser `main` (fast-forward, 38 commits) maintenant —
oui, quand, avec quels credentials (HTTPS : un token est-il configuré) ?
Ou attendre un tag `v*` ?

**Options** :
- **PUSH maintenant** — la première CI réelle prouve le vert en public.
  Risque : un échec CI runner malgré les vérifications locales (environnement
  différent), exposition publique du code (AGPL, assumé).
- **PUSH au tag** — la première CI est déclenchée par un tag `v*`
  (release.yml inclus), plus solennel ; l'attente retarde la première preuve.
- **Ne pas pousser** — le dépôt reste local ; aucune CI, aucune
  visibilité ; contredit la promesse « open source ».

**Effet** : PUSH = la Phase 3.1 se termine vraiment (« CI jamais passée »
→ « CI verte ») ; le Premier Soir peut distribuer via GitHub Releases.

**Statut** : 🟡 **PENDING — décision de Lévy** (action publique +
credentials). En attendant, le cran suivant qui n'en dépend pas :
audit des promesses README (règle produit++ — chaque promesse = test CI
ou commande `polygone *`).

---

## D8 — polygoned doit lire l'ancien format de config (rétro-compat) (tranchée 2026-08-08)

**Déclencheur** : l'itération 16 a corrigé `--gen-config` (format écrit ==
format lu), mais la config réelle de la machine
(`~/.config/polygone/daemon.toml`, écrite le 2026-07-20) est dans le
format **legacy** — `[tier] tier = "Balanced"` encapsulé en table — que
le parseur actuel rejette. Vérifié 2026-08-08 : `polygoned status` et
`polygoned --gen-config` échouent tous deux sur cette config
(`unknown variant 'tier', expected one of 'Eco', 'Balanced', …`).
L'utilisateur réel ne peut donc pas lancer son daemon sans action manuelle.

**Question** : le daemon doit-il accepter les deux formes de config
(legacy `[tier] tier = "X"` et actuelle `tier = "X"`) pour ne jamais
casser un utilisateur existant — ou exiger la régénération ?

**Options** :
- **(a) Désérialisation rétro-compatible** — accepter `[tier] tier = "X"`
  ET `tier = "X"` ; la valeur est identique (« Balanced » est un tier
  valide). Zéro friction utilisateur, robustesse à la frontière (règle
  CLAUDE.md). Lévy n'a rien à faire.
- **(b) Migration one-shot** — `--gen-config` détecte l'ancien format et
  le réécrit ; le parseur reste strict. Plus fragile (état modifié sans
  demande), erreur si le daemon n'est pas lancé avec la migration.
- **(c) Régénération exigée** — l'utilisateur perd ses réglages et subit
  la friction ; c'était l'état « à faire par Lévy » du plan.

**Effet** : (a) = `polygoned status` fonctionne immédiatement sur la
machine réelle, sans action ni perte ; l'ancien format ne casse plus
jamais un utilisateur.

**Statut** : ✅ **TRANCHÉE par l'architecte (2026-08-08)** — option (a),
exécution au cran suivant avec test de round-trip (legacy + actuel).
Fallback conservé : `polygoned --gen-config` régénère quand même.
**Exécutée** : commit `e10c22e` — `polygoned status` lit la vraie config
legacy (exit 0, avant : TOML parse error) ; 4 tests de forme ajoutés ;
113 tests workspace verts. Gate CI durable (smoke) au cran suivant.

---

## D9 — time_sync : câbler ou archiver (Lévy-blocking)

**Déclencheur** : ARCHITECTURE.md (§11) suit « `time_sync` engine with no
consumers — Decision: wire or archive ». Vérifié 2026-08-08 : `crates/
core/src/time_sync/` = 1 019 LOC (engine, filter, protocol, types),
re-exporté par lib.rs, **zéro consommateur** (aucun import dans daemon
ni client) — du code vivant mais mort dans le crate crypto.

**Question** : câbler `time_sync` (feature « synchronisation d'horloge
entre nœuds » — en lien avec la demande récurrente de Lévy sur la
synchronisation inter-nœuds) ou l'archiver ?

**Options** :
- **Câbler maintenant** — feature complète (protocole, intégration
  réseau, tests) ; aucun consommateur ne la demande encore ; le réseau
  n'en a pas besoin pour la promesse centrale (la mort du message).
- **Archiver** (recommandé) — le code reste dans git (rien n'est
  perdu) ; du code mort dans le crate crypto = surface d'attaque + coût
  de maintenance pour zéro bénéfice. Ré-introduire au moment du feature
  « sync inter-nœuds », avec une vraie décision de protocole.
- **Laisser tel quel** — documenté dans ARCHITECTURE.md, mais laisse
  une ambiguïté permanente.

**Effet** : archiver = core allégé (~1 kLOC de moins), une décision
claire ; câbler = un pas vers le rêve « organisme » de Lévy.

**Statut** : 🟡 **PENDING — décision de Lévy** (direction produit :
le sync inter-nœuds est-il dans le périmètre avant la sortie ?).
Recommandation de l'architecte : **archiver**, ré-introduire avec le
feature. En attendant : aucun cran ne dépend de cette décision.
