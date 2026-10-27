# PyInstaller spec: self-contained "Digital Library.app" with Python embedded (no Python needed on target Mac).
import os

ROOT = SPECPATH
VERSION = os.environ.get("APP_VERSION", "1.0.0")
ICON = os.path.join(ROOT, "build", "AppIcon.icns")

a = Analysis(
    [os.path.join(ROOT, "library_app", "server.py")],
    datas=[(os.path.join(ROOT, "library_app", "static"), "static")],
    excludes=["tkinter", "unittest", "pydoc", "test"],
)
pyz = PYZ(a.pure)
exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="DigitalLibrary",
    console=False,
    target_arch=os.environ.get("TARGET_ARCH") or None,
    codesign_identity=os.environ.get("SIGN_IDENTITY") or None,
)
coll = COLLECT(exe, a.binaries, a.datas, name="DigitalLibrary")
app = BUNDLE(
    coll,
    name="Digital Library.app",
    icon=ICON if os.path.exists(ICON) else None,
    bundle_identifier="local.digitallibrary.app",
    version=VERSION,
    info_plist={
        "CFBundleDisplayName": "Digital Library",
        "CFBundleShortVersionString": VERSION,
        "CFBundleVersion": VERSION,
        "LSMinimumSystemVersion": "11.0",
        # Background server: no Dock icon; stop it from Settings in the web page.
        "LSUIElement": True,
    },
)
