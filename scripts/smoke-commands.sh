#!/bin/bash
# Smoke commands for Polygone
set -euo pipefail

cd ~/Projets/Polygone-v2

# Test basic CLI
cargo run --bin polygone -- --help 2>&1 | head -n 20

# Test identity
cargo run --bin polygone -- identity 2>&1 | head -n 20

# Test serverless
cargo run --bin polygone -- serverless --help 2>&1 | head -n 20

# Test drive
cargo run --bin polygone -- drive --help 2>&1 | head -n 20

# Test petals
cargo run --bin polygone -- petals --help 2>&1 | head -n 20

echo "Smoke commands completed"