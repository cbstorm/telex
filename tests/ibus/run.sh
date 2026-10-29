#!/usr/bin/env bash
# Run the IBus end-to-end test against a private ibus-daemon.
#
# The daemon gets its own address, config and cache directories and only
# knows the telex component, so the desktop's IBus session is untouched.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cargo build --quiet --manifest-path "$root/Cargo.toml" --bin ibus-engine-telex
engine="$root/target/debug/ibus-engine-telex"

work="$(mktemp -d)"
daemon_pid=""
cleanup() {
    [[ -n "$daemon_pid" ]] && kill "$daemon_pid" 2>/dev/null || true
    pkill -f "^$engine" 2>/dev/null || true
    rm -rf "$work"
}
trap cleanup EXIT

mkdir -p "$work/component" "$work/config" "$work/cache"
sed "s|@EXEC@|$engine|" "$root/data/telex.xml.in" > "$work/component/telex.xml"

# Abstract socket: a path under a deep $TMPDIR can exceed the 108-byte limit.
export IBUS_ADDRESS="unix:abstract=telex-e2e-$$"
export IBUS_COMPONENT_PATH="$work/component"
export XDG_CONFIG_HOME="$work/config"
export XDG_CACHE_HOME="$work/cache"
export WAYLAND_DISPLAY="telex-e2e"
unset DISPLAY
export DBUS_SESSION_BUS_ADDRESS="disabled:"

ibus-daemon --address="$IBUS_ADDRESS" --panel=disable --emoji-extension=disable \
    --config=disable --cache=none --single &> "$work/daemon.log" &
daemon_pid=$!

for _ in $(seq 50); do
    ibus list-engine &>/dev/null && break
    sleep 0.1
done

python3 "$root/tests/ibus/client.py" || { echo "--- ibus-daemon log ---"; cat "$work/daemon.log"; exit 1; }
