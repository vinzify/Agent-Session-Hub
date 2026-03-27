#!/usr/bin/env sh
set -eu

INSTALL_ROOT="${INSTALL_ROOT:-${HOME}/.local/share/agent-session-hub}"
BIN_ROOT="${BIN_ROOT:-${HOME}/.local/bin}"

if [ -x "${BIN_ROOT}/csx" ]; then
  "${BIN_ROOT}/csx" uninstall-shell >/dev/null 2>&1 || true
fi

rm -f "${BIN_ROOT}/csx" "${BIN_ROOT}/clx" "${BIN_ROOT}/cxs"
rm -rf "${INSTALL_ROOT}"

printf 'Removed Agent Session Hub from %s\n' "$INSTALL_ROOT"
