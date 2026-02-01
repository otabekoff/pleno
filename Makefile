# Pleno Project Management Makefile

# Configuration
PREFIX ?= $(HOME)/.local
BIN_DIR = $(PREFIX)/bin
TARGET_DIR = target/release

# Binaries
BINS = pleno plm pleno-lsp pleno-repl

.PHONY: help build release install uninstall vscode test lint format clean all

help:
	@echo "Pleno Management Commands:"
	@echo "  build          Build all crates in debug mode"
	@echo "  release        Build optimized production binaries"
	@echo "  install        Install binaries to $(BIN_DIR)"
	@echo "  uninstall      Remove binaries from $(BIN_DIR)"
	@echo "  vscode         Package and install VSCode extension"
	@echo "  vscode-watch   Watch and compile VSCode extension"
	@echo "  vscode-test    Run VSCode extension tests"
	@echo "  test           Run all workspace tests"
	@echo "  lint           Run clippy checks"
	@echo "  format         Format Rust and VSCode source code"
	@echo "  clean          Remove build artifacts"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX         Installation prefix (default: $(HOME)/.local)"

all: format lint test release

build:
	cargo build --workspace

release:
	cargo build --release --workspace

install: release
	@mkdir -p $(BIN_DIR)
	@for bin in $(BINS); do \
		if [ -f $(TARGET_DIR)/$$bin ]; then \
			cp $(TARGET_DIR)/$$bin $(BIN_DIR)/; \
			chmod +x $(BIN_DIR)/$$bin; \
			echo "✓ Installed $$bin to $(BIN_DIR)"; \
		else \
			echo "⚠ Warning: $$bin not found in $(TARGET_DIR)"; \
		fi \
	done
	@echo "Installation complete. Ensure $(BIN_DIR) is in your PATH."

uninstall:
	@for bin in $(BINS); do \
		rm -f $(BIN_DIR)/$$bin; \
		echo "Removed $$bin"; \
	done

vscode:
	cd ide/vscode && npm install && npm run all

vscode-watch:
	cd ide/vscode && npm run watch

vscode-test:
	cd ide/vscode && npm test

# Documentation
docs-dev:
	npm run docs:dev

docs-build:
	npm run docs:build

docs-preview:
	npm run docs:preview

# Packaging
package: release
	@mkdir -p dist
	tar -czf dist/pleno-linux-x64.tar.gz -C $(TARGET_DIR) $(BINS)
	@echo "Package created: dist/pleno-linux-x64.tar.gz"

test:
	cargo test --workspace

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

format:
	cargo fmt --all
	cd ide/vscode && npm run compile && npm run lint

clean:
	cargo clean
	rm -rf dist/
	cd ide/vscode && rm -rf out *.vsix
