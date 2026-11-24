"""Fetch a labelled sample of 30 s clips from the Free Music Archive 'small' set, reading only the
needed parts of the big zips with HTTP range requests.

usage: python fetch_fma.py OUT_DIR PER_GENRE (docs/adr/core-0019: evaluation only; the clips are Creative Commons per track and are never stored in the repository)
Writes OUT_DIR/clips/<track_id>.mp3 and OUT_DIR/labels.csv (track_id,genre).
"""
import csv
import io
import os
import random
import sys
import zipfile

import pandas as pd
import requests

BASE = "https://os.unil.cloud.switch.ch/fma/"
BLOCK = 1 << 20


class RangeFile(io.RawIOBase):
    """A read-only seekable file over HTTP range requests, with a small block cache."""

    def __init__(self, url):
        self.url = url
        self.pos = 0
        self.session = requests.Session()
        head = self.session.head(url, allow_redirects=True, timeout=60)
        head.raise_for_status()
        self.size = int(head.headers["Content-Length"])
        self.cache = {}

    def seekable(self):
        return True

    def readable(self):
        return True

    def tell(self):
        return self.pos

    def seek(self, offset, whence=0):
        if whence == 0:
            self.pos = offset
        elif whence == 1:
            self.pos += offset
        else:
            self.pos = self.size + offset
        return self.pos

    def _block(self, index):
        if index not in self.cache:
            start = index * BLOCK
            end = min(start + BLOCK, self.size) - 1
            r = self.session.get(self.url, headers={"Range": f"bytes={start}-{end}"}, timeout=120)
            if r.status_code not in (200, 206):
                raise IOError(f"range {start}-{end}: HTTP {r.status_code}")
            self.cache[index] = r.content
            if len(self.cache) > 64:
                self.cache.pop(next(iter(self.cache)))
        return self.cache[index]

    def readinto(self, b):
        n = min(len(b), self.size - self.pos)
        if n <= 0:
            return 0
        out = bytearray()
        pos = self.pos
        while len(out) < n:
            blk = self._block(pos // BLOCK)
            off = pos % BLOCK
            take = min(n - len(out), len(blk) - off)
            out += blk[off:off + take]
            pos += take
        b[:n] = out
        self.pos += n
        return n


def main():
    out_dir, per_genre = sys.argv[1], int(sys.argv[2])
    os.makedirs(os.path.join(out_dir, "clips"), exist_ok=True)
    print("reading the metadata zip's table of contents...", flush=True)
    meta = zipfile.ZipFile(io.BufferedReader(RangeFile(BASE + "fma_metadata.zip"), BLOCK))
    with meta.open("fma_metadata/tracks.csv") as f:
        tracks = pd.read_csv(f, index_col=0, header=[0, 1])
    small = tracks[tracks[("set", "subset")] == "small"]
    genre = small[("track", "genre_top")].dropna()
    print("small set:", len(small), "tracks;", genre.value_counts().to_dict(), flush=True)
    rng = random.Random(13)
    chosen = []
    for g in sorted(genre.unique()):
        ids = sorted(genre[genre == g].index.tolist())
        rng.shuffle(ids)
        chosen += [(i, g) for i in ids[:per_genre]]
    print("fetching", len(chosen), "clips...", flush=True)
    clips = zipfile.ZipFile(io.BufferedReader(RangeFile(BASE + "fma_small.zip"), BLOCK))
    names = set(clips.namelist())
    with open(os.path.join(out_dir, "labels.csv"), "w", newline="") as lf:
        w = csv.writer(lf)
        w.writerow(["track_id", "genre"])
        for n, (tid, g) in enumerate(chosen, 1):
            name = f"fma_small/{tid:06d}"[:13] + f"/{tid:06d}.mp3"
            if name not in names:
                print("missing", name)
                continue
            with open(os.path.join(out_dir, "clips", f"{tid}.mp3"), "wb") as o:
                o.write(clips.read(name))
            w.writerow([tid, g])
            if n % 20 == 0:
                print(n, "of", len(chosen), flush=True)
    print("done")


if __name__ == "__main__":
    main()
