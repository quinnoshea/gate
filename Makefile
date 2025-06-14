.PHONY: help build test check fmt lint clippy fix clean install-tools dev all

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-15s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# Build targets
build: ## Build all workspace crates
	cargo build --all --all-targets --all-features

build-release: ## Build all workspace crates in release mode
	cargo build --release --all --all-targets --all-features

# Test targets
test: ## Run all tests
	cargo test --all --all-targets --all-features

test-doc: ## Run documentation tests
	cargo test --doc --all

# Code quality targets
check: ## Run cargo check
	cargo check --all --all-targets --all-features

fmt: ## Format code
	cargo fmt --all

fmt-check: ## Check if code is formatted
	cargo fmt --all -- --check

lint: fmt clippy ## Run all linting (format + clippy)

clippy: ## Run clippy lints
	cargo clippy --all --all-targets --all-features -- -D warnings

clippy-fix: ## Run clippy with automatic fixes
	cargo clippy --all --all-targets --all-features --fix --allow-dirty --allow-staged

fix: fmt clippy-fix ## Format code and apply clippy fixes

# Dependency management
udeps: ## Check for unused dependencies (requires cargo-udeps)
	cargo +nightly udeps --all --all-targets

audit: ## Audit dependencies for security vulnerabilities
	cargo audit

outdated: ## Check for outdated dependencies
	cargo outdated

# Documentation
doc: ## Generate documentation
	cargo doc --all --no-deps --open

doc-private: ## Generate documentation including private items
	cargo doc --all --no-deps --document-private-items --open

# Cleaning
clean: ## Clean build artifacts
	cargo clean

# Tool installation
install-tools: ## Install required development tools
	@echo "Installing development tools..."
	cargo install cargo-udeps
	cargo install cargo-audit
	cargo install cargo-outdated
	@echo "Note: cargo-udeps requires nightly toolchain"
	rustup toolchain install nightly

# Development workflow
dev: fmt clippy test ## Run development checks (format, clippy, test)

ci: fmt-check clippy test audit ## Run CI checks (format check, clippy, test, audit)

all: build test clippy ## DOESNT Clean but build, test, and lint everything

# Release preparation
pre-release: ci doc ## Prepare for release (run CI + generate docs)

# Protobuf generation (if needed)
proto: ## Regenerate protobuf files
	cargo build -p hellas-gate-proto

# Individual crate operations
build-core: ## Build core crate
	cargo build -p hellas-gate-core

build-daemon: ## Build daemon crate
	cargo build -p hellas-gate-daemon

build-cli: ## Build CLI crate
	cargo build -p hellas-gate-cli

build-relay: ## Build relay crate
	cargo build -p hellas-gate-relay

test-core: ## Test core crate
	cargo test -p hellas-gate-core

test-daemon: ## Test daemon crate
	cargo test -p hellas-gate-daemon

test-cli: ## Test CLI crate
	cargo test -p hellas-gate-cli

test-relay: ## Test relay crate
	cargo test -p hellas-gate-relay