# BABEL — recherche historique et décision requise (2026-11-14)

> **Instantané daté — 2026-11-14.** Recherche documentaire, aucune
> implémentation. Voir [`ETAT-REEL.md`](../../ETAT-REEL.md) et
> [`STATE.md`](../../STATE.md). La décision `DEC-028` reste **ouverte**.

## 1. Pourquoi ce document

`ADR-014` interdit d'implémenter Babel avant clarification humaine. Les
sources demandées (`user-55`, `user-65`, `user-88`, `user-90`, `user-91`,
`user-92`, `user-93`, `vision-long.md`) ont été relues. Elles décrivent **au
moins trois mécanismes différents**, parfois incompatibles. Ce document les
sépare et formule les questions à trancher.

## 2. Sources et citations

### 2.1 « Library of Babel » comme adressage total

`Polygone-Atlas/_work/corpus/user-55.md` (l. 2933-2956, repris plusieurs fois) :

> `# Library of Babel`
> `libraryofbabel.info/#text=your-search  # trouve adresse hex`
>
> `### 2. Library of Babel Capability`
> - Recherche inversée : donne un texte → trouve l'adresse hex unique dans la bibliothèque
> - Génération test : tous les textes/images/code possibles = données pour debugging/fine-tuning
> - **Compression parfaite : stocke adresse plutôt que contenu (mapping bijectif)**
> - Applications : trouver des formules mathématiques non découvertes, valider la créativité, générer du code optimal.

`Polygone-Atlas/_work/corpus/user-90.md` (l. 864, 878, 892) :

> « Grâce à un système qui utilise le principe de la bibliothèque de Babel pour être hyper rapide, ne rien stocker et déjà avoir la réponse. »

### 2.2 Babel Memory Engine (BME) — reconstruction générative

`Polygone-Atlas/_work/corpus/user-88.md` (l. 2756-2842) :

> `# Projet : Babel Memory Engine (BME)`
> « Système de mémoire virtuelle générative par coordonnées sémantiques »
> « Contrairement à un système de fichiers classique qui stocke les données elles-mêmes, BME stocke une **représentation compacte permettant de reconstruire les données à la demande**. »
> Flux : données → analyse + encodage IA → **coordonnée Babel compacte** → SSD ; puis coordonnée → **modèle génératif** → reconstruction.
> API : `babel.save("image.png")`, `babel.load(id)`, `babel.search("animal félin")` ; index `babel_index.db`.

C'est un modèle **lossy** (encodeur/décodeur génératif), différent d'un CAS.

### 2.3 PRIME — CAS + calcul par connaissance

`Polygone-Atlas/_work/corpus/user-91.md` (l. 2037-2049, 2094, 2145, 3821) :

> Phase 1 « Bibliothèque de Babel » : `prime/babel/address.py` (adresse SHA-256 du contenu), `store.py` (`put`→adresse, `get`→reconstruction, SQLite), `compute.py` (clé canonique des entrées, cache persistant, exceptions jamais mémorisées).
> « la machine sait, elle ne calcule pas » ; « stockage par adresse + calcul par connaissance ».
> Recherche : CAS (Git/IPFS/ZFS/LLVM) et computation reuse (arXiv 2109.01608) existent séparément ; **aucun projet ne les unit**.

`Polygone-Atlas/_work/corpus/user-92.md` (l. 475) : ingestion réelle de 54 atomes par adresse SHA-256, reconstruction de 3 818 octets identiques.

### 2.4 Babel comme couche d'un OS résiduel / Faradey

`Polygone-Atlas/_work/corpus/user-65.md` (l. 5361) :

> « Faradey un os non basé sur le temps et résiduel utilisant la bibliothèque de babel pour performer […] il faudra sur indigo créer une sorte de pilote à installer de la bibliothèque de babel pour ensuite avoir notre patch de nouveau calculateur. »

### 2.5 Babel comme stockage/confidentialité par fragmentation

`Polygone-Atlas/_work/full-pass-20261107/founder-direct-113.md` (message 182434, 2026-08-22) :

> « le calcul résiduel donc le système apprend de tout […] comme une bibliothèque de Babel […] c'est beaucoup plus facile pour le stockage pour la confidentialité, y a toujours la fragmentation en plusieurs artefacts. »

Cette formulation mélange adressage, apprentissage, fragmentation et confidentialité ; elle ne définit pas le mécanisme.

## 3. Les trois modèles, séparés

| Modèle | Ce qui est stocké | Reconstruction | Perte | Ce qu'il faudrait prouver |
|---|---|---|---|---|
| **A. Adressage total** | une adresse hexadécimale dans un espace de tous les textes possibles | retrouver le texte à partir de l'adresse | nulle si l'encodage est bijectif | que l'adresse d'un contenu utile est **calculable** sans parcourir l'espace ; que les fichiers arbitraires sont encodés sans ambiguïté |
| **B. CAS + calcul** | le contenu lui-même, nommé par hash ; le résultat de calcul nommé par clé canonique | lecture du bloc / rejeu du cache | nulle | déduplication, intégrité, persistence, coût de stockage réel |
| **C. Reconstruction générative** | un vecteur/coordonnée latent compact | décodage par modèle génératif | **oui**, contrôlée | qualité, coût du modèle, déterminisme, sécurité, absence d'hallucination |

Ces modèles ne sont pas interchangeables. Le modèle A ne « compresse » pas un
fichier donné : il déplace le problème vers la recherche de son adresse. Le
modèle C ne garantit pas la restitution exacte. Le modèle B est le seul qui
donne une intégrité bit-à-bit, et il est déjà partiellement présent dans
Polygone (`blocks.rs`, `calc.rs`) et dans PRIME.

## 4. Évaluation technique

### Ce qui est mathématiquement défendable

- Une bibliothèque de Babel au sens strict est l'ensemble des chaînes de
  longueur fixe sur un alphabet fini. Elle est **finie** pour une longueur
  donnée. Elle ne contient pas « tous les fichiers » sans encodage préalable.
- L'adresse est une position dans cet espace. Elle n'est pas une compression
  du contenu : pour retrouver un texte utile, il faut déjà savoir où il est,
  ou le rechercher.
- Un hash cryptographique (BLAKE3/SHA-256) est une **adresse de contenu**
  calculable, mais ce n'est pas une bibliothèque de Babel : il ne permet pas
  d'énumérer les contenus, et il ne reconstruit rien sans le contenu.
- Le « calcul par connaissance » (PRIME) est un cache de résultats indexé par
  entrées canoniques. C'est utile, mesurable et sûr si les entrées sont
  normalisées et les exceptions exclues. Cela ne rend pas le calcul infini.

### Ce qui n'est pas défini

- Le format d'adresse (hex, hash, coordonnée latente ?).
- La structure de données et l'index.
- La distribution et la réplication.
- La reconstruction exacte ou approchée.
- La tolérance aux pannes et la disponibilité.
- La confidentialité (chiffrement au repos, fragmentation, accès).
- Les coûts de transfert/récupération et le GC.
- Le rapport avec `blocks.rs`/`calc.rs` existants.

## 5. Décision requise (`DEC-028`)

Choisir **une** direction :

1. **A — Babel = adressage total** : spécifier un encodage bijectif et une
   méthode de recherche d'adresse ; accepter que ce n'est pas une compression
   magique. Risque : complexité mathématique et utilité réelle incertaine.
2. **B — Babel = CAS + calcul par connaissance** : reconnaître que c'est déjà
   `blocks.rs` + `calc.rs` (et PRIME), consolider l'existant, ne pas créer un
   second stockage. C'est la voie la plus compatible avec le code actuel.
3. **C — Babel = reconstruction générative** : spécifier un encodeur/décodeur,
   une tolérance de perte, un modèle et un coût ; ce n'est pas un stockage
   fidèle et cela introduit des risques d'hallucination.
4. **D — Babel reste une métaphore** : ne rien implémenter, documenter
   l'intention et passer à autre chose.

**Recommandation technique, sans engager la décision humaine** : B est la seule
voie qui réutilise l'existant sans promesse non prouvée ; A et C exigent une
recherche et un prototype mesurable avant toute intégration.

## 6. Spécification minimale si B est choisi

Si l'humain choisit B, le contrat vérifiable serait :

- `address(content) = BLAKE3(content)` (ou SHA-256 si interop PRIME) ;
- `put(content) -> address`, `get(address) -> content` avec vérification du
  hash à la lecture ;
- `compute(key, fn)` avec clé canonique (entrées normalisées, version,
  paramètres triés), résultat cache, exceptions jamais cachées ;
- stockage local d'abord, réplication explicite ensuite ;
- test de reconstruction bit-à-bit, test de corruption, test de collision
  improbable, test de cache hit/miss ;
- aucune revendication de compression, de stockage infini ou de recherche
  exhaustive.

Cette spécification ne doit pas être écrite comme « Babel historique » avant la
décision : elle décrit un CAS + cache, pas la bibliothèque de Babel.

## 7. Questions à l'humain

1. Babel désigne-t-il le **stockage par adresse** (CAS), le **calcul par
   connaissance** (cache), la **reconstruction générative**, ou la métaphore ?
2. Si c'est l'adressage total : quel alphabet, quelle longueur, quel encodage
   des fichiers, et comment trouve-t-on l'adresse d'un contenu utile ?
3. Si c'est la reconstruction générative : quelle perte est acceptable, et
   comment empêche-t-on une reconstruction fausse d'être présentée comme
   exacte ?
4. Babel doit-il être **dans Polygone** ou rester dans PRIME/Faradey ?
5. Accepte-t-on que le relais voie les commandes/résultats Compute tant que le
   chiffrement de bout en bout n'est pas fait ?

Tant que ces réponses ne sont pas écrites, **aucun code Babel ne sera ajouté**.
