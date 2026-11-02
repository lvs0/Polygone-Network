# ARCHITECTURE_RESEAU — ce qui se partage, ce qui ne peut pas, et la première brique

> 2b-11a, 30/09/2026. **Aucun code, un seul fichier écrit.** État : 2b-10a — protocole v1 complet, client de
> référence, 98 tests verts. **Mis à jour 2b-13 (01/10/2026)** : un premier consommateur de production
> existe — `polygone node servir` demande un **bail** à son daemon avant d'ouvrir une écoute, s'y
> limite tout seul et s'arrête quand le bail disparaît (`crates/client/src/node.rs`). D5 n'est donc plus
> « qui consomme ? » mais « combien d'autres ? ». **Ce qui reste ASSUMÉ** : tout ce qui touche au
> **réseau entre machines** — le test 2 machines de 2b-12c n'a jamais tourné (pas de second nœud).

## 1. Le tableau qui tranche (obligatoire)

| Ressource | Latence locale | Latence réseau minimale | Partageable ? | Pour quelle charge |
|---|---|---|---|---|
| CPU | ~0,3 ns (1 cycle à 3 GHz) | 0,2-0,5 ms (LAN) / 1-3 ms (ville) | **oui, en lots** | rendu par tranches, inférence batch |
| RAM | **50-100 ns** | 0,2-0,5 ms (LAN) / 60-150 ms (transcontinental) | **NON comme RAM** ; oui comme bloc | pré-chargement d'assets, blocs |
| VRAM / GPU | ~1 ns | comme le CPU : **le débit est le mur, pas la latence** | **lots oui, temps réel non** | rendu par lots, inférence |
| NPU | ~1 ns | idem GPU | lots | inférence |
| Bande passante | — | — | **OUI, immédiatement** | relais, proxy, cache, diffusion |
| Stockage persistant | ~20-70 µs (NVMe), 5-10 ms (HDD) | 1-20 ms | **OUI — c'est le socle** | blocs, seeds, archives |

**Sources.** RAM 50-100 ns : gist « Latency Numbers Every Programmer Should Know » (jboner/2841832), uvm.edu
`~cbcafier`. Réseau : ~0,5 ms intra-datacenter, 60-150 ms transcontinental (StreamNative) — **la ville (1-3 ms)
est un ordre de grandeur assumé**. Stockage : NVMe 20-70 µs, SATA 100-200 µs, HDD 5-10 ms (simplyblock.io).

**Un chiffre diffère de la consigne** : l'écart RAM↔réseau va de **2×10³** (LAN) à **~10⁶** (transcontinental),
pas « 10³ à 10⁵ » — c'est le transcontinental qui dépasse la borne.

## 2. Les trois impossibilités

**a) La RAM réseau n'est pas de la RAM — facteur 2×10³ à 10⁶.** Une lecture RAM coûte 100 ns, le même octet
par le LAN au mieux ~200 000 ns : aucun cache ne rattrape 10³. **À la place**, on ne lit jamais une RAM
distante — on **pré-charge un bloc immuable, on vérifie son hash, on le lit localement** (`drive.rs:28`,
`receipt.rs:54`) : le réseau *amène* le bloc, il ne se consulte pas à travers lui.

**b) GPU temps réel hors LAN immédiat : non — et c'est le débit qui tranche.** Une image 1920×1080 RGBA pèse
1920×1080×4 = **8,29 Mo** ; sur 1 Gbit/s : **66 ms** ; sur 10 Gbit/s : **6,6 ms** — or le budget à 60 images/s
est de 16,7 ms. **Même sur un LAN à 1 Gbit/s, transporter une image coûte 4× le budget** ; le calcul GPU
(1-5 ms) n'est plus le sujet. Le temps réel exige **≥10 Gbit/s et <1 ms**, soit un même commutateur : **les
clusters se font par ville et par bâtiment, et la ville est une unité de LOTS, pas de temps réel.**

**c) iPhone via PWA : relais et cache OUI, stockage OUI, calcul continu NON.** iOS ne fournit **ni Background
Sync, ni Periodic Background Sync, ni Background Fetch**, et une notification ne réveille pas de code
(mobiloud.com, « Do Progressive Web Apps Work on iOS? », 2026 : « No background sync », « No background
wake ») : le JS est gelé dès l'arrière-plan. **Plus dur que « relais/stockage OUI »** : un iPhone **ne
contribue que pendant que son écran est ouvert**. Son stockage est durable (Persistent Storage API, jusqu'à
60 % du disque depuis Safari 17) mais **non servable app fermée** — **cache opportuniste, pas nœud.**

## 3. La convergence — cinq idées, une seule primitive

**Un bloc adressé par son hash, placé chez le pair le plus proche, compté par le ledger, attesté.**

| Vue | Ce qu'elle croit être | Ce qu'elle est réellement |
|---|---|---|
| Cache de calcul | « on ne recalcule pas » | un bloc dont la **clé** contient la version du calculateur (§4) |
| Proxy / relais | « on fait passer du trafic » | le même bloc, servi au nom d'un autre |
| Streaming | « on envoie un flux » | une suite de blocs, servis dans l'ordre par le plus proche |
| Stockage | « on garde des fichiers » | les mêmes blocs, gardés plus longtemps |
| Ledger | « on réserve des ressources » | le compteur des blocs qu'on s'engage à servir |

Les pièces existent, séparées : hash et attestation (`crates/core/src/receipt.rs:8`, `:17`, `:54`) ;
proximité (`crates/client/src/mesh.rs:19`, UDP 7642, `:45`, `:11` « works on the local subnet ») ;
acheminement (`crates/relay/src/relay.rs:39`, `:45` 16 shards, `:56`) ; stockage local (`drive.rs:19-48`) ;
réservation (`daemon/src/socket.rs:451`). **Aucune ne parle aux autres** — d'où le §7. Le candidat
consommateur est tranché dans `docs/PLAN_CONSOMMATEUR_MAJ.md` §2-§4. **`docs/CON.md` ne peut pas être cité :
il n'existe pas** (Découvertes).

## 4. Le piège du cache

**Clé = hash(entrée + version du calculateur). Jamais hash(entrée) seule.** Sinon le premier résultat est servi
**pour toujours**, y compris après correction du modèle ou du code — et rien ne le signale : hash valide, bonne
entrée, résultat périmé. C'est le seul défaut qui rend un cache **pire** que pas de cache.

**Qui décide.** L'**expiration** est inscrite dans le bloc par son **producteur** : lui seul sait quand son
résultat cesse d'être valable. L'**éviction** appartient au nœud qui possède le disque (LRU + plafond
d'octets) : il paie l'espace, aucun pair distant ne doit décider de le remplir. Le **ledger ne décide pas du
contenu**, il compte des octets. Un bloc à version dépassée n'est pas évincé : il est **introuvable**.

**État au Chantier II (02/10/2026) — le cache existe (`crates/client/src/calc.rs`).** La clé est
`BLAKE3(version du calculateur ‖ entrée)` ; le résultat est un bloc content-addressed (donc **vérifié
par son hash**) relié à la clé par un index `clé→hash`. Un bloc à version dépassée devient bien
**introuvable**, comme voulu ci-dessus : le fichier reste, aucune clé ne le désigne plus. **Mais
l'index n'est pas vérifiable** — une clé sémantique ne peut pas lier le contenu de son résultat. Il
faut donc l'écrire noir sur blanc : le hash prouve l'**intégrité**, pas la **justesse**. Un pair peut
servir un résultat **faux dont le hash est valide** ; le lien entrée→résultat repose sur la parole du
pair. C'est un cache de **confiance**, pas de vérification. Seul un recalcul (qui annule le gain) ou
une signature (`receipt.rs` — qui prouve l'**auteur**, pas la justesse) le rendrait vérifiable.
**`hits_distant` restera à zéro** tant que `2b-12c` (LAN entre deux machines réelles) n'a pas tourné ;
`polygone calc --stats` l'écrit en toutes lettres (« chemin distant jamais exercé ») au lieu d'un zéro
nu qui laisserait croire à un essai qui aurait échoué.

## 5. Ce que le ledger doit devenir

`Ledger` (`daemon/src/socket.rs:451`) ne connaît que RAM et CPU : `grants: HashMap<String,(u32,u64)>`, deux
compteurs O(1) `free_ram_bytes` (`:468`) / `free_cpu_cores` (`:475`). Y ajouter **deux ressources** en
réutilisant `reserve` (`:483`), `release` (`:500`), l'**idempotence** de la libération et le `LedgerGuard`
(`:931`, `Drop` `:962`) :

- **Stockage — octets durablement engagés**, décidés contre la marge disque **moins** les réservations, comme `:581`.
- **Bande passante — débit réservé (bit/s)** : un plafond de débit, pas un volume.

**Différence à écrire avant de coder** : RAM, CPU et bande passante sont **relâchées à la déconnexion** (le
`LedgerGuard` suffit) ; le stockage **survit à la connexion et au processus**. Or `grants` est un `HashMap`
**en mémoire** (`:458`) : un redémarrage oublie tout — sans conséquence pour la RAM, **faux pour le
stockage**, où le daemon re-accorderait des octets déjà occupés. L'extension doit donc **persister les
réservations de stockage, et seulement elles**.

## 6. Le site et l'iPhone

`docs/index.html` existe (**24 746 octets**) mais **ne contient ni `manifest`, ni service worker** : les
prérequis PWA de `docs/VISION-COMPLETE.md:20` et `:33` **ne sont pas là**.

**App OUVERTE, un iPhone peut** conserver des blocs, les **servir**, relayer des fragments, tenir un cache.
**Il ne peut pas** travailler en arrière-plan, donc contribuer en continu — ni **servir quoi que ce soit app
fermée**. Le Wake Lock (Safari 18.4) ne crée pas d'exécution : il empêche l'écran de s'éteindre.

**Parcours à construire** : (1) ouvrir `docs/index.html` ; (2) « Ajouter à l'écran d'accueil » — geste
**manuel**, Safari n'a pas d'invite automatique (`VISION-COMPLETE.md:53`) ; (3) consentement explicite
« j'accepte de participer comme nœud » ; (4) **tant que la page est ouverte**, elle s'annonce dans le mesh
(`mesh.rs:19`) comme **cache/relais**, jamais comme calculateur ; (5) fermer l'app **arrête la contribution**.

## 7. LA première brique — une seule

**Le magasin de blocs adressé par hash, servi par un pair, vérifié par le hash.**

**Pourquoi elle** : c'est l'objet dont **toutes** les idées du §3 sont faites ; **faisable sans GPU, sans
cgroup, sans privilège** (un dossier + un port TCP non privilégié) ; elle **prouve du dehors** (un second
processus obtient des octets qu'il n'a jamais eus, le hash le confirme) ; elle est le **critère de sortie** du
§8 ; et elle **branche des pièces existantes** (`mesh.rs:19` trouver le pair, `receipt.rs:54` hacher,
`drive.rs:28` garder, `socket.rs:451` réserver) au lieu d'ouvrir un univers parallèle.

**Test falsifiable** — quatre étapes, chacune avec son code de sortie :
1. A stocke un bloc et affiche son hash. *(observable : le hash)*
2. B — **autre processus, autre dossier** — demande ce hash par le réseau : octets **identiques**, hash recalculé conforme. *(observable : égalité binaire)*
3. **A est tué** ; la même demande échoue. *(prouve que le bloc venait de B, pas du disque local)*
4. Le bloc est altéré côté serveur ; B **refuse** au lieu de servir. *(observable : refus)*

Sans les étapes 3 et 4, on a prouvé un `cp`. Ce sont elles qui en font autre chose qu'un jouet.

**État au 2b-13 (PROUVÉ vs ASSUMÉ).** PROUVÉ, sur **une seule machine, deux `HOME` distincts** :
les 4 étapes (2b-12b) — le bloc vient bien du processus serveur, le hash est vérifié, un bloc altéré est
refusé, et depuis 2b-13 ce service **exige un bail** avant d'ouvrir sa prise (bail refusé → port fermé)
et le rend en sortant. ASSUMÉ : le **LAN lui-même** — la découverte `mesh.rs:53` et le transfert entre
**deux nœuds physiques** n'ont pas été exécutés (2b-12c n'a pas abouti faute de second nœud). `drive.rs:28`
citée ci-dessus a été **supprimée** en 2b-12b : le gardien des blocs est `blocks.rs`.

## 8. Ce qu'on ne fait PAS maintenant

- **Streaming de jeu** : §2b — 8,29 Mo par image, 66 ms sur 1 Gbit/s.
- **VRAM partagée** : impossible *comme VRAM* (§2a appliqué au GPU), et sans intérêt en lots tant que la bande passante n'est pas réservable (§5).
- **Agents IA distribués** : ils consommeraient ce que le §7 n'a pas encore produit.
- **Marketplace** : on ne tarife pas un service qui n'a jamais été rendu une fois.

**Raison unique et suffisante : pas avant qu'un bloc soit réellement servi par un autre nœud.**

## Découvertes inattendues

**Déclencheur rempli : conflit avec ce qui est codé.**

1. **`docs/CON.md` n'existe pas** — la consigne demandait de le citer ; le fichier est absent, et aucun `.md` du dépôt ne le référence. Sources réelles : `VISION-COMPLETE.md`, `VISION_CANONIQUE.md`, `ARCHITECTURE.md`.
2. **`VISION_CANONIQUE.md:75` dit du Drive « le module de stockage et de réseau existe ».** Ce qui existe est `crates/client/src/drive.rs` (69 l.) : `received_dir:19`, `save:28`, `list:42` — **un dossier local, rien d'autre**. Un stockage *réseau* n'existe pas ; le présenter tel quel ferait bâtir le §7 sur une base imaginaire.
3. **`VISION-COMPLETE.md:28` annonce `docs/site/`, qui n'existe pas** (le site est `docs/index.html`) ; `:33` annonce « Background sync via Service Worker + Wake Lock » alors que le §2c établit qu'iOS **n'a pas** de background sync ; `:39` promet un `/api/v1/network/status` donnant « nœuds actifs, latence moyenne », or la passerelle répond `latency_ms_median: 42` en dur (`crates/gateway/src/lib.rs:107`) et `active = nodes.len()` (`:106`).
4. **Le ledger meurt avec le processus** : `socket.rs:458`, `grants` est un `HashMap` en mémoire — sans conséquence pour RAM/CPU, **faux dès qu'on y met du stockage durable** (§5).
5. **La primitive est déjà là, branchée à rien** : `mesh.rs:19` trouve les pairs du LAN, `relay.rs:45` sharde par pair, `receipt.rs:54` hache — et **aucun des trois ne parle au ledger** (`socket.rs:451`), qui ignore jusqu'à leur existence. Le premier pas du §7 n'est pas d'écrire un protocole : c'est de **reconnaître celui qui est déjà là**.
