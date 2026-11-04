# POLY — ce que les documents du projet en disent

> NOTE DE PORTAGE — 4 novembre 2026 : comparaison de documents historiques ; les chemins locaux ont été retirés.

Relevé en lecture seule, sans opinion ajoutée. Chaque affirmation porte son
fichier:ligne. Les documents viennent de `checkout historique Polygone-real/`
sauf mention contraire (`sources historiques locales non publiées/` = Atlas, extrait du corpus réel).

---

## 1. Chronologie des positions

| Date | Document | Position | Citation courte |
|---|---|---|---|
| 2026-06-05 | `POLYGONE-SPEC-AUDIT.md` §4 | **Pour, comme fonctionnalité** | « ❌ **Solde POLY** (économie locale, `~/.polygone/poly.toml`, 0.1 POLY/min) » — listé parmi les manques de la TUI à corriger |
| 2026-06-29 | `LEGAL.md` §2 | **Contre** | « Pas d'abonnement. Pas de token tradable. Pas de NFT. » |
| 2026-07-17 | `COUNCIL_V2_RECONSIDERED.md:32` | **Pour, au passé** | « la source du futur token POLY (que Lévy a retiré parce que trop marketing) » |
| 2026-07-17 | `COUNCIL_V2_RECONSIDERED.md:51` | **Pour le retrait** | « la petite monnaie POLY naissait d'une pulsion marchande, fut retirée — geste qu'il applaudirait » |
| 2026-07-17 | `COUNCIL_V2_RECONSIDERED.md:113` | **Débat ouvert** | « POLY retiré = bon » vs « POLY retiré = erreur » (Gödel aurait préféré le documenter) |
| 2026-07-17 | `COUNCIL_V2_RECONSIDERED.md:215` | **Nuance** | « Gödel aurait classé POLY comme "non-incomplert mais marchand" […] Le débat reste ouvert. » |
| 2026-08-06 | `docs/STRATEGIE.md:51-56` | **Contre, ferme** | « **Pas de token. Pas d'investisseurs.** (Distraction réglementaire MiCA/SEC, pas nécessaire pour le B2B, contraire aux axiomes.) » |
| 2026-08-06 | `docs/STRATEGIE.md` §7 (« Ce qu'on NE fait PAS ») | **Contre, ferme** | « Token / ICO / equity crowdfunding — jamais (LEGAL.md). » |
| 2026-08-12 | `ECOSYSTEM.md:241-242` | **Contre, avec porte entrouverte** | « be a token (le ledger POLY archivé attend une décision explicite — il n'est pas dans le produit) » |
| 2026-08-12 | `PHILOSOPHY.md:61-62` | **Contre (archivé)** | « le monolithe `src/` (11 356 LOC : libp2p, relay HTTP, **ledger POLY**, vieille TUI) est archivé » |
| 2026-08-XX | `docs/BUDGET.md` | **Contre l'esprit du jeton** | « le relay public est un **bien commun** — gratuit pour les utilisateurs » ; financement par grants NLnet, « Aucune publicité », « ne vend rien » |
| avant brand | `index.html.bak-pre-brand:262` | **Contre, affiché publiquement** | « Compte · abonnement · token — **jamais, par design** » |
| avant brand | `index.html.bak-pre-brand:269` | **Contre, slogan** | « AGPL-3.0 · **zéro token** · zéro télémétrie » |
| s.d. | `THREAT_MODEL.md:130` | **Contre, signature** | « AGPL-3.0 · Pas de token · Pas de télémétrie. » |
| 2026-11-02 | `docs/VISION-ET-PLAN-DE-FINALISATION-2026-11-02.md:132` | **Report** | « Un jeton prématuré ajoute complexité, risques réglementaires et friction d'adoption. → Pas de token/marketplace tant qu'un service mesurable n'a pas été rendu » |
| 2026-11-02 | `docs/VISION_CANONIQUE.md:185` | **Débat non tranché** | « **Modèle économique →** distribuer un jeton « POLY » pour louer de la puissance (mémoire et calcul) entre machines **vs** la réticence de l'utilisateur » |
| 2026-11-03 | `sources historiques locales non publiées/VISION-EXTRAITE.md:151` | **Report sine die** | « Token POLY — **Reporté sine die** — Risque réglementaire et friction avant tout service mesurable » |
| 2026-11-03 | `docs/atlas/plan/PLAN-MAITRE.md:315` | **Hypothèse à surveiller** | « H8 — Le jeton `POLY` est **reporté** → On rouvre la conception économique » |

**Lecture de la chronologie.** POLY apparaît d'abord comme **élément d'interface**
(6/06 : un solde à afficher dans la TUI). Il devient un **sujet économique** au
Conseil v2 (7/17), où il est décrit comme *déjà retiré par Lévy*. À partir du
6/08, tous les documents de production le rangent dans les **non-goals**. Aucun
document postérieur à août ne le réintroduit : ils le **reportent** en gardant la
décision ouverte (`ECOSYSTEM.md:241`, `VISION_CANONIQUE.md:185`).

---

## 2. Les arguments POUR, tels qu'écrits

1. **Il donne une unité au marché de ressources.** Le daemon prête CPU/RAM ;
   il faut compter ce qui est prêté et ce qui est consommé.
   `POLYGONE-SPEC-AUDIT.md` §4 veut « 0.1 POLY/min » affiché dans la TUI.
2. **Il rend le prêt incitatif sans argent réel.** `STRATEGIE.md` §6 décrit les
   « nœuds fantômes » (RES) et renvoie au daemon comme « socle technique » ;
   la note interne `COUNCIL_V2_RECONSIDERED.md:32` dit que le daemon est
   « la source du futur token POLY ».
3. **Il matérialise la réciprocité.** `docs/ARCHITECTURE_RESEAU.md:58` définit
   le ledger comme « le compteur des blocs qu'on s'engage à servir ».
4. **Le retrait est jugé regrettable par une voix du Conseil.**
   `COUNCIL_V2_RECONSIDERED.md:113` : « POLY retiré = erreur (Gödel aurait
   préféré le *documenter* honnêtement comme incomplet plutôt que de le
   supprimer) ».

## 3. Les arguments CONTRE, tels qu'écrits

1. **Friction d'adoption.** `VISION_CANONIQUE.md:185` rapporte la crainte de
   l'utilisateur que cela « freine l'intégration du projet par des entreprises
   et "casse tout" ».
2. **Risque réglementaire.** `STRATEGIE.md:55-56` : « Distraction réglementaire
   MiCA/SEC » ; `VISION-ET-PLAN…:132` : « risques réglementaires ».
3. **Contraire aux axiomes.** `STRATEGIE.md:55` : « contraire aux axiomes ».
   L'axiome de gratuité est formulé dans `VISION-EXTRAITE.md:20-24` :
   « "gratuit pour tout le monde" […] Cela interdit le jeton spéculatif avant
   qu'un service réel ne soit rendu ».
4. **Il contredit la promesse du produit.** `ECOSYSTEM.md:241` le range parmi
   les non-goals, à côté de « be a DAO » et « be a "Web3" thing ».
5. **Il n'a aucun service à rémunérer aujourd'hui.**
   `VISION-EXTRAITE.md:55` : « **Pas un token.** aucun jeton `POLY`, aucune
   marketplace, tant qu'un service mesurable n'est pas rendu ».
6. **Il trahirait le modèle de coût assumé.** `BUDGET.md` : le relay est un
   bien commun payé par la poche de Lévy puis des grants ; « le relay ne vend
   rien, ne collecte rien, ne tracke rien ».

---

## 4. Contradictions internes (document vs document)

| Point | Position A | Position B |
|---|---|---|
| **Statut de POLY** | `POLYGONE-SPEC-AUDIT.md` §4 le liste comme **fonctionnalité manquante à livrer** | `ECOSYSTEM.md:241` le liste comme **non-goal** ; `LEGAL.md` §2 : « Pas de token tradable » |
| **Qui l'a retiré, et quand** | `COUNCIL_V2_RECONSIDERED.md:32` : « le futur token POLY (que Lévy a retiré) » — donc **décidé** | `VISION_CANONIQUE.md:185` : contradiction listée comme **à trancher**, donc **non décidé** |
| **Nature de POLY** | `COUNCIL_V2_RECONSIDERED.md:32` : « token », mot qui évoque un actif tradable | `archive/2026-07-src/economy/mod.rs:1-7` : « local POLY token ledger […] **sans recours à une blockchain** » — comptabilité locale, pas un actif |
| **Modèle économique global** | `STRATEGIE.md:51-56` : freemium + consulting B2B + SaaS PQC, donc **revenus** | `BUDGET.md` : bien commun, grants, « Aucune publicité », jamais de vente ; `STRATEGIE.md` §7 : « Paywall / subscription — MIT, $0, forever » |
| **Licence** | `LEGAL.md` §7 : **AGPL-3.0**, décision 2026-08-07 | `VISION_CANONIQUE.md` §4 : « Licence → ouverte, de type **MIT** » |

**Contradiction la plus coûteuse** : `STRATEGIE.md` propose un modèle freemium
(§4) et interdit le paywall dans le même document (§7). Ce n'est pas POLY qui
est incohérent : c'est **la ligne économique du projet** qui l'est. Tant qu'elle
n'est pas tranchée, POLY n'a pas de place assignable — il hériterait de
l'incohérence.

---

## 5. Le cadre légal tel que le corpus le documente

- **MiCA / SEC** : nommés une seule fois, comme raison de refuser le jeton.
  `STRATEGIE.md:55-56` : « Distraction réglementaire MiCA/SEC ». Aucun document
  n'analyse MiCA en détail, ni ne qualifie POLY (utility / asset-referenced /
  e-money). **Il n'existe aucune analyse juridique du jeton dans le corpus.**
- **`LEGAL.md` existe bien** — à la racine, pas dans `docs/` :
  `checkout historique Polygone-real/LEGAL.md` (v0.2, 2026-06-29, 5 097 oct.).
  `STRATEGIE.md` §7 y renvoie pour justifier « Token / ICO — jamais ».
  Ce qu'il dit exactement (§2) : « Polygone **n'est pas** […] Un produit
  commercial. Pas d'abonnement. **Pas de token tradable.** Pas de NFT. »
  La formulation « tradable » est plus étroite que « pas de token » : elle
  interdit la cotation, pas la comptabilité interne.
- **AGPL-3.0** : `LEGAL.md` §7, décision du 2026-08-07, motivée par
  l'anti-appropriation (« quiconque modifie et *héberge* un service Polygone
  doit publier son code »). `Cargo.toml:13-18` confirme (`FEATURE_SPEC:53`).
- **Absence notable** : aucun document ne traite la fiscalité d'un jeton, la
  qualification de « service à prix » au sens TVA, ni le statut d'une monnaie
  interne non remboursable. `BUDGET.md` chiffre des coûts mais pas de revenus.

---

## 6. Ce que ces documents ne disent pas

- Aucun ne décrit **comment** POLY serait émis, distribué, plafonné ou converti.
  Les seuls chiffres existants sont des **taux internes** dans le code archivé
  (`economy/mod.rs` : solde initial 100, 0,1 POLY/min, 10 POLY/cœur-heure,
  5 POLY/Go-heure). Ce sont des paramètres de simulation, pas une politique
  monétaire.
- Aucun ne mentionne de **blockchain**, de chaîne, de contrat, de wallet externe.
  Le seul mécanisme de preuve présent est un reçu signé ML-DSA-65
  (`archive/2026-07-src/crypto/karma.rs`, `WorkVoucher`).
- Aucun ne tranche **à qui** appartiendrait le registre : local
  (`~/.polygone/poly.toml`) ou partagé. C'est pourtant la seule question qui
  distingue une comptabilité d'une monnaie.