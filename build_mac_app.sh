#!/bin/bash
# Builds "Digital Library.app" next to this script. Pass --install to also copy it to ~/Applications.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
APP="$ROOT/Digital Library.app"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp -R "$ROOT/library_app" "$APP/Contents/Resources/app"
find "$APP/Contents/Resources/app" -name "__pycache__" -type d -prune -exec rm -rf {} +

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Digital Library</string>
  <key>CFBundleDisplayName</key><string>Digital Library</string>
  <key>CFBundleIdentifier</key><string>local.digitallibrary.app</string>
  <key>CFBundleVersion</key><string>1.0.0</string>
  <key>CFBundleShortVersionString</key><string>1.0.0</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleExecutable</key><string>DigitalLibrary</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>LSMinimumSystemVersion</key><string>10.15</string>
  <!-- Background server: no Dock icon; stop it from Settings in the web page. -->
  <key>LSUIElement</key><true/>
</dict>
</plist>
PLIST

cat > "$APP/Contents/MacOS/DigitalLibrary" <<'LAUNCHER'
#!/bin/bash
RES="$(cd "$(dirname "$0")/../Resources" && pwd)"
LOG="$HOME/Library/Logs/DigitalLibrary.log"
mkdir -p "$(dirname "$LOG")"
for PY in /opt/homebrew/bin/python3 /usr/local/bin/python3 \
          /Library/Frameworks/Python.framework/Versions/Current/bin/python3 /usr/bin/python3; do
  if [ -x "$PY" ]; then
    exec "$PY" "$RES/app/server.py" >>"$LOG" 2>&1
  fi
done
/usr/bin/osascript -e 'display alert "Digital Library" message "Python 3 is required. Install it from python.org or run: brew install python"'
exit 1
LAUNCHER
chmod +x "$APP/Contents/MacOS/DigitalLibrary"

# Generate an app icon with the system Python + built-in macOS tools (optional).
/usr/bin/env python3 "$ROOT/make_icon.py" "$APP/Contents/Resources/AppIcon.icns" || true

if [[ "${1:-}" == "--install" ]]; then
  mkdir -p "$HOME/Applications"
  rm -rf "$HOME/Applications/Digital Library.app"
  cp -R "$APP" "$HOME/Applications/"
  echo "Installed: $HOME/Applications/Digital Library.app"
fi

echo "Built: $APP"
