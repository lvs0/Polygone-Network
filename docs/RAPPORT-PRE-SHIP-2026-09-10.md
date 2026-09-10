# Rapport pré-ship — Polygone 2026-09-10 (SANS PUSH)
> Préparé par subagent Ship+Lawyer — **aucun push exécuté**, aucune publication, diffs proposés uniquement. À relire/valider avant `git push`.

---

## 1. Vérifications `git` / `gh` (lecture seule)

**`git remote -v`**
```
origin  https://github.com/lvs0/Polygone-Network.git (fetch)
origin  https://github.com/lvs0/Polygone-Network.git (push)
```

**`gh auth status`**
```
github.com
  ✓ Logged in to github.com account lvs0 (keyring)
  - Active account: true
  - Git operations protocol: https
  - Token: github_pat_*** (keyring)
```

**`git branch -vv`**
```
  chore/remove-v2 a0b03e5 chore(branding): remove V2 notion
* main            a0b03e5 fix(clippy): gateway late init + fmt   [ahead 1 vs origin/main 84c8e12]
  wip/ship-v1.0.0 f72be56 [origin/wip/ship-v1.0.0]
```
- `origin/main` = `84c8e12 fix(ci): Windows — remove unused NetInterface import`
- `main` local = `a0b03e5` → **1 commit d'avance** sur origin/main, pas 38. Le "38 commits à a0b03e5" du brief est obsolète/incohérent avec l'état réel (fetch du 10/09: 0 commit en retard). Si l'intention était de pousser 38 commits cumulés depuis un tag, vérifier le tag de référence.

**`git status --short`**
```
 M SPEC.md
?? GOVERNANCE.md
?? docs/RAPPORT-PRE-SHIP-2026-09-10.md  (ce fichier, non commité volontairement)
```
- Aucun push exécuté. `git push --dry-run` non lancé non plus pour éviter toute fuite CI.

---

## 2. `install.sh` — audit SHA256 & fix fail-closed proposé

### 2.1 Fichiers audités
| Fichier | Rôle | Vérif SHA256 actuelle |
|---------|------|----------------------|
| `Projets/Polygone-v2/install.sh` (11541 o, `curl|bash` universel) | `fetch_release()` télécharge `polygone-<os>-<arch>.tar.gz` depuis `github.com/lvs0/Polygone-Network/releases` | **Aucune vérif** — `curl ... -o /tmp/asset.tar.gz && tar -xzf` direct, fail-open total |
| `Projets/Polygone-v2/scripts/install.sh` (5795 o, one-click) | `install_binary()` télécharge `polygone-<os>-<arch>` binaire unique | **Fail-open** : le `curl .sha256` est optionnel — si le serveur ne renvoie pas de `.sha256`, le binaire est installé sans vérif (branche `else` inexistante → skip silencieux) |
| `/home/l-vs/install.sh` (MovieBox-Tui, hors scope Polygone) | installateur MovieBox | Non audité pour Polygone |
| `.github/workflows/release.yml` | publie `polygone-*.tar.gz` + `SHA256SUMS` global | Ne publie **pas** de `.sha256` par asset → incompatible avec `scripts/install.sh` qui attend `${url}.sha256` |

**Conclusion** : P8 n'est pas rempli. Les deux installateurs sont fail-open. La release ne fournit pas l'artefact attendu par le script.

### 2.2 Fix proposé — principe fail-closed
- Si la somme attendue est absente ou invalide → **refus d'installer**, message explicite, suppression du fichier téléchargé, `exit 1`/`return 1`.
- Le fallback "compilation from source" reste possible mais ne s'exécute que *après* un échec vérifié, jamais comme contournement silencieux d'un MITM.
- Support des deux formats : `SHA256SUMS` (release actuelle) + `.sha256` par asset (compat).

### 2.3 Diff proposé — `install.sh` (universel, racine)

```diff
--- a/install.sh
+++ b/install.sh
@@ fetch_release() @@
 fetch_release() {
     local platform=$1
     local tag=$2
     local asset_name
+    local tmp_dir="/tmp"
 
     case "$platform" in
         linux-x86_64)     asset_name="${BINARY_NAME}-${platform}.tar.gz" ;;
         linux-aarch64)    asset_name="${BINARY_NAME}-${platform}.tar.gz" ;;
         macos-x86_64)     asset_name="${BINARY_NAME}-${platform}.tar.gz" ;;
         macos-aarch64)    asset_name="${BINARY_NAME}-${platform}.tar.gz" ;;
         *)                return 1 ;;
     esac
 
     local url
     if [[ "$tag" == "latest" ]]; then
         url="https://github.com/${REPO}/releases/latest/download/${asset_name}"
     else
         url="https://github.com/${REPO}/releases/download/${tag}/${asset_name}"
     fi
 
+    local sums_url
+    if [[ "$tag" == "latest" ]]; then
+        sums_url="https://github.com/${REPO}/releases/latest/download/SHA256SUMS"
+    else
+        sums_url="https://github.com/${REPO}/releases/download/${tag}/SHA256SUMS"
+    fi
+
     log "Tentative téléchargement release: $url"
-    if curl -fsSL -o "/tmp/${asset_name}" "$url" 2>/dev/null; then
-        tar -xzf "/tmp/${asset_name}" -C /tmp/
+    if ! curl -fsSL -o "${tmp_dir}/${asset_name}" "$url" 2>/dev/null; then
+        warn "Aucune release pré-compilée pour $platform (tag: $tag)"
+        return 1
+    fi
+    # ── SHA256 fail-closed ──────────────────────────────────
+    log "Vérification SHA256 (fail-closed)..."
+    local sums_file="${tmp_dir}/SHA256SUMS"
+    if ! curl -fsSL -o "$sums_file" "$sums_url" 2>/dev/null; then
+        err "SHA256SUMS introuvable à $sums_url — refus d'installer (fail-closed)."
+        err "Attendez la release signée ou compilez depuis source: git clone && cargo build --release"
+        rm -f "${tmp_dir}/${asset_name}" "$sums_file"
+        return 1
+    fi
+    # extrait la ligne attendue pour cet asset (supporte format BSD/GNU)
+    local expected
+    expected=$(grep -F "  ${asset_name}" "$sums_file" 2>/dev/null | awk '{print $1}' || true)
+    if [[ -z "$expected" ]]; then
+        expected=$(grep -F "${asset_name}" "$sums_file" 2>/dev/null | awk '{print $1}' || true)
+    fi
+    if [[ -z "$expected" ]]; then
+        err "Empreinte SHA256 absente pour ${asset_name} dans SHA256SUMS — refus d'installer."
+        rm -f "${tmp_dir}/${asset_name}" "$sums_file"
+        return 1
+    fi
+    local actual
+    if has_cmd sha256sum; then
+        actual=$(sha256sum "${tmp_dir}/${asset_name}" | awk '{print $1}')
+    elif has_cmd shasum; then
+        actual=$(shasum -a 256 "${tmp_dir}/${asset_name}" | awk '{print $1}')
+    else
+        err "sha256sum/shasum introuvable — impossible de vérifier (fail-closed)."
+        rm -f "${tmp_dir}/${asset_name}" "$sums_file"
+        return 1
+    fi
+    if [[ "$expected" != "$actual" ]]; then
+        err "SHA256 invalide pour ${asset_name}"
+        err "  attendu: $expected"
+        err "  obtenu : $actual"
+        rm -f "${tmp_dir}/${asset_name}" "$sums_file"
+        return 1
+    fi
+    ok "SHA256 vérifié: $actual"
+    # ── extraction ──────────────────────────────────────────
+    if ! tar -xzf "${tmp_dir}/${asset_name}" -C /tmp/; then
+        err "Extraction échouée pour ${asset_name}"
+        rm -f "${tmp_dir}/${asset_name}" "$sums_file"
+        return 1
+    fi
-        ok "Release téléchargée et extraite"
-        echo "/tmp"
-        return 0
-    fi
-    warn "Aucune release pré-compilée pour $platform (tag: $tag)"
-    return 1
+    ok "Release téléchargée, vérifiée et extraite"
+    echo "/tmp"
+    return 0
 }
```

*Notes* : `has_cmd` déjà défini dans le fichier. Compat Linux/macOS via `sha256sum`/`shasum`. Ne pas `set -e` piéger le `grep` vide → `|| true`.

### 2.4 Diff proposé — `scripts/install.sh` (one-click)

```diff
--- a/scripts/install.sh
+++ b/scripts/install.sh
@@ install_binary() @@
 install_binary() {
     local os=$1 arch=$2
     local version="2.0.0"
-    local url="https://github.com/lvs0/Polygone-Network/releases/download/v${version}/polygone-${os}-${arch}"
-    local sha_url="${url}.sha256"
+    local base="https://github.com/lvs0/Polygone-Network/releases/download/v${version}"
+    local asset="polygone-${os}-${arch}.tar.gz"
+    local url="${base}/${asset}"
+    local sums_url="${base}/SHA256SUMS"
+    local sha_url="${url}.sha256"  # compat si .sha256 par-asset publié
 
     log_info "Tentative d'installation binaire v${version}..."
 
-    if curl -fsSL "$url" -o /tmp/polygone 2>/dev/null; then
-        chmod +x /tmp/polygone
-        
-        # Verify SHA256 if available
-        if curl -fsSL "$sha_url" -o /tmp/polygone.sha256 2>/dev/null; then
-            local expected=$(cat /tmp/polygone.sha256 | awk '{print $1}')
-            local actual=$(sha256sum /tmp/polygone | awk '{print $1}')
-            if [ "$expected" = "$actual" ]; then
-                log_info "✓ Signature SHA256 vérifiée"
-            else
-                log_error "Signature SHA256 invalide"
-                rm -f /tmp/polygone /tmp/polygone.sha256
-                return 1
-            fi
-            rm -f /tmp/polygone.sha256
-        fi
-        
-        sudo mv /tmp/polygone /usr/local/bin/polygone
-        log_info "✓ Binaire installé : /usr/local/bin/polygone"
-        return 0
+    local tmp_asset="/tmp/${asset}"
+    if ! curl -fsSL "$url" -o "$tmp_asset" 2>/dev/null; then
+        log_warn "Binaire non disponible pour ${os}-${arch}, compilation..."
+        return 1
+    fi
+    # ── SHA256 fail-closed ──────────────────────────────────
+    local expected="" actual="" sums_file="/tmp/SHA256SUMS"
+    if curl -fsSL "$sums_url" -o "$sums_file" 2>/dev/null; then
+        expected=$(grep -F "  ${asset}" "$sums_file" 2>/dev/null | awk '{print $1}' || true)
+        [ -z "$expected" ] && expected=$(grep -F "${asset}" "$sums_file" 2>/dev/null | awk '{print $1}' || true)
+    elif curl -fsSL "$sha_url" -o /tmp/polygone.sha256 2>/dev/null; then
+        expected=$(awk '{print $1}' /tmp/polygone.sha256)
+        rm -f /tmp/polygone.sha256
     fi
+    if [ -z "$expected" ]; then
+        log_error "SHA256 manquant pour ${asset} (SHA256SUMS et .sha256 absents) — refus d'installer (fail-closed)."
+        log_error "Compilez depuis source: cargo build --release --workspace"
+        rm -f "$tmp_asset" "$sums_file"
+        return 1
+    fi
+    if command -v sha256sum >/dev/null 2>&1; then
+        actual=$(sha256sum "$tmp_asset" | awk '{print $1}')
+    else
+        actual=$(shasum -a 256 "$tmp_asset" | awk '{print $1}')
+    fi
+    if [ "$expected" != "$actual" ]; then
+        log_error "SHA256 invalide pour ${asset} — attendu $expected, obtenu $actual"
+        rm -f "$tmp_asset" "$sums_file"
+        return 1
+    fi
+    log_info "✓ SHA256 vérifié: $actual"
+    rm -f "$sums_file"
+    # extraction tar.gz → binaires
+    mkdir -p /tmp/polygone-extract
+    tar -xzf "$tmp_asset" -C /tmp/polygone-extract
+    chmod +x /tmp/polygone-extract/polygone 2>/dev/null || true
+    sudo mv /tmp/polygone-extract/polygone /usr/local/bin/polygone 2>/dev/null || mv /tmp/polygone-extract/polygone /usr/local/bin/polygone
+    # autres binaires si présents
+    for b in polygone-client polygone-relay polygoned; do
+        [ -f "/tmp/polygone-extract/$b" ] && sudo mv "/tmp/polygone-extract/$b" /usr/local/bin/ 2>/dev/null || true
+    done
+    rm -rf "$tmp_asset" /tmp/polygone-extract
+    log_info "✓ Binaire installé : /usr/local/bin/polygone"
+    return 0
-    else
-        log_warn "Binaire non disponible pour ${os}-${arch}, compilation..."
-        return 1
-    fi
 }
```

*Effet* : plus de branche "vérif optionnelle" → absence = erreur fatale. Le tarball est vérifié avant extraction, compatible macOS.

### 2.5 Diff proposé — `.github/workflows/release.yml` (publier `.sha256` par asset)

```diff
--- a/.github/workflows/release.yml
+++ b/.github/workflows/release.yml
@@ release / Package tarball @@
       - name: Package tarball
         shell: bash
         run: |
           mkdir -p dist
           tar -czf "dist/polygone-${{ matrix.target }}.tar.gz" \
             -C target/release \
             polygone polygone-client polygone-relay polygoned
+          sha256sum "dist/polygone-${{ matrix.target }}.tar.gz" | awk '{print $1}' > "dist/polygone-${{ matrix.target }}.tar.gz.sha256"
+          echo "SHA256 $(cat dist/polygone-${{ matrix.target }}.tar.gz.sha256)  polygone-${{ matrix.target }}.tar.gz"
 
       - name: Upload binaries
         uses: softprops/action-gh-release@v2
         with:
-          files: dist/polygone-${{ matrix.target }}.tar.gz
+          files: |
+            dist/polygone-${{ matrix.target }}.tar.gz
+            dist/polygone-${{ matrix.target }}.tar.gz.sha256
```

Garde le job `SHA256SUMS` existant tel quel — les deux artefacts coexistent.

---

## 3. `docs/observation-premier-soir.md` — rituel 5 min testable

**État** : le fichier existe déjà (`docs/observation-premier-soir.md`, 112 lignes, modèle v1 du 2026-08-08) et est rempli avec le soir du 2026-09-02 (Halluin). **Il n'était pas manquant** contrairement au backlog — pas d'écrasement effectué dans cette passe pour ne pas perdre la preuve du 02/09.

**Squelette 5 min testable attendu** : un rituel qu'un pair non-dev peut exécuter en ≤5 min, avec critères binaires, sans interprétation.

Le modèle actuel couvre déjà les 5 min mais gagnerait à être rendu plus testable (commandes copiable-collables, timebox). Proposition d'évolution **non appliquée** (à valider avant écrasement) — diff proposé si Lévy veut un squelette vierge réutilisable :

```diff
 # Carnet d'observation — Premier Soir
-> *Modèle : v1* → *Rituel 5 min — timebox stricte, 1 carnet = 1 soir, commité le jour même*
+## Rituel 5 min (à lancer après `cargo build --release`)
+0. [30s] `polygone --version && polygoned --version && polygone-client --version`
+1. [60s] Alice: `echo "hello polygone $(date +%s)" | polygone envoyer --to Bob`
+2. [60s] Bob: `polygone recevoir --json | jq .` → vérifier TTL>0, 7 fragments
+3. [60s] Adversaire (relay): `curl -s http://127.0.0.1:9100/verite | jq .` → doit renvoyer `null`/`"rien"`
+4. [60s] Fichier: `echo test > /tmp/p.txt && polygone envoyer --file /tmp/p.txt --to Bob && cmp /tmp/p.txt <(polygone recevoir --file -)`
+5. [30s] Remplir Q1-Q5 ci-dessous, `git add docs/observation-premier-soir.md && git commit`
```

**Recommandation** : garder le fichier rempli du 02/09 tel quel comme **preuve**. Pour le prochain soir, dupliquer le modèle vierge en `docs/observation-YYYY-MM-DD.md` plutôt que d'écraser celui-ci. Si tu veux que je crée un `docs/observation-premier-soir.TEMPLATE.md` vierge, je le fais en une commande.

---

## 4. Lettres État — `docs/letters/{cnil,anssi,eff}.md`

**État** : les 3 fichiers existent déjà, datés du 2026-09-02, **≈1 page chacun, factuels**, sans promesse non tenue :
- `CNIL.md` (44 l., 2058 o) — RGPD, minimisation, relay aveugle, pas de DPO centralisé
- `ANSSI.md` (39 l., 2222 o) — primitives FIPS, modèle de menace, kill-switch, état de l'audit
- `EFF.md` (49 l., 2519 o) — liberté d'expression, métadonnées nulles, AGPL-3.0

Ils respectent le cahier des charges "1 page, factuel". Aucune réécriture n'a été poussée dans cette passe. Squelettes conformes — prêts à envoyer après relecture juridique.

**Optionnel** (proposition non appliquée) :
- Ajouter `security.txt` et PGP fingerprint réel (actuellement `0x...` placeholder dans ANSSI).
- Harmoniser le contact (`lvs0@protonmail.com` vs `security@polygone.network`) — choisir un seul.
- Dater du jour d'envoi réel, pas du 02/09 si envoi reporté.

---

## 5. TUI & CI — point rapide (hors scope direct, mais backlog)

- **TUI 2 tabs** : `polygone-client` (Envoyer/Quitter) — vérifié dans `install.sh` et `welcome()` (`polygone --help`, `polygone premier-soir`). Pas de modif proposée ici.
- **CI GitHub** : `ci.yml` + `release.yml` présents. `release.yml` publie bien `SHA256SUMS` mais pas de `.sha256` par asset → fix §2.5 ci-dessus.
- **C-PUSH-MAIN** : local `main` = `a0b03e5`, origin/main = `84c8e12` → 1 commit d'avance, pas 38. Vérifier la référence du comptage (tag `v2.0.0` ? `wip/ship-v1.0.0` ?).

---

## 6. Actions proposées (ordre) — rien n'est poussé

1. **Revoir les 3 diffs §2.3-2.5** (fail-closed + compat `shasum`). Si OK → `patch` localement, tester `shellcheck install.sh scripts/install.sh` + `curl --head` sur une release existante pour valider le chemin `SHA256SUMS`.
2. **Choisir le sort de `observation-premier-soir.md`** : garder la preuve du 02/09 et créer un `TEMPLATE.md` vierge, ou écraser avec le squelette 5 min ci-dessus.
3. **Relire les 3 lettres** côté juridique (placeholders PGP, contact unique, date d'envoi).
4. Ensuite seulement :
   ```bash
   git add install.sh scripts/install.sh .github/workflows/release.yml
   git commit -m "fix(install): SHA256 fail-closed — SHA256SUMS + .sha256, refus si absent (P8)"
   git push   # C-PUSH-MAIN — déclenche CI, à faire depuis main propre
   ```

---

## 7. Fichiers touchés / non touchés dans cette passe

- **Lus** : `install.sh`, `scripts/install.sh`, `.github/workflows/release.yml`, `.github/workflows/ci.yml`, `docs/observation-premier-soir.md`, `docs/letters/*.md` (+ `git remote -v`, `gh auth status`, `git branch/status`).
- **Créé** : ce rapport uniquement (`docs/RAPPORT-PRE-SHIP-2026-09-10.md`) — non commité volontairement, à déplacer/supprimer avant push si souhaité.
- **Non modifiés** : tous les installateurs et lettres (diffs seulement proposés), aucun `git add/commit/push` exécuté.

---

*Fin du rapport — en attente de validation avant push.*
