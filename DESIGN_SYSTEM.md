# DESIGN_SYSTEM.md — Le langage visuel de Polygone

> *Ce document décrit les choix de design appliqués à Polygone v2.0.0-rc1.*
> *Source : Comité 2 Conseil des Sages 2026-06-29 (Ive ✕ Hitchcock ✕ Bach).*
> *Pas une aspiration, une décision.*

---

## 1. Couleurs

Deux couleurs. Pas trois.

| Token | Hex | Usage | Pourquoi |
|-------|-----|-------|----------|
| `--cyber-slate-900` | `#0f172a` | Background (TUI + web) | Le silence matériel (Ive) |
| `--cyber-amber-500` | `#f59e0b` | Accent (matière en mouvement) | Le silence est *chaud*, pas gris (Ive) |
| `--cyber-slate-700` | `#334155` | Border | Bord entre matière et vide |
| `--text-muted` | `#94a3b8` | Métadonnée | Pas un silence, une annotation |

**Pas de gradient décoratif.** Une matière unie, en suspension.

**Pas de cyan** (l'ancien choix). Le cyan fait « tech 2015 ». L'ambre fait « matière 2030 ».

---

## 2. Typographie

Deux police. Pas trois.

| Police | Usage |
|--------|-------|
| `JetBrains Mono` (TUI + code) | Tout ce qui est technique ou lisible |
| `Georgia` (web hero poetic) | Une seule belle phrase, dans une seule page |

Pas d'autre famille. Pas de serif pour les exemples de code. Pas de sans-serif dans la TUI.

**Règle** : si le texte est `Georgia italique`, c'est une phrase poétique. S'il est `JetBrains Mono`, c'est une affirmation technique. Les deux poids doivent être **égaux** sur la même baseline.

---

## 3. Tactilité (Ive)

Le silence du site n'est pas plat. C'est **grain**.

```
.hero {
  background: var(--cyber-slate-900);
  /* Grain visible : effet matière papier ancien */
  background-image: radial-gradient(
    circle at 20% 50%,
    rgba(245, 158, 11, 0.04) 0%,
    transparent 60%
  );
}
```

Le `.logo-hex` pulse à 4s (long, lent, suggestif). Pas un blink. Pas un flash. Une *respiration*.

```
@keyframes pulse {
  0%, 100% { text-shadow: 0 0 60px var(--accent-glow); }
  50%      { text-shadow: 0 0 40px var(--accent-glow); }
}
```

---

## 4. Suspense typographique (Hitchcock)

**Quand un message est en transit, l'utilisateur VOIT l'attente.**

```html
<div class="transit">
  <div class="transit-bar">⬡</div>
  <div class="transit-tick">640 ms</div>
  <div class="transit-step shade-1">[f1]</div>
  <div class="transit-step shade-2">[f2]</div>
  <div class="transit-step shade-3">[f3]</div>
  <div class="transit-step shade-4">[f4 — got it]</div>
</div>
```

| Vitesse cible | Animation | Pourquoi |
|---------------|-----------|----------|
| `0-50 ms` | Pulse amber 4× rapides | Préparation, latence réseau |
| `50-200 ms` | 4 fragments qui s'allument un à un | Reconstruction visible |
| `200-400 ms` | Tick à 60 fps | L'horloge du TTL |
| `400+ ms` | Texte ambre fade in : « ✓ delivered » | Hitchcock resolution |

Le spinner est **interdit**. La barre de progression est **interdite**. Uniquement typographie temporelle.

---

## 5. Contrepoint (Bach)

Chaque voix déclare son inverse dans la même mesure.

| Voix principale | Contre-voix | Format |
|-----------------|-------------|--------|
| Tagline poétique | Footnote technique | 1:1 pixel ratio |
| `Available` (vert) | `Built from source, dev-only` (gris italique) | Côte à côte |
| `Live` badge | `audit pending` micro-note | Inline |
| Crypto slogan | RFC + standard number | Inline |
| Service name | Re-entry condition if removed | Inline |

**Règle dure :** aucune promesse poétique n'est imprimée sans sa note technique. La phrase sans footnote n'est *pas livrée*.

---

## 6. TUI — 2 onglets (Jobs)

```
╔════════════════════════════════════════╗
║  ⬡ POLYGONE                  v2.0.0-rc1║
║  ───────────────────────────────────  ║
║  « L'information n'existe pas.        ║
║    Elle traverse. »                  ║
║  ───── aucun fragment reconstructible ║
║    sans 4-of-7, jamais. ─────        ║
╠════════════════════════════════════════╣
║                                        ║
║  ▶ Envoyer                            ║
║    Quitter                            ║
║                                        ║
║  [↑↓] naviguer  [⏎] valider  [:] cmd  ║
╚════════════════════════════════════════╝
```

- **2 onglets** : `Envoyer` et `Quitter`.
- Tout le reste est paramètre caché derrière `:` (vim-style).
- Couleurs : cyan supprimé ; ambre pour l'`▶` actif ; slate pour inactif.

---

## 7. Anti-patterns visuels (interdits)

| Pattern | Pourquoi interdit |
|---------|-------------------|
| Glassmorphism | Dit 2019. Pas intemporel. |
| Particules de fond | Distrayant. Pas du design. |
| Emoji décoratif hors fonctionnalité | Bruit visuel (sauf légende explicite) |
| Animations > 600 ms | Suspense ≠ attente utilisateur frustrée |
| Gradient toolbar | 2020 web-app fatigue |
| Multiple CTA rows | Le Jobs-isme : un seul bouton par écran |

---

*Fin de `DESIGN_SYSTEM.md`.*
*Hérite de : `ECOSYSTEM.md` (Comité 1), `Conseil des Sages 2026-06-29` (Comité 3).*
