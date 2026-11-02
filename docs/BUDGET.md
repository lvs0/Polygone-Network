# 💶 BUDGET — la soutenabilité du relay, noir sur blanc

> **La règle :** le relay public est un **bien commun** — gratuit pour les
> utilisateurs, payé par un budget assumé et documenté. Si le budget ne
> tient plus, le relay s'arrête **en le disant**, jamais en silence.
> C'est la traduction financière de la décision D5 (relay public assumé).

---

## 1. Ce que coûte le relay (estimation honnête)

Un relay Polygone est un petit binaire stateful-in-RAM (table de routage,
sessions, aucun stockage disque). Les chiffres ci-dessous sont des bornes
réalistes pour le premier relay public (`relay.polygone.network`), pas des
promesses marketing.

| Poste | Coût / mois | Assomption |
|-------|-------------|-----------|
| VPS (1 vCPU · 1 Go RAM · 20 Go SSD) | 4–8 € | Un relay, une table de routage, pas de disque. La borne haute = HA/réplication. |
| Domaine `polygone.network` | ~1 € | ~10–12 €/an, lissé. |
| Surveillance (uptime, alertes) | 0–5 € | Gratuit (UptimeRobot) → payant (Pingdom) selon besoin. |
| Sauvegardes | 0 € | Aucune : le relay n'a rien à sauvegarder (zéro état persistant). |
| **Total borne basse** | **~5 €/mois** | VPS minimal + domaine. |
| **Total borne haute** | **~14 €/mois** | VPS confortable + domaine + surveillance. |

**Règle de transparence :** le coût réel mensuel est affiché ici, mis à jour
à chaque changement. Le chiffre qui compte n'est pas le total — c'est
**€/mois ÷ utilisateurs actifs du mois** : quand il y a 1 utilisateur, le
silence coûte 5 €/mois ; quand il y en a 100, il coûte 0,05 €.

## 2. Les sources de financement (dans l'ordre)

| Source | Statut | Rôle |
|--------|--------|------|
| **Poche de Lévy** | Actif | Le relay tourne aujourd'hui sur cette base (~5 €/mois). Aucune éligibilité requise. |
| **Grants NLnet NGI / Prototype Fund** | À demander | Le poste naturel pour un outil anti-métadonnées post-quantique européen : NLnet (NGI0 Privacy & Trust) et Prototype Fund financent exactement ce type de bien commun. Montant cible : couvrir 12–24 mois d'exploitation. |
| **Lettres D3 (CNIL / ANSSI / EFF)** | À envoyer | Pas de l'argent directement — de la **reconnaissance de posture**. Une réponse favorable ouvre les portes des grants et de la presse. Voir [`DECISIONS.md`](./DECISIONS.md) D3. |
| **Dons** | Jamais demandés | La seule règle absolue : **le relay ne vend rien, ne collecte rien, ne tracke rien**. Un bouton « donate-free » n'aura jamais de tracker — et les dons ne conditionnent jamais l'accès. |

## 3. Ce qui n'est PAS financé par ce budget

- **Aucun salaire.** Le budget couvre l'exploitation du relay, pas le temps
  de développement (bénévole, assumé).
- **Aucune publicité.** Un relay financé par la pub trahit sa promesse.
- **Aucune collecte de données.** Même « anonymisée » — le produit est la
  négation des métadonnées, pas leur monétisation.

## 4. Le plan si le budget ne tient plus (par ordre)

1. **Annonce publique** — 30 jours de préavis, le coût réel affiché, la date
   d'arrêt.
2. **Appel aux opérateurs** — le protocole permet à quiconque de lancer son
   propre relay (`polygone-relay` est open-source, un binaire). Migration
   documentée.
3. **Arrêt propre** — le relay s'éteint sans état à détruire (il n'en a
   aucun). C'est, ironiquement, la promesse tenue jusqu'au bout.

---

*Budget · v2.0.0-rc2 · « Le silence a un coût. Il se paie en le disant. »*
