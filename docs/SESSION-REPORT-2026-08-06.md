# SESSION REPORT — 2026-08-06 · Polygone v2.0.0-rc2 + système

> *Session autonome longue (loop /loop 5m). Tout ce qui a été fait, dans
> l'ordre, avec l'état vérifié. Source de vérité pour la reprise.*

---

## 1. PRODUIT — Polygone v2.0.0-rc2 (le « truc de fou »)

### État vérifié
- **119 tests** (`cargo test --workspace`) · 0 warning · fmt propre
- Binaire : `polygone` (alias `polygone-client`) + `polygone-relay` + `polygoned`
- Repo : `~/Projets/Polygone-v2` → GitHub `lvs0/Polygone-Network` (60 commits)

### Ce qui a été construit (dans l'ordre)
1. **Crypto core complet** — `polygone-core` porte enfin le SPEC :
   `crypto/kem.rs` (ML-KEM-1024, FIPS 203) · `shamir.rs` (4/7) ·
   `symmetric.rs` (AES-256-GCM) · `SharedSecret` + KDF BLAKE3 ·
   `sign.rs` (ML-DSA-65, FIPS 204 — build réparé, il était rouge)
2. **Démo E2E** (`polygone demo`) — relay aveugle + audit « on voit rien » +
   simulation d'adversaire (3/7 et 7/7 sans clé) + signature ML-DSA
3. **D1 GO** — TUI 2 commandes style vim (`:envoyer`/`:quitter`), événementielle
4. **Messagerie + Drive réels** — `msg.rs` (wire `KEM_CT/SENDER_PK/FRAG`
   interopérable) · fichiers via `--fichier`, reçus dans `~/.polygone/received/`
5. **Réseau réel (plane 2)** — relay TCP qui route par node (`HELLO <node_id>`,
   NDJSON, ne lit que kind/to/session) · `ecouter` · tests d'intégration
6. **Mesh (Phase 4)** — `voisins`/`annoncer` : découverte LAN UDP (7642),
   zéro dépendance · **envoi zéro-config** (`--a <node>` sans `--via`)
7. **RES** — `compute` : nœuds fantômes (RAM live dans l'annonce) · prêt
   (`--emprunter` → grant via relay) · **exécution sandboxée**
   (`--executer` → systemd-run : MemoryMax 256M, NoNewPrivileges,
   ProtectSystem=strict, PrivateTmp, PrivateNetwork, CPU 50 %)
8. **IA locale** — `petals status/models/ask` (Ollama, client HTTP minimal
   sans dépendance + décodage chunked) · `:ia <q>` dans la TUI
9. **Kill-switch réel (Axiome 5)** — `polygone duress [--confirmer]`
10. **`polygone test`** — self-test crypto 7/7 (utilisé par le CI)
11. **Produit** — installer 1-clic (`scripts/install.sh`, chemin précompilé
    + fallback build) · landing `site/index.html` (design-system slate+ambre,
    zéro dépendance) · `scripts/demo.sh` (7 étapes réelles) ·
    `docs/cli.md` · `docs/STRATEGIE.md` (notes Bear consolidées)
12. **Docs S2** — `threat-commodity.md` + `threat-high-value.md` (en retard
    depuis le 13/07, livrés) · `kill-switch.md` v1.0 + runbook ·
    `config.md` · `CHANGELOG.md` v2.0.0-rc2
13. **CI/Release** — `ci.yml` aligné produit (artefacts réels, workspace
    tests, `polygone test`) · `release.yml` (tarballs `polygone-<os>-<arch>.tar.gz`)
    · version workspace `2.0.0-rc2`
14. **D2** — bench handshake enregistré dans `DECISIONS.md` : sign ~265 µs
    (goulot), verify ~79 µs, ~2900 handshakes/sec/cœur — gate 200 µs non
    atteint, capacité non-bloquante ; retour ML-DSA-87 serait PLUS lent.
    Décision finale : à Lévy.

### Ajouts ultérieurs (après la rédaction initiale)
- **Exécution WASM** — `polygone compute --wasm <fichier.wasm>` : le module
  (base64) traverse le relay et tourne dans le **sandbox wasmi** du fantôme
  (memory-safe, vérifiable). wasmi/wasmi_wasi ajoutés (crates.io était revenu).
- **`:wasm <fichier>` dans la TUI** — l'exécution WASM native au produit.
- **Réputation des fantômes** — `~/.polygone/reputation.json` (ok/fail/score),
  affichée dans `polygone compute` (« réputation 100% »). La couche de
  confiance RES est livrée ; les reçus signés ML-DSA restent Phase 8+.

### Commandes produit (résumé)
```
polygone                     → TUI (10 commandes derrière :)
polygone demo                → démo E2E + audit
polygone test                → self-test crypto 7/7
polygone clef / id           → identité
polygone envoyer [-d clef] [--via r --a n] [--fichier p] "msg"
polygone recevoir [fichier]  → reconstruire + déchiffrer
polygone ecouter [--annoncer] [--compute]
polygone voisins / annoncer  → mesh LAN
polygone compute [--emprunter n] [--executer "cmd" --emprunter n]
polygone petals status|models|ask
polygone duress [--confirmer]
```

---

## 2. GITHUB — état et nettoyage

- **Panne GitHub « Partial System Outage »** depuis ~18:00 (7h30+ au moment
  de l'écriture). Actions bloquées (CI/Release en file), git protocol OK.
- **Historique nettoyé** : 52 → **40 commits** (15 commits v1 squassés en
  « legacy: v1 heritage », arbre vérifié identique) · force-push accepté
- **Branches mortes supprimées** : `advanced`, `analyser-tous-les-fichiers-repo-31dbc`,
  `produit-grand-public-final-5dd4d`, `src`
- **En attente** : CI + Release `v2.0.0-rc2` (le tag pointe sur le commit
  corrigé cf6fba3→ désormais l'historique réécrit) · l'installateur
  « précompilé » dépend de la release GitHub
- ⚠️ Le token GitHub est dans l'URL du remote (`git remote get-url origin`).
  À révoquer/limiter si non voulu.

---

## 3. SYSTÈME — conflits réglés + Hyprland

1. **zram surdimensionné** (`ram * 2` = 15 G sur 7,5 G) → `ram * 0.75` =
   5,6 G. Erreurs d'écriture swap (zram0) réglées ; le swap disque (8 G)
   prend le relais.
2. **Waybar** chargeait l'ancien `config` (barre bas) → `config.jsonc`
   (barre pill haut) via `exec-once = waybar -c …`. Virgule finale corrigée.
3. **bemenu** (absent) → **wofi** partout (lanceur `SUPER+ESPACE`, waybar
   launcher, power menu `SUPER+P`).
4. **brightnessctl** installé (touches luminosité + module waybar).
5. Chemin durci `palantir/web/app.py` supprimé du hyprland.conf.
6. **GDM → `DefaultSession=hyprland.desktop`** — au prochain login, plus
   de GNOME (économise de la RAM sur 7,5 G).
7. **hyprpaper.conf** créé (wallpaper depuis `~/Images/`).
8. **wofi-power-menu** créé (`~/.local/bin/`).
9. Config Hyprland complétée : volume (`XF86Audio*`, `SUPER+F2`), luminosité,
   média (`playerctl`), plein écran (`SUPER+F`), flottant (`SUPER+T`),
   split (`SUPER+J`), capture (`PRINT` → presse-papiers).
10. **SSD Transcend monté** en lecture seule : `/mnt/transcend`
    (`/dev/sdb1`, NTFS) — home_backup_20260709, arch-portable-backups,
    STARMANIA… accessibles.

---

## 4. POUR REPRENDRE (prochaines pistes)

1. **Quand GitHub respire** : surveiller CI (`fmt/clippy/build/test/
   crypto-selftest`) et Release `v2.0.0-rc2` (tarballs). Point de vigilance :
   build macOS. Corriger tout échec.
2. **Installer précompilé** : tester `curl …/install | bash` avec la vraie
   release (chemin précompilé).
3. **Exécution RES WASM** (wasmi/wasmi_wasi) + réputation — crates.io était
   injoignable pendant la panne, à réessayer.
4. **`:executer <node> <tâche>`** explicite dans la TUI (déjà partiellement
   supporté — node préfixé).
5. **Decision D2** (à Lévy) : garder ML-DSA-65 + réviser la cible ≤ 400 µs,
   ou valider ~2900 handshakes/sec.
6. **Hyprland** : tester la session au prochain login ; logs dans
   `~/.local/share/hyprland/` si un module coince.

---

*Rapport de session · 2026-08-06 · « On voit rien. Et c'est comme ça que ça devrait être. »*
