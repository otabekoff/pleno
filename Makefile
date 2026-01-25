# Pleno Project Management Makefile

.PHONY: help repl run test build lint format docs-dev docs-build docs-preview package clean

help:
	@echo "Pleno Management Commands:"
	@echo "  repl           Start the interactive REPL"
	@echo "  run FILE=path  Run a specified .pln file"
	@echo "  test           Run all unit and integration tests"
	@echo "  build          Build optimized production binaries"
	@echo "  lint           Run clippy for static analysis"
	@echo "  format         Format the entire codebase"
	@echo "  docs-dev       Start Vitepress development server"
	@echo "  docs-build     Build static documentation"
	@echo "  docs-preview   Preview the built documentation"
	@echo "  package        Package the binary into a compressed archive"
	@echo "  install        Install binaries to user path (Windows)"
	@echo "  all            Run format, lint, test, and build"
	@echo "  clean          Remove build artifacts"

all: format lint test build

install:
	pwsh -File tasks/install.ps1

repl:
	cargo run -p pleno-repl

run:
	@if [ -z "$(FILE)" ]; then echo "Error: FILE variable is required. Example: make run FILE=examples/hello.pln"; exit 1; fi
	cargo run -p pleno-cli -- run $(FILE)

test:
	cargo test --workspace

build:
	cargo build --release --workspace

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

format:
	cargo fmt --all
	npm run format

docs-dev:
	npm run docs:dev

docs-build:
	npm run docs:build

docs-preview:
	npm run docs:preview

package: build
	@mkdir -p dist
	tar -czf dist/pleno-linux-x64.tar.gz -C target/release pleno-cli pleno-repl
	@echo "Package created: dist/pleno-linux-x64.tar.gz"

clean:
	cargo clean
	rm -rf dist/
	rm -rf docs/.vitepress/dist
	rm -rf docs/.vitepress/cache
