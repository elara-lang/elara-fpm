MAKEFLAGS += --no-print-directory

.DEFAULT_GOAL := help

.PHONY: help discover

help: ## Show available targets
	@echo "elara-http - Available targets"
	@echo ""
	@grep -hE '^[a-zA-Z_-]+:.*## ' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*## "} {printf "  %-15s %s\n", $$1, $$2}'
	@echo ""
	@echo "Elara discovery (install mode wins if both are set):"
	@echo "  ELARA_PREFIX  Install prefix with include/elara.h and lib/libelara.*"
	@echo "  ELARA_ROOT    Elara repository root (sibling checkout); default ../elara"

discover: ## Resolve elara.h / libelara (sibling or prefix); fail with env hints
	@.make/elara-env.sh --print
