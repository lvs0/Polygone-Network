# BRAND_GUIDELINES.md — Polygone

> *L'identité visuelle de Polygone, en un seul fichier. Le système est déjà décidé
> par le Conseil des Sages du 29 juin 2026 (Ive ✕ Hitchcock ✕ Bach). Ce document
> le rend utilisable : tokens exportables, déclinaisons prêtes, anti-patterns verrouillés.*

---

## 1. Le système en une phrase

**Une matière unie en suspension, avec un seul point chaud.**
Slate profond comme silence matériel. Ambre comme la chaleur de l'information qui traverse.
Deux polices, deux voix : la prose technique en monospace, la phrase poétique en italique serif.
Chaque voix principale a son contre-voix (Bach). Chaque attente utilisateur est visible, pas cachée (Hitchcock).
Le verre dépoli est interdit. La particule de fond est interdite. Le gradient décoratif est interdit.

---

## 2. Tokens (CSS variables)

```css
:root {
  /* Couleurs — deux couleurs, pas trois */
  --slate-900: #0f172a;   /* Background. Le silence matériel. */
  --slate-800: #1e293b;   /* Surface élevée (modale, onglet actif) */
  --slate-700: #334155;   /* Bord, hairline, séparateur */
  --slate-500: #64748b;   /* Texte désactivé, métadonnée faible */
  --text-muted: #94a3b8;  /* Métadonnée, annotation */
  --text: #e2e8f0;        /* Texte principal */
  --amber-500: #f59e0b;   /* Accent. La matière en mouvement. */
  --amber-glow: rgba(245, 158, 11, 0.25);  /* Halo (jamais un border-decoration) */
  --amber-dim: rgba(245, 158, 11, 0.06);   /* Tache chaude, jamais un gradient CTA */

  /* Typographie — deux familles, pas trois */
  --font-mono: "JetBrains Mono", "Cascadia Code", "Fira Code", ui-monospace, monospace;
  --font-poetic: Georgia, "Times New Roman", serif;

  /* Géométrie */
  --hex-radius: 1;        /* Hexagone régulier, aucune distorsion */
  --mark-stroke: 2;        /* Stroke du mark, en unité viewBox */
  --mark-tick: 6;          /* Longueur d'un tick d'arête (en unité viewBox) */

  /* Mouvement — lents, suggérés, jamais boucle décorative */
  --pulse-amber: 4s;       /* Respiration du mark */
  --tick-ms: 16;           /* 60 fps : tick de l'horloge TTL */
}
```

### Règle d'usage

- **Maximum un accent ambre par section.** Pas d'ambre en double, en triple.
- **Pas de cyan.** Il a été supprimé par le Conseil (tech 2015, pas intemporel).
- **Pas de noir pur** (`#000000`). Toujours `--slate-900` ou plus haut.
- **Pas de blanc pur** (`#ffffff`). Toujours `--text` ou `--slate-*`.

---

## 3. Le mark hexagonal

### Géométrie

Hexagone régulier, 6 sommets équidistants du centre. Aucun sommet nommé, aucune orientation
privilégiée — la symétrie est la promesse d'anonymat topologique.

**Centre** : (32, 32) dans un viewBox 64×64. **Circonradius** : 24.
**Stroke** : 2 unités, `miter`, pas d'arrondi. Le trait est une arête, pas un filon.
**Six ticks** : un par arête, centrés sur le milieu du segment, longueur 6 unités, perpendiculaire à l'arête.
**Pip central** : un disque, rayon 1.5. Pas une étoile, pas un point cardinal — un point.

### Pourquoi cette géométrie

- L'hexagone est la **forme du réseau** (six arêtes = six voisins dans un maillage régulier).
- Les six ticks sont les **six portes de transit** — un fragment entre, un fragment sort, l'observateur ne sait pas lequel.
- Le pip central est la **promesse de réunion** — 4-of-7 fragments se rencontrent ici, brièvement, sans laisser de trace.
- Aucun coin n'est plus "haut" qu'un autre : la marque n'a pas de sens de lecture préféré.

### Fichiers (dans `brand/mark/`)

| Fichier | Usage |
|---------|-------|
| `mark.svg` | Universel, fond slate-900. Default. |
| `mark-dark.svg` | Sur fond `#0a0f1a` (overlay PWA, status bar sombre). |
| `mark-light.svg` | Sur fond clair (`#f8fafc`). Stroke et ticks en slate, pas ambre. |
| `mark-mono.svg` | Sans champ, monochrome ambre. Pour tamponner sur photo / bois. |
| `favicon.svg` | 32×32, géométrie simplifiée (pas de ticks, pip conservé). |
| `favicon-16.svg` | 16×16, stroke aminci à 1.2 pour rester lisible. |
| `favicon-32.svg` | 32×32, stroke 1.4. |
| `apple-touch-icon.svg` | 180×180, iOS applique les coins arrondis. |

### Espace de respiration

Le mark exige **une zone de silence** autour de lui, égale à la moitié de sa largeur.
Aucun texte, aucun bord, aucune ligne décorative ne peut entrer dans cette zone.
C'est la règle Ive : le silence matériel n'est pas du vide, c'est du champ.

---

## 4. Le wordmark

### Voix typographique

| Voix | Famille | Usage |
|------|---------|-------|
| Technique | `JetBrains Mono` | Wordmark, code, métadonnée, footnote, badge, état |
| Poétique | `Georgia` italique | Tagline, contre-voix, une seule phrase par surface |

Pas de sans-serif hors monospace. Pas d'autre serif. La discipline est le choix.

### Lockups (dans `brand/wordmark/`)

| Fichier | Usage |
|---------|-------|
| `wordmark-horizontal.svg` | Signature, README header, page "about". |
| `wordmark-vertical.svg` | Cartes de visite, splash, app icon avec texte. |
| `wordmark-compact.svg` | Navbar, terminal prompt, code-fence title. |
| `wordmark-horizontal-light.svg` | README GitHub en mode clair, blog post imprimé. |

### Règle du contrepoint (Bach)

Toute phrase poétique imprimée **doit** être accompagnée de sa footnote technique
dans la même composition. Pas plus tard, pas sur une autre page — côte à côte.
La phrase sans footnote n'est *pas livrée*.

Exemple :
```
« L'information n'existe pas. Elle traverse. »
   ─ aucun fragment reconstructible sans 4-of-7, jamais. ─
```

---

## 5. Mouvement

Trois animations, pas quatre. Aucune n'est une boucle décorative.

| Animation | Durée | Quand |
|-----------|-------|-------|
| **Pulsation du mark** | 4s | Toujours visible, à l'écran. Longue. Suggestive. Pas un blink. |
| **Typographie temporelle** (transit) | 0-400ms+ | Pendant qu'un message est en transit. Voir `DESIGN_SYSTEM.md` §4. |
| **Entrée des sections** | < 600ms | À l'apparition d'une section. Pas d'oscillation. |

### Interdits

- Animations > 600ms (suspense ≠ attente frustrée).
- Particules de fond.
- Spinners circulaires, barres de progression.
- Boucles infinies sans raison sémantique.

---

## 6. Tactilité (Ive)

- **Pas de glassmorphism.** Dit 2019. Pas intemporel.
- **Pas d'ombre portée pure noire.** Si ombre, alors teintée du fond (`rgba(15, 23, 42, 0.4)`).
- **Coins arrondis modérés** (8-12px pour cartes, 6-8px pour boutons). Pas de "grosse pilule" partout.
- **Bordure hairline** (`--slate-700`, 1px) entre les sections, pas d'ombre.
- **Grain visible** : un `background-image` SVG noise très subtil (1-2% d'opacité) sur les hero, jamais sur le contenu scrollé.

---

## 7. Anti-patterns visuels (verrouillés par le Conseil)

| Pattern | Pourquoi interdit |
|---------|-------------------|
| Glassmorphism / liquid glass | Dit 2019. Pas intemporel. |
| Particules de fond | Distrayant. Pas du design. |
| Gradient décoratif | 2020 web-app fatigue. |
| Cyan comme accent | Tech 2015. Supprimé. |
| Multiple CTA rows | Le Jobs-isme : un seul bouton par écran. |
| Emoji décoratif hors fonctionnalité | Bruit visuel (sauf légende explicite). |
| Animations > 600ms | Suspense ≠ attente frustrée. |
| Coin arrondi > 16px systématique | Effet "bubble". |
| Ombre noire pure | Tue la profondeur. |
| Texte serif partout | Conflit avec la prose technique. |
| Inter comme police par défaut | L'IA défaut. Refusé. |
| "Three equal feature cards" | Le LLM défaut. Refusé. |
| Eyebrow uppercase tracking sur chaque section | Le LLM défaut. Refusé. |
| Em-dash `—` | Voir `design-taste-frontend` §9.G. |

---

## 8. Déclinaisons livrées

```
brand/
├── mark/                   (8 SVG : mark, mark-dark, mark-light, mark-mono, favicon, favicon-16, favicon-32, apple-touch-icon)
├── wordmark/               (4 SVG : horizontal, vertical, compact, horizontal-light)
├── social/
│   └── og-card.svg         (1200×630, partage Twitter/LinkedIn/Slack)
├── github/
│   └── banner.svg          (1280×640, social preview GitHub)
├── badges/                 (5 SVG : license, version, tests, platform, crypto)
├── docs/                   (ce document + DESIGN_SYSTEM.md + PHILOSOPHY.md)
└── examples/
    └── index.html          (landing incarne le système complet)
```

---

## 9. Procédure d'usage

1. **Choisir le bon mark** : `mark.svg` par défaut, `mark-light.svg` sur fond clair, `mark-mono.svg` sur photo.
2. **Choisir le bon wordmark** : `wordmark-horizontal.svg` pour 90% des cas.
3. **Toujours appliquer le contrepoint** : si la phrase poétique apparaît, la footnote technique suit.
4. **Vérifier l'espace de respiration** : demi-largeur du mark autour, jamais de texte dedans.
5. **Vérifier les anti-patterns** : aucun glassmorphism, aucune particule, aucun gradient, aucun cyan.
6. **Si doute, lire `DESIGN_SYSTEM.md`** (le Conseil) avant `design-taste-frontend` (le skill).

---

*Polygone v2.0.0 — brand pack livré le 2026-08-20.*
*Conseil des Sages 2026-06-29 (Comités 1, 2, 3). AGPL-3.0.*
