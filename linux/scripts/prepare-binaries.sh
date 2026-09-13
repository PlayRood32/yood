#!/usr/bin/env bash
# Stage Linux helper binaries (yt-dlp, ffmpeg) for the Tauri bundle.
# Linux-only build; see windows/scripts/build-windows.ps1 for Windows.
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
platform="${1:-linux-x86_64}"
destination="${project_root}/src-tauri/binaries/${platform}"
mkdir -p "${destination}"

copy_tool() {
  local name="$1"
  local source_var="$2"
  local default_path
  default_path="$(command -v "${name}" 2>/dev/null || true)"
  local source="${!source_var:-${default_path}}"
  if [[ -z "${source}" || ! -f "${source}" ]]; then
    echo "Missing ${name}. Set ${source_var} to a real executable." >&2
    exit 1
  fi
  if [[ -L "${source}" ]]; then
    source="$(readlink -f "${source}")"
  fi
  if [[ "${name}" == "yt-dlp" ]] && file -b "${source}" | grep -qiE 'python script|ascii text'; then
    # The system yt-dlp is just a Python launcher, so Yood needs the official
    # standalone binary. Reuse the staged copy when it is already valid —
    # this also keeps builds working when the network is down.
    if [[ -f "${destination}/${name}" ]] \
      && [[ -s "${destination}/${name}" ]] \
      && ! file -b "${destination}/${name}" | grep -qiE 'python script|ascii text|empty'; then
      echo "Reusing staged ${name} in ${destination} (already a standalone binary)."
      return
    fi
    if ! command -v curl >/dev/null 2>&1; then
      echo "${source} is a Python launcher, and curl is required to fetch the standalone yt-dlp binary." >&2
      exit 1
    fi
    echo "The system yt-dlp is a Python launcher; fetching the official standalone binary (~40MB, may take a minute)."
    # Download to a temp file first: a failed download (e.g. DNS outage)
    # must never truncate a previously staged working binary.
    local tmpfile
    tmpfile="$(mktemp "${destination}/.${name}.tmp.XXXXXX")"
    if [[ -t 2 ]]; then
      curl -L --fail --progress-bar \
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux" \
        -o "${tmpfile}"
    else
      curl -L --fail --silent --show-error \
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux" \
        -o "${tmpfile}"
    fi
    chmod 0755 "${tmpfile}"
    mv -f "${tmpfile}" "${destination}/${name}"
    return
  fi
  install -m 0755 "${source}" "${destination}/${name}"
}

case "${platform}" in
  linux-x86_64)
    copy_tool yt-dlp YOOD_YTDLP_PATH
    copy_tool ffmpeg YOOD_FFMPEG_PATH
    ;;
  *)
    echo "Unsupported binary staging platform: ${platform} (this Linux build supports only linux-x86_64)" >&2
    exit 2
    ;;
esac

echo "Staged real yt-dlp and FFmpeg binaries in ${destination}"
