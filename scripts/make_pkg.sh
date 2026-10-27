#!/bin/bash
# Wraps a built "Digital Library.app" in a .pkg installer that installs to /Applications.
# Usage: make_pkg.sh <path/to/Digital Library.app> <version> <arm64|x86_64|universal> <output.pkg>
set -euo pipefail

APP="$1"
VERSION="$2"
ARCH="$3"
PKG="$4"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE_ID="com.digitallibrary.desktop"
case "$ARCH" in
  universal) HOST_ARCHS="arm64,x86_64" ;;
  *) HOST_ARCHS="$ARCH" ;;
esac

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/root" "$WORK/resources"
cp -R "$APP" "$WORK/root/"
cp "$ROOT/installer/"*.html "$WORK/resources/"
chmod +x "$ROOT/installer/scripts/preinstall"

# Non-relocatable: otherwise Installer "upgrades" any copy with the same bundle ID found elsewhere on disk.
pkgbuild --analyze --root "$WORK/root" "$WORK/component.plist" >/dev/null
plutil -replace 0.BundleIsRelocatable -bool NO "$WORK/component.plist"
pkgbuild --quiet --root "$WORK/root" --component-plist "$WORK/component.plist" \
  --scripts "$ROOT/installer/scripts" --install-location /Applications \
  --identifier "$BUNDLE_ID" --version "$VERSION" "$WORK/DigitalLibrary-app.pkg"

cat > "$WORK/distribution.xml" <<XML
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
    <pkg-ref id="$BUNDLE_ID" version="$VERSION" onConclusion="none">DigitalLibrary-app.pkg</pkg-ref>
</installer-gui-script>
XML

SIGN_ARGS=()
[ -n "${INSTALLER_SIGN_IDENTITY:-}" ] && SIGN_ARGS=(--sign "$INSTALLER_SIGN_IDENTITY")
productbuild --quiet --distribution "$WORK/distribution.xml" --resources "$WORK/resources" \
  --package-path "$WORK" ${SIGN_ARGS[@]+"${SIGN_ARGS[@]}"} "$PKG"
