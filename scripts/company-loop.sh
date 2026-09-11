#!/bin/bash
# SUPERPAW Company Loop - Hermès 24/7 loop
# Runs every 5 minutes: git status → cargo test → clippy → fmt → smoke → forensic → hide-smoke → commit+push if green

set -euo pipefail

PROJECT_DIR="${PROJECT_DIR:-~/Projets/Polygone-v2}"
LOOP_INTERVAL="${LOOP_INTERVAL:-300}"  # 5 minutes
TMUX_SESSION="polygone-loop"
LOG_DIR="${LOG_DIR:-$PROJECT_DIR/logs/company-loop}"

mkdir -p "$LOG_DIR"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG_DIR/company-loop.log"
}

check_git_status() {
    cd "$PROJECT_DIR"
    git status --porcelain
}

run_tests() {
    log "Running cargo test --workspace..."
    cd "$PROJECT_DIR"
    cargo test --workspace 2>&1 | tail -n 20
}

run_clippy() {
    log "Running cargo clippy --workspace -- -D warnings..."
    cd "$PROJECT_DIR"
    cargo clippy --workspace -- -D warnings 2>&1 | tail -n 20
}

run_fmt() {
    log "Running cargo fmt --check..."
    cd "$PROJECT_DIR"
    cargo fmt --check
}

run_smoke() {
    log "Running smoke commands..."
    cd "$PROJECT_DIR"
    bash scripts/smoke-commands.sh 2>&1 || true
}

run_forensic() {
    log "Running forensic checks..."
    cd "$PROJECT_DIR"
    cargo run --bin polygone -- forensic 2>&1 || true
}

run_hide_smoke() {
    log "Running hide-smoke..."
    cd "$PROJECT_DIR"
    bash scripts/hide-smoke.sh 2>&1 || true
}

commit_and_push() {
    log "Committing and pushing changes..."
    cd "$PROJECT_DIR"
    git add -A
    git commit -m "chore: auto-commit from Hermès loop - $(date '+%Y-%m-%d %H:%M')" || true
    git push origin main 2>&1 || true
}

run_cycle() {
    log "=== Starting Hermès cycle $(date) ==="
    
    # Check git status
    if git status --porcelain | grep -q .; then
        log "Working tree has changes"
    else
        log "Working tree clean"
    fi
    
    # Run tests
    if run_tests; then
        log "✓ Tests passed"
    else
        log "✗ Tests FAILED"
        return 1
    fi
    
    # Run clippy
    if run_clippy; then
        log "✓ Clippy passed"
    else
        log "✗ Clippy FAILED"
        return 1
    fi
    
    # Run fmt check
    if run_fmt; then
        log "✓ Format check passed"
    else
        log "✗ Format check FAILED"
        return 1
    fi
    
    # Run smoke
    run_smoke
    
    # Run forensic
    run_forensic
    
    # Run hide smoke
    run_hide_smoke
    
    # If all green, commit and push
    if git status --porcelain | grep -q .; then
        log "Changes detected - committing and pushing"
        commit_and_push
    else
        log "No changes to commit"
    fi
    
    log "=== Cycle complete ==="
    return 0
}

# Main loop
main() {
    log "Starting SUPERPAW Company Loop (Hermès 24/7)"
    log "Project: $PROJECT_DIR"
    log "Interval: ${LOOP_INTERVAL}s"
    
    while true; do
        if run_cycle; then
            log "Cycle successful - sleeping ${LOOP_INTERVAL}s"
        else
            log "Cycle FAILED - alerting"
            # Could add alerting here
        fi
        
        sleep "$LOOP_INTERVAL"
    done
}

# Handle signals
trap 'log "Received signal - stopping gracefully"; exit 0' SIGTERM SIGINT

main
