#!/usr/bin/env bash
# Simple per-user Linux installation for private/family use.
#
# Usage:
#   ./scripts/install.sh [path-to-yood-binary-or-AppImage]
#
# Without an argument the script looks for a release binary in
# src-tauri/target/release/yood. It installs Yood under ~/.local, creates a
# .desktop entry with the yood:// scheme handler, and refreshes the desktop
# database. Everything stays inside the user's home directory, so no sudo is
# needed and removal is a single command.
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin_dir="${HOME}/.local/bin"
app_dir="${HOME}/.local/share/yood"
applications_dir="${HOME}/.local/share/applications"
icons_dir="${HOME}/.local/share/icons/hicolor/scalable/apps"

source="${1:-}"
if [[ -z "${source}" ]]; then
  candidate="${project_root}/src-tauri/target/release/yood"
  if [[ ! -f "${candidate}" ]]; then
    echo "No binary given and no ${candidate} found. Build first:" >&2
    echo "  cargo tauri build --config src-tauri/tauri.conf.json" >&2
    exit 1
  fi
  source="${candidate}"
fi
source="$(realpath "${source}")"
if [[ ! -f "${source}" ]]; then
  echo "Installer source does not exist: ${source}" >&2
  exit 1
fi

mkdir -p "${bin_dir}" "${app_dir}" "${applications_dir}" "${icons_dir}"

install -m 0755 "${source}" "${app_dir}/yood"
ln -sf "${app_dir}/yood" "${bin_dir}/yood"

# AppImages carry their own resources; a plain binary needs the staged
# binaries and gstreamer plugins next to it.
if [[ "${source}" != *.AppImage ]]; then
  if [[ -d "${project_root}/src-tauri/binaries/linux-x86_64" ]]; then
    mkdir -p "${app_dir}/binaries/linux-x86_64"
    for tool in yt-dlp ffmpeg; do
      if [[ -f "${project_root}/src-tauri/binaries/linux-x86_64/${tool}" ]]; then
        install -m 0755 \
          "${project_root}/src-tauri/binaries/linux-x86_64/${tool}" \
          "${app_dir}/binaries/linux-x86_64/${tool}"
      fi
    done
  fi
fi

# Bundle Brave next to the binary so the installed app is self-contained
# (the yood binary launches it directly — no CLI needed). Falls back to a
# system Brave when bundling is impossible (offline with nothing staged).
brave_src="${project_root}/src-tauri/binaries/linux-x86_64/brave/brave"
if [[ ! -x "${brave_src}" ]]; then
  "${project_root}/scripts/fetch-brave.sh" linux-x86_64 || true
fi
if [[ -x "${brave_src}" ]]; then
  rm -rf "${app_dir}/brave"
  cp -r "${project_root}/src-tauri/binaries/linux-x86_64/brave" "${app_dir}/brave"
else
  echo "WARNING: no bundled Brave available; installed Yood will use a system Brave." >&2
fi

icon="${project_root}/src-tauri/icons/icon.svg"
if [[ -f "${icon}" ]]; then
  install -m 0644 "${icon}" "${icons_dir}/yood.svg"
fi

# Companion extension for Brave app mode (download buttons). cmd_brave
# picks it up from here when the repo copy is not around.
if [[ -d "${project_root}/../shared/extension/yood" ]]; then
  mkdir -p "${app_dir}/extension"
  rm -rf "${app_dir}/extension/yood"
  cp -r "${project_root}/../shared/extension/yood" "${app_dir}/extension/yood"
fi

desktop_file="${applications_dir}/yood.desktop"
cat > "${desktop_file}" <<EOF
[Desktop Entry]
Type=Application
Name=Yood
Comment=Lightweight private YouTube desktop application
Exec=${app_dir}/yood %u
Icon=yood
Terminal=false
Categories=AudioVideo;Video;Network;
MimeType=x-scheme-handler/yood;
StartupWMClass=yood
EOF
chmod 0644 "${desktop_file}"

# Register the yood:// scheme (Yood's own deep-link scheme, not all https).
if command -v xdg-mime >/dev/null 2>&1; then
  xdg-mime default yood.desktop x-scheme-handler/yood || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${applications_dir}" || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "${HOME}/.local/share/icons" >/dev/null 2>&1 || true
fi

echo "Yood installed:"
echo "  binary:  ${app_dir}/yood"
echo "  launcher: ${desktop_file}"
echo "  start with: ${bin_dir}/yood"
echo
echo "Remove with:"
echo "  rm -f ${bin_dir}/yood ${desktop_file} ${icons_dir}/yood.svg && rm -rf ${app_dir}"