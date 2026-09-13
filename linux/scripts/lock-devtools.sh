#!/usr/bin/env bash
# Opt-in DevTools lock for Brave (F12 / Ctrl+Shift+I/J/C stop working).
#
# Mechanism: the standard Chromium enterprise policy DeveloperToolsAvailability=2.
# There is NO per-profile command-line switch for this — policies on Linux
# live in /etc/brave/policies/managed, so this script needs sudo AND affects
# every Brave profile on this machine (including your main Brave, if any).
# Fully reversible with: ./scripts/lock-devtools.sh --revert
#
# NOT tested here (no passwordless sudo in this environment): after running,
# open Brave and check brave://policy shows YoodDevToolsLock = 2 (Disabled).
set -euo pipefail

POLICY_DIR="/etc/brave/policies/managed"
POLICY_FILE="${POLICY_DIR}/yood-devtools.json"

if [[ "${1:-}" == "--revert" ]]; then
  echo "Removing ${POLICY_FILE} (DevTools return to normal)..."
  sudo rm -f "${POLICY_FILE}"
  echo "Done. Restart Brave to apply."
  exit 0
fi

cat <<'EOF'
This writes a system-wide Brave policy:
  {"DeveloperToolsAvailability": 2}   (2 = devtools fully disabled)

Consequences:
  - F12 / Ctrl+Shift+I/J/C will do nothing in ANY Brave window/profile.
  - Revert any time with: ./scripts/lock-devtools.sh --revert
EOF
read -r -p "Apply? [y/N] " answer
[[ "${answer}" == [yY] ]] || { echo "Aborted."; exit 1; }

sudo mkdir -p "${POLICY_DIR}"
printf '%s\n' '{"DeveloperToolsAvailability": 2}' | sudo tee "${POLICY_FILE}" >/dev/null
echo "Applied. Restart Brave, then verify at brave://policy (YoodDevToolsLock)."
