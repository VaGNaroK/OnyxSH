#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# install.sh — Instalador de compatibilidade e wrapper oficial do OnyxSH
# Suporta chamadas diretas e legado de atualização automática (INSTALL_MODE=local)
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="${SCRIPT_DIR}"

log() { echo "[$(date +%H:%M:%S)] [INSTALLER] $*"; }
warn() { echo "[$(date +%H:%M:%S)] [INSTALLER] WARNING: $*" >&2; }
die() { echo "[$(date +%H:%M:%S)] [INSTALLER] ERROR: $*" >&2; exit 1; }

detect_distro() {
  if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    echo "${ID:-unknown}"
  else
    echo "unknown"
  fi
}

install_deb() {
  log "Preparando instalação via pacote Debian nativo (.deb)..."
  if [ ! -x "${REPO_ROOT}/scripts/build_deb.sh" ]; then
    chmod +x "${REPO_ROOT}/scripts/build_deb.sh" 2>/dev/null || true
  fi

  "${REPO_ROOT}/scripts/build_deb.sh" --clean-cache

  local deb_file
  deb_file="$(ls -t "${REPO_ROOT}/dist"/onyxsh_*.deb 2>/dev/null | head -n1 || true)"
  if [ -z "$deb_file" ] || [ ! -f "$deb_file" ]; then
    die "Falha ao localizar o pacote .deb gerado em ${REPO_ROOT}/dist"
  fi

  log "Instalando pacote oficial: ${deb_file}"
  sudo apt-get update -y || true
  sudo apt-get install -y "${deb_file}" || sudo dpkg -i "${deb_file}" || sudo apt-get install -f -y

  # Migração de lançador legado (/opt/onyxsh -> /usr/bin/onyxsh)
  if [ -f "/usr/local/bin/onyxsh" ]; then
    if grep -q "/opt/onyxsh" "/usr/local/bin/onyxsh" 2>/dev/null; then
      log "Removendo wrapper legado /usr/local/bin/onyxsh para usar o novo binário oficial /usr/bin/onyxsh..."
      sudo rm -f "/usr/local/bin/onyxsh"
    fi
  fi

  log "✅ OnyxSH instalado e atualizado com sucesso via .deb!"
}

install_flatpak() {
  log "Preparando instalação via pacote Flatpak..."
  if [ ! -x "${REPO_ROOT}/scripts/build_flatpak.sh" ]; then
    chmod +x "${REPO_ROOT}/scripts/build_flatpak.sh" 2>/dev/null || true
  fi

  "${REPO_ROOT}/scripts/build_flatpak.sh" --clean-cache --install
  log "✅ OnyxSH instalado com sucesso via Flatpak!"
}

main() {
  local mode="${1:-install}"
  local distro
  distro="$(detect_distro)"

  case "$mode" in
    install|local|auto)
      if [[ "$distro" =~ ^(ubuntu|debian|linuxmint|pop|elementary|zorin)$ ]]; then
        install_deb
      elif command -v flatpak >/dev/null 2>&1; then
        install_flatpak
      else
        install_deb
      fi
      ;;
    deb)
      install_deb
      ;;
    flatpak)
      install_flatpak
      ;;
    uninstall)
      log "Desinstalando OnyxSH..."
      if command -v apt-get >/dev/null 2>&1; then
        sudo apt-get remove -y onyxsh || true
      fi
      if command -v flatpak >/dev/null 2>&1; then
        flatpak uninstall -y io.github.vagnarok.OnyxSH 2>/dev/null || true
      fi
      if [ -f "/usr/local/bin/onyxsh" ]; then
        sudo rm -f /usr/local/bin/onyxsh
      fi
      log "Desinstalação concluída."
      ;;
    *)
      if [[ "$distro" =~ ^(ubuntu|debian|linuxmint|pop|elementary|zorin)$ ]]; then
        install_deb
      else
        install_flatpak
      fi
      ;;
  esac
}

main "${1:-install}"
