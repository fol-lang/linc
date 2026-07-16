PROJECT_NAME := $(shell sed -n 's/^name = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
PROJECT_CAP  := $(shell echo $(PROJECT_NAME) | tr '[:lower:]' '[:upper:]')
CURRENT_VERSION := $(shell grep '^version = ' Cargo.toml | sed -E 's/version = "(.*)"/\1/')
LATEST_TAG   ?= $(shell git describe --tags --abbrev=0 2>/dev/null)
TOP_DIR      := $(CURDIR)
BUILD_DIR    := $(TOP_DIR)/target

# Native fixture builds are intentionally non-incremental. This keeps the
# required evidence lane bounded on small CI/worktree filesystems.
export CARGO_INCREMENTAL ?= 0

ifeq ($(PROJECT_NAME),)
$(error Error: project name not found in Cargo.toml)
endif

$(info ------------------------------------------)
$(info Project: $(PROJECT_NAME))
$(info Version: $(CURRENT_VERSION))
$(info ------------------------------------------)

.PHONY: build b compile c fmt fmt-check lint check-features test t test-contract test-native test-package test-system docs-check verify help h clean docs release

SHELL := /bin/bash


build:
	@cargo build --release

b: build

compile:
	@cargo clean
	@make build

c: compile

test:
	@cargo test -- --test-threads=1
	@cargo test --doc

t: test

fmt:
	@cargo fmt

fmt-check:
	@cargo fmt -- --check

lint:
	@cargo clippy --no-deps --all-targets --all-features -- -D warnings

check-features:
	@cargo check --all-targets
	@cargo check --all-targets --all-features
	@cargo check --all-targets --no-default-features
	@cargo check --all-targets --no-default-features --features contracts
	@cargo check --all-targets --no-default-features --features native-inspection

test-contract:
	@tools/require-nonzero-tests.sh contract \
		cargo test --test contract_corpus -- --test-threads=1

test-native:
	@test "$$(uname -s)" = Linux || { echo "native evidence tests require Linux"; exit 1; }
	@command -v cc >/dev/null 2>&1 || { echo "native evidence tests require cc"; exit 1; }
	@command -v ar >/dev/null 2>&1 || { echo "native evidence tests require ar"; exit 1; }
	@test -x /bin/kill || { echo "native evidence tests require /bin/kill"; exit 1; }
	@tools/require-nonzero-tests.sh native-linux \
		env LINC_TEST_CC="$$(command -v cc)" LINC_TEST_AR="$$(command -v ar)" \
		cargo test --features native-inspection --test native_evidence -- --nocapture --test-threads=1

test-package:
	@tools/test-package.sh follang-linc linc

test-system:
	@tools/require-nonzero-tests.sh system-h1-corpus \
		cargo test --test contract_corpus -- --nocapture --test-threads=1

docs-check:
	@command -v mdbook >/dev/null 2>&1 || { echo "mdbook is required"; exit 1; }
	@mdbook build $(TOP_DIR)/book --dest-dir $(BUILD_DIR)/book
	@cargo doc --no-deps --all-features

VERIFY_ALLOW_DIRTY ?= 0

verify:
	@set -eu; \
		before="$$(mktemp "$${TMPDIR:-/tmp}/linc-verify-before.XXXXXX")"; \
		after="$$(mktemp "$${TMPDIR:-/tmp}/linc-verify-after.XXXXXX")"; \
		trap 'rm -f "$$before" "$$after"' EXIT; \
		git status --porcelain=v1 --untracked-files=all >"$$before"; \
		if test -s "$$before" && test "$(VERIFY_ALLOW_DIRTY)" != 1; then \
			echo "verification requires a clean worktree (or VERIFY_ALLOW_DIRTY=1)"; \
			cat "$$before"; \
			exit 1; \
		fi; \
		$(MAKE) fmt-check; \
		$(MAKE) lint; \
		$(MAKE) check-features; \
		$(MAKE) test; \
		$(MAKE) test-contract; \
		$(MAKE) test-native; \
		$(MAKE) clean; \
		$(MAKE) test-package; \
		$(MAKE) clean; \
		$(MAKE) test-system; \
		$(MAKE) docs-check; \
		git status --porcelain=v1 --untracked-files=all >"$$after"; \
		diff -u "$$before" "$$after"

help:
	@echo
	@echo "Usage: make [target]"
	@echo
	@echo "Available targets:"
	@echo "  build        Build project"
	@echo "  compile      Configure and generate build files"
	@echo "  fmt          Format this package"
	@echo "  fmt-check    Check Rust formatting"
	@echo "  lint         Run Clippy with warnings denied"
	@echo "  check-features  Check default, all, and no-default features"
	@echo "  test         Run tests"
	@echo "  test-contract  Run contract tests"
	@echo "  test-native    Build real ELF fixtures and test native evidence"
	@echo "  test-package   Test the package archive and clean consumer"
	@echo "  test-system    Run required system tests"
	@echo "  docs-check     Build Rust and mdBook documentation"
	@echo "  verify         Run the complete non-mutating gate"
	@echo "  docs         Build documentation (TYPE=mdbook|doxygen)"
	@echo "  release      Create a new release (TYPE=patch|minor|major)"
	@echo

h : help

clean:
	@echo "Cleaning build directory..."
	@rm -rf $(BUILD_DIR)
	@echo "Build directory cleaned."

docs:
ifeq ($(TYPE),mdbook)
	@$(MAKE) docs-check
else ifeq ($(TYPE),doxygen)
	@command -v doxygen >/dev/null 2>&1 || { echo "doxygen is not installed. Please install it first."; exit 1; }
else
	$(error Invalid documentation type. Use 'make docs TYPE=mdbook' or 'make docs TYPE=doxygen')
endif

TYPE ?= patch
HAS_REL := $(shell command -v git-rel 2>/dev/null)

release:
	@if [ -z "$(HAS_REL)" ]; then \
		echo "git-rel is not installed. Please install it first."; \
		exit 1; \
	fi
	@if [ -z "$(TYPE)" ]; then \
		echo "Release type not specified. Use 'make release TYPE=[patch|minor|major|m.m.p]'"; \
		exit 1; \
	fi
	@git rel $(TYPE)
