#!/usr/bin/env sh
set -eu

REPOSITORY="${REPOSITORY:-vinzify/Agent-Session-Hub}"
VERSION="${VERSION:-latest}"
INSTALL_ROOT="${INSTALL_ROOT:-${HOME}/.local/share/agent-session-hub}"
BIN_ROOT="${BIN_ROOT:-${HOME}/.local/bin}"
SKIP_SHELL_INTEGRATION="${SKIP_SHELL_INTEGRATION:-0}"

script_dir() {
  CDPATH= cd -- "$(dirname -- "$1")" && pwd 2>/dev/null || return 1
}

has_local_source() {
  [ -f "$1/Cargo.toml" ] && [ -f "$1/src/main.rs" ]
}

is_file_backed_local_source() {
  script_path="$1"
  case "$(basename -- "$script_path")" in
    install.sh) ;;
    *) return 1 ;;
  esac
  [ -f "$script_path" ] || return 1
  script_root="$(script_dir "$script_path")" || return 1
  has_local_source "$script_root"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    exit 1
  fi
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$arch" in
    x86_64|amd64) arch="x86_64" ;;
    aarch64|arm64) arch="aarch64" ;;
    *)
      echo "Unsupported architecture: $arch" >&2
      exit 1
      ;;
  esac

  case "$os" in
    Linux) printf '%s\n' "${arch}-unknown-linux-gnu" ;;
    Darwin) printf '%s\n' "${arch}-apple-darwin" ;;
    *)
      echo "Unsupported operating system: $os" >&2
      exit 1
      ;;
  esac
}

download_release_binary() {
  target="$1"
  tmp_dir="$2"
  archive_path="${tmp_dir}/agent-session-hub.tar.gz"
  if [ "$VERSION" = "latest" ]; then
    url="https://github.com/${REPOSITORY}/releases/latest/download/agent-session-hub-${target}.tar.gz"
  else
    url="https://github.com/${REPOSITORY}/releases/download/${VERSION}/agent-session-hub-${target}.tar.gz"
  fi

  require_cmd curl
  require_cmd tar
  curl -fsSL "$url" -o "$archive_path"
  tar -xzf "$archive_path" -C "$tmp_dir"
  find "$tmp_dir" -type f -name agent-session-hub | head -n 1
}

build_local_binary() {
  source_root="$1"
  require_cmd cargo
  (
    cd "$source_root"
    cargo build --release
  ) >&2
  printf '%s\n' "${source_root}/target/release/agent-session-hub"
}

atomic_install_binary() {
  source_path="$1"
  target_path="$2"
  staged_path="$(mktemp "${target_path}.tmp.XXXXXX")"
  if cp "$source_path" "$staged_path" &&
    chmod +x "$staged_path" &&
    mv -f "$staged_path" "$target_path"
  then
    return 0
  fi
  rm -f "$staged_path"
  return 1
}

install_binary() {
  binary_path="$1"

  mkdir -p "${INSTALL_ROOT}/bin" "$BIN_ROOT"
  installed_binary="${INSTALL_ROOT}/bin/agent-session-hub"
  atomic_install_binary "$binary_path" "$installed_binary"
  atomic_install_binary "$installed_binary" "${BIN_ROOT}/csx"
  atomic_install_binary "$installed_binary" "${BIN_ROOT}/clx"
  atomic_install_binary "$installed_binary" "${BIN_ROOT}/opx"
  atomic_install_binary "$installed_binary" "${BIN_ROOT}/sessionhub"
}

run_shell_install() {
  if [ "$SKIP_SHELL_INTEGRATION" = "1" ]; then
    return 0
  fi
  "${BIN_ROOT}/csx" install-shell
}

main() {
  script_root=""
  tmp_dir=""
  cleanup() {
    if [ -n "$tmp_dir" ] && [ -d "$tmp_dir" ]; then
      rm -rf "$tmp_dir"
    fi
  }
  trap cleanup EXIT INT TERM

  if is_file_backed_local_source "$0"; then
    script_root="$(script_dir "$0")"
    binary_path="$(build_local_binary "$script_root")"
  else
    tmp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t agent-session-hub-install)"
    binary_path="$(download_release_binary "$(detect_target)" "$tmp_dir")"
  fi

  if [ ! -f "$binary_path" ]; then
    echo "Unable to locate built agent-session-hub binary." >&2
    exit 1
  fi

  install_binary "$binary_path"
  run_shell_install

  printf 'Installed Agent Session Hub to %s\n' "$INSTALL_ROOT"
  printf 'Command shims installed in %s\n' "$BIN_ROOT"
  if [ "$SKIP_SHELL_INTEGRATION" = "1" ]; then
    printf 'Shell integration was skipped.\n'
  else
    printf 'Run: sessionhub help or csx/clx/opx doctor\n'
  fi
}

main "$@"
