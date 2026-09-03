MAKEFLAGS += --no-print-directory

.DEFAULT_GOAL := help

.PHONY: build check clean discover fmt help lint quality run setup test

help: ## Show available targets
	@$(MAKE) -C elara_http help
	@echo ""
	@echo "Elara discovery (install mode wins if both are set):"
	@echo "  ELARA_PREFIX  Install prefix with include/elara.h and lib/libelara.*"
	@echo "  ELARA_ROOT    Elara repository root (sibling checkout); default ../elara"
	@echo ""
	@echo "  discover        Resolve elara.h / libelara (also available here)"

discover: ## Resolve elara.h / libelara (sibling or prefix); fail with env hints
	@.make/elara-env.sh --print

setup: ## Fetch crate dependencies
	@$(MAKE) -C elara_http setup

build: ## Release-build the host package
	@$(MAKE) -C elara_http build

run: ## Run host binary (when available)
	@$(MAKE) -C elara_http run

test: ## Run cargo test (links libelara)
	@$(MAKE) -C elara_http test

check: ## Type-check with cargo check
	@$(MAKE) -C elara_http check

lint: ## Run clippy
	@$(MAKE) -C elara_http lint

fmt: ## Format with rustfmt
	@$(MAKE) -C elara_http fmt

quality: ## fmt + clippy + test
	@$(MAKE) -C elara_http quality

clean: ## Remove target/ and bin artifacts
	@$(MAKE) -C elara_http clean
