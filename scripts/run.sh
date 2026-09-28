#!/usr/bin/env bash
# Run LearnKit: `bun run tauri dev` (Vite dev server + Tauri desktop window).
#
#   bash scripts/run.sh          # launch the app
#   bash scripts/run.sh --check  # verify prerequisites without launching
#   make run                     # same as bash scripts/run.sh
set -euo pipefail

CHECK=0
ARGS=()
for arg in "$@"; do
  case "$arg" in
    --check) CHECK=1 ;;
    -h|--help) sed -n '2,6p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) ARGS+=("$arg") ;;
  esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"

FAILS=0
GREEN=$'\033[32m'; RED=$'\033[31m'; RESET=$'\033[0m'
ok()  { printf '  %s[ok]%s %s\n' "$GREEN" "$RESET" "$1"; }
bad() { printf '  %s[!!]%s %s\n' "$RED" "$RESET" "$1"; FAILS=$((FAILS + 1)); }
have() { command -v "$1" >/dev/null 2>&1; }

node_ok() {
  have node || return 1
  node -e 'process.exit(parseInt(process.versions.node, 10) >= 18 ? 0 : 1)' 2>/dev/null
}

if [ "$CHECK" -eq 1 ]; then
  printf 'Prerequisites for `bun run tauri dev`:\n'
  if have rustc && have cargo; then ok "$(rustc --version)"; else bad "rust/cargo missing - run: bash scripts/setup.sh"; fi
  if have bun; then ok "bun $(bun --version)"; else bad "bun missing - run: bash scripts/setup.sh"; fi
  if node_ok; then ok "node $(node --version)"; else bad "node >= 18 missing - run: bash scripts/setup.sh"; fi
  if [ -d node_modules ]; then ok "node_modules present"; else bad "dependencies not installed - run: bash scripts/setup.sh"; fi
  [ "$FAILS" -eq 0 ] || exit 1
  printf '\nWould run: bun run tauri dev\n'
  exit 0
fi

for dep in cargo bun node; do
  if ! have "$dep"; then
    printf '%sMissing %s.%s Run the bootstrap first:\n' "$RED" "$dep" "$RESET" >&2
    printf '  bash scripts/setup.sh   (or: make setup / setup.cmd)\n' >&2
    exit 1
  fi
done
if [ ! -d node_modules ]; then
  printf '%snode_modules missing.%s Run: bash scripts/setup.sh\n' "$RED" "$RESET" >&2
  exit 1
fi

exec bun run tauri dev "${ARGS[@]+"${ARGS[@]}"}"
