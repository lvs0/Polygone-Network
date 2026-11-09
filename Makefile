# Porte de qualité du dépôt canonique Polygone.
# `make check` doit passer avant toute PR ; c'est la même commande en local et en CI.

CARGO ?= cargo

.PHONY: help check fmt clippy test docs smoke build clean

help:
	@echo "make check   fmt + clippy + tests workspace (porte complète)"
	@echo "make fmt     vérifie le formatage sans réécrire"
	@echo "make clippy  lints en erreur"
	@echo "make test    tests workspace"
	@echo "make docs    cohérence documentaire (scripts/check-docs.mjs)"
	@echo "make smoke   parcours réel relais + deux processus (scripts/p0-two-process.sh)"
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

build:
	$(CARGO) build --release --workspace

clean:
	$(CARGO) clean
