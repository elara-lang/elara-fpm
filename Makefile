MAKEFLAGS += --no-print-directory

.DEFAULT_GOAL := help

.PHONY: build check clean discover fmt help install install-system lint quality run setup test uninstall uninstall-system

# Default cleartext server; override with ARGS='-S host:port -t dir'
ARGS ?= -S 127.0.0.1:8080 -t demos

help: ## Show available targets
	@$(MAKE) -C elara_fpm help
	@echo ""
	@echo "Elara discovery (/usr/local when installed; ELARA_PREFIX wins over ELARA_ROOT):"
	@echo "  default       /usr/local if include/elara.h and lib/libelara.* are there; else ../elara"
	@echo "  ELARA_PREFIX  Install prefix with include/elara.h and lib/libelara.*"
	@echo "  ELARA_ROOT    Elara repository root (sibling checkout; overrides /usr/local)"
	@echo ""
	@echo "  discover        Resolve elara.h / libelara (also available here)"

discover: ## Resolve elara.h / libelara (/usr/local, prefix, or sibling)
	@.make/elara-env.sh --print

setup: ## Fetch crate dependencies
	@$(MAKE) -C elara_fpm setup

build: ## Release-build and copy binary to bin/
	@$(MAKE) -C elara_fpm build

run: ## Dev server (default -S 127.0.0.1:8080 -t demos; override with ARGS='...')
	@$(MAKE) -C elara_fpm run ARGS="$(ARGS)"

test: ## Run cargo test (links libelara)
	@$(MAKE) -C elara_fpm test

install: ## Install to ~/.local/bin (requires prior make build)
	@$(MAKE) -C elara_fpm install

install-system: ## Install to /usr/local/bin (requires prior make build; sudo only for copy)
	@$(MAKE) -C elara_fpm install-system

uninstall: ## Remove from ~/.local/bin (sudo only if needed)
	@$(MAKE) -C elara_fpm uninstall

uninstall-system: ## Remove from /usr/local/bin (sudo only if needed)
	@$(MAKE) -C elara_fpm uninstall-system

check: ## Type-check with cargo check
	@$(MAKE) -C elara_fpm check

lint: ## Run clippy
	@$(MAKE) -C elara_fpm lint

fmt: ## Format with rustfmt
	@$(MAKE) -C elara_fpm fmt

quality: ## fmt + clippy + test
	@$(MAKE) -C elara_fpm quality

clean: ## Remove target/ and bin artifacts
	@$(MAKE) -C elara_fpm clean
