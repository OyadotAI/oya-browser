# Mars developer commands: the same checks CI runs, in the order to run them, and the app.
# Every target uses one target directory, so builds are shared between targets.
# Override it per tree (one per worktree): make gate TARGET=target-mine

TARGET ?= target
CARGO = CARGO_TARGET_DIR=$(TARGET) cargo
FEATURES = --features mars/test-support
QUIET = --offline -q

.PHONY: help fmt fmt-check clippy test-v9 test-eye test-relay test-avif test-core test-mars test-window test gate app run clean-incremental

help: ## List the targets.
	@grep -E '^[a-z0-9-]+:.*## ' $(MAKEFILE_LIST) | sed 's/:.*## /\t/'

fmt: ## Format the workspace.
	cargo fmt --all

fmt-check: ## Fail when anything is not formatted.
	cargo fmt --all -- --check

clippy: ## Lint the whole workspace, test code included; any warning fails.
	$(CARGO) clippy $(QUIET) --workspace --all-targets $(FEATURES) -- -D warnings

test-v9: ## The JS engine's suite.
	$(CARGO) test $(QUIET) -p mars-v9

test-eye: ## Style, layout and paint.
	$(CARGO) test $(QUIET) -p mars-eye

test-relay: ## Network.
	$(CARGO) test $(QUIET) -p mars-relay

test-avif: ## The AVIF decoder.
	$(CARGO) test $(QUIET) -p mars-avif

test-core: ## Shared types.
	$(CARGO) test $(QUIET) -p mars-core

test-mars: ## The runtime and desktop suites that need no window.
	$(CARGO) test $(QUIET) -p mars $(FEATURES) --lib --test desktop --test js_page --test runtime --test forms

test-window: ## Real-window tests: they open windows, so leave the screen alone while they run.
	$(CARGO) test $(QUIET) -p mars $(FEATURES) --test window -- --ignored --test-threads=1

test: test-core test-v9 test-eye test-relay test-avif test-mars ## Every suite except the real-window one.

gate: fmt-check clippy test ## What must be green before a commit.

app: ## Build and sign dist/Mars.app.
	CARGO_TARGET_DIR=$(TARGET) bash packaging/build-app.sh

run: app ## Build Mars.app and open it.
	open dist/Mars.app

clean-incremental: ## Reclaim disk without losing compiled dependencies.
	rm -rf $(TARGET)/debug/incremental $(TARGET)/release/incremental
