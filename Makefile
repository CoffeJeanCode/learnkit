# LearnKit - toolchain bootstrap + common tasks.
#
# Delegates to the plain scripts in scripts/, so nothing here needs make:
#   bash scripts/setup.sh   and   bash scripts/run.sh   do the same thing.
# On Windows without GNU make use setup.cmd / run.cmd or .\scripts\*.ps1.

SHELL := bash
MANIFEST := src-tauri/Cargo.toml

# Windows-GNU imports WebView2Loader.dll dynamically (MSVC links it statically),
# so GNU installers must ship it: tauri.gnu.conf.json adds it as a bundle resource
# (build.rs stages the file). Left out for MSVC/macOS/Linux builds.
RUST_HOST := $(shell rustc -vV 2>/dev/null | sed -n 's/^host: //p')
ifneq (,$(findstring windows-gnu,$(RUST_HOST)))
TAURI_BUILD_ARGS := --config src-tauri/tauri.gnu.conf.json
endif

.PHONY: help setup check run build test typecheck dist dist-linux clean clean-all

help: ## show this help
	@grep -E '^[a-z0-9-]+:[^#]*## ' Makefile | sed -E 's/^([a-z0-9-]+):[^#]*## /\1\t/'

setup: ## install missing toolchain (rustup, bun, node check) + bun install + cargo fetch
	bash scripts/setup.sh

check: ## read-only probe of every prerequisite (exits non-zero if one is missing)
	bash scripts/setup.sh --check
	bash scripts/run.sh --check

run: ## bun run tauri dev (Vite + Tauri desktop app)
	bash scripts/run.sh

build: ## typecheck + production frontend build + cargo build
	bun run build
	cargo build --manifest-path $(MANIFEST)

typecheck: ## tsc --noEmit
	bunx tsc --noEmit

dist: ## Windows installer + MSI (NSIS + WiX) — run on Windows
	@if [ -n "$${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then \
		bun run tauri build $(TAURI_BUILD_ARGS); \
	else \
		key="$$HOME/.tauri/learnkit.key"; \
		if [ ! -f "$$key" ]; then \
			echo "error: signing key not found at $$key"; \
			echo "       generate it: bunx tauri signer generate -w ~/.tauri/learnkit.key"; \
			echo "       (bundle.createUpdaterArtifacts cannot be disabled, so every"; \
			echo "        release build needs a private key to sign the updater bundle)"; \
			exit 1; \
		fi; \
		TAURI_SIGNING_PRIVATE_KEY="$$key" bun run tauri build $(TAURI_BUILD_ARGS); \
	fi

dist-linux: ## Linux .deb + AppImage inside Docker (Tauri cannot cross-compile)
	bash scripts/build-linux.sh

test: typecheck ## tsc --noEmit + UI render checks + cargo test
	bun run scripts/check-rendering.tsx
	bun run scripts/check-mermaid-repair.ts
	bun run scripts/check-capability-map.ts
	bun run scripts/check-study-presentation.ts
	bun run scripts/check-release.ts
	bun run scripts/check-version.ts
	cargo test --manifest-path $(MANIFEST)

clean: ## remove build output (dist/ + src-tauri/target)
	rm -rf dist src-tauri/target

clean-all: clean ## also remove node_modules
	rm -rf node_modules
