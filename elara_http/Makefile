MAKEFLAGS += --no-print-directory

.DEFAULT_GOAL := help

.PHONY: build check clean fmt help lint quality setup test

help: ## Show available targets
	@echo "elara_http - Available targets"
	@echo ""
	@grep -hE '^[a-zA-Z_-]+:.*## ' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*## "} {printf "  %-15s %s\n", $$1, $$2}'

setup: ## Fetch crate dependencies
	@../.make/setup.sh

build: ## Release-build and copy binary to bin/ when present
	@../.make/build.sh

run: ## Reserved for the host binary
	@../.make/run.sh

test: ## Run cargo test (links libelara)
	@../.make/test.sh

check: ## Type-check with cargo check
	@../.make/check.sh

lint: ## Run clippy
	@../.make/lint.sh

fmt: ## Format with rustfmt
	@../.make/fmt.sh

quality: ## fmt + clippy + test
	@../.make/quality.sh

clean: ## Remove target/ and bin artifacts
	@../.make/clean.sh
