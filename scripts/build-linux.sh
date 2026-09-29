#!/usr/bin/env bash
# Builds the Linux bundles (.deb + AppImage) on a Linux toolchain inside Docker.
#
# Tauri cannot cross-compile: a Windows host will not produce a Linux binary,
# so this script runs the exact `bun run tauri build` inside a container built
# from scripts/docker/Dockerfile.linux. From a Linux host you can skip Docker
# and run `bun run tauri build -- --bundles deb,appimage` after installing the
# packages listed in that Dockerfile.
set -euo pipefail

cd "$(dirname "$0")/.."

# Git Bash on Windows exposes `pwd -W` (C:/ style), which Docker Desktop needs
# for the bind mount; plain Linux bash only has `pwd`.
HOST_DIR=$(pwd -W 2>/dev/null || pwd)

# Git Bash rewrites arguments that look like Unix paths when calling Windows
# executables (`-w /app` became `C:/…/Git/app`), which the daemon rejects.
# Safe on Linux too: these variables only exist for the MSYS runtime.
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL='*'

echo "==> building the Linux toolchain image"
docker build -f scripts/docker/Dockerfile.linux -t learnkit-linux-builder scripts/docker

echo "==> installing JS deps + building bundles (.deb, AppImage)"
# Named volumes keep cargo's registry and git checkouts between runs — without
# them every invocation recompiles the whole Rust dependency tree.
#
# The container runs DETACHED so an interrupted shell (Ctrl-C, CI timeout) can
# never kill it: re-running this script resumes the same build instead of
# starting a second one. `learnkit-linux-build` is the container's name.
if docker ps -a --format '{{.Names}}' | grep -qx learnkit-linux-build; then
  echo "==> resuming the build already in flight"
else
  docker run -d --name learnkit-linux-build \
    -v "$HOST_DIR:/app" \
    -v learnkit-cargo-registry:/usr/local/cargo/registry \
    -v learnkit-cargo-git:/usr/local/cargo/git \
    -w /app \
    learnkit-linux-builder \
    bash -c "bun install --frozen-lockfile && bun run tauri -- build --bundles deb,appimage"
fi

set +e
docker logs -f learnkit-linux-build
code=$(docker wait learnkit-linux-build)
set -e

echo "==> container exit code: $code"
docker logs --tail 40 learnkit-linux-build 2>/dev/null || true
docker rm learnkit-linux-build >/dev/null 2>&1 || true
[ "$code" -eq 0 ] || exit "$code"

echo "==> bundles:"
echo "    src-tauri/target/release/bundle/deb/"
echo "    src-tauri/target/release/bundle/appimage/"

# The container installed node_modules with Linux binaries into the shared
# worktree; restore the host's platform packages so the next `bun run build`
# on Windows/macOS does not trip over a linux-x64 esbuild/rollup.
if command -v bun >/dev/null 2>&1; then
  echo "==> restoring host JS deps (platform swap: linux-x64 -> host)"
  bun install
fi
