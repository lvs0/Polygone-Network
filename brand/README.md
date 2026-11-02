# Brand pack — Polygone

Identité visuelle complète, pack livré le 2026-08-20.

## Structure

```
brand/
├── mark/              Mark hexagonal (4 variantes + favicon multi-taille + apple-touch)
├── wordmark/          Wordmark (4 lockups : horizontal, vertical, compact, light)
├── social/            og-card.svg (1200x630, partage social)
├── github/            banner.svg (1280x640, social preview GitHub)
├── badges/            5 badges style shields.io (license, version, tests, platform, crypto)
├── docs/
│   ├── BRAND_GUIDELINES.md   Ce document + tokens + règles + anti-patterns
│   ├── DESIGN_SYSTEM.md       Système visuel (Conseil des Sages 2026-06-29)
│   └── PHILOSOPHY.md          Les 5 axiomes du produit
└── examples/
    └── index.html    Landing incarne le système complet
```

## Quick start

### README GitHub
```markdown
![Polygone](brand/github/banner.svg)

[![license](brand/badges/badge-license.svg)](LICENSE)
[![version](brand/badges/badge-version.svg)](Cargo.toml)
[![tests](brand/badges/badge-tests.svg)]()
[![crypto](brand/badges/badge-crypto.svg)]()
```

### HTML head (favicon, OG)
```html
<link rel="icon" type="image/svg+xml" href="brand/mark/favicon.svg">
<link rel="apple-touch-icon" href="brand/mark/apple-touch-icon.svg">
<meta property="og:image" content="brand/social/og-card.svg">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:image" content="brand/social/og-card.svg">
```

## Règles d'or (système verrouillé)

1. **Slate + ambre, jamais de cyan.** Deux couleurs.
2. **Monospace + italique serif, pas de sans.** Deux voix.
3. **Contrepoint Bach** : toute phrase poétique a sa footnote technique à côté.
4. **Pas de glassmorphism, pas de particules, pas de gradient décoratif.**
5. **Espace de respiration** = demi-largeur du mark, jamais rien dedans.

Voir [`brand/docs/BRAND_GUIDELINES.md`](docs/BRAND_GUIDELINES.md) pour le détail complet.
