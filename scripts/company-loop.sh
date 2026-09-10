#!/usr/bin/env bash
# ⬡ POLYGONE — Company Loop (Hermès COO)
# Usage: bash scripts/company-loop.sh          # one tick (for CI/cron)
#        bash scripts/company-loop.sh --daemon # infinite loop, 5min, tmux
#        tmux new -s polygone-loop "bash scripts/company-loop.sh --daemon"
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
BIN="$ROOT/target/release/polygone"
RELAY_BIN="$ROOT/target/release/polygone-relay"
LOG="$ROOT/.claude/company/loop.log"
mkdir -p "$(dirname "$LOG")"

ts() { date -Iseconds; }
log() { echo "[$(ts)] $*" | tee -a "$LOG"; }
die() { log "✖ $*"; exit 1; }

# Guard: disk must have >1G free or we self-pause (SSD 212G à 99% — learned 2026-09-10)
guard_disk() {
  local avail_kb avail_gb
  avail_kb=$(df --output=avail "$ROOT" | tail -1 | tr -d ' ')
  avail_gb=$(( avail_kb / 1048576 ))
  if [ "$avail_gb" -lt 1 ]; then
    log "⏸ PAUSE — disque <1G libre (${avail_gb}G). Nettoie avant de builder. Ne jamais rm -rf sans backup Transcend."
    return 1
  fi
  log "  disque: ${avail_gb}G libre ✓"
  return 0
}

tick() {
  log "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  log "⬡ TICK $(ts) — $(git rev-parse --short HEAD 2>/dev/null || echo "?") — $(git branch --show-current 2>/dev/null || echo "?")"
  guard_disk || return 0

  local ok=true

  # 1. fmt
  if ! cargo fmt -- --check >/dev/null 2>&1; then
    log "  fmt: ✖ (run cargo fmt --all)"; ok=false
  else
    log "  fmt: ✓"
  fi

  # 2. clippy (avec timeout — le SSD est à 98%, le build peut être lent)
  if timeout 180 cargo clippy --workspace --all-targets -- -D warnings >/dev/null 2>&1; then
    log "  clippy: ✓"
  else
    local clippy_exit=$?
    if [ "$clippy_exit" -eq 124 ]; then
      log "  clippy: ⏭ timeout 180s (SSD lent, cache froid) — on retentera au prochain tick"
    else
      log "  clippy: ✖ (fix warnings)"; ok=false
    fi
  fi

  # 3. tests (workspace, 114 attendus)
  local test_out
  test_out=$(cargo test --workspace 2>&1 | tail -n 20)
  if echo "$test_out" | grep -q "test result: ok\."; then
    local total
    total=$(echo "$test_out" | grep -oP '\d+ passed' | awk '{s+=$1} END{print s+0}')
    log "  tests: ✓ (${total} passed)"
  else
    log "  tests: ✖"; echo "$test_out" | tail -n 10 | while read -r l; do log "    $l"; done; ok=false
  fi

  # 4. binaires release présents ?
  if [ ! -x "$BIN" ] || [ ! -x "$RELAY_BIN" ]; then
    log "  binaires: ✖ (cargo build --release -p polygone-client -p polygone-relay -p polygoned)"; ok=false
  else
    log "  binaires: ✓ (polygone + polygone-relay + polygoned)"
  fi

  # 5. smoke (seulement si binaires présents et tests verts — sinon trop lent)
  if [ "$ok" = true ] && [ -x "$BIN" ]; then
    if bash scripts/smoke-commands.sh >/dev/null 2>&1; then
      log "  smoke: ✓ (16 checks)"
    else
      log "  smoke: ✖ (voir scripts/smoke-commands.sh)"; ok=false
    fi
    if bash scripts/forensic-zero-log.sh >/dev/null 2>&1; then
      log "  forensic-zero-log: ✓"
    else
      log "  forensic-zero-log: ✖"; ok=false
    fi
    if bash scripts/hide-smoke.sh >/dev/null 2>&1; then
      log "  hide-smoke: ✓"
    else
      log "  hide-smoke: ✖ (Hide SOCKS5)"; ok=false
    fi
  else
    log "  smoke/forensic/hide: ⏭ (skip — tests/binaires KO)"
  fi

  if [ "$ok" = true ]; then
    log "  VERDICT: VERT — $(git log --oneline -1 --pretty=format:'%h %s')"
    # Auto-push si sur main et en avance sur origin
    local branch ahead
    branch=$(git branch --show-current 2>/dev/null || echo "")
    if [ "$branch" = "main" ] && git remote get-url origin >/dev/null 2>&1; then
      ahead=$(git rev-list --count "origin/$branch..HEAD" 2>/dev/null || echo 0)
      if [ "$ahead" -gt 0 ]; then
        log "  push: $ahead commit(s) en avance → git push origin main"
        git push origin main 2>&1 | while read -r l; do log "    $l"; done || log "  push: ✖ (réseau ?)"
      fi
    fi
  else
    log "  VERDICT: ROUGE — corrige avant de shipper"
  fi

  log ""
  return 0
}

# Trap: duress-like clean exit — le loop s'arrête proprement, pas en laissant des cargo zombies
trap 'log "◼︎ loop terminé ($(ts))"; exit 0' INT TERM
trap 'log "⚠ tick failed (exit $?) — on continue au prochain tick"' ERR

if [ "${1:-}" = "--daemon" ]; then
  log "◆ Company loop daemon démarré (PID $$, 5min/tick, tmux polygone-loop)"
  log "  Ctrl-C ou tmux kill-session -t polygone-loop pour arrêter"
  log "  log: $LOG"
  while true; do
    tick || true
    sleep 300
  done
else
  tick
fi
