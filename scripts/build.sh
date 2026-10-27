#!/bin/bash
# Builds the Tauri desktop app and collects installers in dist/.
#   ./scripts/build.sh                                  # this computer's platform/architecture
#   TARGET=universal-apple-darwin ./scripts/build.sh    # macOS: Intel + Apple Silicon in one app
#   APP_VERSION=1.2.0 ./scripts/build.sh                # override the version from tauri.conf.json
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
VERSION="${APP_VERSION:-$(node -p "require('./src-tauri/tauri.conf.json').version")}"

ARGS=()
[ -n "${APP_VERSION:-}" ] && ARGS+=(--config "{\"version\":\"$APP_VERSION\"}")
OUT="$ROOT/src-tauri/target/release"
if [ -n "${TARGET:-}" ]; then
  ARGS+=(--target "$TARGET")
  OUT="$ROOT/src-tauri/target/$TARGET/release"
fi

npx tauri build ${ARGS[@]+"${ARGS[@]}"}

BUNDLE="$OUT/bundle"
rm -rf dist
mkdir -p dist
case "$(uname -s)" in
  Darwin)
    ARCH="$(uname -m)"
    [ "${TARGET:-}" = "universal-apple-darwin" ] && ARCH="universal"
    NAME="DigitalLibrary-$VERSION-macos-$ARCH"
    APP="$BUNDLE/macos/Digital Library.app"
    cp "$BUNDLE"/dmg/*.dmg "dist/$NAME.dmg"
    # ditto keeps the symlinks and code signature that a plain 'zip' would break.
    ditto -c -k --sequesterRsrc --keepParent "$APP" "dist/$NAME.zip"
    "$ROOT/scripts/make_pkg.sh" "$APP" "$VERSION" "$ARCH" "dist/$NAME.pkg"
    ;;
  MINGW* | MSYS* | CYGWIN*)
    NAME="DigitalLibrary-$VERSION-windows-x64"
    cp "$BUNDLE"/nsis/*-setup.exe "dist/$NAME-setup.exe"
    cp "$BUNDLE"/msi/*.msi "dist/$NAME.msi"
    # PowerShell needs a Windows path, not Git Bash's /d/... form.
    EXE="$(cygpath -w "$OUT/DigitalLibrary.exe")"
    powershell -NoProfile -Command "Compress-Archive -Force -Path '$EXE' -DestinationPath 'dist/$NAME-portable.zip'"
    ;;
  *)
    cp -R "$BUNDLE"/. dist/
    ;;
esac

echo
ls -lh dist
