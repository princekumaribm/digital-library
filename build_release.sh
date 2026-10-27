#!/bin/bash
# Builds a self-contained, offline "Digital Library.app" (Python embedded) and ships it in dist/ as:
#   .pkg  installer that puts the app in /Applications
#   .dmg  drag-to-Applications disk image
#   .zip  plain archive of the app
#
#   ./build_release.sh                      # app for this Mac's architecture
#   TARGET_ARCH=universal2 ./build_release.sh   # Intel + Apple Silicon (needs python.org universal2 Python: PYTHON=/usr/local/bin/python3)
#   SIGN_IDENTITY="Developer ID Application: Name (TEAMID)" INSTALLER_SIGN_IDENTITY="Developer ID Installer: Name (TEAMID)" ./build_release.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT"
export APP_VERSION="${APP_VERSION:-1.0.0}"
PYTHON="${PYTHON:-python3}"
VENV="$ROOT/build/venv"
APP="$ROOT/dist/Digital Library.app"
BUNDLE_ID="local.digitallibrary.app"
ARCH_LABEL="${TARGET_ARCH:-$(uname -m)}"
BASENAME="$ROOT/dist/DigitalLibrary-$APP_VERSION-$ARCH_LABEL"
DMG="$BASENAME.dmg"
PKG="$BASENAME.pkg"
ZIP="$BASENAME.zip"
case "$ARCH_LABEL" in
  universal2) HOST_ARCHS="arm64,x86_64" ;;
  *) HOST_ARCHS="$ARCH_LABEL" ;;
esac

echo "==> Preparing build environment"
[ -x "$VENV/bin/python" ] || "$PYTHON" -m venv "$VENV"
"$VENV/bin/pip" install --quiet --upgrade pip pyinstaller

echo "==> Generating icon"
mkdir -p "$ROOT/build"
"$VENV/bin/python" "$ROOT/make_icon.py" "$ROOT/build/AppIcon.icns"

echo "==> Building app"
"$VENV/bin/pyinstaller" --noconfirm --clean --log-level WARN \
  --distpath "$ROOT/dist" --workpath "$ROOT/build/pyinstaller" "$ROOT/DigitalLibrary.spec"
rm -rf "$ROOT/dist/DigitalLibrary"
codesign --verify --deep --strict "$APP"

echo "==> Creating disk image"
STAGE="$ROOT/build/dmg"
rm -rf "$STAGE" "$DMG"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
# hdiutil intermittently fails with "Resource busy" on CI runners.
for attempt in 1 2 3; do
  hdiutil create -quiet -volname "Digital Library" -srcfolder "$STAGE" -ov -format UDZO "$DMG" && break
  [ "$attempt" -lt 3 ] || exit 1
  sleep 5
done
rm -rf "$STAGE"

echo "==> Creating installer package"
PKG_WORK="$ROOT/build/pkg"
rm -rf "$PKG_WORK" "$PKG"
mkdir -p "$PKG_WORK/root" "$PKG_WORK/resources"
cp -R "$APP" "$PKG_WORK/root/"
cp "$ROOT/installer/"*.html "$PKG_WORK/resources/"
chmod +x "$ROOT/installer/scripts/preinstall"
# Non-relocatable: otherwise Installer "upgrades" any copy with the same bundle ID found elsewhere on disk.
pkgbuild --analyze --root "$PKG_WORK/root" "$PKG_WORK/component.plist" >/dev/null
plutil -replace 0.BundleIsRelocatable -bool NO "$PKG_WORK/component.plist"
pkgbuild --quiet --root "$PKG_WORK/root" --component-plist "$PKG_WORK/component.plist" \
  --scripts "$ROOT/installer/scripts" --install-location /Applications \
  --identifier "$BUNDLE_ID" --version "$APP_VERSION" "$PKG_WORK/DigitalLibrary-app.pkg"
cat > "$PKG_WORK/distribution.xml" <<XML
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
    <title>Digital Library</title>
    <welcome file="welcome.html" mime-type="text/html"/>
    <conclusion file="conclusion.html" mime-type="text/html"/>
    <options customize="never" require-scripts="false" hostArchitectures="$HOST_ARCHS"/>
    <domains enable_localSystem="true"/>
    <volume-check><allowed-os-versions><os-version min="11.0"/></allowed-os-versions></volume-check>
    <choices-outline><line choice="app"/></choices-outline>
    <choice id="app" visible="false"><pkg-ref id="$BUNDLE_ID"/></choice>
    <pkg-ref id="$BUNDLE_ID" version="$APP_VERSION" onConclusion="none">DigitalLibrary-app.pkg</pkg-ref>
</installer-gui-script>
XML
SIGN_ARGS=()
[ -n "${INSTALLER_SIGN_IDENTITY:-}" ] && SIGN_ARGS=(--sign "$INSTALLER_SIGN_IDENTITY")
productbuild --quiet --distribution "$PKG_WORK/distribution.xml" --resources "$PKG_WORK/resources" \
  --package-path "$PKG_WORK" ${SIGN_ARGS[@]+"${SIGN_ARGS[@]}"} "$PKG"
rm -rf "$PKG_WORK"

echo "==> Creating zip"
rm -f "$ZIP"
# ditto keeps the symlinks and code signature that a plain 'zip' would break.
ditto -c -k --sequesterRsrc --keepParent "$APP" "$ZIP"

echo
echo "App: $APP"
echo "PKG: $PKG"
echo "DMG: $DMG"
echo "ZIP: $ZIP"
