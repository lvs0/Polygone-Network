# Audit de véracité — Polygone v2

> Rapport produit le 2026-10-12 à partir de la collecte de preuves du sous-agent
> `deleg_c31f5e54` / task 2 (mort en HTTP 429 après 1 h 35), **reprise et
> vérifiée manuellement** (greps + lectures réexécutés sur `main` @ `060c11d`).
>
> Méthode : chaque affirmation est confrontée à `fichier:ligne`. Une
> affirmation sans preuve de code = mensonge documentaire.

---

## Verdict en une ligne

Le **code** est honnête — ses commentaires déclarent eux-mêmes ce qui est
factice. **La documentation, elle, ment** en promettant des capacités qui
n'existent pas. L'écart est concentré sur `docs/VISION-COMPLETE.md`.

---

## 1. MENSONGES DOCUMENTAIRES

### 1.1 — Serverless : « fragmentation Shamir 4-of-7 » — **FAUX**

**Promesse** (`docs/VISION-COMPLETE.md:11`) :
> Modèle : demande → fragmentation Shamir → exécution sur ≥4 nœuds → résultat reconstruit.

**Réalité** — `crates/client/src/serverless.rs:45-53` :
```rust
pub fn shards(&self) -> Vec<Vec<u8>> {
    let mut out = Vec::with_capacity(7);
    for i in 0..7 {
        let mut shard = Vec::new();
        shard.push(self.id.as_bytes()[0]);
        shard.push(i as u8);
        shard.extend_from_slice(&self.payload);   // ← payload COMPLET dans chaque shard
        out.push(shard);
    }
    out
}
```

**Ce que ça fait vraiment** : 7 copies **identiques et complètes** du payload,
chacune préfixée d'un index. **Aucun appel à `shamir::split`.** Aucun seuil
cryptographique. Ce n'est pas du partage de secret — c'est de la duplication
avec un numéro de série. Le nom `shards()` est un mensonge en soi.

**Preuve que Shamir existe mais n'est pas utilisé ici** :
- `shamir::split` / `shamir::reconstruct` sont appelés dans
  `demo.rs:222,309,326,355`, `msg.rs:122,158`, `net/mod.rs:797`, `self_test.rs:49,59`
- **jamais** dans `serverless.rs`
- `serverless.rs:3` revendique pourtant « Shamir 4-of-7 » en commentaire d'en-tête

### 1.2 — Serverless : « exécution sur ≥4 nœuds » — **JAMAIS ÉMIS**

**Réalité** : `ServerlessResult` (`serverless.rs:29-34`) est derrière
`#[cfg(feature = "experimental")]` et le commentaire l'admet :
> « Ébauche NON CÂBLÉE : personne ne construit ni ne lit un `ServerlessResult`. »

Le seul point d'entrée est `main.rs:832` → `ServerlessRequest::new(payload)`.
Il n'y a **aucun envoi aux nœuds**, aucune exécution distante, aucune
réassemblage. La chaîne s'arrête à la fabrication des pseudo-shards.

### 1.3 — Enveloppe cryptographique : « crypto » — **PLACEHOLDER**

**Réalité** — `crates/core/src/envelope.rs:1-5` :
> « Cryptography-shaped envelope — a **placeholder** for the real ML-KEM /
> Shamir work. […] We swap in real crypto at the encryption step **later**. »

Et `envelope.rs:50-58` :
```rust
/// Dummy content hash for development — real impl uses blake3.
pub fn dummy_hash(content: &[u8]) -> [u8; 32] {
    let mut h = [0u8; 32];
    for (i, b) in content.iter().enumerate() {
        h[i % 32] ^= b;          // ← XOR, PAS blake3
    }
    h
}
```

**`dummy_hash` est un XOR tronqué** — pas un hash cryptographique. Il est
utilisé en `envelope.rs:163` dans les tests. Le vrai hachage (blake3) est
utilisé ailleurs (`blocks.rs:77` via `Receipt::hash_payload`), mais l'enveloppe
qui « circule sur le réseau » porte un faux hash.

### 1.4 — `docs/site/` : annoncé, inexistant

**Promesse** (`docs/VISION-COMPLETE.md:28`) :
> Hébergement statique : `docs/site/` + CI deploy vers Pages/Vercel/Cloudflare.

**Réalité** : `docs/site/` **n'existe pas**. Le site est en `site/index.html`
(11 721 o) et `docs/index.html` (24 746 o). Aucune CI de deploy trouvée.

### 1.5 — PWA / Service Worker / Background sync — **PROMIS, JAMAIS ÉCRIT**

**Promesses** :
- `docs/VISION-COMPLETE.md:20` : « HTTPS + Service Worker + WebCrypto côté client »
- `docs/VISION-COMPLETE.md:32` : « PWA installable avec consentement explicite »
- `docs/VISION-COMPLETE.md:33` : « Background sync via Service Worker + Wake Lock API »

**Réalité** : recherche sur tout le dépôt → **aucun** `sw.js`,
**aucun** `service-worker.js`, **aucun** `site.webmanifest`. Les seuls
`manifest.json` trouvés appartiennent à des skills tierces
(`.agents/skills/ruflo/...`), pas au projet.

Pas de Service Worker = pas de PWA installable, pas de background sync,
pas de Wake Lock. **Trois capacités annoncées, zéro ligne de code.**

---

## 2. GARDES AFFAIBLIS

### 2.1 — `Receipt::sign` / `Receipt::verify` : jamais appelés en production

`crates/core/src/receipt.rs` implémente des reçus vérifiables ML-DSA-65.
Recherche des appelants hors tests : **aucun**.

- Seul `Receipt::hash_payload` est appelé, et uniquement pour **nommer** un
  bloc (`blocks.rs:77`) — pas pour signer ni vérifier quoi que ce soit.
- `reputation.rs:8` l'admet : « **Production (Phase 8+) would add** verifiable
  receipts ».

**Conséquence** : aucune preuve d'auteur ni de justesse n'est émise sur le
réseau. La primitive existe, elle est morte.

### 2.2 — `install_service` : défini, jamais appelé

Défini en `daemon/src/resources/linux.rs:306` et `macos.rs:263`, déclaré dans
le trait (`mod.rs:256`). Recherche des appelants : **aucun**.

Les commentaires l'assument :
- `glow_up.rs:564` : « à câbler dans `install_service` (**incrément dédié**) »
- `mod.rs:174` : « l'incrément qui câblera `install_service` »

Le daemon ne peut donc **pas s'installer comme service système**. Toute
promesse d'auto-installation / persistance est vide.

### 2.3 — `Fragment::dummy_hash` exporté publiquement

`envelope.rs:51` est `pub`. Une fonction de hash bidon est accessible depuis
l'extérieur du crate. `calc.rs:82` met en garde : « ne pas réintroduire un
second hachage ni `Fragment::dummy_hash` ». La garde repose sur la discipline
humaine, pas sur le type system.

---

## 3. CAPACITÉS ANNONCÉES, JAMAIS ÉMISES

| Capacité | Source doc | Réalité |
|---|---|---|
| Fragmentation Shamir 4-of-7 (serverless) | `VISION-COMPLETE.md:11` | duplication complète, `serverless.rs:45` |
| Exécution sur ≥4 nœuds | `VISION-COMPLETE.md:11` | non câblé, `serverless.rs:22` |
| Réassemblage de résultat | `VISION-COMPLETE.md:11` | `ServerlessResult` derrière feature, jamais instancié |
| Chiffrement ML-KEM | `envelope.rs:1` (auto-déclaré) | placeholder, XOR (`dummy_hash`) |
| Service Worker | `VISION-COMPLETE.md:20` | inexistant |
| PWA installable | `VISION-COMPLETE.md:32` | inexistant (pas de manifest) |
| Background sync + Wake Lock | `VISION-COMPLETE.md:33` | inexistant |
| Hébergement `docs/site/` | `VISION-COMPLETE.md:28` | répertoire inexistant |
| Reçus vérifiables signés | `receipt.rs` (Phase 8+) | `sign`/`verify` jamais appelés |
| Auto-installation service | `mod.rs:256` | `install_service` jamais appelé |
| Transport libp2p | `envelope.rs:62` | **mentionné en commentaire seulement**, aucune dépendance libp2p dans les `Cargo.toml` |

---

## 4. CE QUI EST VRAI (à ne pas jeter)

Pour être juste, l'audit a aussi confirmé du solide :

- **327 `#[test]`** dans le workspace, `cargo test --workspace` = 214 passés / 0 échec
- **Aucun `#[ignore]`** — pas de test désactivé pour faire passer la CI
- **Aucun `assert!(true)` / `assert_eq!(1,1)`** — pas de test fantôme
- **Shamir réel** (`sharks` 0.5) et réellement exercé dans `msg.rs`, `demo.rs`, `net/mod.rs`
- **Magasin de blocs BLAKE3 réel** (`blocks.rs`) avec vérification par hash
- **`mesh.rs`** implémente une vraie découverte UDP (port 7642), sans dépendance
- **Gateway** : les 5 endpoints déclarés (`lib.rs:102-171`) existent réellement
- **Le code se dénonce lui-même** — c'est rare et c'est une force

---

## 5. RECOMMANDATION

L'écart n'est **pas** entre le code et la réalité : il est entre **la doc et le
code**. Le remède est éditorial avant d'être technique.

1. **Réécrire `docs/VISION-COMPLETE.md`** — c'est la source quasi exclusive des
   mensonges. Marquer chaque item « fait » / « ébauche » / « cible ».
2. **Renommer `ServerlessRequest::shards()`** → `duplicate()` ou implémenter le
   vrai `shamir::split`. Le nom actuel est un mensonge dans l'API publique.
3. **Décider du sort de `dummy_hash`** — la supprimer ou la passer privée.
4. **Retirer les promesses PWA** tant qu'aucun Service Worker n'existe.
5. `docs/VISION_CANONIQUE.md:143` avait déjà raison : *« pas de manifeste
   exagéré »*. Le document le dit — puis `VISION-COMPLETE.md` l'ignore.
