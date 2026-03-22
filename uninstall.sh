#!/usr/bin/env sh
set -eu

REPOSITORY="${REPOSITORY:-vinzify/Agent-Session-Hub}"
REF="${REF:-master}"

if ! command -v pwsh >/dev/null 2>&1; then
  echo "Agent Session Hub requires PowerShell 7 (pwsh) in PATH." >&2
  exit 1
fi

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${0}")" && pwd 2>/dev/null || true)"
if [ -n "${SCRIPT_DIR}" ] && [ -f "${SCRIPT_DIR}/uninstall.ps1" ]; then
  exec pwsh -NoProfile -File "${SCRIPT_DIR}/uninstall.ps1" "$@"
fi

TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t agent-session-hub-uninstall)"
cleanup() {
  rm -rf "${TMP_DIR}"
}
trap cleanup EXIT INT TERM

SCRIPT_URL="https://raw.githubusercontent.com/${REPOSITORY}/${REF}/uninstall.ps1"
curl -fsSL "${SCRIPT_URL}" -o "${TMP_DIR}/uninstall.ps1"
exec pwsh -NoProfile -File "${TMP_DIR}/uninstall.ps1" "$@"
