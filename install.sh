#!/usr/bin/env bash
# Install or remove the telex IBus engine.
#
#   ./install.sh                     system-wide (sudo): engine in /usr/local/libexec,
#                                    component in /usr/share/ibus/component
#   ./install.sh --user              per-user, no sudo: files under ~/.local and
#                                    IBUS_COMPONENT_PATH set in ~/.config/environment.d
#                                    (takes effect after logging out and back in)
#   ./install.sh --uninstall [--user]
set -euo pipefail
cd "$(dirname "$0")"

mode=system
action=install
for arg in "$@"; do
    case "$arg" in
        --user) mode=user ;;
        --uninstall) action=uninstall ;;
        -h | --help)
            sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "unknown argument: $arg" >&2
            exit 2
            ;;
    esac
done

system_components=/usr/share/ibus/component
if [[ $mode == system ]]; then
    sudo=sudo
    libexec=/usr/local/libexec/telex
    components=$system_components
else
    sudo=
    libexec="$HOME/.local/libexec/telex"
    components="$HOME/.local/share/ibus/component"
    env_file="${XDG_CONFIG_HOME:-$HOME/.config}/environment.d/90-telex-ibus.conf"
fi
engine="$libexec/ibus-engine-telex"
component="$components/telex.xml"

restart_ibus() {
    if command -v ibus >/dev/null && ibus address >/dev/null 2>&1; then
        echo "Restarting IBus..."
        ibus restart >/dev/null 2>&1 || true
    fi
}

if [[ $action == uninstall ]]; then
    $sudo rm -f "$engine" "$component"
    $sudo rmdir "$libexec" 2>/dev/null || true
    [[ $mode == user ]] && rm -f "$env_file"
    restart_ibus
    echo "telex removed."
    exit 0
fi

cargo build --release --bin ibus-engine-telex

xml="$(mktemp)"
trap 'rm -f "$xml"' EXIT
sed "s|@EXEC@|$engine|" data/telex.xml.in > "$xml"

$sudo install -Dm755 target/release/ibus-engine-telex "$engine"
$sudo install -Dm644 "$xml" "$component"
echo "Installed $engine"
echo "Installed $component"

if [[ $mode == user ]]; then
    # IBUS_COMPONENT_PATH replaces the default directory, so keep it in the list.
    mkdir -p "$(dirname "$env_file")"
    echo "IBUS_COMPONENT_PATH=$system_components:$components" > "$env_file"
    echo "Wrote $env_file"
    echo
    echo "Log out and back in so ibus-daemon picks up IBUS_COMPONENT_PATH."
else
    restart_ibus
fi

cat <<'EOF'

Then add the input source:
  Settings → Keyboard → Input Sources → Add → Vietnamese → Vietnamese (Telex)
or from a terminal (keeps your current sources):
  gsettings set org.gnome.desktop.input-sources sources \
    "$(gsettings get org.gnome.desktop.input-sources sources | sed "s/]$/, ('ibus', 'telex')]/")"
Switch input sources with Super+Space.
EOF
