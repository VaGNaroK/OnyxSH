#!/usr/bin/env bash
set -euo pipefail

PACKAGE_NAME="zashterminal"
DESKTOP_ID="org.leoberbert.zashterminal"
REPO_URL="https://github.com/VaGNaroK/zashterminal-Fork.git"

INSTALL_ROOT="/opt/${PACKAGE_NAME}"
VENV_DIR="${INSTALL_ROOT}/venv"
BIN_PATH="/usr/local/bin/${PACKAGE_NAME}"
APP_DIR="/usr/share/applications"
ICON_DIR="/usr/share/icons/hicolor/scalable/apps"
PIXMAP_DIR="/usr/share/pixmaps"
LOCALE_BASE_DIR="/usr/share/locale"

MANIFEST_DIR="/var/lib/${PACKAGE_NAME}"
MANIFEST_FILE="${MANIFEST_DIR}/installed-files.txt"

DANGER_TOKEN="REMOVER-DEPENDENCIAS"

DISTRO_FAMILY=""
PKG_MANAGER=""
INSTALL_MODE="${INSTALL_MODE:-auto}"   # auto | local | aur
ARCH_AUR_HELPER="${ARCH_AUR_HELPER:-}" # yay | paru
NIX_FEATURES="nix-command flakes"

BASE_PACKAGES=()
PYTHON_RUNTIME_PACKAGES=()
OPTIONAL_PYTHON_PACKAGES=()

log() { echo "[$(date +%H:%M:%S)] $*"; }
warn() { echo "[$(date +%H:%M:%S)] WARNING: $*" >&2; }
die() { echo "[$(date +%H:%M:%S)] ERROR: $*" >&2; exit 1; }

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "Required command not found: $1"
}

detect_system() {
  [ -r /etc/os-release ] || die "/etc/os-release not found; unsupported Linux distribution."

  # shellcheck disable=SC1091
  . /etc/os-release

  local key="${ID_LIKE:-} ${ID:-}"

  case " ${key} " in
    *" arch "* | *" manjaro "*)
      DISTRO_FAMILY="arch"
      PKG_MANAGER="pacman"
      ;;
    *" ubuntu "* | *" debian "* | *" linuxmint "* | *" pop "*)
      DISTRO_FAMILY="debian"
      PKG_MANAGER="apt"
      ;;
    *" fedora "* | *" rhel "* | *" centos "* | *" rocky "* | *" alma "*)
      DISTRO_FAMILY="fedora"
      PKG_MANAGER="dnf"
      ;;
    *" opensuse "* | *" suse "*)
      DISTRO_FAMILY="suse"
      PKG_MANAGER="zypper"
      ;;
    *" nixos "*)
      DISTRO_FAMILY="nixos"
      PKG_MANAGER="nix"
      ;;
    *)
      die "Unsupported distro (${ID:-unknown}). Add package mapping in install.sh."
      ;;
  esac

  log "Detected distro: ${PRETTY_NAME:-${ID:-unknown}} (${DISTRO_FAMILY}, ${PKG_MANAGER})"
}

define_dependency_lists() {
  BASE_PACKAGES=()
  PYTHON_RUNTIME_PACKAGES=()
  OPTIONAL_PYTHON_PACKAGES=()

  case "$DISTRO_FAMILY" in
    arch)
      BASE_PACKAGES=(
        python python-pip git rsync sshpass gettext
        gtk4 libadwaita vte4 libsecret
        gobject-introspection python-gobject python-cairo
      )
      PYTHON_RUNTIME_PACKAGES=(
        python-requests python-psutil python-regex python-pygments
        python-py7zr python-setproctitle python-cryptography
      )
      ;;
    debian)
      BASE_PACKAGES=(
        python3 python3-venv python3-pip git rsync sshpass gettext
        libgtk-4-1 libadwaita-1-0 libvte-2.91-gtk4-0 libsecret-1-0
        gir1.2-gtk-4.0 gir1.2-adw-1 gir1.2-vte-3.91 gir1.2-secret-1
        python3-gi python3-gi-cairo python3-cairo
      )
      PYTHON_RUNTIME_PACKAGES=(
        python3-requests python3-psutil python3-regex python3-pygments
      )
      OPTIONAL_PYTHON_PACKAGES=(
        python3-py7zr python3-setproctitle python3-cryptography
      )
      ;;
    fedora)
      BASE_PACKAGES=(
        python3 python3-pip git rsync sshpass gettext
        gtk4 libadwaita vte291-gtk4 libsecret gobject-introspection
        python3-gobject python3-cairo
      )
      PYTHON_RUNTIME_PACKAGES=(
        python3-requests python3-psutil python3-pygments
        python3-cryptography
      )
      OPTIONAL_PYTHON_PACKAGES=(
        python3-regex python3-py7zr python3-setproctitle
      )
      ;;
    suse)
      BASE_PACKAGES=(
        python3 python3-pip git rsync sshpass gettext-tools
        gtk4 libadwaita-1-0 libvte-2_91-0 libsecret-1-0
        typelib-1_0-Gtk-4_0 typelib-1_0-Adw-1 typelib-1_0-Vte-3_91 typelib-1_0-Secret-1
        python3-gobject python3-cairo
      )
      PYTHON_RUNTIME_PACKAGES=(
        python3-requests python3-psutil python3-regex python3-Pygments
      )
      OPTIONAL_PYTHON_PACKAGES=(
        python3-py7zr python3-setproctitle python3-cryptography
      )
      ;;
    nixos)
      return 0
      ;;
    *)
      die "No dependency mapping for distro family: $DISTRO_FAMILY"
      ;;
  esac
}

pkg_update_once() {
  case "$PKG_MANAGER" in
    apt) sudo apt update ;;
    pacman) sudo pacman -Syu --noconfirm ;;
    dnf | zypper) : ;;
    *) die "Unsupported package manager: $PKG_MANAGER" ;;
  esac
}

install_pkg_group() {
  local group_name="$1"
  local fail_on_error="$2"
  shift 2

  local packages=("$@")
  local failed=()

  [ "${#packages[@]}" -gt 0 ] || return 0

  log "Installing ${group_name} packages (${#packages[@]})..."

  if [ "$fail_on_error" != "true" ]; then
    for pkg in "${packages[@]}"; do
      case "$PKG_MANAGER" in
        apt) sudo apt install -y "$pkg" >/dev/null 2>&1 || failed+=("$pkg") ;;
        pacman) sudo pacman -S --needed --noconfirm "$pkg" >/dev/null 2>&1 || failed+=("$pkg") ;;
        dnf) sudo dnf install -y "$pkg" >/dev/null 2>&1 || failed+=("$pkg") ;;
        zypper) sudo zypper --non-interactive install "$pkg" >/dev/null 2>&1 || failed+=("$pkg") ;;
        *) die "Unsupported package manager: $PKG_MANAGER" ;;
      esac
    done

    if [ "${#failed[@]}" -gt 0 ]; then
      warn "Optional packages not installed for ${DISTRO_FAMILY}: ${failed[*]}"
    fi

    return 0
  fi

  case "$PKG_MANAGER" in
    apt) sudo apt install -y "${packages[@]}" || failed=("${packages[@]}") ;;
    pacman) sudo pacman -S --needed --noconfirm "${packages[@]}" || failed=("${packages[@]}") ;;
    dnf) sudo dnf install -y "${packages[@]}" || failed=("${packages[@]}") ;;
    zypper) sudo zypper --non-interactive install "${packages[@]}" || failed=("${packages[@]}") ;;
    *) die "Unsupported package manager: $PKG_MANAGER" ;;
  esac

  if [ "${#failed[@]}" -gt 0 ]; then
    die "Failed to install ${group_name} packages. Review package names for ${DISTRO_FAMILY}."
  fi
}

install_system_dependencies() {
  define_dependency_lists
  pkg_update_once

  if [ "${#BASE_PACKAGES[@]}" -gt 0 ]; then
    install_pkg_group "required" "true" "${BASE_PACKAGES[@]}"
  fi

  if [ "${#PYTHON_RUNTIME_PACKAGES[@]}" -gt 0 ]; then
    install_pkg_group "python-runtime" "true" "${PYTHON_RUNTIME_PACKAGES[@]}"
  fi

  if [ "${#OPTIONAL_PYTHON_PACKAGES[@]}" -gt 0 ]; then
    install_pkg_group "python-optional" "false" "${OPTIONAL_PYTHON_PACKAGES[@]}"
  fi
}

ensure_runtime_prereqs() {
  require_cmd sudo

  if command -v python3 >/dev/null 2>&1; then
    return 0
  fi

  if command -v python >/dev/null 2>&1; then
    return 0
  fi

  die "Python is not available after dependency installation."
}

python_cmd() {
  if command -v python3 >/dev/null 2>&1; then
    echo "python3"
  else
    echo "python"
  fi
}

find_arch_aur_helper() {
  if [ -n "${ARCH_AUR_HELPER}" ]; then
    if command -v "${ARCH_AUR_HELPER}" >/dev/null 2>&1; then
      echo "${ARCH_AUR_HELPER}"
      return 0
    fi
    return 1
  fi

  if command -v paru >/dev/null 2>&1; then
    echo "paru"
    return 0
  fi

  if command -v yay >/dev/null 2>&1; then
    echo "yay"
    return 0
  fi

  return 1
}

choose_arch_aur_helper() {
  local helper

  if helper="$(find_arch_aur_helper)"; then
    echo "$helper"
    return 0
  fi

  die "AUR helper not found (expected yay or paru)."
}

resolve_install_mode() {
  case "${INSTALL_MODE}" in
    auto | local | aur) ;;
    *)
      die "Invalid INSTALL_MODE='${INSTALL_MODE}'. Use: auto | local | aur"
      ;;
  esac

  if [ "${DISTRO_FAMILY}" = "nixos" ]; then
    log "Install mode: nix-profile (flake)"
    return 0
  fi

  if [ "${DISTRO_FAMILY}" != "arch" ]; then
    log "Install mode: local (AUR mode only applies to Arch/Manjaro)"
    return 0
  fi

  local helper

  if [ "${INSTALL_MODE}" = "aur" ]; then
    helper="$(choose_arch_aur_helper)" || die "INSTALL_MODE=aur requires yay or paru."
    log "Install mode: aur (${helper})"
    return 0
  fi

  if [ "${INSTALL_MODE}" = "local" ]; then
    log "Install mode: local"
    return 0
  fi

  if helper="$(find_arch_aur_helper)"; then
    log "Install mode: aur (${helper}) [auto]"
  else
    log "Install mode: local [auto] (yay/paru not found)"
  fi
}

install_arch_via_aur() {
  local helper
  helper="$(choose_arch_aur_helper)" || die "AUR helper not found (expected yay or paru)."

  log "Installing ${PACKAGE_NAME} via AUR using ${helper}..."
  "${helper}" -S --noconfirm "${PACKAGE_NAME}"
}

nixos_features_configured() {
  if [ -r /etc/nix/nix.conf ] &&
    grep -Eq '^[[:space:]]*experimental-features[[:space:]]*=' /etc/nix/nix.conf &&
    grep -Eq '^[[:space:]]*experimental-features[[:space:]]*=.*nix-command' /etc/nix/nix.conf &&
    grep -Eq '^[[:space:]]*experimental-features[[:space:]]*=.*flakes' /etc/nix/nix.conf; then
    return 0
  fi

  if [ -r /etc/nixos/configuration.nix ] &&
    grep -Eq 'nix\.settings\.experimental-features[[:space:]]*=' /etc/nixos/configuration.nix &&
    grep -Eq 'nix\.settings\.experimental-features[[:space:]]*=.*"nix-command"' /etc/nixos/configuration.nix &&
    grep -Eq 'nix\.settings\.experimental-features[[:space:]]*=.*"flakes"' /etc/nixos/configuration.nix; then
    return 0
  fi

  return 1
}

warn_if_nixos_features_not_configured() {
  [ "${DISTRO_FAMILY}" = "nixos" ] || return 0

  if nixos_features_configured; then
    return 0
  fi

  warn "NixOS experimental features not found in system config."
  warn "Recommended permanent setting in /etc/nixos/configuration.nix:"
  warn '  nix.settings.experimental-features = [ "nix-command" "flakes" ];'
  warn "Then apply with: sudo nixos-rebuild switch"
  warn "Continuing using temporary flags: --extra-experimental-features \"${NIX_FEATURES}\""
}

prepare_source() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

  if [ -f "${script_dir}/pyproject.toml" ] && [ -d "${script_dir}/src/zashterminal" ]; then
    echo "$script_dir"
    return 0
  fi

  require_cmd git

  local tmp_dir
  tmp_dir="$(mktemp -d)"

  log "Cloning ${PACKAGE_NAME} source into temporary directory..." >&2
  git clone --depth 1 "$REPO_URL" "${tmp_dir}/${PACKAGE_NAME}" >/dev/null

  echo "${tmp_dir}/${PACKAGE_NAME}"
}

record_file() {
  local target="$1"

  sudo mkdir -p "$MANIFEST_DIR"

  if ! sudo test -f "$MANIFEST_FILE"; then
    sudo touch "$MANIFEST_FILE"
    sudo chmod 644 "$MANIFEST_FILE" 2>/dev/null || true
  fi

  if ! sudo grep -qxF -- "$target" "$MANIFEST_FILE" 2>/dev/null; then
    printf '%s\n' "$target" | sudo tee -a "$MANIFEST_FILE" >/dev/null
  fi
}

install_nixos_menu_entries() {
  local profile_dir="${HOME}/.nix-profile"
  local profile_desktop="${profile_dir}/share/applications/${DESKTOP_ID}.desktop"
  local profile_icon="${profile_dir}/share/icons/hicolor/scalable/apps/${PACKAGE_NAME}.svg"

  local user_app_dir="${HOME}/.local/share/applications"
  local user_icon_dir="${HOME}/.local/share/icons/hicolor/scalable/apps"
  local user_pixmap_dir="${HOME}/.local/share/pixmaps"

  mkdir -p "${user_app_dir}" "${user_icon_dir}" "${user_pixmap_dir}"

  if [ -f "${profile_desktop}" ]; then
    install -m 644 "${profile_desktop}" "${user_app_dir}/${DESKTOP_ID}.desktop"
  fi

  if [ -f "${profile_icon}" ]; then
    install -m 644 "${profile_icon}" "${user_icon_dir}/${PACKAGE_NAME}.svg"
    install -m 644 "${profile_icon}" "${user_pixmap_dir}/${PACKAGE_NAME}.svg"
  fi

  command -v update-desktop-database >/dev/null 2>&1 &&
    update-desktop-database "${user_app_dir}" >/dev/null 2>&1 || true

  command -v gtk-update-icon-cache >/dev/null 2>&1 &&
    gtk-update-icon-cache "${HOME}/.local/share/icons/hicolor" >/dev/null 2>&1 || true
}

install_nixos_via_flake() {
  local src_dir="$1"

  require_cmd nix
  warn_if_nixos_features_not_configured

  log "Building ${PACKAGE_NAME} via flake..."
  nix --extra-experimental-features "${NIX_FEATURES}" \
    build "${src_dir}#zashterminal"

  log "Installing ${PACKAGE_NAME} to user profile via nix profile..."
  nix --extra-experimental-features "${NIX_FEATURES}" \
    profile add "${src_dir}#zashterminal"

  log "Installing desktop/menu integration for current user..."
  install_nixos_menu_entries

  log "NixOS installation complete (user profile)."
  log "Run: ${PACKAGE_NAME}"
}

compile_locales_if_possible() {
  local src_dir="$1"

  [ -d "${src_dir}/locale" ] || return 0
  command -v msgfmt >/dev/null 2>&1 || return 0

  log "Compiling translation files (.po -> .mo)..."

  local po lang out

  find "${src_dir}/locale" -name '*.po' -print0 | while IFS= read -r -d '' po; do
    lang="$(basename "${po%.po}")"
    out="${LOCALE_BASE_DIR}/${lang}/LC_MESSAGES/${PACKAGE_NAME}.mo"

    sudo mkdir -p "$(dirname "$out")"

    if sudo msgfmt -o "$out" "$po"; then
      record_file "$out" || true
    else
      warn "Failed to compile ${po}"
    fi
  done
}

install_python_app() {
  local src_dir="$1"
  local pybin
  pybin="$(python_cmd)"

  log "Installing application into system venv: ${VENV_DIR}"

  sudo mkdir -p "${INSTALL_ROOT}"
  sudo rm -rf "${VENV_DIR}"

  sudo "${pybin}" -m venv --system-site-packages "${VENV_DIR}"
  sudo "${VENV_DIR}/bin/python" -m pip install --upgrade pip >/dev/null
  sudo "${VENV_DIR}/bin/python" -m pip install --no-deps "${src_dir}" >/dev/null

  # Default extras in the venv.
  sudo "${VENV_DIR}/bin/python" -m pip install \
    requests psutil regex Pygments cryptography py7zr setproctitle >/dev/null ||
    warn "Some Python packages could not be installed in the venv (continuing)."

  record_file "$INSTALL_ROOT"
  record_file "$VENV_DIR"
}

install_launcher() {
  log "Installing launcher to ${BIN_PATH}..."

  sudo tee "${BIN_PATH}" >/dev/null <<EOF
#!/bin/sh
exec "${VENV_DIR}/bin/${PACKAGE_NAME}" "\$@"
EOF

  sudo chmod +x "${BIN_PATH}"
  record_file "${BIN_PATH}"
}

install_desktop_files() {
  local src_dir="$1"
  local f base target

  sudo mkdir -p "${APP_DIR}" "${ICON_DIR}" "${PIXMAP_DIR}"

  if [ -f "${src_dir}/usr/share/applications/${DESKTOP_ID}.desktop" ]; then
    sudo install -Dm644 \
      "${src_dir}/usr/share/applications/${DESKTOP_ID}.desktop" \
      "${APP_DIR}/${DESKTOP_ID}.desktop"
    record_file "${APP_DIR}/${DESKTOP_ID}.desktop"
  fi

  if [ -f "${src_dir}/usr/share/icons/hicolor/scalable/apps/${PACKAGE_NAME}.svg" ]; then
    sudo install -Dm644 \
      "${src_dir}/usr/share/icons/hicolor/scalable/apps/${PACKAGE_NAME}.svg" \
      "${ICON_DIR}/${PACKAGE_NAME}.svg"

    sudo install -Dm644 \
      "${src_dir}/usr/share/icons/hicolor/scalable/apps/${PACKAGE_NAME}.svg" \
      "${PIXMAP_DIR}/${PACKAGE_NAME}.svg"

    record_file "${ICON_DIR}/${PACKAGE_NAME}.svg"
    record_file "${PIXMAP_DIR}/${PACKAGE_NAME}.svg"
  fi

  # File manager integrations (KDE KIO, Nautilus, Nemo)
  if [ -d "${src_dir}/usr/share/kio/servicemenus" ]; then
    sudo mkdir -p "/usr/share/kio/servicemenus"

    for f in "${src_dir}/usr/share/kio/servicemenus"/*.desktop; do
      [ -e "$f" ] || continue
      base="$(basename "$f")"
      target="/usr/share/kio/servicemenus/${base}"

      sudo install -Dm644 "$f" "$target"
      record_file "$target"
    done
  fi

  if [ -d "${src_dir}/usr/share/nautilus-python/extensions" ]; then
    sudo mkdir -p "/usr/share/nautilus-python/extensions"

    for f in "${src_dir}/usr/share/nautilus-python/extensions"/*.py; do
      [ -e "$f" ] || continue
      base="$(basename "$f")"
      target="/usr/share/nautilus-python/extensions/${base}"

      sudo install -Dm644 "$f" "$target"
      record_file "$target"
    done
  fi

  if [ -d "${src_dir}/usr/share/nemo/actions" ]; then
    sudo mkdir -p "/usr/share/nemo/actions"

    for f in "${src_dir}/usr/share/nemo/actions"/*.nemo_action; do
      [ -e "$f" ] || continue
      base="$(basename "$f")"
      target="/usr/share/nemo/actions/${base}"

      sudo install -Dm644 "$f" "$target"
      record_file "$target"
    done
  fi

  # Polkit & Admin Helper integration
  sudo mkdir -p "/usr/lib/${PACKAGE_NAME}"
  record_file "/usr/lib/${PACKAGE_NAME}"

  if [ -f "${src_dir}/src/zashterminal/admin/helper.py" ]; then
    sudo install -Dm755 \
      "${src_dir}/src/zashterminal/admin/helper.py" \
      "/usr/lib/${PACKAGE_NAME}/${PACKAGE_NAME}-admin-helper"

    record_file "/usr/lib/${PACKAGE_NAME}/${PACKAGE_NAME}-admin-helper"
  fi

  if [ -f "${src_dir}/usr/share/polkit-1/actions/${DESKTOP_ID}.policy" ]; then
    sudo install -Dm644 \
      "${src_dir}/usr/share/polkit-1/actions/${DESKTOP_ID}.policy" \
      "/usr/share/polkit-1/actions/${DESKTOP_ID}.policy"

    record_file "/usr/share/polkit-1/actions/${DESKTOP_ID}.policy"
  fi

  sudo update-desktop-database "${APP_DIR}" >/dev/null 2>&1 || true
  sudo gtk-update-icon-cache /usr/share/icons/hicolor >/dev/null 2>&1 || true
}

post_install_notes() {
  log "Installation complete (system-wide with venv)."
  log "  Venv: ${VENV_DIR}"
  log "  Launcher: ${BIN_PATH}"
  log "  Desktop: ${APP_DIR}/${DESKTOP_ID}.desktop"
  log "  Manifest: ${MANIFEST_FILE}"
  log "Run: ${PACKAGE_NAME}"
}

is_arch_package_installed() {
  [ "${DISTRO_FAMILY}" = "arch" ] || return 1
  command -v pacman >/dev/null 2>&1 || return 1

  pacman -Qi "${PACKAGE_NAME}" >/dev/null 2>&1
}

is_nix_profile_installed() {
  [ "${DISTRO_FAMILY}" = "nixos" ] || return 1
  command -v nix >/dev/null 2>&1 || return 1

  local list

  if ! list="$(nix --extra-experimental-features "${NIX_FEATURES}" profile list 2>/dev/null)"; then
    return 1
  fi

  [[ "$list" == *"${PACKAGE_NAME}"* ]]
}

is_local_installed() {
  [ -x "${BIN_PATH}" ] ||
    [ -d "${INSTALL_ROOT}" ] ||
    [ -f "${APP_DIR}/${DESKTOP_ID}.desktop" ] ||
    [ -f "${MANIFEST_FILE}" ]
}

is_nix_user_installed() {
  [ -f "${HOME}/.local/share/applications/${DESKTOP_ID}.desktop" ] ||
    [ -f "${HOME}/.local/share/icons/hicolor/scalable/apps/${PACKAGE_NAME}.svg" ] ||
    [ -f "${HOME}/.local/share/pixmaps/${PACKAGE_NAME}.svg" ]
}

is_installed() {
  case "${DISTRO_FAMILY}" in
    nixos)
      is_nix_profile_installed || is_nix_user_installed
      ;;
    arch)
      is_arch_package_installed || is_local_installed
      ;;
    *)
      is_local_installed
      ;;
  esac
}

installation_state_text() {
  local parts=()

  if is_arch_package_installed; then
    parts+=("pacote Arch/AUR")
  fi

  if is_nix_profile_installed; then
    parts+=("nix profile")
  fi

  if is_local_installed; then
    parts+=("instalação local")
  fi

  if is_nix_user_installed; then
    parts+=("entradas de usuário NixOS")
  fi

  if [ "${#parts[@]}" -eq 0 ]; then
    echo "não instalado"
    return 0
  fi

  local out=""
  local p

  for p in "${parts[@]}"; do
    if [ -n "$out" ]; then
      out+=", "
    fi
    out+="$p"
  done

  echo "$out"
}

ask_yes_no() {
  local prompt="$1"
  local answer

  if ! read -rp "$prompt [s/N]: " answer; then
    return 1
  fi

  [[ "$answer" =~ ^[SsYy] ]]
}

confirm_dangerous_dependency_removal() {
  warn "ATENÇÃO: remover dependências pode quebrar outros programas e o sistema."
  warn "Esta ação remove pacotes que podem ser usados por outros aplicativos."
  warn "Não é recomendada para usuários iniciantes."

  local answer

  if ! read -rp "Digite ${DANGER_TOKEN} para confirmar: " answer; then
    return 1
  fi

  [ "$answer" = "${DANGER_TOKEN}" ]
}

package_is_installed() {
  local pkg="$1"

  case "$PKG_MANAGER" in
    apt)
      dpkg -s "$pkg" >/dev/null 2>&1
      ;;
    pacman)
      pacman -Qi "$pkg" >/dev/null 2>&1
      ;;
    dnf | zypper)
      rpm -q "$pkg" >/dev/null 2>&1
      ;;
    *)
      return 1
      ;;
  esac
}

remove_dependency_packages() {
  define_dependency_lists

  local all_packages=()

  if [ "${#BASE_PACKAGES[@]}" -gt 0 ]; then
    all_packages+=("${BASE_PACKAGES[@]}")
  fi

  if [ "${#PYTHON_RUNTIME_PACKAGES[@]}" -gt 0 ]; then
    all_packages+=("${PYTHON_RUNTIME_PACKAGES[@]}")
  fi

  if [ "${#OPTIONAL_PYTHON_PACKAGES[@]}" -gt 0 ]; then
    all_packages+=("${OPTIONAL_PYTHON_PACKAGES[@]}")
  fi

  if [ "${#all_packages[@]}" -eq 0 ]; then
    log "Nenhuma dependência conhecida para esta distribuição."
    return 0
  fi

  local installed=()
  local pkg

  for pkg in "${all_packages[@]}"; do
    if package_is_installed "$pkg"; then
      installed+=("$pkg")
    fi
  done

  if [ "${#installed[@]}" -eq 0 ]; then
    log "Nenhuma dependência conhecida está atualmente instalada."
    return 0
  fi

  log "As seguintes dependências serão removidas:"
  printf ' - %s\n' "${installed[@]}"

  case "$PKG_MANAGER" in
    apt)
      sudo apt-get purge -y "${installed[@]}" || warn "Falha ao remover algumas dependências via apt."
      log "Se quiser remover pacotes órfãos adicionais, execute manualmente e com cuidado: sudo apt autoremove --purge"
      ;;
    pacman)
      sudo pacman -Rns --noconfirm "${installed[@]}" || warn "Falha ao remover algumas dependências via pacman."
      ;;
    dnf)
      sudo dnf remove -y "${installed[@]}" || warn "Falha ao remover algumas dependências via dnf."
      ;;
    zypper)
      sudo zypper --non-interactive remove "${installed[@]}" || warn "Falha ao remover algumas dependências via zypper."
      ;;
    *)
      warn "Não sei remover dependências para o gerenciador: $PKG_MANAGER"
      ;;
  esac
}

remove_local_manifest() {
  if sudo test -f "$MANIFEST_FILE"; then
    log "Removendo arquivos registrados no manifesto..."

    sudo cat "$MANIFEST_FILE" | while IFS= read -r f; do
      [ -n "$f" ] || continue

      if sudo test -d "$f"; then
        sudo rm -rf "$f" 2>/dev/null || true
      else
        sudo rm -f "$f" 2>/dev/null || true
      fi
    done

    sudo rm -f "$MANIFEST_FILE"
  fi

  sudo rmdir "$MANIFEST_DIR" 2>/dev/null || true
}

remove_known_local_files() {
  log "Removendo arquivos conhecidos do ${PACKAGE_NAME}..."

  sudo rm -rf "${INSTALL_ROOT}"
  sudo rm -f "${BIN_PATH}"

  sudo rm -f "${APP_DIR}/${DESKTOP_ID}.desktop"
  sudo rm -f "${ICON_DIR}/${PACKAGE_NAME}.svg"
  sudo rm -f "${PIXMAP_DIR}/${PACKAGE_NAME}.svg"

  sudo rm -rf "/usr/lib/${PACKAGE_NAME}"
  sudo rm -f "/usr/share/polkit-1/actions/${DESKTOP_ID}.policy"

  if [ -d "${LOCALE_BASE_DIR}" ]; then
    sudo find "${LOCALE_BASE_DIR}" -type f -path "*/LC_MESSAGES/${PACKAGE_NAME}.mo" -delete 2>/dev/null || true
  fi
}

remove_file_manager_integrations_from_source() {
  local src_dir="$1"
  local f base

  if [ -d "${src_dir}/usr/share/kio/servicemenus" ]; then
    for f in "${src_dir}/usr/share/kio/servicemenus"/*.desktop; do
      [ -e "$f" ] || continue
      base="$(basename "$f")"
      sudo rm -f "/usr/share/kio/servicemenus/${base}" 2>/dev/null || true
    done
  fi

  if [ -d "${src_dir}/usr/share/nautilus-python/extensions" ]; then
    for f in "${src_dir}/usr/share/nautilus-python/extensions"/*.py; do
      [ -e "$f" ] || continue
      base="$(basename "$f")"
      sudo rm -f "/usr/share/nautilus-python/extensions/${base}" 2>/dev/null || true
    done
  fi

  if [ -d "${src_dir}/usr/share/nemo/actions" ]; then
    for f in "${src_dir}/usr/share/nemo/actions"/*.nemo_action; do
      [ -e "$f" ] || continue
      base="$(basename "$f")"
      sudo rm -f "/usr/share/nemo/actions/${base}" 2>/dev/null || true
    done
  fi
}

maybe_remove_file_manager_integrations() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

  if [ -f "${script_dir}/pyproject.toml" ] && [ -d "${script_dir}/src/zashterminal" ]; then
    remove_file_manager_integrations_from_source "$script_dir"
  else
    warn "Não foi possível remover integrações de gerenciadores de arquivos automaticamente sem o código-fonte ou manifesto."
    warn "Verifique manualmente, se necessário:"
    warn "  /usr/share/kio/servicemenus"
    warn "  /usr/share/nautilus-python/extensions"
    warn "  /usr/share/nemo/actions"
  fi
}

uninstall_local() {
  local had_manifest=false

  if sudo test -f "$MANIFEST_FILE"; then
    had_manifest=true
  fi

  remove_local_manifest
  remove_known_local_files

  if [ "$had_manifest" = false ]; then
    maybe_remove_file_manager_integrations
  fi

  sudo update-desktop-database "${APP_DIR}" >/dev/null 2>&1 || true
  sudo gtk-update-icon-cache /usr/share/icons/hicolor >/dev/null 2>&1 || true
}

uninstall_nixos() {
  log "Desinstalando ${PACKAGE_NAME} no NixOS..."

  if command -v nix >/dev/null 2>&1; then
    nix --extra-experimental-features "${NIX_FEATURES}" \
      profile remove ".*${PACKAGE_NAME}.*" ||
      warn "Não foi possível remover automaticamente do nix profile. Verifique: nix profile list"
  else
    warn "nix não encontrado."
  fi

  rm -f "${HOME}/.local/share/applications/${DESKTOP_ID}.desktop"
  rm -f "${HOME}/.local/share/icons/hicolor/scalable/apps/${PACKAGE_NAME}.svg"
  rm -f "${HOME}/.local/share/pixmaps/${PACKAGE_NAME}.svg"

  command -v update-desktop-database >/dev/null 2>&1 &&
    update-desktop-database "${HOME}/.local/share/applications" >/dev/null 2>&1 || true

  command -v gtk-update-icon-cache >/dev/null 2>&1 &&
    gtk-update-icon-cache "${HOME}/.local/share/icons/hicolor" >/dev/null 2>&1 || true
}

uninstall_arch_package() {
  local remove_deps="$1"
  local helper
  local args=(-R --noconfirm)

  if [ "$remove_deps" = true ]; then
    args=(-Rns --noconfirm)
  fi

  if helper="$(find_arch_aur_helper)"; then
    log "Removendo pacote ${PACKAGE_NAME} via ${helper}..."
    "${helper}" "${args[@]}" "${PACKAGE_NAME}"
  else
    log "Removendo pacote ${PACKAGE_NAME} via pacman..."
    sudo pacman "${args[@]}" "${PACKAGE_NAME}"
  fi
}

do_install() {
  if is_installed; then
    warn "Parece que ${PACKAGE_NAME} já está instalado."
    do_status

    if ! ask_yes_no "Deseja continuar/reinstalar?"; then
      log "Instalação cancelada."
      return 0
    fi
  fi

  resolve_install_mode

  local src_dir
  src_dir="$(prepare_source)"

  if [ "${DISTRO_FAMILY}" = "nixos" ]; then
    install_nixos_via_flake "${src_dir}"
    return 0
  fi

  if [ "${DISTRO_FAMILY}" = "arch" ]; then
    local helper

    if helper="$(find_arch_aur_helper)" && [ "${INSTALL_MODE}" != "local" ]; then
      install_arch_via_aur
      log "Installed via AUR (${helper})."
      return 0
    fi
  fi

  # Local/system-wide installation
  sudo mkdir -p "$MANIFEST_DIR"
  sudo rm -f "$MANIFEST_FILE"
  sudo touch "$MANIFEST_FILE"
  sudo chmod 644 "$MANIFEST_FILE" 2>/dev/null || true

  install_system_dependencies
  ensure_runtime_prereqs
  install_python_app "${src_dir}"
  install_launcher
  install_desktop_files "${src_dir}"
  compile_locales_if_possible "${src_dir}"
  post_install_notes
}

do_uninstall() {
  local remove_deps="${1:-false}"

  if [ "$remove_deps" = true ]; then
    warn "Você selecionou desinstalação com remoção de dependências."
    warn "Isso pode remover pacotes usados por outros programas e quebrar o sistema."

    if ! confirm_dangerous_dependency_removal; then
      log "Operação cancelada."
      return 0
    fi
  fi

  if ! is_installed; then
    warn "Nenhuma instalação detectada."

    if ! ask_yes_no "Deseja remover arquivos residuais mesmo assim?"; then
      return 0
    fi
  fi

  case "${DISTRO_FAMILY}" in
    nixos)
      uninstall_nixos

      if [ "$remove_deps" = true ]; then
        warn "No NixOS, este instalador não remove dependências de sistema automaticamente."
      fi
      ;;
    arch)
      if is_arch_package_installed; then
        uninstall_arch_package "$remove_deps"

        if is_local_installed; then
          if ask_yes_no "Também foram detectados resíduos locais. Remover agora?"; then
            uninstall_local
          fi
        fi
      else
        uninstall_local

        if [ "$remove_deps" = true ]; then
          remove_dependency_packages
        fi
      fi
      ;;
    *)
      uninstall_local

      if [ "$remove_deps" = true ]; then
        remove_dependency_packages
      fi
      ;;
  esac

  log "Desinstalação concluída."
}

do_status() {
  log "Verificando instalação de ${PACKAGE_NAME}..."
  log "Distro: ${DISTRO_FAMILY} (${PKG_MANAGER})"

  if is_installed; then
    log "Status geral: instalado ($(installation_state_text))"
  else
    log "Status geral: não instalado"
  fi

  if is_arch_package_installed; then
    pacman -Qi "${PACKAGE_NAME}" 2>/dev/null | grep -E 'Name|Version|Install Size' || true
  fi

  if is_nix_profile_installed; then
    nix --extra-experimental-features "${NIX_FEATURES}" profile list 2>/dev/null | grep "${PACKAGE_NAME}" || true
  fi

  if is_local_installed; then
    log "Arquivos locais detectados:"

    local p

    for p in \
      "${INSTALL_ROOT}" \
      "${BIN_PATH}" \
      "${APP_DIR}/${DESKTOP_ID}.desktop" \
      "${ICON_DIR}/${PACKAGE_NAME}.svg" \
      "${PIXMAP_DIR}/${PACKAGE_NAME}.svg" \
      "/usr/lib/${PACKAGE_NAME}"; do
      if [ -e "$p" ]; then
        echo "  - $p"
      fi
    done

    if [ -f "${MANIFEST_FILE}" ]; then
      echo "  - ${MANIFEST_FILE} ($(wc -l <"${MANIFEST_FILE}" 2>/dev/null || echo 0) linhas)"
    fi
  fi

  if is_nix_user_installed; then
    log "Entradas de usuário NixOS detectadas:"

    local p

    for p in \
      "${HOME}/.local/share/applications/${DESKTOP_ID}.desktop" \
      "${HOME}/.local/share/icons/hicolor/scalable/apps/${PACKAGE_NAME}.svg" \
      "${HOME}/.local/share/pixmaps/${PACKAGE_NAME}.svg"; do
      if [ -e "$p" ]; then
        echo "  - $p"
      fi
    done
  fi
}

main_menu() {
  while true; do
    echo
    log "Estado atual: $(installation_state_text)"

    cat <<'EOF'

===== Menu do zashterminal =====
1) Instalar
2) Verificar instalação
3) Desinstalar (manter dependências)
4) Desinstalar e remover dependências (ARRISCADO)
5) Sair
EOF

    local option

    if ! read -rp "Escolha uma opção: " option; then
      echo
      break
    fi

    case "$option" in
      1)
        do_install
        ;;
      2)
        do_status
        ;;
      3)
        do_uninstall false
        ;;
      4)
        do_uninstall true
        ;;
      5)
        break
        ;;
      *)
        warn "Opção inválida."
        ;;
    esac
  done
}

main() {
  detect_system

  if [ $# -gt 0 ]; then
    case "$1" in
      install)
        do_install
        ;;
      uninstall)
        do_uninstall false
        ;;
      uninstall-purge-deps | purge-deps)
        do_uninstall true
        ;;
      status)
        do_status
        ;;
      menu)
        main_menu
        ;;
      *)
        die "Uso: $0 [install|uninstall|uninstall-purge-deps|status|menu]"
        ;;
    esac
  else
    if [ ! -t 0 ]; then
      die "Uso: $0 [install|uninstall|uninstall-purge-deps|status|menu]"
    fi

    main_menu
  fi
}

main "$@"
