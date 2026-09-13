#!/usr/bin/env bash
# Yood for Linux — run / compile helper.
# Usage: ./run.sh [command]
# With no arguments an interactive TUI menu opens.
#
#   menu           Open interactive TUI menu (default, no args)
#   dev            Run in development mode (frontend build + cargo tauri dev)
#   run            Run the already-built release binary (no compilation)
#   build          Alias for build-linux
#   build-linux    Compile for Linux (asks: bin / appimage / deb / rpm)
#   build-linux-run Build for Linux, then run the release binary
#   brave          Launch YouTube in Brave app mode (stable engine + Shields)
#   check          Type-check frontend + cargo check (no bundle)
#   test           Run Rust unit tests
#   self-test      Run `yood --self-test` (no WebView needed)
#   install        Per-user install (app icon, yood:// handler, Brave bundle)
#   clean          Remove build artifacts
#   help           Show this help
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${project_root}"

info() { printf '%s\n' "$*"; }
die() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

webkit_warning() {
  printf '%s\n' "WARNING: the Tauri/WebKit engine segfaults on YouTube player pages" \
    "(upstream WebKitGTK bug, proven by bisection). For stable watching use:" \
    "  ./run.sh brave" >&2
}

ensure_frontend() {
  if [[ ! -d ../shared/frontend/node_modules ]]; then
    info "Installing frontend dependencies (npm install)..."
    npm install --prefix ../shared/frontend
  fi
  info "Building frontend (tsc)..."
  npm run build --prefix ../shared/frontend
}

cmd_dev() {
  ensure_frontend
  webkit_warning
  info "Starting Yood in dev mode..."
  exec cargo tauri dev --config src-tauri/tauri.conf.json "$@"
}

cmd_run() {
  local bin_release="src-tauri/target/release/yood"
  local bin_debug="src-tauri/target/debug/yood"
  if [[ -f "${bin_release}" ]]; then
    webkit_warning
    info "Running ${bin_release} ..."
    exec "${bin_release}" "$@"
  elif [[ -f "${bin_debug}" ]]; then
    webkit_warning
    info "No release binary — running ${bin_debug} ..."
    exec "${bin_debug}" "$@"
  else
    printf 'ERROR: No built binary found. Run dev or build-linux first.\n' >&2
    return 1
  fi
}

pick_build_targets() {
  # Prints chosen targets, one per line. Empty input in a terminal opens a
  # checklist; without a terminal it defaults to a plain runnable binary.
  if [[ $# -eq 0 && -t 0 && -t 1 ]]; then
    if command -v whiptail >/dev/null 2>&1; then
      local choice
      choice=$(whiptail --title "Yood build for Linux" --checklist \
        "Space toggles, Enter confirms (Esc cancels):" 20 72 5 \
        "bin" "Runnable binary (Arch: use this)" ON \
        "appimage" "Portable AppImage (needs mksquashfs)" OFF \
        "deb" "Debian / Ubuntu package" OFF \
        "rpm" "Fedora package" OFF \
        3>&1 1>&2 2>&3) || return 1
      # shellcheck disable=SC2206
      local picked=(${choice//\"/})
      [[ "${#picked[@]}" -gt 0 ]] || return 1
      printf '%s\n' "${picked[@]}"
      return 0
    fi
    echo "Choose build targets (e.g. '1 3', empty cancels):" >&2
    echo "  1) bin       Runnable binary (Arch: use this)" >&2
    echo "  2) appimage  Portable AppImage (needs mksquashfs)" >&2
    echo "  3) deb       Debian / Ubuntu package" >&2
    echo "  4) rpm       Fedora package" >&2
    local answer names=(bin appimage deb rpm) n picked=()
    read -r -p "> " answer || return 1
    for n in ${answer}; do
      if [[ "${n}" =~ ^[1-4]$ ]]; then picked+=("${names[$((n-1))]}"); fi
    done
    [[ "${#picked[@]}" -gt 0 ]] || return 1
    printf '%s\n' "${picked[@]}"
    return 0
  fi
  if [[ $# -eq 0 ]]; then
    echo bin
    return 0
  fi
  printf '%s\n' "$@"
}

cmd_build_linux() {
  ensure_frontend
  local targets=() passthrough=() seen_sep=false arg
  for arg in "$@"; do
    if ! $seen_sep && [[ "${arg}" == "--" ]]; then seen_sep=true; continue; fi
    if ! $seen_sep && [[ "${arg}" =~ ^(bin|appimage|deb|rpm)$ ]]; then
      targets+=("${arg}"); continue
    fi
    passthrough+=("${arg}")
  done
  if [[ "${#targets[@]}" -eq 0 ]]; then
    local picked
    picked="$(pick_build_targets)" || { info "Build cancelled."; return 1; }
    while IFS= read -r line; do targets+=("${line}"); done <<< "${picked}"
  fi
  # AppImage needs mksquashfs; drop it with an explanation instead of failing.
  local kept=() t
  for t in "${targets[@]}"; do
    if [[ "${t}" == "appimage" ]] && ! command -v mksquashfs >/dev/null 2>&1; then
      info "NOTE: skipping AppImage (mksquashfs not found — sudo pacman -S --needed squashfs-tools)."
    else
      kept+=("${t}")
    fi
  done
  targets=("${kept[@]}")
  if [[ "${#targets[@]}" -eq 0 ]]; then
    info "Nothing left to build — falling back to a runnable binary."
    targets=(bin)
  fi
  info "Staging Linux binaries (yt-dlp, ffmpeg)..."
  ./scripts/prepare-binaries.sh linux-x86_64
  info "Building Yood for Linux (this takes a while)..."
  local bundles=()
  for t in "${targets[@]}"; do
    [[ "${t}" != "bin" ]] && bundles+=("${t}")
  done
  if [[ "${#bundles[@]}" -eq 0 ]]; then
    cargo tauri build --config src-tauri/tauri.conf.json --no-bundle "${passthrough[@]}"
  else
    local csv
    csv="$(IFS=,; echo "${bundles[*]}")"
    cargo tauri build --config src-tauri/tauri.conf.json --bundles "${csv}" "${passthrough[@]}"
  fi
  info "Done."
  info "  runnable binary: src-tauri/target/release/yood"
  info "  bundles are in src-tauri/target/release/bundle/ (if any were built)"
  info "  install with: ./run.sh install   |   run it with: ./run.sh build-linux-run"
}

cmd_build_linux_run() {
  # Args before `--` go to the build, args after go to the binary:
  #   ./run.sh build-linux-run -- https://www.youtube.com/watch?v=...
  local build_args=() run_args=() seen_sep=false arg
  for arg in "$@"; do
    if ! $seen_sep && [[ "${arg}" == "--" ]]; then seen_sep=true; continue; fi
    if $seen_sep; then run_args+=("${arg}"); else build_args+=("${arg}"); fi
  done
  cmd_build_linux "${build_args[@]}"
  local bin="src-tauri/target/release/yood"
  [[ -f "${bin}" ]] || die "Build finished but ${bin} not found."
  webkit_warning
  info "Build finished — launching ${bin} ..."
  exec "${bin}" "${run_args[@]}"
}

cmd_brave() {
  # The WebKitGTK engine segfaults deterministically on YouTube's full
  # player pages (watch/shorts/mobile/live) on this system — verified with
  # 13 configurations (rendering, UA, JIT, hooks, GStreamer, profile), while
  # embed/MP4/MSE/homepage/search all survive, and Brave renders the same
  # watch page cleanly. Until upstream WebKit is fixed, this is the stable
  # way to watch: Brave + Shields ad blocking in app mode (no address bar,
  # no tabs — feels like Yood), with an isolated profile.
  #
  # Debloat policy — only mechanisms verified against the real binary:
  #  * BraveWallet binary-disabled via --disable-features (name confirmed
  #    in the shipped binary; Rewards/News/VPN have no such feature name,
  #    so they are neutralised by never surfacing: app mode hides all
  #    toolbars, NTP is never opened, profile is fresh and isolated).
  #  * Chromium bloat off: Translate, MediaRouter route providers, Sync,
  #    crash reporting, first-run, default-browser check, domain reliability.
  #  * Economy: renderer-process-limit caps RAM (one-window app needs few
  #    renderers); notifications + permission prompts are denied outright
  #    (no popups on top, no notification spam).
  #  * Component updates stay ON on purpose: Brave Shields filter lists
  #    update through them; turning them off would silently age ad blocking.
  local brave_bin=""
  if [[ -n "${YOOD_BRAVE_DIR:-}" && -x "${YOOD_BRAVE_DIR}/brave" ]]; then
    brave_bin="${YOOD_BRAVE_DIR}/brave"
  elif [[ -x src-tauri/binaries/linux-x86_64/brave/brave ]]; then
    brave_bin="${PWD}/src-tauri/binaries/linux-x86_64/brave/brave"
  fi
  if [[ -z "${brave_bin}" ]]; then
    # Prefer the real binary over distro wrapper scripts (e.g. Arch's
    # /usr/bin/brave injects --password-store and user flag files).
    for candidate in /opt/brave-bin/brave /opt/brave.com/brave/brave; do
      if [[ -x "${candidate}" ]]; then brave_bin="${candidate}"; break; fi
    done
  fi
  if [[ -z "${brave_bin}" ]]; then
    brave_bin="$(command -v brave 2>/dev/null || command -v brave-browser 2>/dev/null || command -v brave-bin 2>/dev/null || true)"
  fi
  [[ -n "${brave_bin}" ]] || die "No Brave found. Run ./scripts/fetch-brave.sh linux-x86_64 or install brave-bin."
  local profile="${YOOD_BRAVE_PROFILE:-${HOME}/.local/share/yood/brave-profile}"
  mkdir -p "${profile}/Default"
  if [[ ! -f "${profile}/Default/Preferences" ]]; then
    cat > "${profile}/Default/Preferences" <<'EOF'
{"brave":{"ai_chat":{"context_menu_enabled":false,"show_toolbar_button":false},"p3a":{"enabled":false,"notice_acknowledged":true}}}
EOF
    if [[ ! -f "${profile}/Local State" ]]; then
      printf '%s' '{"brave":{"p3a":{"enabled":false,"notice_acknowledged":true}}}' > "${profile}/Local State"
    fi
  else
    # Existing profile: fill in missing silencing keys without overriding
    # anything the user already chose (kills the P3A top infobar, etc.).
    # Covers both Default/Preferences and the profile-independent Local State.
    python3 - "${profile}" <<'EOF' 2>/dev/null || true
import json, os, sys

def patch(path):
    try:
        with open(path) as f:
            data = json.load(f)
    except Exception:
        data = {}
    if not isinstance(data, dict):
        data = {}
    brave = data.setdefault("brave", {})
    p3a = brave.setdefault("p3a", {})
    p3a.setdefault("enabled", False)
    p3a.setdefault("notice_acknowledged", True)
    ai = brave.setdefault("ai_chat", {})
    ai.setdefault("context_menu_enabled", False)
    ai.setdefault("show_toolbar_button", False)
    try:
        with open(path, "w") as f:
            json.dump(data, f)
    except Exception:
        pass

base = sys.argv[1]
os.makedirs(os.path.join(base, "Default"), exist_ok=True)
patch(os.path.join(base, "Default", "Preferences"))
patch(os.path.join(base, "Local State"))
EOF
  fi
  local url="https://www.youtube.com"
  local extra=()
  local arg
  for arg in "$@"; do
    if [[ "${arg}" == "--" ]]; then continue; fi
    if [[ "${arg}" =~ ^https?:// ]]; then url="${arg}"; else extra+=("${arg}"); fi
  done
  if ! [[ "${url}" =~ ^https://([a-z0-9-]+\.)*(youtube\.com|youtu\.be|google\.com|accounts\.google\.com|accounts\.youtube\.com)(/|$|\?) ]]; then
    die "Only YouTube/Google URLs are allowed (got: ${url})."
  fi
  info "Launching Brave app mode with Yood profile..."
  info "  binary:  ${brave_bin}"
  info "  profile: ${profile}"
  info "  Shields (ad blocking) stays ON — do not disable it."
  local brave_args=(
    --app="${url}" --user-data-dir="${profile}"
    --no-first-run --no-default-browser-check
    --disable-sync --disable-breakpad --no-report-upload --disable-domain-reliability
    --disable-features=BraveWallet,BraveVPN,AIChat,Translate,DialMediaRouteProvider
    --renderer-process-limit=4 --disable-notifications --deny-permission-prompts
  )
  local ext=""
  if [[ -d ../shared/extension/yood ]]; then
    ext="${PWD}/../shared/extension/yood"
  elif [[ -d "${HOME}/.local/share/yood/extension/yood" ]]; then
    ext="${HOME}/.local/share/yood/extension/yood"
  fi
  if [[ -n "${ext}" ]]; then
    brave_args+=(--load-extension="${ext}")
    info "  download buttons: Yood companion extension loaded"
  else
    info "  download buttons: extension not found (expected at ../shared/extension/yood)"
  fi
  exec "${brave_bin}" "${brave_args[@]}" "${extra[@]}"
}

cmd_check() {
  ensure_frontend
  cargo check --manifest-path src-tauri/Cargo.toml "$@"
  info "Check OK."
}

cmd_test() {
  ensure_frontend
  cargo test --manifest-path src-tauri/Cargo.toml "$@"
}

cmd_self_test() {
  ensure_frontend
  cargo run --manifest-path src-tauri/Cargo.toml -- --self-test "$@"
}

cmd_clean() {
  info "Cleaning frontend dist + Rust target..."
  rm -f ../shared/frontend/dist/bootstrap.js ../shared/frontend/dist/injection.js ../shared/frontend/dist/player.js
  cargo clean --manifest-path src-tauri/Cargo.toml
  info "Clean done."
}

cmd_install() {
  # Per-user install: double-clickable app, no terminal needed afterwards.
  # Uses the AppImage when the bundle step produced one, else the raw binary
  # (install.sh then bundles Brave + extension + registers yood://).
  local appimages=(src-tauri/target/release/bundle/appimage/yood_*_amd64.AppImage)
  if [[ "${appimages[0]}" != *"*"* && -f "${appimages[0]}" ]]; then
    exec ./scripts/install.sh "${appimages[0]}" "$@"
  elif [[ -f src-tauri/target/release/yood ]]; then
    exec ./scripts/install.sh src-tauri/target/release/yood "$@"
  else
    die "Nothing to install. Run './run.sh build-linux' first."
  fi
}

cmd_menu() {
  while true; do
    local choice=""
    if command -v whiptail >/dev/null 2>&1 && [[ -t 0 && -t 1 ]]; then
      choice=$(whiptail --title "Yood (Linux)" --menu "Recommended: brave (stable). Tauri/WebKit options crash on video pages:" 24 78 13 \
        "brave" "RECOMMENDED: YouTube in Brave app mode (stable)" \
        "dev" "Tauri dev (UNSTABLE: crashes on videos)" \
        "run" "Tauri binary (UNSTABLE: crashes on videos)" \
        "build-linux" "Compile Tauri for Linux" \
        "build-linux-run" "Build Tauri, then run it (UNSTABLE)" \
        "check" "Type-check (tsc + cargo check)" \
        "test" "Rust unit tests" \
        "self-test" "yood --self-test (no WebView)" \
        "install" "Per-user install (no terminal needed after)" \
        "clean" "Remove build artifacts" \
        "quit" "Quit" \
        3>&1 1>&2 2>&3) || break
    else
      [[ -t 0 ]] || { cmd_help; return 0; }
      echo "== Yood Linux (recommended: brave — Tauri/WebKit crashes on video pages) =="
      local options=(dev run build-linux build-linux-run brave check test self-test install clean quit)
      select choice in "${options[@]}"; do
        [[ -n "${choice:-}" ]] || { echo "Invalid choice, try again."; continue 2; }
        break
      done
    fi
    case "${choice}" in
      # dev/run replace this process via exec and never return here.
      dev) cmd_dev ;;
      run) cmd_run || true ;;
      build-linux|build) cmd_build_linux || true ;;
      build-linux-run) cmd_build_linux_run || true ;;
      brave) cmd_brave || true ;;
      check) cmd_check || true ;;
      test) cmd_test || true ;;
      self-test|selftest) cmd_self_test || true ;;
      install) cmd_install || true ;;
      clean) cmd_clean || true ;;
      quit|exit|q) break ;;
      *) echo "Unknown choice: ${choice}" ;;
    esac
    printf '\nDone. Press Enter to return to the menu...'
    read -r _ || break
  done
}

cmd_help() {
  sed -n '2,18p' "${BASH_SOURCE[0]}" | sed 's/^# \?//'
  echo
  echo "Examples:"
  echo "  ./run.sh                # open interactive TUI menu"
  echo "  ./run.sh brave          # stable YouTube (Brave app mode + Shields)"
  echo "  ./run.sh dev            # Tauri/WebKit build (currently crashes on watch)"
  echo "  ./run.sh build-linux-run  # compile for Linux and run it"
  echo "  ./run.sh build-linux    # compile for Linux"
}

cmd="${1:-menu}"
shift || true
case "${cmd}" in
  menu|tui) cmd_menu ;;
  dev) cmd_dev "$@" ;;
  run) cmd_run "$@" ;;
  build|build-linux) cmd_build_linux "$@" ;;
  build-linux-run) cmd_build_linux_run "$@" ;;
  brave) cmd_brave "$@" ;;
  check) cmd_check "$@" ;;
  test) cmd_test "$@" ;;
  self-test|selftest) cmd_self_test "$@" ;;
  install) cmd_install "$@" ;;
  clean) cmd_clean "$@" ;;
  help|-h|--help) cmd_help ;;
  *) die "Unknown command '${cmd}'. Run './run.sh help'." ;;
esac
