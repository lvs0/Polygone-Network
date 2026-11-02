# Polygone — vision réalignée (document d'autorité)

> Ce document remplace les visions contradictoires présentes dans le corpus
> (`VISION_CANONIQUE.md`, `VISION-COMPLETE.md`, `VISION-RECONSTRUITE-ET-ETAT-REEL.md`,
> `MANIFESTO.md`, `whitepaper.md`). En cas de désaccord, **celui-ci gagne.**
> Il ne décrit pas ce que le code fait — pour ça, voir `ETAT-REEL.md`.
> Il décrit ce que le projet décide d'être, et dans quel ordre.

---

## 1. Une phrase

**Polygone rend la confidentialité et la puissance de calcul utilisables par
n'importe qui, sans plateforme au milieu.**

Le slogan d'origine reste la boussole : *« Tes données sont à toi. Personne ne
doit les voir, les toucher ou même savoir qu'elles existent. »*

## 2. Trois couches, un seul produit

| Couche | Ce qu'elle fait | Ce qu'elle ne prétend pas |
|---|---|---|
| **Socle** | Identités locales, crypto post-quantique, transport chiffré, relais aveugle, découverte de pairs | Être anonyme, être « mieux que Tor », résister à tout |
| **Ressources** | Mesurer, plafonner, prêter et reprendre CPU/RAM/stockage/réseau avec consentement | Remplacer la RAM locale, garantir une latence uniforme |
| **Services** | Msg, Drive, Hide, Petals, Serverless, Gateway, SDK | Être tous livrés en même temps |

**Un seul produit visible : `polygone`.** Les services sont des sous-commandes,
pas des applications séparées. Une API stable, deux clients (GUI grand public,
CLI/TUI pour l'administration).

## 3. Ordre de construction (non négociable)

1. **Vérité documentaire** — aucun README ne promet ce qui n'est pas appelé.
2. **Ressources réellement contraintes** — bail + isolation OS, pas un chiffre
   d'allocation.
3. **Réseau entre deux machines physiques** — LAN puis WAN, avec pertes, NAT,
   reconnexion.
4. **Premier parcours utilisateur** — installer, appairer, envoyer, pauser,
   désinstaller.
5. **Services additionnels** — dans l'ordre des dépendances.
6. **Durcissement et publication** — audit externe, CI multi-OS, releases signées.

Toute demande qui saute une marche est **notée et reportée**, pas absorbée.

## 4. Secrets et honnêteté

- Aucun mot « anonyme », « zéro métadonnée », « impossible à tracer »,
  « résistant à tout » sans modèle de menace et preuve correspondants.
- Le relais voit l'IP source et les identifiants de routage. On le dit.
- Un hash prouve l'intégrité des octets, pas la vérité du contenu.
- Un bail prouve une autorisation, pas une limite OS appliquée.
- Un test entre deux processus locaux ne prouve pas un réseau entre deux hôtes.
- Une compilation Windows ne prouve pas un fonctionnement Windows.

## 5. Ressources : le contrat

- **Désactivé par défaut.** Aucun partage avant un consentement explicite.
- **Mesure d'abord.** Si une mesure est inconnue, la politique est conservatrice.
  `Unknown` ne signifie ni « inactif » ni « sûr ».
- **Plafonds appliqués**, pas seulement calculés. Sans isolation OS, l'UI doit
  écrire « auto-imposé », pas « limité par le système ».
- **Retrait rapide et idempotent**, testé sur crash, perte réseau, veille et
  arrêt brutal.
- **Isolation dédiée** — jamais le cgroup de la session graphique.
- **Capot fermé** : option explicite, jamais une hypothèse silencieuse.

## 6. Économie

| Palier | Contenu | Statut |
|---|---|---|
| **Gratuit réel** | Auto-hébergement, P2P, Ollama local, relais bénévoles | Seul palier autorisé aujourd'hui |
| **Contribution = accès** | Prêter des ressources mesurées donne des crédits de service | Après preuve du socle ressources |
| **Paiement à l'usage** | Entreprises, services hébergés | Après un service mesurable |

**Aucun jeton `POLY`, aucune marketplace, aucune spéculation** tant que le
palier 1 n'est pas prouvé. Les documents `tokenomics.md` et `business-model.md`
sont conservés dans `salvage/` **comme traces rejetées**, pas comme plan.

## 7. Licence et gouvernance

- **AGPL-3.0-or-later** pour le code (état actuel de `Cargo.toml`).
- La licence MIT historique de l'ancienne ligne v1 (`Projets/Polygone`) n'est
  pas rétroactive et ne s'applique pas à ce workspace.
- Changement de licence = décision du propriétaire, documentée, jamais silencieuse.
- Canal de divulgation : `SECURITY.md` + `.well-known/security.txt`. Pas de
  bug bounty rémunéré tant qu'il n'existe pas.

## 8. Ce qui est explicitement hors périmètre (et le reste jusqu'à décision)

- Interface neuronale, IA biomimétique, essaims cognitifs, OS Polygone.
- Téléphone comme nœud de calcul (la PWA est une interface, pas un nœud).
- Jeton, marketplace, levée de fonds.
- Promesse « multi-hop anonyme » avant un transport adapté et une analyse externe.
- Support Windows/macOS déclaré sans runner de test.
- Tout module supplémentaire avant que les six marches du §3 soient franchies.

## 9. Définition de « terminé »

Une fonctionnalité est terminée quand **les sept** conditions sont vraies :

1. Un appelant de production l'utilise (pas seulement un test).
2. Un test la couvre, y compris un cas négatif.
3. Le comportement est observé sur le système réel.
4. Un test crash/restart nettoie correctement.
5. La documentation correspond au code.
6. Le résultat est reproductible (commande, OS, environnement).
7. Une sauvegarde restaurable existe et Git est propre ou explicitement listé.

Autrement dit : **pas de ✅ sans preuve.**
