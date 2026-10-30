#!/usr/bin/env bash
set -e

# Resolve repository root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# 1. Fast path for global shortcut trigger (invoked by GNOME custom keybinding)
if [ "$1" = "--trigger" ] || [ "$1" = "-t" ] || [ "$1" = "trigger" ]; then
  SOCKET="/tmp/desklens_$(id -u).sock"
  if [ -S "$SOCKET" ]; then
    python3 -c "import socket; s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); s.connect('$SOCKET'); s.sendall(b'trigger\n'); s.close()" 2>/dev/null && exit 0
  fi
  # If socket isn't active, fallback to binary if compiled, otherwise start full dev instance
  if [ -x "$SCRIPT_DIR/src-tauri/target/release/desklens" ]; then
    exec "$SCRIPT_DIR/src-tauri/target/release/desklens" --trigger
  elif [ -x "$SCRIPT_DIR/src-tauri/target/debug/desklens" ]; then
    exec "$SCRIPT_DIR/src-tauri/target/debug/desklens" --trigger
  fi
fi

# 2. Auto-configure GNOME global shortcut Ctrl+Shift+X if on GNOME
if [ "$XDG_CURRENT_DESKTOP" = "ubuntu:GNOME" ] || [ "$XDG_CURRENT_DESKTOP" = "GNOME" ] || [ -n "$GNOME_DESKTOP_SESSION_ID" ]; then
  python3 - << 'EOF' 2>/dev/null || true
import subprocess

path = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/desklens/"
key_schema = f"org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:{path}"

try:
    out = subprocess.check_output(["gsettings", "get", "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings"]).decode().strip()
    bindings = eval(out) if out and out.startswith("[") else []
    if path not in bindings:
        bindings.append(path)
        subprocess.run(["gsettings", "set", "org.gnome.settings-daemon.plugins.media-keys", "custom-keybindings", str(bindings)], check=True)
    subprocess.run(["gsettings", "set", key_schema, "name", "DeskLens Screen Capture"], check=True)
    subprocess.run(["gsettings", "set", key_schema, "command", "/home/geminiano/Documents/projects/Desklens/run-linux.sh --trigger"], check=True)
    subprocess.run(["gsettings", "set", key_schema, "binding", "<Primary><Shift>x"], check=True)
    print("✅ [DeskLens] GNOME global shortcut (Ctrl+Shift+X) registered successfully.")
except Exception as e:
    pass
EOF
fi

echo "🚀 [DeskLens] Configuring Linux compatibility environment..."
if [ -f "$HOME/.cargo/env" ]; then
  source "$HOME/.cargo/env"
fi
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
export PKG_CONFIG_PATH="$HOME/.local/usr/lib/x86_64-linux-gnu/pkgconfig:${PKG_CONFIG_PATH:-}"
export LIBRARY_PATH="$HOME/.local/usr/lib/x86_64-linux-gnu:${LIBRARY_PATH:-}"
export BINDGEN_EXTRA_CLANG_ARGS="-I/usr/lib/gcc/x86_64-linux-gnu/15/include -I/usr/include"
export GDK_BACKEND=x11
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export NO_AT_BRIDGE=1

echo "ℹ️ [DeskLens] Environment configured:"
echo "   • GDK_BACKEND=${GDK_BACKEND}"
echo "   • WEBKIT_DISABLE_COMPOSITING_MODE=${WEBKIT_DISABLE_COMPOSITING_MODE}"
echo "   • NO_AT_BRIDGE=${NO_AT_BRIDGE}"

echo "🚀 [DeskLens] Launching application in development mode..."
exec npm run tauri dev "$@"
