"""Fail when a dependency's licence has not been reviewed (plan section 24).

    python3 -I -B tools/ci/check_licences.py tools/ci/licences.json core server tools
    python3 -I -B tools/ci/check_licences.py --self-test

For each Cargo workspace it reads `cargo metadata --locked` (every package for every
target, including build and dev dependencies, so nothing slips in through one
platform) and checks each registry or git package's SPDX expression. An expression
passes when one of its OR choices is made entirely of reviewed identifiers
(licences.json "apps", plus "serverOnly" for the server workspace). A package with
no licence passes only if licences.json "undeclared" names it.
Standard library only.
"""

import json
import re
import subprocess
import sys


PREFERRED = {"MIT", "Apache-2.0"}


def dnf(expression):
    """SPDX expression -> list of alternatives, each a set of identifiers that must
    all hold. "A OR (B AND C)" -> [{A}, {B, C}]. "/" is the old spelling of OR."""
    tokens = re.findall(r"\(|\)|[^\s()]+", expression.replace("/", " OR "))
    pos = 0

    def peek():
        return tokens[pos] if pos < len(tokens) else None

    def take():
        nonlocal pos
        if pos >= len(tokens):
            raise ValueError(f"ends early: {expression!r}")
        pos += 1
        return tokens[pos - 1]

    def factor():
        t = take()
        if t == "(":
            alts = expr()
            if take() != ")":
                raise ValueError(f"unbalanced: {expression!r}")
            return alts
        if t is None or t.upper() in ("AND", "OR", "WITH", ")"):
            raise ValueError(f"unexpected {t!r} in {expression!r}")
        if peek() and peek().upper() == "WITH":
            take()
            t = f"{t} WITH {take()}"
        return [{t}]

    def term():
        alts = factor()
        while peek() and peek().upper() == "AND":
            take()
            right = factor()
            alts = [a | b for a in alts for b in right]
        return alts

    def expr():
        alts = term()
        while peek() and peek().upper() == "OR":
            take()
            alts = alts + term()
        return alts

    result = expr()
    if pos != len(tokens):
        raise ValueError(f"trailing tokens in {expression!r}")
    return result


def check(packages, allowed, undeclared):
    """Returns (problems, chosen): one line per package that fails, and the
    alternative used for each that passes."""
    problems, chosen = [], {}
    for p in packages:
        name, version, lic = p["name"], p["version"], p.get("license")
        if not lic:
            if name in undeclared:
                lic = undeclared[name]["licence"]
            else:
                problems.append(f"{name} {version}: declares no licence (read its LICENSE file, then add it to \"undeclared\")")
                continue
        try:
            alts = dnf(lic)
        except ValueError as e:
            problems.append(f"{name} {version}: {e}")
            continue
        # Where a crate offers a choice, MIT or Apache-2.0 is the one taken
        # (THIRD_PARTY_LICENSES.md), so report that one.
        ok = sorted((a for a in alts if a <= allowed), key=lambda a: (not a <= PREFERRED, len(a)))
        if ok:
            chosen[f"{name} {version}"] = " AND ".join(sorted(ok[0]))
        else:
            missing = sorted(set().union(*alts) - allowed)
            problems.append(f"{name} {version}: \"{lic}\" needs a licence not yet reviewed: {', '.join(missing)}")
    return problems, chosen


def self_test():
    assert dnf("MIT") == [{"MIT"}]
    assert dnf("MIT/Apache-2.0") == [{"MIT"}, {"Apache-2.0"}]
    assert dnf("(MIT OR Apache-2.0) AND Unicode-3.0") == [{"MIT", "Unicode-3.0"}, {"Apache-2.0", "Unicode-3.0"}]
    assert dnf("Apache-2.0 WITH LLVM-exception OR MIT") == [{"Apache-2.0 WITH LLVM-exception"}, {"MIT"}]
    allowed = {"MIT", "Apache-2.0", "ISC"}
    pk = lambda n, l: {"name": n, "version": "1.0.0", "license": l}
    problems, chosen = check(
        [
            pk("a", "MIT OR Apache-2.0 OR LGPL-2.1-or-later"),
            pk("b", "Apache-2.0 AND ISC"),
            pk("c", "GPL-3.0-only"),
            pk("d", "MIT AND BSD-3-Clause"),
            pk("e", None),
            pk("f", None),
        ],
        allowed,
        {"f": {"licence": "MIT"}},
    )
    assert chosen == {"a 1.0.0": "MIT", "b 1.0.0": "Apache-2.0 AND ISC", "f 1.0.0": "MIT"}, chosen
    assert len(problems) == 3 and "GPL-3.0-only" in problems[0] and "BSD-3-Clause" in problems[1], problems
    assert "declares no licence" in problems[2]
    for bad in ("MIT OR", "(MIT", "MIT AND AND ISC"):
        try:
            dnf(bad)
        except ValueError:
            continue
        raise AssertionError(f"accepted {bad!r}")
    print("check_licences self-test passed")


def main():
    if sys.argv[1:] == ["--self-test"]:
        self_test()
        return
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    with open(sys.argv[1], encoding="utf-8") as f:
        reviewed = json.load(f)
    failed = False
    for ws in sys.argv[2:]:
        allowed = set(reviewed["apps"])
        if ws.rstrip("/").endswith("server"):
            allowed |= set(reviewed["serverOnly"])
        out = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked"],
            cwd=ws, check=True, capture_output=True, text=True,
        ).stdout
        packages = [p for p in json.loads(out)["packages"] if p.get("source")]
        problems, chosen = check(packages, allowed, reviewed["undeclared"])
        used = sorted({part for c in chosen.values() for part in c.split(" AND ")})
        print(f"{ws}: {len(packages)} dependencies, {len(chosen)} on reviewed licences ({', '.join(used)})")
        for line in problems:
            print(f"::error title=Unreviewed licence ({ws})::{line}")
            failed = True
    if failed:
        sys.exit("A dependency needs a licence that has not been reviewed: read it, then update "
                 "tools/ci/licences.json and core/THIRD_PARTY_LICENSES.md.")


if __name__ == "__main__":
    main()
