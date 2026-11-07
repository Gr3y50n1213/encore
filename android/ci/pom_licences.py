"""Reads a Gradle dependency tree, resolves each module's final version, and prints its
licence from the POMs in the Gradle cache (following <parent> when a POM declares none)."""
import re, sys, pathlib, xml.etree.ElementTree as ET

tree, cache = sys.argv[1], pathlib.Path(sys.argv[2]) / "caches/modules-2/files-2.1"
mods = {}
for line in open(tree, encoding="utf-8", errors="replace"):
    m = re.search(r"[-\\] ([\w.\-]+):([\w.\-]+):([^\s]+)", line)
    if not m:
        continue
    g, a, v = m.groups()
    if "->" in line:
        v = line.split("->")[1].strip().split()[0]
    v = v.replace("{strictly", "").strip("{}")
    mods[(g, a)] = v

def pom(g, a, v):
    d = cache / g / a / v
    for p in d.glob(f"*/{a}-{v}.pom"):
        return p
    return None

def lic(g, a, v, depth=0):
    p = pom(g, a, v)
    if p is None or depth > 4:
        return None
    root = ET.parse(p).getroot()
    ns = {"m": root.tag.split("}")[0].strip("{")} if root.tag.startswith("{") else {"m": ""}
    q = (lambda t: f"m:{t}") if ns["m"] else (lambda t: t)
    names = [n.text.strip() for n in root.findall(f"{q('licenses')}/{q('license')}/{q('name')}", ns) if n.text]
    if names:
        return " / ".join(names)
    par = root.find(q("parent"), ns)
    if par is not None:
        pg = par.find(q("groupId"), ns).text; pa = par.find(q("artifactId"), ns).text; pv = par.find(q("version"), ns).text
        return lic(pg, pa, pv, depth + 1)
    return None

for (g, a), v in sorted(mods.items()):
    if v.startswith("project"):
        continue
    print(f"{g}:{a}\t{v}\t{lic(g, a, v) or 'NOT FOUND'}")
