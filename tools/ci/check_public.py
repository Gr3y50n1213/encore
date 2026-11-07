"""Fail when a tracked file holds something that must never be public.

    python3 -I -B tools/ci/check_public.py            check every file git tracks
    python3 -I -B tools/ci/check_public.py --self-test

The repository is public: this is the guard behind docs/PUBLIC-REPO-CHECKLIST.md.
It refuses:
  - email addresses (except placeholder domains such as example.com);
  - private keys, and tokens with a known prefix (GitHub, AWS, Slack, Google,
    OpenAI-style "sk-" keys, Encore's own "eat_"/"ert_" companion tokens);
  - a home folder path naming a person (C:\\Users\\<name>, /Users/<name>, /home/<name>;
    Jellyfin's own /Users/<user ID>/ API paths are not folders);
  - files that hold secrets or listening history by their kind: .env, keys and
    certificates, SQLite databases, anything under a private-history folder;
  - any file over 8 MB (recordings and art belong in fixtures, small and anonymized).
A line that must contain such a string on purpose (a test of this checker, say)
ends with the marker `public-ok`. Standard library only.
"""

import re
import subprocess
import sys

PATTERNS = [
    ("an email address", re.compile(r"[A-Za-z0-9._%+-]+@(?!example\.(?:com|org|net)\b)[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}")),
    ("a private key", re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----")),
    ("a token", re.compile(r"\b(?:ghp_[A-Za-z0-9]{20,}|gho_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,}|AIza[0-9A-Za-z_-]{35}|sk-[A-Za-z0-9]{32,}|e(?:a|r)t_[A-Za-z0-9]{40,})")),
    ("a person's home folder", re.compile(r"(?i)(?:[A-Z]:[\\/]+Users[\\/]+(?!Public\b|runneradmin\b|<)[A-Za-z0-9._-]+|/Users/(?!Shared\b|runner\b|<|\{|Me/|u\d+/|[0-9a-f]{32}/)[a-z0-9._-]+/|/home/(?!runner\b|<|user\b)[a-z0-9._-]+/)")),
]

BAD_FILES = re.compile(r"(?i)(?:^|/)(?:\.env(?:\..*)?|.*\.(?:pem|key|p12|jks|keystore|sqlite|sqlite-wal|sqlite-shm|db))$|(?:^|/)private-history/")
MAX_BYTES = 8 * 1024 * 1024
MARKER = "public-ok"


def problems_in(path, data):
    out = []
    if BAD_FILES.search(path):
        out.append(f"{path}: a file of this kind must never be committed")
    if len(data) > MAX_BYTES:
        out.append(f"{path}: {len(data) // 1024} KB, over the {MAX_BYTES // 1024 // 1024} MB limit")
    if b"\0" in data[:8192]:
        return out  # binary: only the name and size checks apply
    text = data.decode("utf-8", errors="replace")
    for n, line in enumerate(text.splitlines(), 1):
        if MARKER in line:
            continue
        for what, rx in PATTERNS:
            m = rx.search(line)
            if m:
                shown = m.group(0)
                shown = shown[:4] + "..." if what in ("a token", "an email address") else shown
                out.append(f"{path}:{n}: {what} ({shown})")
    return out


def self_test():
    bad = {
        "a.txt": b"mail me at someone@gmail.com\n",  # public-ok
        "b.rs": b'let k = "-----BEGIN PRIVATE KEY-----";\n',  # public-ok
        "c.md": b"token ghp_abcdefghijklmnopqrstuvwxyz0123\n",  # public-ok
        "d.ps1": b'$x = "C:\\Users\\Alex\\tools"\n',  # public-ok
        "e/.env": b"X=1\n",
        "f/library.sqlite": b"",
        "tools/target/private-history/h.json": b"{}",
    }
    good = {
        "g.md": b"write to someone@example.com or ops@example.org\n",
        "h.yml": b"path: /home/runner/work and C:\\Users\\runneradmin\n",
        "i.rs": b"let mail = \"x@y.zz\"; // public-ok\n",
        "j.json": b"\"path\": \"/Users/014f1e79ae0f8eacc81b08b5fae17697/Images\", \"/Users/u1/Items\"\n",
    }
    for name, data in bad.items():
        assert problems_in(name, data), f"missed {name}"
    for name, data in good.items():
        assert not problems_in(name, data), f"wrongly refused {name}: {problems_in(name, data)}"
    print("check_public self-test OK")


def main():
    if "--self-test" in sys.argv:
        return self_test()
    files = subprocess.run(["git", "ls-files", "-z"], capture_output=True, check=True).stdout.split(b"\0")
    found = []
    for f in filter(None, files):
        path = f.decode("utf-8", errors="replace")
        try:
            with open(path, "rb") as fh:
                data = fh.read()
        except OSError:
            continue
        found += problems_in(path, data)
    if found:
        print("Not safe to publish:")
        print("\n".join("  " + p for p in found))
        sys.exit(1)
    print(f"{len([f for f in files if f])} tracked files: nothing that must stay private")


if __name__ == "__main__":
    main()
