"""Fetches Material Symbols (Rounded, Apache-2.0) from google/material-design-icons and
writes them as Android vector drawables, so the app ships only the icons it uses.

    python3 -I -B android/ci/symbols.py android/app/src/main/res/drawable

Each icon becomes sym_<name>.xml (and sym_<name>_fill.xml for the filled style the
selected navigation items use). Run it again after adding a name below.
"""
import re
import sys
import urllib.request

NAMES = {
    # name: also fetch the filled style
    "arrow_back": False, "visibility": False, "visibility_off": False,
    "home": True, "graphic_eq": False, "library_music": True, "search": False,
    "play_arrow": True, "pause": True, "skip_next": True, "skip_previous": True,
    "sort": False, "more_vert": False, "shuffle": False, "repeat": False, "repeat_one": False,
    "favorite": True, "queue_music": False, "close": False, "keyboard_arrow_down": False,
    "grid_view": False, "view_list": False, "album": False, "person": False, "music_note": False,
    "cloud_off": False, "sync": False, "history": False,
    "lyrics": False, "cast": False, "drag_handle": False, "playlist_add": False, "add": False,
    "check": False, "block": False, "info": False, "settings": False, "lock": False, "logout": False,
    "delete": False, "privacy_tip": False, "dns": False, "devices": False, "chevron_right": False,
    "refresh": False, "push_pin": True, "auto_awesome": False, "queue_play_next": False,
    "remove": False, "person_add": False, "person": False, "radio": False, "share": False,
    "expand_more": False, "expand_less": False, "phone_android": False, "computer": False,
    "speaker": False, "fast_forward": False, "fast_rewind": False, "translate": False,
    "download": False, "autoplay": False,
}
BASE = "https://raw.githubusercontent.com/google/material-design-icons/master/symbols/web/{n}/materialsymbolsrounded/{n}{f}_24px.svg"
TEMPLATE = """<?xml version="1.0" encoding="utf-8"?>
<!-- Material Symbols Rounded "{name}"{style} (Apache-2.0), from google/material-design-icons. -->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp"
    android:height="24dp"
    android:viewportWidth="{size}"
    android:viewportHeight="{size}"
    android:autoMirrored="{mirrored}"
    android:tint="?android:attr/colorControlNormal">
    <group android:translateY="{shift}">
        <path
            android:fillColor="@android:color/white"
            android:pathData="{path}" />
    </group>
</vector>
"""
# Icons that point a direction flip in right-to-left languages.
MIRRORED = {"arrow_back", "skip_next", "skip_previous", "queue_music", "sort", "chevron_right", "queue_play_next", "logout", "fast_forward", "fast_rewind"}


def fetch(name, filled):
    url = BASE.format(n=name, f="_fill1" if filled else "")
    with urllib.request.urlopen(url, timeout=30) as r:
        svg = r.read().decode("utf-8")
    m = re.search(r'viewBox="0 -960 960 960".*?<path d="([^"]+)"', svg, re.S)
    if m:
        return m.group(1), 960
    # A few icons still use the older 24-unit drawing with no viewBox.
    m = re.search(r'<svg[^>]*height="24"[^>]*>\s*<path d="([^"]+)"', svg, re.S)
    if m:
        return m.group(1), 24
    raise SystemExit(f"unexpected SVG for {name}")


def main(out):
    for name, with_fill in NAMES.items():
        for filled in ([False, True] if with_fill else [False]):
            path, size = fetch(name, filled)
            file = f"{out}/sym_{name}{'_fill' if filled else ''}.xml"
            mirrored = "true" if name in MIRRORED else "false"
            with open(file, "w", encoding="utf-8", newline="\n") as f:
                f.write(TEMPLATE.format(name=name, style=", filled" if filled else "", mirrored=mirrored, path=path, size=size, shift=960 if size == 960 else 0))
            print(file)


if __name__ == "__main__":
    main(sys.argv[1])
