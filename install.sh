#!/usr/bin/env bash
set -euo pipefail
# cargo install wrapper. Does not edit shell profiles.

from_source=0
dry_run=0
while [[ $# -gt 0 ]]; do
  case $1 in
    --from-source) from_source=1 ;;
    --dry-run) dry_run=1 ;;
    -h | --help)
      printf '%s\n' \
        'Install published `codensity` via cargo install.' \
        './install.sh [--from-source] [--dry-run]' \
        'Optional: CODENSITY_VERSION CARGO_INSTALL_ROOT'
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 1
      ;;
  esac
  shift
done

command -v cargo >/dev/null || {
  echo 'cargo is required. Install Rust from https://rustup.rs' >&2
  exit 1
}

args=(install --locked)
[[ -n ${CARGO_INSTALL_ROOT:-} ]] && args+=(--root "$CARGO_INSTALL_ROOT")
if ((from_source)); then
  args+=(--path "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)")
else
  args+=(codensity)
  [[ -n ${CODENSITY_VERSION:-} ]] && args+=(--version "$CODENSITY_VERSION")
fi

if ((dry_run)); then
  printf 'cargo'
  printf ' %q' "${args[@]}"
  printf '\n'
  exit 0
fi

cargo "${args[@]}"
codensity --version
