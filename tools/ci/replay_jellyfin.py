"""Serve a recorded Jellyfin session over real HTTP on 127.0.0.1, for the smoke tests.

The Swift, Kotlin and Dart smoke tests sign in, sync, browse, play and search through
the generated bindings against this, so every record crosses the FFI boundary on
every platform. The Rust tests replay the same file in-process (FixtureTransport);
this is the same matching over a socket, standard library only.

    python3 -I -B tools/ci/replay_jellyfin.py RECORDED.json URL_FILE

It binds a free port, writes "http://127.0.0.1:<port>" to URL_FILE (atomically), and
serves until killed. Matching follows FixtureTransport: method, path, and every
recorded query parameter, replayed in recorded order with the last match repeating.
Three differences, all so the apps' real settings work against a recording made with
test settings:
  - `limit` is ignored: the core asks for pages of library.syncPageSize (500) and
    advances by the number of items each page returns, so pages of 50 replay fine.
  - A run of identical Quick Connect polls answers with its last reply at once, so
    sign-in takes one poll instead of the 36 the recording needed. (Only those:
    other repeated requests, a playlist read before and after a write, say, keep
    their own replies.)
  - A cover (GET .../Images/Primary) is served whatever its size parameters: the
    recording holds each one at the 64 px the palette samples, and the apps ask for
    the size they draw (fillWidth, fillHeight), so the end-to-end screenshots show
    the covers, scaled.
  - A song's audio (GET /Audio/<id>/stream or /universal) is a few seconds of
    silence made here, as WAV, with ranges honoured: the recording holds no audio,
    and Phase 8's emulator test downloads songs and plays them offline. No audio is
    committed anywhere.
"""

import email.utils
import json
import os
import re
import struct
import sys
import threading
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qsl, urlsplit

IGNORED_PARAMS = {"limit"}
AUDIO_PATH = re.compile(r"^/Audio/[^/]+/(stream|universal)$")
SAMPLE_RATE = 8000
SECONDS = 4


def silence_wav():
    """SECONDS of 16-bit mono silence as a WAV file."""
    frames = b"\x00\x00" * SAMPLE_RATE * SECONDS
    header = b"RIFF" + struct.pack("<I", 36 + len(frames)) + b"WAVE"
    header += b"fmt " + struct.pack("<IHHIIHH", 16, 1, 1, SAMPLE_RATE, SAMPLE_RATE * 2, 2, 16)
    header += b"data" + struct.pack("<I", len(frames))
    return header + frames


SILENCE = silence_wav()


def load(path):
    with open(path, encoding="utf-8") as f:
        exchanges = json.load(f)
    # Transport failures ("refused", ...) cannot be served over a socket.
    exchanges = [x for x in exchanges if not x.get("error")]

    def key(x):
        return (x["method"].upper(), x["path"], sorted(x.get("query", {}).items()))

    collapsed = []
    for x in exchanges:
        polling = x["path"].endswith("/QuickConnect/Connect")
        if polling and collapsed and key(collapsed[-1]) == key(x):
            collapsed[-1] = x
        else:
            collapsed.append(x)
    return collapsed


def http_date(value):
    """The recorded Date header, as sent or in ISO 8601 form."""
    if value is None:
        return email.utils.formatdate(usegmt=True)
    if value[:3].isalpha():
        return value
    dt = datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone(timezone.utc)
    return email.utils.format_datetime(dt, usegmt=True)


class Replay:
    def __init__(self, exchanges):
        self.exchanges = exchanges
        self.used = [False] * len(exchanges)
        self.lock = threading.Lock()

    def find(self, method, path, query):
        pairs = [(k.lower(), v) for k, v in query]
        any_size = method == "GET" and path.endswith("/Images/Primary")
        matching = [
            i
            for i, x in enumerate(self.exchanges)
            if x["method"].upper() == method
            and x["path"] == path
            and (
                any_size
                or all(
                    (k.lower(), v) in pairs
                    for k, v in x.get("query", {}).items()
                    if k.lower() not in IGNORED_PARAMS
                )
            )
        ]
        with self.lock:
            pick = next((i for i in matching if not self.used[i]), None)
            if pick is None and matching:
                pick = matching[-1]
            if pick is None:
                return None
            self.used[pick] = True
            return self.exchanges[pick]


def handler(replay):
    class Handler(BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"
        server_version = "encore-replay"
        sys_version = ""
        recorded_date = None

        def date_time_string(self, timestamp=None):
            return self.recorded_date or super().date_time_string(timestamp)

        def answer(self, method):
            length = int(self.headers.get("Content-Length") or 0)
            if length:
                self.rfile.read(length)
            url = urlsplit(self.path)
            if method == "GET" and AUDIO_PATH.match(url.path):
                self.audio()
                return
            x = replay.find(method, url.path, parse_qsl(url.query, keep_blank_values=True))
            if x is None:
                # Never echo the query: it can hold the ApiKey parameter.
                body = f"no recorded reply for {method} {url.path}".encode()
                self.recorded_date = None
                self.send_response(404)
                self.send_header("Content-Type", "text/plain; charset=utf-8")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
                return
            self.recorded_date = http_date(x.get("date"))
            status = x.get("status") or 200
            if x.get("body") is not None:
                body = json.dumps(x["body"]).encode()
                kind = "application/json; charset=utf-8"
            elif x.get("bodyText") is not None:
                body = x["bodyText"].encode()
                kind = "text/plain; charset=utf-8"
            elif x.get("bodyHex") is not None:
                body = bytes.fromhex(x["bodyHex"])
                kind = x.get("contentType") or "application/octet-stream"
            else:
                body, kind = b"", None
            self.send_response(status)
            if status != 204:
                if kind:
                    self.send_header("Content-Type", kind)
                self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            if status != 204:
                self.wfile.write(body)

        def audio(self):
            """The made-up song, whole or from a `Range: bytes=N-`."""
            self.recorded_date = None
            body, status, start = SILENCE, 200, 0
            asked = re.match(r"bytes=(\d+)-$", self.headers.get("Range") or "")
            if asked and int(asked.group(1)) < len(SILENCE):
                start, status = int(asked.group(1)), 206
            self.send_response(status)
            self.send_header("Content-Type", "audio/wav")
            self.send_header("Accept-Ranges", "bytes")
            if status == 206:
                self.send_header("Content-Range", f"bytes {start}-{len(body) - 1}/{len(body)}")
            self.send_header("Content-Length", str(len(body) - start))
            self.end_headers()
            self.wfile.write(body[start:])

        def do_GET(self):
            self.answer("GET")

        def do_POST(self):
            self.answer("POST")

        def do_DELETE(self):
            self.answer("DELETE")

        def log_message(self, fmt, *args):
            # Path only, never the query (ApiKey).
            sys.stderr.write(f"replay: {self.command} {urlsplit(self.path).path} -> {args[1] if len(args) > 1 else ''}\n")

    return Handler


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: replay_jellyfin.py RECORDED.json URL_FILE")
    recorded, url_file = sys.argv[1], sys.argv[2]
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler(Replay(load(recorded))))
    tmp = url_file + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(f"http://127.0.0.1:{server.server_address[1]}")
    os.replace(tmp, url_file)
    server.serve_forever()


if __name__ == "__main__":
    main()
