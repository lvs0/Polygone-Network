# 🌙 Protocole Premier Soir — la première sortie de Polygone

> *« Mieux vaut un projet imparfait sorti aujourd'hui qu'un projet parfait
> dans 6 mois. »* — ta propre règle (second_brain).
>
> *« Je ne prends plus jamais de décision stratégique sans test
> utilisateur. »* — ta propre règle (second_brain).
>
> Ce protocole est l'application de tes deux règles, pour la première fois.
> **Quatre mois, 513 mentions, zéro utilisateur attesté. Ça s'arrête ici.**

---

## 1. La promesse du soir

Polygone est un **réseau de transit éphémère post-quantique**. Ce soir, on
ne démontre que deux choses, dans l'ordre :

1. **Un message traverse et s'évapore.** Alice envoie un message à Bob via
   le relay. Bob le lit. Personne d'autre — ni toi, ni le relay — ne peut
   le lire.
2. **Un fichier traverse en 4/7.** Alice envoie un fichier. Bob le reçoit,
   intact. Le relay n'a jamais vu ni le contenu ni le nom.

Rien d'autre. Pas de compute, pas de pets, pas de token. **Deux promesses,
tenues.**

> **En amont du soir : `polygone premier-soir`** — le même scénario en solo,
> 5 minutes, pour que toi, l'opérateur, tu aies déjà vu le message mourir
> avant de le montrer à quelqu'un. La promesse devient une commande ; le
> soir réel reste le protocole ci-dessous.

## 2. Les trois personnes

Tu as dit : *« recruter des potes, des cousins, des personnes de
confiance »*. Concrètement :

| Rôle | Qui | Pourquoi |
|------|-----|----------|
| Alice (envoyeur) | Toi | Tu connais le protocole |
| Bob (récepteur) | Ton cousin ou ton pote | La personne de confiance |
| Observateur | La troisième | Regarde ce que le relay voit — et ce qu'il ne voit pas |

Si la troisième personne est technique : elle joue **l'adversaire**. Elle
a accès au terminal du relay (les logs, les tailles, les noms) — et doit
conclure elle-même : *« je ne peux pas lire le contenu »*.

## 3. La checklist avant le soir

- [ ] `cargo test --workspace` → 108 verts (uniques : produit 46, core 34, relay 7, daemon 21)
- [ ] `cargo clippy --all --all-targets -- -D warnings -A clippy::all` → 0 erreur
- [ ] `cargo fmt --all -- --check` → propre
- [ ] `cargo build --release` sur **deux machines différentes** (pas deux
      fois la même)
- [ ] `./target/release/polygone test` → **7/7**
- [ ] `./target/release/polygone premier-soir` → le scénario se déroule
      (les promesses produit sont des commandes qui tournent)
- [ ] Un tarball `polygone-<os>-<arch>.tar.gz` + `sha256sum` noté sur un
      papier (les deux machines l'installent depuis ce tarball, PAS depuis
      un pipe-to-bash)
- [ ] Un relay : `./target/release/polygone-relay` (port 7000) sur une
      machine accessible des deux autres

## 4. Le scénario (30 minutes, pas plus)

```
19:00  — Alice et Bob lancent `polygone clef`, échangent les clés
         (sur un canal hors-ligne : dans la main, sur un papier).
19:05  — Alice : polygone envoyer --via <relay> --a <bob_node> \
            -d <clef_bob> "ton premier message éphémère"
19:07  — Bob : polygone ecouter <relay> → il lit le message.
         → PROMESSE 1 TENUE. Les trois personnes constatent.
19:12  — Alice : polygone envoyer --via <relay> --a <bob_node> \
            -d <clef_bob> --fichier <une photo ou un texte>
19:15  — Bob : le fichier arrive dans ~/.polygone/received/, vérifié.
         → PROMESSE 2 TENUE.
19:20  — L'adversaire (s'il y en a un) regarde le relay : tailles,
         sessions, adresses — mais AUCUN contenu, AUCUN nom de fichier.
19:25  — Question ouverte aux trois : « qu'est-ce que tu veux qu'il
         traverse, toi ? » → NOTER les réponses mot pour mot.
19:30  — Fin. On ne dérive pas. Les observations sont commitées.
```

## 5. Le carnet d'observation (commité, pas dans un tiroir)

Après le soir, réponds à ces questions dans `docs/observation-premier-soir.md`
et commite-le le jour même :

1. Quelle promesse a été tenue ? Quelle promesse a failli ? (sois précis,
   cite la commande)
2. Qu'ont dit Alice/Bob/l'adversaire **textuellement** ?
3. Qu'est-ce qui les a bloqués (install, clé, réseau) ?
4. Qu'est-ce qu'ils ont voulu faire que le produit ne permet pas ?
5. La décision stratégique de la semaine suivante se prend SUR CE
   DOCUMENT, pas sur une intuition.

## 6. Définition de fait

La sortie n'est pas un tag. La sortie est **une personne qui n'est pas toi
qui utilise Polygone un soir, et ton carnet qui le prouve.**

*Une fois le Premier Soir fait, mets à jour le README : « testé par
N personnes le <date> ». C'est le seul chiffre qui compte.*
