#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# build_deb.sh — Script para gerar pacote Debian (.deb) do Zashterminal
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

PACKAGE_NAME="zashterminal"
OUTPUT_DIR="${REPO_ROOT}/dist"
BUILD_DIR="${REPO_ROOT}/.deb-build"
CLEAN_CACHE=false
TARGET_ARCH="all"

log() { echo "[$(date +%H:%M:%S)] [DEB-BUILD] $*"; }
warn() { echo "[$(date +%H:%M:%S)] [DEB-BUILD] WARNING: $*" >&2; }
die() { echo "[$(date +%H:%M:%S)] [DEB-BUILD] ERROR: $*" >&2; exit 1; }

usage() {
  cat <<EOF
Uso: $0 [opções]

Gera o pacote Debian (.deb) para o Zashterminal.

Opções:
  -c, --clean-cache      Remove a pasta de cache temporária (.deb-build) após a compilação.
  -o, --output-dir DIR   Define o diretório de saída do .deb (padrão: dist/).
  -a, --arch ARCH        Define a arquitetura do pacote (padrão: all).
  -h, --help             Exibe esta mensagem de ajuda.

Exemplos:
  $0 --clean-cache
  $0 -o /tmp/meus-pacotes
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
    -a|--arch)
      TARGET_ARCH="$2"
      shift 2
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

# Require dpkg-deb
if ! command -v dpkg-deb >/dev/null 2>&1; then
  die "dpkg-deb não encontrado. Instale dpkg ou dpkg-dev (sudo apt install dpkg-dev)."
fi

# Detect version
VERSION=""
if [ -f "${REPO_ROOT}/scripts/sync_version.py" ]; then
  VERSION="$(python3 "${REPO_ROOT}/scripts/sync_version.py" --print-current 2>/dev/null || true)"
fi
if [ -z "$VERSION" ] && [ -f "${REPO_ROOT}/src/zashterminal/settings/config.py" ]; then
  VERSION="$(grep -Po 'APP_VERSION\s*=\s*"\K[^"]+' "${REPO_ROOT}/src/zashterminal/settings/config.py" || true)"
fi
if [ -z "$VERSION" ]; then
  VERSION="0.8.17"
fi

log "Iniciando empacotamento .deb do ${PACKAGE_NAME} v${VERSION} (${TARGET_ARCH})..."

STAGE="${BUILD_DIR}/zashterminal_${VERSION}_${TARGET_ARCH}"
rm -rf "${STAGE}"
mkdir -p "${STAGE}/DEBIAN"
mkdir -p "${STAGE}/usr/bin"
mkdir -p "${STAGE}/usr/lib/${PACKAGE_NAME}"
mkdir -p "${STAGE}/usr/share/applications"
mkdir -p "${STAGE}/usr/share/icons/hicolor/scalable/apps"
mkdir -p "${STAGE}/usr/share/pixmaps"
mkdir -p "${STAGE}/usr/share/polkit-1/actions"
mkdir -p "${STAGE}/usr/share/kio/servicemenus"
mkdir -p "${STAGE}/usr/share/nautilus-python/extensions"
mkdir -p "${STAGE}/usr/share/nemo/actions"
mkdir -p "${STAGE}/usr/share/locale"
mkdir -p "${OUTPUT_DIR}"

log "Copiando arquivos de código-fonte e recursos..."
cp -r "${REPO_ROOT}/src/zashterminal/"* "${STAGE}/usr/lib/${PACKAGE_NAME}/"
cp "${REPO_ROOT}/usr/bin/zashterminal" "${STAGE}/usr/bin/${PACKAGE_NAME}"
chmod 755 "${STAGE}/usr/bin/${PACKAGE_NAME}"

# Copy assets
if [ -d "${REPO_ROOT}/usr/share" ]; then
  cp -r "${REPO_ROOT}/usr/share/"* "${STAGE}/usr/share/" 2>/dev/null || true
fi

# Compile locales
if command -v msgfmt >/dev/null 2>&1 && [ -d "${REPO_ROOT}/locale" ]; then
  log "Compilando arquivos de tradução (.po -> .mo)..."
  for po_file in "${REPO_ROOT}/locale"/*.po; do
    [ -f "$po_file" ] || continue
    lang="$(basename "$po_file" .po)"
    target_dir="${STAGE}/usr/share/locale/${lang}/LC_MESSAGES"
    mkdir -p "${target_dir}"
    msgfmt "$po_file" -o "${target_dir}/${PACKAGE_NAME}.mo" 2>/dev/null || true
  done
fi

# Calculate installed size in KB
INSTALLED_SIZE="$(du -sk "${STAGE}" | cut -f1)"

# Generate DEBIAN/control
log "Gerando DEBIAN/control..."
cat <<EOF > "${STAGE}/DEBIAN/control"
Package: ${PACKAGE_NAME}
Version: ${VERSION}
Section: utils
Priority: optional
Architecture: ${TARGET_ARCH}
Maintainer: Leonardo Berbert <leo4berbert@gmail.com>
Homepage: https://github.com/VaGNaroK/zashterminal-Fork
Installed-Size: ${INSTALLED_SIZE}
Depends: python3 (>= 3.9), python3-gi, python3-gi-cairo, python3-cairo, gir1.2-gtk-4.0, gir1.2-adw-1, gir1.2-vte-3.91, gir1.2-secret-1, libgtk-4-1, libadwaita-1-0, libvte-2.91-gtk4-0, libsecret-1-0, python3-requests, python3-psutil, python3-regex, python3-pygments, rsync, sshpass
Recommends: python3-pycryptodomex | python3-crypto, gettext
Description: Modern and secure GTK4/Libadwaita terminal emulator with AI integration
 Zashterminal is a fast, tabbed, customizable terminal emulator built on GTK4,
 Libadwaita and VTE, featuring AI Assistant and secure agent workflows.
EOF

# Generate postinst
cat <<'EOF' > "${STAGE}/DEBIAN/postinst"
#!/bin/sh
set -e
if [ "$1" = "configure" ]; then
  if command -v python3 >/dev/null 2>&1; then
    python3 -m compileall /usr/lib/zashterminal/ >/dev/null 2>&1 || true
  fi
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor 2>/dev/null || true
  fi
  if command -v update-mime-database >/dev/null 2>&1; then
    update-mime-database /usr/share/mime >/dev/null 2>&1 || true
  fi
fi
exit 0
EOF
chmod 755 "${STAGE}/DEBIAN/postinst"

# Generate postrm
cat <<'EOF' > "${STAGE}/DEBIAN/postrm"
#!/bin/sh
set -e
if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
  fi
  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor 2>/dev/null || true
  fi
fi
exit 0
EOF
chmod 755 "${STAGE}/DEBIAN/postrm"

DEB_FILE="${OUTPUT_DIR}/${PACKAGE_NAME}_${VERSION}_${TARGET_ARCH}.deb"
log "Construindo pacote com dpkg-deb em ${DEB_FILE}..."
dpkg-deb --build --root-owner-group "${STAGE}" "${DEB_FILE}"

if [ "${CLEAN_CACHE}" = true ]; then
  log "Limpando diretório de cache temporário (${BUILD_DIR})..."
  rm -rf "${BUILD_DIR}"
fi

log "✅ Pacote .deb gerado com sucesso: ${DEB_FILE}"
log "👉 Para instalar: sudo apt install \"${DEB_FILE}\" (ou: sudo dpkg -i \"${DEB_FILE}\")"
