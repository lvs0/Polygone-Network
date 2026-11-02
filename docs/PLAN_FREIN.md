# PLAN_FREIN — « utilisateur actif » : décider, pas deviner

Incrément 2b-4a : **plan de décision, aucun code écrit.** Un seul fichier créé.
Références : `DAEMON_STATE.md` §3.4 et §3.6, `docs/VISION_CANONIQUE.md`.

---

## 0. Les faits, avec preuves

**Où `user_active()` est défini, et ce qu'il renvoie.**

| Endroit | Ligne | Contenu |
|---|---|---|
| Trait `Platform` | `resources/mod.rs:248` | `fn user_active(&self) -> anyhow::Result<bool>;` |
| Impl Linux | `resources/linux.rs:297-299` | `Ok(false)` — en dur |
| Impl macOS | `resources/macos.rs:250-252` | `Ok(false)` — en dur |
| Impl Windows | — | **inexistante** : `windows.rs` n'implémente pas `Platform` (`compile_error!` dans `resources/mod.rs`) |

**Qui lit `snap.user_active` — deux freins, pas un.**

| Endroit | Ligne | Rôle |
|---|---|---|
| `policy/glow_up.rs` | **434** | le frein **vivant** : `throttle_on_user_activity && snap.user_active` |
| `allocator.rs` | **171** | un **second** frein, dans le moteur `Allocator` — **mort** (Découvertes #1) |
| `main.rs` | 350 | affichage `polygoned status` uniquement |

**Ce que fait le frein vivant** (`glow_up.rs:434-437`) : `ram_bytes` **et**
`bandwidth_mbps` × 0.5. **Le CPU n'est pas touché.** Second effet d'une chaîne de
quatre : plafond CPU (427) → frein (434) → plancher RAM (441) → plancher 64 Mio (447).

**Le champ** : `system.rs:74` `pub user_active: bool`, capturé à `system.rs:93`
par `platform.user_active().unwrap_or(false)`. Un **booléen sans état « je ne
sais pas »**, et toute erreur devient `false`.

**Le contrat documenté : aucun.** Le trait (`resources/mod.rs:244-252`) ne porte
aucun commentaire — ni sur `user_active`, ni sur ses trois voisines. La promesse
trahie n'est donc pas dans le code mais dans `VISION_CANONIQUE.md` (« sans
friction, ne gêne jamais »). Le seul commentaire existant est sur une fonction
**morte** (Découvertes #3).

**Ce qui est mesuré mais jamais consommé — §3.6 confirmé, et dépassé.** Six
réglages sont déclarés, sérialisés, documentés dans la config d'exemple, et
**jamais lus par la logique** : `min_free_cpu_cores` (`glow_up.rs:76`/défaut :87),
`min_free_vram_mb` (:77 / :88), `bandwidth_shaping` (:246 / :269 — pas même cité
par un test), `gpu_allocation_enabled` (:248 / :270), `memory_limit_enabled`
(:244 / :268 — seulement par le test :657) et `service_integration` (:250 / :271
— tests :658, :671). `DAEMON_STATE.md` en listait **quatre** ; il y en a **six**.
Seul `min_free_ram_gb` vit (`glow_up.rs:441`).

**`apply_safety` (`glow_up.rs:300-306`) ne borne QUE `max_cpu_percent`**, et son
paramètre s'appelle `_snap` : elle **ignore l'état du système** alors que sa doc
annonce « Apply safety floors … given current system state ». Aucun plancher
RAM / BW / GPU n'est appliqué.

---

## 1. Option A — `Unknown` explicite, pas de frein

`enum UserActivity { Active, Idle, Unknown }` ; `Platform::user_active()` renvoie
`Result<UserActivity>` ; Linux et macOS renvoient `Unknown` **explicitement** au
lieu de mentir par `false`. `SystemSnapshot.user_active` change de type, et
l'échec de capture devient `Unknown`. `glow_up.rs:434` traite `Unknown` d'une
façon **à décider** (§5). `doctor` et `polygoned status` affichent
`actif` / `inactif` / **`inconnu`**.

**Fichiers et lignes estimées** : `resources/mod.rs` +14, `resources/linux.rs` +3,
`resources/macos.rs` +3, `system.rs` +6, `policy/glow_up.rs` +12, `main.rs` +4,
`runtime.rs` +2, `allocator.rs` +2 (ou 0 s'il est supprimé), ~40 lignes de tests.
**≈ 45 lignes, aucun fichier nouveau, aucune dépendance.**

**Ce que ça change** : rien de fonctionnel — le frein ne freinait déjà jamais.
Mais `doctor` cesse d'afficher un « os » péremptoire sur une donnée non collectée,
et le prochain lecteur ne peut plus croire que le frein marche. **Ce que ça NE
change PAS** : le frein reste inopérant, et une valeur `Active` ne produirait
**toujours pas** la « division par 2 » promise (Découvertes #2).

## 2. Option B — une dépendance pour détecter l'inactivité

| Source | En Rust | Poids | Ce que la machine expose **réellement** |
|---|---|---|---|
| logind `IdleHint` (D-Bus **système**) | `zbus` (≈15 crates, async) ou `dbus` (liaison C) | lourd | API OK, **producteur ABSENT** : `b false`, `IdleHint=no`, et `hypridle`/`swayidle` ne sont installés nulle part |
| `ext-idle-notify-v1` | `wayland-client` + protocoles générés | lourd | annoncé par Hyprland, mais c'est un **abonnement à un événement**, pas une requête ; exige l'accès au socket du compositeur, absent d'un service utilisateur |
| X11 `XScreenSaver` | `x11rb` / `xcb` | moyen | `DISPLAY=:0` existe (XWayland) mais ne voit que les clients X, pas l'activité Wayland |
| `hyprctl` | aucun (sous-processus) | quasi nul | **inadapté** : aucune requête d'inactivité (`monitors`, `clients`, `activewindow`, `cursorpos`) |
| heuristique CPU **déjà écrite** | aucune | nul | `user_is_active()` (`system.rs:221`) « un cœur > 50 % » — déjà présente, déjà testée, déjà morte |

Sortie brute : `busctl get-property … /session/auto IdleHint` → `b false` ;
`loginctl show-session 1 -p IdleHint` → `IdleHint=no` ; `loginctl list-sessions`
→ 2 sessions, dont une `class=user` sur `seat0`, `Type=unspecified`.

**Conclusion : non — parce que ça ne fonctionnerait pas.** Le seul candidat avec
une API exploitable (logind) a un `IdleHint` que **personne ne met à jour** ici :
on paierait un client D-Bus complet pour lire `false` en boucle. Le problème n'a
jamais été la plomberie — la preuve, `system.rs:221` — mais la **source de vérité**.

## 3. Option C — frein sur la pression système (PSI)

`/proc/pressure/{cpu,memory,io}` : présents, mode **`-rw-rw-rw-`** (donc lisibles par un service utilisateur sans privilège), aucune dépendance.

```
cpu  some avg10=4.08  |  cpu full avg10=0.00 total=0      (ligne lisible, voir plus bas)
     t   cpu.some   mem.some   mem.full    io.some
     1       2.35      14.76      11.92      57.13
     8       1.91       6.63       5.35      50.26
    15       2.01       4.18       3.30      53.94
```

**Sémantique** : `some` = au moins une tâche bloquée sur la ressource ;
`full` = **toutes** les tâches non-idles le sont (arrêt complet). **Pourquoi
`some avg10` mesure mieux que « il a tapé sur une touche »** : un clavier n'est
qu'une façon d'être gêné — un build, une visio, un `rsync` subissent un manque de
CPU / mémoire / IO sans la moindre frappe. PSI mesure la **gêne subie**, pas
l'intention supposée, et `avg10` est lissé sur 10 s, donc insensible aux pics
d'un tick.

**`cpu.full` existe et est lisible** (`avg10=0.00 total=0`) mais reste
**structurellement inutilisable** : sur un CPU, il reste presque toujours une
tâche qui progresse. Un frein CPU doit lire `some`.

**Coût** : 1 à 3 lectures par tick (5 s), négligeable. **Échecs** : fichier absent
(kernel < 4.20 ou `CONFIG_PSI=n`) → retomber sur `Unknown`, jamais sur `false` ;
format susceptible de gagner des champs (`irq` existe déjà) → parseur tolérant.

**Effet sur le taux de contribution — le vrai risque.** Mesuré ici : `cpu.some`
à **1,5–2,4 %**, `mem.some` à 4–15 %, `io.some` **entre 44 % et 57 % en
permanence** (18,3 % au calme lors d'un relevé antérieur). Donc, avec un seuil de
10 % : `cpu.some` → **jamais déclenché** ; `mem.some` → ~30 % du temps ;
`io.some` → **100 %**, le frein s'activant pour du disque qui ne le concerne pas
(échange : 3,8 Gio utilisés). PSI est **disponible et honnête, mais son seuil est
une décision de politique produit** : « aucun signal » et « signal permanent »
sont deux échecs symétriques.

## 4. Comparaison

| | A (`Unknown`) | B (dépendance) | C (PSI) |
|---|---|---|---|
| **Coût** | ≈45 lignes, 0 dépendance | ≈15 crates ou liaison C + plomberie d'accès | ≈60 lignes + 1 parseur, 0 dépendance |
| **Gagné** | honnêteté : fin du `false` menteur, `doctor` vrai, frein testable | **rien ici** : producteur absent | une vraie mesure de la gêne, immédiate, bon marché |
| **Perdu** | rien (le frein était déjà inerte) | du poids pour lire `false` | un seuil à trancher ; risque de frein permanent |
| **Risque** | croire le problème réglé | croire avoir réglé alors que la source est muette | freiner à 100 % (io) ou jamais (cpu) |

## 5. Recommandation

**Option A maintenant, C ensuite, B écartée pour cette machine.**

- **A tout de suite** : elle supprime la seule chose aujourd'hui **fausse** — un
  `false` présenté comme une mesure — et rend un mensonge silencieux visible dans
  `doctor`, pour peu de code et aucune dépendance.
- **`Unknown` doit signifier « ne pas freiner »** : freiner sur une valeur
  illisible diviserait la contribution de tous au nom d'une supposition, sans
  diagnostic possible ; ne pas freiner est l'état actuel, donc aucune régression.
- **A doit embarquer le constat des Découvertes #2** : même avec un capteur
  parfait, la « division par 2 » n'aurait pas lieu. Fournir le signal seul
  laisserait la promesse cassée — c'est `smooth_transition` et sa condition
  `trend_ram() < 0` qu'il faut rouvrir.
- **C ensuite**, en mesurant d'abord `cpu.some` en production pour fixer le seuil
  sur des données réelles, et en le branchant **en complément** de A.
- **B : non** — non par principe, mais parce que le seul candidat exploitable n'a
  **aucun producteur** sur la cible. Le jour où `hypridle` sera là, une lecture
  de sous-processus (`loginctl show-session -p IdleHint`) suffira encore.

---

## Découvertes inattendues

1. **Deux freins, et le seul testé est le frein mort.** `allocator.rs:171`
   applique lui aussi la moitié, mais le moteur `Allocator` n'a **aucun appelant**
   hors de `allocator.rs` (vérifié) : le daemon utilise `GlowUpEngine`. Le test
   `test_user_active_halves_allocation` (`allocator.rs:328`) couvre donc du code
   mort — **le frein vivant (`glow_up.rs:434`) n'a, lui, aucun test.**

2. **Le frein ne divise pas par 2, et ne le ferait presque jamais même
   alimenté.** `apply_constraints` produit une **cible** ; `smooth_transition`
   (451) s'y rend par pas de `shrink_step_pct = 5 %`, et pour la RAM **seulement
   si `history.trend_ram() < 0`** (352 : `dernier − premier` sur tout
   l'historique), sinon `result.ram_bytes = current.ram_bytes` — **rien ne se
   passe**. Donc : (a) sur une machine qui monte en charge, le frein **n'entame
   jamais** la RAM, il ne peut que renforcer un repli déjà en cours ; (b) pour la
   bande passante, 5 %/tick ⇒ **≈14 ticks (≈70 s) pour atteindre la moitié**. La
   promesse est fausse pour **trois** raisons indépendantes : signal en dur à
   `false`, effet RAM conditionné par une tendance, effet BW étalé par le lissage.

3. **Une heuristique « utilisateur actif » existe déjà, complète et testée, et
   elle est morte.** `system.rs:221` :
   `pub fn user_is_active() -> bool { with_sys(|s| s.cpus().iter().any(|c| c.cpu_usage() > 50.0)) }`.
   Son seul appelant est `LegacySystemSnapshot::capture()` (249), utilisé
   **uniquement par son propre test** (`system.rs:263`) — et le struct se présente
   comme « still used by the old `allocator` module », lui aussi mort. Deux
   enseignements : la plomberie n'a jamais manqué ; et cette heuristique est un
   **proxy de charge**, donc de la famille de C — pas un capteur d'activité
   humaine, contrairement à ce que son nom promet.

4. **`apply_safety` ne fait pas ce que sa doc dit.** Elle n'agit que sur
   `max_cpu_percent`, **ignore** le snapshot (`_snap`) malgré « given current
   system state » et « Apply safety floors », et ne borne ni RAM, ni BW, ni GPU.
   Sur les quatre champs de `SafetyMargins`, un seul (`min_free_ram_gb`) vit.

5. **`Type=unspecified` dans logind** : la session de `seat0` est `Active=yes`
   mais son `Type` n'est pas renseigné — un futur lecteur d'inactivité ne pourra
   pas se fier au type de session pour savoir s'il s'adresse à un poste de travail.
