# La maquette TUI et le solde POLY — ce que l'écran promettait

> NOTE DE PORTAGE — 4 novembre 2026 : analyse d’une maquette historique ; les choix de navigation restent à confirmer dans le produit réel.

> Périmètre : `source documentaire historique/` (3 fichiers) et
> `archive/2026-07-src/economy/mod.rs`. Aucune modification hors de ce fichier.

## 1. Ce que montrait l'écran

La source est `00-vision-source-2026-05-10.md`. Avertissement de méthode : ce
n'est **pas** la maquette elle-même. Le fichier est une description rédigée
(« Tu avais fourni des maquettes précises ») ; les images ou specs d'écran
d'origine ne sont pas dans ce dossier. Le même texte y figure **trois fois**
(artefact de duplication), avec une note de bascule de modèle au milieu : les
répétitions ne sont pas trois décisions distinctes.

L'écran décrit est un TUI à quatre onglets, navigable **aux flèches** :

| Onglet | Contenu tel que décrit |
|---|---|
| **Accueil** | État du nœud (actif/désactivé, **durée**) ; **solde en POLY**, qualifié d'« unité interne, représentant la contribution au réseau » ; **consommation actuelle par minute** ; boutons Désactiver / Mettre à jour / Redémarrer / Mettre en pause |
| **Favoris** | Liste des services préférés, activation/désactivation d'une touche |
| **Services** | Contrôle fin par service, **pauses temporisées 10 min / 30 min / 1 h / 2 h / 4 h**, réactivation, désactivation |
| **Paramètres** | URL et port du nœud, ouverture du Drive et espace utilisé, personnalisation des favoris |

Sous les onglets, une **barre d'état permanente** réaffiche « le solde POLY et la
consommation ». La promesse explicite : « L'utilisateur ne voit aucune ligne de
code, aucun fichier de configuration, aucun port à ouvrir. »

Deux détails comptent plus que le reste. D'abord, le solde est **permanent** :
il n'est pas dans un écran « facturation », il est sur l'accueil et dans la
barre d'état — c'est un indicateur de premier plan, au même rang que « le nœud
est-il actif ». Ensuite, il est **couplé à une durée** : « état du nœud (actif/
désactivé, **durée**) » et « consommation par minute » décrivent un compteur qui
tourne, pas une photo. L'écran raconte donc une économie de flux : du temps de
service converti en unité.

`VISION-RECONSTRUITE-ET-ETAT-REEL.md` §3 reprend cette maquette à l'identique et
ajoute la seule exigence qui la rend honnête : « L'interface doit montrer les
conséquences réelles d'une action, et non simuler un état. »

## 2. Ce que ça impliquait mécaniquement

Un solde affiché qui **se consomme au temps** suppose cinq mécanismes. Aucun
n'est décrit dans le document de vision, et c'est là que le projet a buté.

1. **Une autorité d'émission** — qui crée les POLY initiaux, et selon quelle
   règle. Sans elle, le solde de départ est un nombre arbitraire.
2. **Une autorité de débit** — qui a le droit de retirer. Si c'est le fichier
   local, l'utilisateur est juge et partie.
3. **Une mesure de la contribution** — « représentant la contribution au
   réseau » exige de prouver qu'un service a bien été rendu, à qui, et combien
   de temps. Un compteur local ne prouve rien.
4. **Un règlement entre pairs** — si A consomme et B fournit, il faut un
   transfert vérifiable de A vers B, donc une protection contre la
   double-dépense.
5. **Une convertibilité** — l'écran ne dit jamais si 1 POLY vaut de l'argent.
   C'est précisément le point qui fait basculer un compteur interne dans le
   champ réglementaire (MiCA).

Ce que le code archivé fournit réellement (`archive/2026-07-src/economy/mod.rs`) :
un ledger **TOML local** dans `~/.polygone/poly.toml`, explicitement « sans
recours à une blockchain », avec écriture atomique `.tmp` + `rename`.
Solde initial **100 POLY**, débit de **0,1 POLY/min par service actif**, gain de
**10 POLY par cœur-heure** et **5 POLY par Go-heure de RAM**.
`crypto/karma.rs` contient la brique qui manquait : un `WorkVoucher` signé en
ML-DSA-65 (worker, requester, units, horodatage) — mais aucun appelant.

Le trou est donc net : la signature existe, le reçu existe, **la circulation
n'existe pas**. Un `poly.toml` est éditable au `vim` ; le supprimer en régénère
un à 100 POLY. Un solde affiché dans ces conditions ne mesure pas la
contribution, il la **déclare**.

## 3. La trace de l'abandon

L'abandon est documenté, daté, et répété dans quatre registres indépendants :

- **`LEGAL.md` §2, l. 24** (posture v0.2, 2026-06-29) : « ❌ Un produit
  commercial. Pas d'abonnement. **Pas de token tradable. Pas de NFT.** »
- **`ECOSYSTEM.md`, l. 241-242** : Polygone ne doit pas « be a token (le ledger
  POLY archivé **attend une décision explicite** — il n'est pas dans le
  produit) ».
- **`COUNCIL_V2_RECONSIDERED.md`, l. 32** : le daemon est « la source du futur
  token POLY (**que Lévy a retiré parce que trop marketing**) » ; l. 51 : « la
  petite monnaie POLY naissait d'une pulsion marchande, **fut retirée** ».
- **`STRATEGIE.md` §4 et §7** : « **Pas de token. Pas d'investisseurs.**
  (Distraction réglementaire MiCA/SEC, pas nécessaire pour le B2B, contraire aux
  axiomes.) » et « Token / ICO / equity crowdfunding — **jamais** ».
- Vitrine publique : `web/index.html` l. 488 « Pas d'investisseurs. Pas de
  token. » ; `site/index.html` l. 262 « Compte · abonnement · token — **jamais,
  par design** ».

Le sujet n'est pas clos pour autant : `VISION_CANONIQUE.md` §5 le classe encore
en **contradiction n° 7** (« distribuer un jeton POLY pour louer de la puissance
vs la réticence de l'utilisateur, qui craint que cela freine l'intégration par
des entreprises et casse tout »), et
`VISION-ET-PLAN-DE-FINALISATION-2026-11-02.md` l. 132 tranche en sursis :
« Pas de token/marketplace **tant qu'un service mesurable n'a pas été rendu** ».
L'abandon est donc un **retrait d'interface**, pas une décision d'architecture
enterrée : le code est archivé, pas supprimé, et attend un arbitrage.

## 4. Maquette contre ledger : cohérent sur la forme, faux sur l'autorité

**Ce qui concorde.** Les chiffres de l'écran tombent juste : consommation « par
minute » et `RATE_PER_MIN = 0.1` ; 100 POLY de départ = 1 000 minutes de service
= **16 h 40** de nœud actif avant épuisement à un service. Les pauses temporisées
(10 min → 4 h) sont les unités de temps de ce même compte. La maquette et le
ledger ont été conçus ensemble.

**Ce qui ne concorde pas.** L'écran présente POLY comme « représentant la
contribution au réseau » — une grandeur collective. Le ledger en fait un
compteur privé, non signé, non partagé, remis à 100 à la première exécution et
plafonné à zéro (un `record_renting_expense` supérieur au solde ne crée pas de
dette). Trois conséquences : on ne peut ni **prouver** sa contribution, ni
**payer** quelqu'un d'autre, ni **être en dette** — donc aucun marché ne peut
naître de ce fichier, même en le voulant.

Conclusion de la comparaison : la maquette décrit correctement un **tableau de
bord de réciprocité**, et le ledger en est l'implémentation locale honnête. Ce
qui n'a jamais existé, et que l'écran laissait croire, c'est la **couche de
règlement entre nœuds**. C'est exactement le périmètre que `WorkVoucher` couvre
et que personne n'a câblé.