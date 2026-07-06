# PHILOSOPHY.md — Les 5 axiomes de Polygone

> *Ce document n'est pas du marketing. Ce sont les principes structurels du produit.*
> *Chaque axiome a deux voix : une poétique (Bach, Orwell, Jobs) et une technique (Galois, Wozniak, Gödel). Les deux doivent être vraies simultanément — sinon l'axiome est brisé.*
> *Source : Conseil des Sages 2026-06-29 (Comités 1, 2, 3).*

---

## Axiome 1 — **L'information n'existe nulle part fixe**

### Voix poétique (Orwell ✕ Philip K. Dick)
> « L'information n'existe pas. Elle traverse. »

Ce que la phrase *signifie* : aucun message ne réside en aucun nœud après son TTL. Aucune base, aucune trace, aucun log. La métaphore est vraie dans le sens où la chose est **indifférenciée du battement d'aile**.

### Voix technique (Galois ✕ Turing)
Aucun fragment reconstructible sans réunion d'au moins **4 fragments Shamir sur 7**. La clé partagée vit sur les deux endpoints le temps de la session, puis est dérivée puis oubliée (`zeroize`). Les fragments chiffrés en AES-256-GCM sont intransmissibles hors contexte de session.

**Invariant vérifiable :** un observateur extérieur *ne peut pas prouver* qu'un message donné a existé entre Alice et Bob. C'est une promesse **de design**, pas une déclaration métaphysique.

---

## Axiome 2 — **Le produit a deux tons, pas trois**

### Voix poétique (Ive ✕ Hitchcock)
Le matériel est `cyber-amber` (#f59e0b) sur `cyber-slate` (#0f172a). Le silence est ambre, pas gris. Le mouvement est une attente visible — typographique, animée, rythmée — pas un spinner ni une barre de chargement.

### Voix technique (Wozniak ✕ Gödel)
La TUI a **deux onglets** : `Envoyer` et `Quitter`. Tout le reste est paramètre caché derrière `:` (vim-style). L'utilisateur ne voit pas les 6 services « en construction » — il voit ce qui marche.

**Invariant vérifiable :** `cargo test` sur `views::two_tab_layout` passe. Au démarrage, l'utilisateur ne peut pas confondre ce qui marche avec ce qui ne marche pas.

---

## Axiome 3 — **La menace est honnêtement divisée en deux**

### Voix poétique (Assange)
Le projet ne prétend pas protéger le manifestant turc ET le banlieusard parisien. Deux audiences = deux SLA = deux documents.

### Voix technique (Coplan ✕ Archiviste)
`docs/threat-commodity.md` (monsieur-tout-le-monde) et `docs/threat-high-value.md` (dissident) sont des fichiers **séparés**. Pas un seul document qui couvre les deux et ment par omission.

**Invariant vérifiable :** `grep "tracking\|keylogger\|rubber-hose" docs/threat-*.md` doit retourner des non-dits explicites, pas du silence.

---

## Axiome 4 — **Le silence est le produit, pas le silence marketing**

### Voix poétique (Bach)
Chaque voix principale a un contre-voix qui la complète. Encryption (silence contenu) ↔ métadonnées warning (silence observable). Hardware sensor (silence physique) ↔ kill-switch (silence actif).

### Voix technique (Musk ✕ Wozniak)
**Couper 90% des fonctionnalités**. Deux services ship : `msg` (éphémère) et `drive` (fichiers chiffrés 4-of-7). Les 6 autres (`compute`, `hide`, `mesh`, `brain`, `petals`, `shell`) sont archivés dans `STAGING.md` avec conditions explicites de ré-introduction.

**Invariant vérifiable :** `wc -l src/` après cette coupe ≤ 3500 lignes utiles. Pas 5500.

---

## Axiome 5 — **La machine est la menace, pas la cryptographie**

### Voix poétique (Mitnick)
« Vous chiffrez les messages. Bravo. Mais les clés sont sur un laptop. Le laptop est dans un bureau. Le bureau est dans une entreprise. La patronne a un badge physique. *Elle possède déjà la machine*. »

### Voix technique (Expert privacy ✕ Coplan)
Polygone expose un **mode duress** documenté sans détail d'implémentation, et un kill-switch runbook pour subpoena (`LEGAL.md` §4). Pas de prétention que la cryptographie protège contre `rubber-hose attack`. C'est l'utilisateur humain qui protège l'utilisateur humain.

**Invariant vérifiable :** un état de la machine peut déclencher une autodestruction des clés locales et fragments. Aucun service cloud de Polygone n'existe pour empêcher ce geste.

---

## Anti-axiomes — ce que Polygone n'est PAS

| Anti-axiome | Pourquoi c'est rejeté |
|-------------|----------------------|
| Persistance utilisateur (compte, profil, social-graph) | Privacy-by-default (Comité 3) |
| Cloud-only inference (toute IA dans le cloud) | Offline-first (Comité 3) |
| Telemetry / analytics | Trust via verifiability (S5) |
| Auto-exécution sur onboarding | Plan approval gate (POLYGONE-PATTERNS §6) |
| Subscription paywall | Free forever, no token (Comité 3) |
| Vendor lock-in | Open formats (Comité 3) |
| Inner-monologue streaming | Events stream, never thoughts (POLYGONE-PATTERNS §2) |

---

*L'axiome 6 sera ajouté quand le premier audit externe aura été réalisé. Pour l'instant, il est honnête de dire : aucun audit n'existe (cf. `LEGAL.md` §5 + Comité 2 S6).*

*MIT License · Posture « honest-first » · Conseil des Sages 2026-06-29.*
