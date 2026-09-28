#!/usr/bin/env bash
# LearnKit bootstrap (bash / make). Installs whatever toolchain is missing,
# then the JS and Rust dependencies.
#
#   bash scripts/setup.sh          # install what's missing + bun install + cargo fetch
#   bash scripts/setup.sh --check  # verify only; exit 1 if something is missing
set -euo pipefail

CHECK=0
for arg in "$@"; do
  case "$arg" in
    --check) CHECK=1 ;;
    -h|--help)
      sed -n '2,6p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) printf 'setup.sh: unknown argument: %s\n' "$arg" >&2; exit 2 ;;
  esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FAILS=0
GREEN=$'\033[32m'; RED=$'\033[31m'; YELLOW=$'\033[33m'; RESET=$'\033[0m'
step() { printf '\n%s\n' "$1"; }
ok()   { printf '  %s[ok]%s %s\n' "$GREEN" "$RESET" "$1"; }
bad()  { printf '  %s[!!]%s %s\n' "$RED" "$RESET" "$1"; FAILS=$((FAILS + 1)); }
warn() { printf '  %s!%s %s\n' "$YELLOW" "$RESET" "$1"; }
have() { command -v "$1" >/dev/null 2>&1; }

uname_s="$(uname -s 2>/dev/null || echo unknown)"
case "$uname_s" in
  MINGW*|MSYS*|CYGWIN*) ON_WINDOWS=1 ;;
  *) ON_WINDOWS=0 ;;
esac

# A shell started right after the toolchain install still has the old PATH.
export PATH="$HOME/.cargo/bin:$HOME/.bun/bin:$PATH"

node_ok() {
  have node || return 1
  node -e 'process.exit(parseInt(process.versions.node, 10) >= 18 ? 0 : 1)' 2>/dev/null
}

step "1/5 Rust (rustup + cargo, stable)"
if have rustup && have cargo && have rustc; then
  ok "$(rustc --version)"
elif [ "$CHECK" -eq 1 ]; then
  bad "rustup/cargo not found (install: https://rustup.rs)"
else
  if [ "$ON_WINDOWS" -eq 1 ]; then
    RUSTUP_URL="https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe"
    TMP_EXE="$(mktemp -t rustup-init-XXXXXX.exe)"
    curl --proto '=https' --tlsv1.2 -sSfL "$RUSTUP_URL" -o "$TMP_EXE"
    "$TMP_EXE" -y --default-toolchain stable
    rm -f "$TMP_EXE"
  else
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  fi
  if have rustc && have cargo; then ok "$(rustc --version)"; else bad "rustup install did not put cargo on PATH"; fi
fi

step "2/5 Bun (JS package manager + script runner)"
if have bun; then
  ok "bun $(bun --version)"
elif [ "$CHECK" -eq 1 ]; then
  bad "bun not found (install: https://bun.sh)"
else
  if [ "$ON_WINDOWS" -eq 1 ]; then
    powershell -NoProfile -ExecutionPolicy Bypass -Command "irm bun.sh/install.ps1 | iex"
  else
    curl -fsSL https://bun.sh/install | bash
  fi
  if have bun; then ok "bun $(bun --version)"; else bad "bun install failed (open a new shell and re-run)"; fi
fi

step "3/5 Node >= 18 (Tauri CLI internals)"
if node_ok; then
  ok "node $(node --version)"
elif have node; then
  warn "node $(node --version) is older than 18 - upgrade it (Tauri CLI needs >= 18)"
  FAILS=$((FAILS + 1))
elif [ "$CHECK" -eq 1 ]; then
  bad "node not found (need >= 18)"
elif have apt-get; then
  warn "node missing - install it (e.g. 'sudo apt-get install -y nodejs' or nvm)"
elif have brew; then
  warn "node missing - run 'brew install node'"
else
  warn "node missing - install Node >= 18 from https://nodejs.org"
fi

step "4/5 JS dependencies (bun install)"
if [ "$CHECK" -eq 1 ]; then
  if [ -d node_modules ]; then ok "node_modules present"; else bad "node_modules missing (run: bun install)"; fi
elif have bun; then
  bun install
  ok "bun install done"
else
  warn "skipped (bun missing)"
fi

step "5/5 Rust dependencies (cargo fetch)"
if [ "$CHECK" -eq 1 ]; then
  if have cargo; then ok "cargo available"; else bad "cargo missing"; fi
elif have cargo; then
  cargo fetch --manifest-path src-tauri/Cargo.toml
  ok "crates fetched"
else
  warn "skipped (cargo missing)"
fi

printf '\n'
if [ "$FAILS" -gt 0 ]; then
  printf '%s%d prerequisite(s) missing.%s Fix them, then run:\n' "$RED" "$FAILS" "$RESET"
  printf '  make run   |   bash scripts/run.sh   |   run.cmd\n'
  exit 1
fi
printf '%sAll set.%s Run the app with one of:\n' "$GREEN" "$RESET"
printf '  make run            (bash:  bash scripts/run.sh)\n'
printf '  run.cmd             (cmd:   double-click or type it)\n'
printf '  .\\run.ps1           (PowerShell)\n'
