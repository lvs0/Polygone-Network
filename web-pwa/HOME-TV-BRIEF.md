# Home TV — Brief anti-slop (impeccable + hallmark)

## 1. Données réelles (de state.db.bak)
- Fichier : /home/l-vs/.hermes/state.db.bak (2,2 Go)
- Tables : sessions, messages, async_delegations, gateway_routing
- 0 hits directs "hometv" dans messages (179273 lignes) ou sessions (5391 lignes)
- Projet physique : /home/l-vs/Projets/hometv (existe? NON VÉRIFIÉ — doit être confirmé)
- PWA existante : Polygone-v2/web-pwa (nouveau, non liée)

## 2. Mode design (selon impeccable)
- Mode : Operate (utilisateur effectue une tâche : regarder la TV)
- Surface : PWA + interface TV (dashboard, loop, lecteur)

## 3. Questions obligatoires (avant tout code)
Pour NE PAS produire du slop :

Q1 — Audience :
  a) L'utilisateur seul dans son salon (Home TV = TV domestique) ?
  b) Ou usage public (bar, hall, événement) ?

Q2 — Cas d'usage principal :
  a) TV en boucle continue (contenu généré, style loop tmux) ?
  b) TV interactive (le spectateur choisit le flux / le canal) ?
  c) TV comme tableau de bord (affiche l'état du système, des agents, des projets) ?

Q3 — Tone (choix extrême, pas "clean et modern") :
  - Brutalist (brut, monolithique, pas d'ornement)
  - Atmospheric (noir profond, lueurs, ambiance cinématique)
  - Playful (couleurs, interactions légères, animations)
  - Editorial (texte dominant, structure de journal / magazine)
  - Utilitarian (fonctionnel, pas de déco — style terminal / CLI)

Q4 — Contenu réel :
  - Quelles sources de données doivent s'afficher ? (sessions Hermes, messages actifs, délégations, projets en cours ?)
  - Faut-il intégrer le loop wakeup (/loop wakeup) comme widget ?
  - Liens avec Polygone PWA / Cube / MomentoBooth ?

Q5 — Références anti-slop (interdites) :
  - Pas de métriques inventées (ex : "+47% conversion")
  - Pas de boutons génériques ("Click me") — chaque bouton doit avoir un verbe réel
  - Pas de chrome faux (pas de fausses barres de navigateur, faux cadres iPhone)
  - Pas d'italiques sur les titres
  - Pas de section tags / numéros de chapitre sauf si le contenu est vraiment ordinal (ex : "Chapitre 3" dans une histoire réelle)

## 4. Structure suggérée (macrostructure)
Selon la réponse Q2, on choisira une macrostructure du catalogue (voir references/macrostructures.md) :
- Si Q2a (loop) → Manifesto ou Bento Grid (contenu en boucle, sections répétées)
- Si Q2b (interactif) → Workbench ou Marquee Hero (action principale visible)
- Si Q2c (dashboard) → Stat-Led ou Bento Grid (données en blocs)

## 5. Prochaines actions (non exécutées)
- Confirmer le dossier /home/l-vs/Projets/hometv et son contenu réel
- Vérifier si un DESIGN.md ou un tokens.css existe déjà dans le projet
- Confirmer Q1–Q5 ci-dessus
- Charger hallmark + impeccable et exécuter le Design flow (pre-flight → genre → macrostructure → build → slop-test 58 portes)
- Ne créer AUCUN fichier de production avant que le user confirme le brief

LOOP_COMPLETE : en attente des réponses Q1–Q5 et confirmation du dossier physique.
