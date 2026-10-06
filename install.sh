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

usage() {
  cat <<EOF
Uso: $0 [comando|opção]

Gerenciador e instalador oficial do OnyxSH.

Comandos:
  install, auto, local   Detecta a distribuição e instala o formato ideal (.deb ou Flatpak) [padrão]
  deb                    Força a compilação e instalação via pacote Debian (.deb)
  flatpak                Força a compilação e instalação via pacote Flatpak
  uninstall              Remove todas as instalações locais do OnyxSH (.deb, Flatpak e lançadores)

Opções:
  -h, --help, help       Exibe esta mensagem de ajuda.

Exemplos:
  $0
  $0 deb
  $0 flatpak
  $0 uninstall
EOF
}

detect_distro() {
  if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    echo "${ID:-unknown}"
  else
    echo "unknown"
  fi
}

is_debian_compatible() {
  local distro="$1"
  if [[ "$distro" =~ ^(ubuntu|debian|linuxmint|pop|elementary|zorin|kali|raspbian)$ ]]; then
    return 0
  fi
  if [ -r /etc/os-release ] && grep -qiE "ID_LIKE=.*(debian|ubuntu)" /etc/os-release 2>/dev/null; then
    return 0
  fi
  if command -v apt-get >/dev/null 2>&1 && command -v dpkg >/dev/null 2>&1; then
    return 0
  fi
  return 1
}

ensure_sudo() {
  if [ "${EUID:-$(id -u)}" -eq 0 ]; then
    return 0
  fi
  if ! command -v sudo >/dev/null 2>&1; then
    die "Privilégios de superusuário (root) são necessários, mas o comando 'sudo' não foi encontrado."
  fi
  log "Validando permissões de administrador (sudo)..."
  if ! sudo -v; then
    die "Falha na autenticação via sudo. Abortando instalação."
  fi
}

install_deb() {
  local build_script="${REPO_ROOT}/scripts/build_deb.sh"
  [ -f "$build_script" ] || die "Script de compilação não encontrado: ${build_script}"

  ensure_sudo

  log "Preparando instalação via pacote Debian nativo (.deb)..."
  if [ ! -x "$build_script" ]; then
    chmod +x "$build_script" 2>/dev/null || true
  fi

  "$build_script" --clean-cache

  local deb_file=""
  for f in "${REPO_ROOT}/dist"/onyxsh_*.deb; do
    [ -e "$f" ] || continue
    if [ -z "$deb_file" ] || [ "$f" -nt "$deb_file" ]; then
      deb_file="$f"
    fi
  done

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
  local build_script="${REPO_ROOT}/scripts/build_flatpak.sh"
  [ -f "$build_script" ] || die "Script de compilação não encontrado: ${build_script}"

  if ! command -v flatpak >/dev/null 2>&1; then
    die "O comando 'flatpak' não está instalado no sistema. Por favor instale o Flatpak antes de prosseguir."
  fi

  log "Preparando instalação via pacote Flatpak..."
  if [ ! -x "$build_script" ]; then
    chmod +x "$build_script" 2>/dev/null || true
  fi

  "$build_script" --clean-cache --install
  log "✅ OnyxSH instalado com sucesso via Flatpak!"
}

uninstall_all() {
  ensure_sudo
  log "Desinstalando OnyxSH..."

  if command -v apt-get >/dev/null 2>&1 && dpkg -s onyxsh >/dev/null 2>&1; then
    log "Removendo pacote Debian 'onyxsh'..."
    sudo apt-get remove -y onyxsh || true
  fi

  if command -v flatpak >/dev/null 2>&1; then
    if flatpak info io.github.vagnarok.OnyxSH >/dev/null 2>&1; then
      log "Removendo pacote Flatpak 'io.github.vagnarok.OnyxSH'..."
      flatpak uninstall -y io.github.vagnarok.OnyxSH 2>/dev/null || true
    fi
  fi

  if [ -f "/usr/local/bin/onyxsh" ]; then
    log "Removendo /usr/local/bin/onyxsh..."
    sudo rm -f /usr/local/bin/onyxsh
  fi

  log "✅ Desinstalação concluída."
}

main() {
  local mode="${1:-install}"
  local distro
  distro="$(detect_distro)"

  case "$mode" in
    -h|--help|help)
      usage
      exit 0
      ;;
    install|local|auto)
      if is_debian_compatible "$distro"; then
        install_deb
      elif command -v flatpak >/dev/null 2>&1; then
        install_flatpak
      else
        die "Distribuição '${distro}' não suporta pacotes .deb nativos e 'flatpak' não foi encontrado. Por favor instale o Flatpak (ex: via gerenciador de pacotes da sua distro) ou utilize um sistema Debian/Ubuntu."
      fi
      ;;
    deb)
      if ! is_debian_compatible "$distro"; then
        warn "Aviso: Sua distribuição '${distro}' não foi identificada como Debian-compatível."
      fi
      install_deb
      ;;
    flatpak)
      install_flatpak
      ;;
    uninstall)
      uninstall_all
      ;;
    *)
      warn "Comando ou opção desconhecida: ${mode}"
      usage
      exit 1
      ;;
  esac
}

main "$@"
