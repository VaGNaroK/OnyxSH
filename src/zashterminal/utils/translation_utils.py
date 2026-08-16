import gettext
import os
import sys

# Locate candidate locale directories in priority order
script_dir = os.path.dirname(os.path.abspath(__file__))  # .../zashterminal/utils
pkg_dir = os.path.dirname(script_dir)                    # .../zashterminal
repo_dir = os.path.dirname(pkg_dir)                      # root / repo dir

candidate_locale_dirs = [
    os.path.join(pkg_dir, 'locale'),                     # Internal app locale dir (e.g. /app/lib/zashterminal/locale)
    '/app/share/locale',                                 # Flatpak locale
    '/usr/share/locale',                                 # System package install
    '/usr/local/share/locale',                           # Local system install
    '/opt/zashterminal/locale',                          # /opt install
    os.path.join(repo_dir, 'locale'),                    # Source tree locale
]

# Check AppImage
if 'APPIMAGE' in os.environ or 'APPDIR' in os.environ:
    share_dir = os.path.dirname(pkg_dir)
    appimage_locale = os.path.join(share_dir, 'locale')
    if os.path.isdir(appimage_locale):
        candidate_locale_dirs.insert(0, appimage_locale)

locale_dir = '/usr/share/locale'
for c_dir in candidate_locale_dirs:
    if os.path.isdir(c_dir):
        # Check if contains any .mo files
        has_mo = False
        for _, _, files in os.walk(c_dir):
            if any(f.endswith(".mo") for f in files):
                has_mo = True
                break
        if has_mo:
            locale_dir = c_dir
            break

# Configure the translation text domain for zashterminal
gettext.bindtextdomain("zashterminal", locale_dir)
gettext.textdomain("zashterminal")

# Export _ directly as the translation function
_ = gettext.gettext
