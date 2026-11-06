"""Check an Android shared library before an app ships it.

Usage: python3 -I check_android_elf.py READELF NM LIBRARY.so SYMBOL...

Fails unless
  * every LOAD segment is aligned to at least 16 KB, and the GNU_RELRO segment
    cannot make writable data read-only on a 16 KB-page device. Both rules are
    from https://developer.android.com/guide/practices/page-sizes ("Check the
    alignment of ELF segments" and "Check the RELRO security flag"); and
  * every SYMBOL is exported, so the Kotlin bindings can find it through JNA.
It also prints the libraries the file needs at run time.
"""

import subprocess
import sys

PAGE_16K = 0x4000


def run(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout


def segments(readelf, path):
    """(type, vaddr, memsz, align) for each LOAD and GNU_RELRO program header."""
    found = []
    for line in run(readelf, "-lW", path).splitlines():
        f = line.split()
        # Type Offset VirtAddr PhysAddr FileSiz MemSiz Flg Align; Flg can be two words ("R E").
        if f and f[0] in ("LOAD", "GNU_RELRO"):
            found.append((f[0], int(f[2], 16), int(f[5], 16), int(f[-1], 16)))
    return found


def main(argv):
    if len(argv) < 4:
        print(__doc__, file=sys.stderr)
        return 2
    readelf, nm, path, symbols = argv[0], argv[1], argv[2], argv[3:]
    problems = []

    segs = segments(readelf, path)
    loads = [s for s in segs if s[0] == "LOAD"]
    if not loads:
        problems.append("no LOAD segments found")
    for _, vaddr, _, align in loads:
        if align < PAGE_16K:
            problems.append(f"LOAD at {vaddr:#x} is aligned to {align:#x}, under 16 KB")

    for _, vaddr, memsz, _ in (s for s in segs if s[0] == "GNU_RELRO"):
        end = vaddr + memsz
        whole_load = any(lv <= vaddr and lv + lm == end for _, lv, lm, _ in loads)
        if end % PAGE_16K != 0 and not whole_load:
            problems.append(
                f"GNU_RELRO ends at {end:#x}, not on a 16 KB boundary, "
                "and writable data follows it in the same LOAD segment"
            )

    exported = set(run(nm, "-D", "--defined-only", "--format=just-symbols", path).split())
    for sym in symbols:
        if sym not in exported:
            problems.append(f"symbol {sym} is not exported")

    needed = [
        line.split("[", 1)[1].rstrip("]")
        for line in run(readelf, "-dW", path).splitlines()
        if "(NEEDED)" in line
    ]
    print(f"{path}: {len(loads)} LOAD segments, needs {', '.join(needed) or 'nothing'}")

    for p in problems:
        print(f"FAIL {path}: {p}", file=sys.stderr)
    if not problems:
        print(f"OK {path}: 16 KB aligned, {len(symbols)} symbols exported")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
