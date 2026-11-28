#!/usr/bin/env bash
# Builds the Encore Pebble watchapp (pebble/) into pebble/build/pebble.pbw, on an Ubuntu
# 24.04 runner: .github/workflows/pebble.yml (every change) and release.yml (a release).
#
# Build tools only, never shipped with Encore (stage W2, docs/adr/android-0009-pebble.md):
#   uv           0.13.8   (PyPI; Astral, MIT or Apache-2.0), which pebble-tool needs
#   pebble-tool  5.2.0    (PyPI; Core Devices, github.com/coredevices/pebble-tool, MIT)
#   Pebble SDK   4.44     (sdk-core and the arm-none-eabi toolchain, from Core Devices'
#                         dash.repebble.com, checked against its SHA-256 by pebble-tool;
#                         its own Python packages, and webpack 1 from npm for PebbleKit JS,
#                         which this app doesn't have)
# UV_EXCLUDE_NEWER and npm's "before" hold every Python and npm package pebble-tool and the
# SDK pull in to what was published by that date, so a new release upstream can't change
# the build unseen.
set -euo pipefail

UV_VERSION=0.13.8
PEBBLE_TOOL_VERSION=5.2.0
PEBBLE_SDK_VERSION=4.44
export UV_EXCLUDE_NEWER=2026-11-13T00:00:00Z
export npm_config_before=2026-11-13T00:00:00Z

python3 -m pip install --quiet --disable-pip-version-check "uv==$UV_VERSION"
# pebble-tool runs on Python 3.10 to 3.13: the workflow's setup-python gives 3.13.
uv tool install --quiet --python "$(command -v python3)" "pebble-tool==$PEBBLE_TOOL_VERSION"
export PATH="$HOME/.local/bin:$PATH"
pebble --version

# Installing makes it the active SDK.
pebble sdk install "$PEBBLE_SDK_VERSION"
pebble sdk list

cd "$(dirname "$0")/../../pebble"
pebble build
ls -l build/*.pbw
# The bundle holds one app for each platform package.json names.
python3 -I -B - build/pebble.pbw <<'EOF'
import json, sys, zipfile
names = zipfile.ZipFile(sys.argv[1]).namelist()
info = json.loads(zipfile.ZipFile(sys.argv[1]).read("appinfo.json"))
want = json.load(open("package.json"))["pebble"]["targetPlatforms"]
print("platforms:", ", ".join(info["targetPlatforms"]))
missing = [p for p in want if not any(n.startswith(p + "/") and n.endswith("pebble-app.bin") for n in names)]
if missing:
    sys.exit("no app in the bundle for: " + ", ".join(missing))
EOF
