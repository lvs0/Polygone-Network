# PLAN_CONSOMMATEUR_MAJ — qui consomme le socket, après 2b-9

> Incrément 2b-10a, 30/09/2026. **Aucun code, un seul fichier écrit.** État de départ : 2b-9
> (98 tests verts, `clippy -D warnings` et `fmt --check` propres). Ce document **remplace**
> `docs/PLAN_CONSOMMATEUR.md` sur les seuls points listés au §3 ; tout ce qui n'y est pas
> contredit reste valide. Chaque affirmation cite fichier + lignes.

## 1. `crates/gateway` — ce qu'il contient

**1.1 Le paquet.** `polygone-gateway` (`crates/gateway/Cargo.toml:2`), binaire
`polygone-gateway` sur `src/main.rs` (`:8-10`), lib `src/lib.rs` (212 l.). **Membre du
workspace** (`Cargo.toml:8`). Douze dépendances (`:13-24`) dont **cinq jamais appelées** :
`polygone-core` (`:13`), `hex` (`:20`), `base64` (`:21`), `qrcode` (`:22`), `image` (`:23`) —
`grep` de chacune dans `crates/gateway/src/` → 0 usage. `qrcode` + `image` trahissent une
fonctionnalité QR jamais écrite.

**1.2 Ce qu'il fait.** Une passerelle HTTP/1.1 écrite à la main sur tokio, écoutant
`127.0.0.1:8787` **en dur** (`src/main.rs:14`), une tâche par connexion (`lib.rs:65-77`). Six
routes (`lib.rs:3-9`, `:96-179`) : accueil, état de réseau, deux routes d'appairage, QR, info
nœud. L'état vit en mémoire (`lib.rs:47-53`). Ce n'est **pas** une API de calcul : aucun
travail lourd n'y passe.

**1.3 Parle-t-il au daemon ? Non — pas un mot.**
`grep -rniE "daemon|socket|polygoned|\.polygone|cgroup|alloc" crates/gateway/` → **sortie
vide, RC=1**. Aucun chemin, aucun `POLYGONE_DAEMON_SOCKET`, aucun protocole. C'est une seconde
pile réseau, indépendante de celle du daemon.

**1.4 Appelants et tests.** Aucun appelant Rust : le nom n'apparaît que dans `Cargo.lock:3247`
et une phrase de vision (`docs/VISION-COMPLETE.md:19`). **Aucun test** : `grep -rn
"#\[test\]|#\[tokio::test\]" crates/gateway/` → vide, et pas de `tests/`. Comme
`crates/petals/`, il est **non suivi par git** (`git status` → `??` pour les deux).

**1.5 `cargo check -p polygone-gateway`** → **RC=0** :

```
    Checking polygone-gateway v2.0.0 (/home/lvs0/Projects/Polygone-real/crates/gateway)
    Finished `dev` profile [unoptimized] target(s) in 22.45s
CHECK_RC=0
```

Aucun avertissement : les cinq dépendances mortes ne produisent aucun diagnostic.

## 2. Comparaison honnête des trois candidats

| Candidat | État | Ce qu'il faudrait faire | Ce que ça coûte |
|---|---|---|---|
| `crates/gateway` | compile (RC=0) et **démarre** ; mais ne parle pas au daemon (1.3), 0 test, 0 appelant, 5 deps mortes, doc interne fausse (`lib.rs:7` annonce « QR SVG », `:143` rend du JSON) | lui greffer un client v1 **et** inventer une charge à limiter | peu de lignes, **aucune valeur** : la passerelle ne calcule rien, une allocation n'y aurait rien à contraindre ni à observer |
| `crates/petals` | **mort** : pas de `[[bin]]` (`Cargo.toml:11-16` : lib + bench), 0 test, 0 appelant, client faux (`src/daemon.rs:20-21` `/tmp/polygoned.sock` ; `:187` `#[serde(tag="type")]` sans `v` ; `:188` `AllocateResources`) | porter `daemon.rs` sur v1, puis écrire un hôte | remplacer ~50 lignes de client, écrire un binaire, et un test de bout en bout |
| rien — consommateur neuf | n'existe pas | écrire le premier vrai consommateur | tout ; sans charge réelle choisie, ça reste un jouet |

**Ferme-t-il la boucle, ou la déplace-t-il ?** `gateway` : **la déplace** — il répondrait
« oui » à une allocation sans rien faire de contraint, et « qui consomme ? » resterait ouvert.
`petals` : **la ferme, seul candidat** — l'inférence est la charge réelle pour laquelle le
daemon existe (CPU/RAM/GPU), donc une allocation y a un effet observable. Consommateur neuf :
la déplace vers « quelle charge ? ».

## 3. Ce qui est FAUX dans `docs/PLAN_CONSOMMATEUR.md`

Le doc a été écrit **avant** 2b-6, 2b-7a/b et 2b-9 ; vérifié, ces passages ne sont plus vrais.

- **§1 « le consommateur prévu ».** Petals reste mort (§2), mais §1 ignore qu'**un client de
  référence existe maintenant** : `daemon/src/client.rs` (637 l. — `connect:91`,
  `connect_with_timeout:106`, `request:148`, `status:195`, `allocate:204`, `release:219`),
  exporté (`daemon/src/lib.rs:26`) et utilisé par la CLI (`cli.rs:317`, `:333`). « Le
  consommateur prévu » n'est donc plus le bon point de départ.
- **§2 « personne ne parle le protocole v1 ».** Vrai pour tout consommateur **externe** (petals,
  gateway : 0 occurrence), **faux depuis 2b-7a/2b-9** pour le daemon lui-même : sa CLI le parle.
- **§3 ligne 1 « `~/.polygone/daemon.sock`, sans env »** → **faux**. `ENV_SOCKET_PATH =
  "POLYGONE_DAEMON_SOCKET"` (`socket.rs:39`), **prioritaire** (`:45`), appliqué par
  `socket_path_from` (`:52`, `:62`), testé (`:1493`). Ajouté en 2b-6. Le désaccord de chemin
  devient un désaccord de **défaut**, réglable par variable des deux côtés.
- **§3 ligne 9 « subscribe/unsubscribe répondent mais TickUpdate n'est jamais émis »** →
  **faux** : les trois ont été **retirés** en 2b-6 ; il n'en subsiste qu'un commentaire
  (`socket.rs:97-99`).
- **§3 ligne 10 « Release n'efface rien, AllocGranted ne réserve rien »** → **faux**. `Ledger`
  (`socket.rs:451`) réserve (`:483`) et libère (`:500`) ; la décision se prend contre la marge
  **moins** les réservations (`:581`, `:588`) ; `LedgerGuard` (`:931`) libère à la fermeture de
  connexion (`:962`).
- **§5 A.1, A.2, A.3** → **faits** (2b-6) ; ne pas les reprendre. Ce qui reste vrai : **l'incrément
  B / 2b-7**, le consommateur, jamais commencé (`crates/petals/` porte encore la date du 03/09).
- **Découverte #1 du doc** (« une seule ligne, dans `archive/…/polygone-install.rs:72` ») est
  **imprécise** : cette ligne est une **chaîne** `"polygone-petals",` dans une liste
  d'installation, pas un appel de `PetalsEngine`. Aucun appelant, donc — mais pas « un appel ».

## 4. Recommandation — qui consomme, et pourquoi

**`crates/petals`, porté sur le protocole v1, doit consommer.** C'est le seul des trois dont le
travail consomme réellement des ressources (inférence CPU/RAM/GPU), donc le seul où une
allocation accordée peut être **observée**. `gateway` ne calcule rien : lui donner v1
produirait une boucle qui tourne à vide, et un consommateur neuf n'aurait pas plus de charge à
confier. Concrètement : réutiliser le client de référence au lieu d'en réécrire un (le paquet
du daemon expose `client`, `daemon/src/lib.rs:26`) ; écrire un petit hôte binaire qui demande,
travaille, puis libère ; et un test qui **prouve** que le refus est respecté. `gateway` garde sa
place de passerelle HTTP — pas celle de consommateur du daemon.

> **Suite :** l'architecture réseau (ce qui se partage, les trois impossibilités chiffrées, et la première
> brique à coder) est tranchée dans `docs/ARCHITECTURE_RESEAU.md` (2b-11a). Le présent document reste la
> référence sur le *consommateur* ; l'autre tranche la *ressource* — et confirme le choix fait ici.

## Découvertes inattendues

**La condition de déclenchement n'est PAS remplie, et c'est un résultat.** Cette section était
obligatoire « si `gateway` parle au daemon, **ou** s'il est plus mort que `petals` » : ni l'un ni
l'autre.

1. **`gateway` ne parle pas au daemon** (1.3, grep vide). Ce n'était pas acquis : membre du
   workspace (`Cargo.toml:8`) et nommé « gateway », il avait tout pour être le chaînon manquant.
2. **Il n'est pas plus mort que `petals`** : il compile (RC=0) et son binaire **démarre**, là où
   petals n'a aucun binaire. Et le critère « jamais commité » **ne les sépare pas** — les deux
   sont `??`. Je croyais tenir là une distinction ; la vérification m'a démenti.
3. **Son propre code se décrit faux.** `lib.rs:7` annonce « QR SVG deep link » ; `:143` répond du
   JSON. `qrcode` + `image` (`Cargo.toml:22-23`) sont déclarées et jamais appelées : annoncé,
   puis oublié.
4. **Ses métriques sont des fictions.** `latency_ms_median: 42` en dur (`lib.rs:107`),
   `active = nodes.len()` (`:106`), et `uptime_seconds` est en réalité l'horodatage Unix
   (`:108-110`) — pas une durée de fonctionnement.
5. **Il répond `200 OK` à tout** (`lib.rs:189`), y compris « not found » (`:183`), et lit la
   requête par `peek` sur 4096 octets (`:81`) : le corps d'un POST plus gros n'est jamais lu.
6. **`GET /` est cassé par défaut** : `main.rs:12` vise `docs/site`, qui **n'existe pas**
   (`ls -d docs/site` → absent) ; le site est à `docs/index.html` (24 746 o), un niveau au-dessus.
