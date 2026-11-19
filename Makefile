# Porte de qualité du dépôt canonique Polygone.
# `make check` doit passer avant toute PR ; c'est la même commande en local et en CI.

CARGO ?= cargo

.PHONY: help check fmt clippy test docs smoke migrate-proof release-dry-run audit build clean

help:
	@echo "make check   fmt + clippy + tests workspace (porte complète)"
	@echo "make fmt     vérifie le formatage sans réécrire"
	@echo "make clippy  lints en erreur"
	@echo "make test    tests workspace"
	@echo "make docs    cohérence documentaire (scripts/check-docs.mjs)"
	@echo "make smoke   parcours réel relais + deux processus (scripts/p0-two-process.sh)"
	@echo "make migrate-proof  migration peers.json v1 → v2 (script de preuve)"
	@echo "make release-dry-run  release à blanc : manifeste + détection d'altération, sans publier"
	@echo "make audit   cargo-audit + cargo-deny (outils absents = exit 2, jamais contourné)"
	@echo "make build   binaires de release"

check: fmt clippy test docs

fmt:
	$(CARGO) fmt --all -- --check

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

test:
	$(CARGO) test --workspace

docs:
	node scripts/check-docs.mjs

smoke:
	./scripts/p0-two-process.sh

migrate-proof:
	./scripts/peers-migration-smoke.sh

# Porte de release à blanc. Ne publie rien ; échoue si le manifeste de
# l'artefact ne détecte pas une altération. `make release-dry-run BUILD=1`
# compile d'abord (lourd).
release-dry-run:
	./scripts/release-dry-run.sh $(if $(BUILD),--build,)

# Sécurité de la chaîne d'approvisionnement. Volontairement hors de `make check` :
# l'absence d'outil est un état constaté (exit 2), pas un succès silencieux.
audit:
	@command -v cargo-audit >/dev/null || { echo "cargo-audit absent : 'cargo install cargo-audit' (non exécuté)"; exit 2; }
	@command -v cargo-deny >/dev/null || { echo "cargo-deny absent : 'cargo install cargo-deny' (non exécuté)"; exit 2; }
	cargo audit
	cargo deny check

build:
	$(CARGO) build --release --workspace

clean:
	$(CARGO) clean
