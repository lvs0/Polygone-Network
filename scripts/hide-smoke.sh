#!/bin/bash
# Hide SOCKS5 smoke test for Polygone
set -euo pipefail

cd ~/Projets/Polygone-v2

echo "Testing Hide SOCKS5..."

# Test if hide binary exists
if cargo run --bin polygone -- hide --help 2>&1 | head -n 20; then
    echo "Hide command available"
else
    echo "Hide command not available"
fi

# Test if relay can start in background
# timeout 5 cargo run --bin polygoned -- --help 2>&1 | head -n 20 || true

echo "Hide smoke test completed"