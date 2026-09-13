#!/usr/bin/env bash
# Download ONLY the official Brave portable build needed by Yood on Linux —
# no source tree, no SDK, no build tools. Result lands in
# src-tauri/binaries/linux-x86_64/ and is used first by `./run.sh brave`
# (system Brave is only a fallback).
#
# Usage:
#   ./scripts/fetch-brave.sh [linux-x86_64] [version]
#
# Version defaults to the latest GitHub release; pass e.g. 1.94.121 to pin,
# or set BRAVE_VERSION. Files are verified against Brave's published .sha256
# before extraction. Reuses the staged copy when the version already matches.
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
platform="${1:-linux-x86_64}"
version="${2:-${BRAVE_VERSION:-}}"
destination="${project_root}/src-tauri/binaries/${platform}/brave"

case "${platform}" in
  linux-x86_64) asset="brave-browser-{V}-linux-amd64.zip" ;;
  *)
    echo "Unsupported Brave platform: ${platform} (this Linux build supports only linux-x86_64)" >&2
    exit 2
    ;;
esac

if [[ -z "${version}" ]]; then
  if ! command -v curl >/dev/null 2>&1; then
    echo "curl is required to resolve the latest Brave version (or pass one explicitly)." >&2
    exit 1
  fi
  tag="$(curl -sIL --max-time 30 -o /dev/null -w "%{url_effective}" \
    "https://github.com/brave/brave-browser/releases/latest" || true)"
  version="${tag##*/v}"
  if ! [[ "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Could not resolve latest Brave version (got: '${tag}'). Pass one explicitly, e.g.:" >&2
    echo "  ./scripts/fetch-brave.sh ${platform} 1.94.121" >&2
    exit 1
  fi
fi

if [[ -f "${destination}/VERSION" ]] && [[ "$(cat "${destination}/VERSION")" == "${version}" ]]; then
  if [[ -x "${destination}/brave" ]]; then
    echo "Reusing staged Brave ${version} in ${destination}."
    exit 0
  fi
fi

for tool in curl unzip sha256sum; do
  command -v "${tool}" >/dev/null 2>&1 || { echo "Missing required tool: ${tool}" >&2; exit 1; }
done

# Removes only payload items verified safe to drop for YouTube app mode:
#  - locales/*.pak except en-US (Chromium fallback) and he (Hebrew UI).
#    (~77MB on linux-x86_64.) Missing locales fall back to en-US; nothing
#    breaks, the UI language just defaults to English.
#  - chrome-management-service: enterprise cloud-policy helper;
#    verified by a 30s headed run with it removed — no error, no crash.
# Deliberately NOT removed (verified by testing):
#  - chrome_crashpad_handler: Brave FATAL-aborts at startup without it, even
#    with crash reporting flags off.
#  - GPU/SwiftShader/Vulkan libs, sandbox helper,
#    HiDPI paks, Qt shims, xdg scripts: small and/or load-bearing.
#  - The ~305MB core binary, resource paks, ICU data, V8 snapshot: the
#    irreducible floor of a Chromium browser (only a source rebuild, which
#    needs 100GB+/hours, could shrink that).
# WidevineCdm is not shipped in these portable builds at all (no DRM
# rentals; regular YouTube does not need it).
prune_brave_payload() {
  local dir="$1" target="$2"
  local removed=0 path
  if [[ -d "${dir}/locales" ]]; then
    while IFS= read -r -d '' path; do
      rm -f "${path}" && removed=$((removed + 1))
    done < <(find "${dir}/locales" -maxdepth 1 -name '*.pak' \
      ! -name 'en-US.pak' ! -name 'he.pak' -print0)
  fi
  if [[ -e "${dir}/chrome-management-service" ]]; then
    rm -f "${dir}/chrome-management-service" && removed=$((removed + 1))
  fi
  echo "Pruned ${removed} unneeded locale files (${target})."
}

file="${asset//\{V\}/${version}}"
base="https://github.com/brave/brave-browser/releases/download/v${version}"
tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

echo "Fetching Brave ${version} for ${platform} (~200MB, one-time)..."
# --speed-time/--speed-limit: fail fast on stalled connections instead of
# hanging forever (then just rerun; nothing half-written is kept). If a
# download stalls midway, delete the temp dir and rerun — curl resumes are
# deliberately not used because a bad resume silently corrupts the archive.
if [[ -t 2 ]]; then
  curl -L --fail --progress-bar --speed-time 30 --speed-limit 30000 \
    "${base}/${file}" -o "${tmpdir}/brave.zip"
  curl -sL --fail --speed-time 30 --speed-limit 30000 \
    "${base}/${file}.sha256" -o "${tmpdir}/brave.zip.sha256"
else
  curl -L --fail --silent --show-error --speed-time 30 --speed-limit 30000 \
    "${base}/${file}" -o "${tmpdir}/brave.zip"
  curl -sL --fail --silent --show-error --speed-time 30 --speed-limit 30000 \
    "${base}/${file}.sha256" -o "${tmpdir}/brave.zip.sha256"
fi

echo "Verifying checksum..."
# Compare hashes directly: Brave's .sha256 names the release file, not our
# local temp name, so `sha256sum -c` would false-fail on the filename.
expected="$(awk '{print $1}' "${tmpdir}/brave.zip.sha256")"
actual="$(sha256sum "${tmpdir}/brave.zip" | awk '{print $1}')"
if [[ -z "${expected}" || "${expected}" != "${actual}" ]]; then
  echo "Checksum mismatch for Brave ${version}; refusing to extract." >&2
  exit 1
fi

echo "Extracting to ${destination} ..."
rm -rf "${destination}"
mkdir -p "${destination}"
unzip -q -o "${tmpdir}/brave.zip" -d "${destination}"
if [[ ! -x "${destination}/brave" ]]; then
  inners=()
  while IFS= read -r entry; do inners+=("${entry}"); done < <(
    find "${destination}" -maxdepth 1 -mindepth 1 -type d
  )
  if [[ "${#inners[@]}" -eq 1 ]]; then
    mv "${inners[0]}"/* "${destination}/"
    rmdir "${inners[0]}"
  fi
fi
if [[ ! -x "${destination}/brave" ]]; then
  echo "Unexpected Brave zip layout (no brave binary)." >&2
  exit 1
fi
# Debloat the browser payload itself: keep only what YouTube in app mode
# needs. See prune_brave_payload() for the exact, verified-safe list.
prune_brave_payload "${destination}" "${platform}"
echo "${version}" > "${destination}/VERSION"
echo "Staged Brave ${version} in ${destination}."
