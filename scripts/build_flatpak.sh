#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# build_flatpak.sh — Script para gerar pacote Flatpak (.flatpak) do Zashterminal
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

PACKAGE_NAME="zashterminal"
APP_ID="org.leoberbert.zashterminal"
MANIFEST_FILE="${REPO_ROOT}/manifests/${APP_ID}.yaml"
OUTPUT_DIR="${REPO_ROOT}/dist"
BUILD_DIR="${REPO_ROOT}/.flatpak-build"
BUILDER_CACHE="${REPO_ROOT}/.flatpak-builder"
REPO_DIR="${REPO_ROOT}/.flatpak-repo"
CLEAN_CACHE=false
INSTALL_SDK=false
INSTALL_BUNDLE=false

log() { echo "[$(date +%H:%M:%S)] [FLATPAK-BUILD] $*"; }
warn() { echo "[$(date +%H:%M:%S)] [FLATPAK-BUILD] WARNING: $*" >&2; }
die() { echo "[$(date +%H:%M:%S)] [FLATPAK-BUILD] ERROR: $*" >&2; exit 1; }

usage() {
  cat <<EOF
Uso: $0 [opções]

Gera o pacote Flatpak bundle (.flatpak) para o Zashterminal.

Opções:
  -c, --clean-cache      Remove as pastas de cache temporárias (.flatpak-builder, .flatpak-build, .flatpak-repo) após a compilação.
  -o, --output-dir DIR   Define o diretório de saída do .flatpak (padrão: dist/).
  --install-sdk          Instala automaticamente o GNOME SDK/Platform (46) via Flathub caso não estejam presentes.
  --install              Instala o bundle localmente após a compilação (flatpak install).
  -h, --help             Exibe esta mensagem de ajuda.

Exemplos:
  $0 --clean-cache
  $0 --install-sdk --clean-cache
EOF
}

# Parse options
while [[ $# -gt 0 ]]; do
  case "$1" in
    -c|--clean-cache)
      CLEAN_CACHE=true
      shift
      ;;
    -o|--output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --install-sdk)
      INSTALL_SDK=true
      shift
      ;;
    --install)
      INSTALL_BUNDLE=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "Opção desconhecida: $1 (use --help para ver opções)"
      ;;
  esac
done

# Check tools
if ! command -v flatpak >/dev/null 2>&1; then
  die "flatpak não encontrado. Instale o flatpak no sistema (sudo apt install flatpak / sudo pacman -S flatpak)."
fi

if ! command -v flatpak-builder >/dev/null 2>&1; then
  die "flatpak-builder não encontrado. Instale flatpak-builder (sudo apt install flatpak-builder / sudo pacman -S flatpak-builder)."
fi

if [ ! -f "${MANIFEST_FILE}" ]; then
  die "Manifesto não encontrado: ${MANIFEST_FILE}"
fi

# Detect version
VERSION=""
if [ -f "${REPO_ROOT}/scripts/sync_version.py" ]; then
  VERSION="$(python3 "${REPO_ROOT}/scripts/sync_version.py" --print-current 2>/dev/null || true)"
fi
if [ -z "$VERSION" ]; then
  VERSION="0.8.17"
fi

# Check if GNOME Sdk / Platform 46 are installed
has_sdk=false
has_platform=false

if flatpak list | grep -q "org.gnome.Sdk.*46"; then
  has_sdk=true
fi
if flatpak list | grep -q "org.gnome.Platform.*46"; then
  has_platform=true
fi

if [ "$has_sdk" = false ] || [ "$has_platform" = false ]; then
  log "O SDK ou Runtime do GNOME 46 (org.gnome.Sdk//46 / org.gnome.Platform//46) não está instalado no sistema."
  should_install=true
  if [ -t 0 ] && [ "${INSTALL_SDK}" = false ]; then
    read -rp "Deseja baixar e instalar o GNOME Sdk/Platform 46 do Flathub agora? [S/n]: " ans
    ans="$(echo "$ans" | tr '[:upper:]' '[:lower:]')"
    if [ "$ans" = "n" ] || [ "$ans" = "nao" ] || [ "$ans" = "não" ]; then
      should_install=false
    fi
  fi

  if [ "$should_install" = true ]; then
    log "Configurando repositório Flathub e instalando GNOME Sdk e Platform 46..."
    flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo || true
    flatpak install --user -y flathub org.gnome.Sdk//46 org.gnome.Platform//46
  else
    die "Compilação cancelada: org.gnome.Sdk//46 é obrigatório para compilar o Flatpak."
  fi
fi

log "Iniciando compilação Flatpak do ${APP_ID} v${VERSION}..."
mkdir -p "${OUTPUT_DIR}"
mkdir -p "${REPO_DIR}"

log "Executando flatpak-builder..."
flatpak-builder \
  --force-clean \
  --user \
  --install-deps-from=flathub \
  --repo="${REPO_DIR}" \
  --state-dir="${BUILDER_CACHE}" \
  "${BUILD_DIR}" \
  "${MANIFEST_FILE}"

BUNDLE_FILE="${OUTPUT_DIR}/${PACKAGE_NAME}_${VERSION}.flatpak"
log "Exportando bundle Flatpak em ${BUNDLE_FILE}..."
flatpak build-bundle "${REPO_DIR}" "${BUNDLE_FILE}" "${APP_ID}"

if [ "${INSTALL_BUNDLE}" = true ]; then
  log "Instalando bundle localmente..."
  flatpak install -y --user --bundle "${BUNDLE_FILE}"
fi

if [ "${CLEAN_CACHE}" = true ]; then
  log "Limpando diretórios de cache temporário do flatpak-builder..."
  rm -rf "${BUILD_DIR}" "${BUILDER_CACHE}" "${REPO_DIR}"
fi

log "✅ Pacote Flatpak gerado com sucesso: ${BUNDLE_FILE}"
log "👉 Para instalar: flatpak install --user -y \"${BUNDLE_FILE}\""
log "👉 Para executar: flatpak run ${APP_ID}"
