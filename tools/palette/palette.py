"""Encore colour schemes: Material 3 tonal palettes from the profile accent seeds.

Generates design/tokens.json -> color.accents: for each of the eight profile accents,
the light and dark Material 3 colour roles that Android and the Windows and Linux app
use when no album colour applies (the default is Iris, the boards' Android accent).
Apple devices use Apple's own system colours and are not generated here.

Method (HCT, as in google/material-color-utilities, Apache-2.0): CAM16 hue and chroma
with CIELAB L* as the tone. Palettes follow Material's tonal spot spread (secondary
chroma 16, tertiary chroma 24 at hue + 60, neutral 6, neutral variant 8, error hue 25
chroma 84) with the primary kept at the seed's own chroma, at least 48, so Play keeps
the boards' saturated colour. With the seed #6750A4 this reproduces Material's
baseline roles, the ones the tablet and desktop boards are drawn in.

Every text pair is checked for WCAG 4.5:1 and every non-text pair for 3:1.

  python3 -I palette.py tokens > color.json    print the color.accents group
  python3 -I palette.py check  TOKENS.json     exit 1 unless tokens.json matches
"""
import json
import math
import sys

# ---------- sRGB and CIELAB L* (D65) ----------

def srgb_to_lin(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

def lin_to_srgb(c):
    return 12.92 * c if c <= 0.0031308 else 1.055 * (c ** (1 / 2.4)) - 0.055

def hex_to_rgb(h):
    h = h.lstrip('#')
    return tuple(int(h[i:i + 2], 16) / 255 for i in (0, 2, 4))

def rgb_to_hex(rgb):
    return '#' + ''.join(f'{round(max(0, min(1, c)) * 255):02X}' for c in rgb)

def rgb_to_xyz(rgb):
    r, g, b = (srgb_to_lin(c) for c in rgb)
    return (0.4124564 * r + 0.3575761 * g + 0.1804375 * b,
            0.2126729 * r + 0.7151522 * g + 0.0721750 * b,
            0.0193339 * r + 0.1191920 * g + 0.9503041 * b)

def f(t):
    return t ** (1 / 3) if t > (6 / 29) ** 3 else t / (3 * (6 / 29) ** 2) + 4 / 29

def finv(t):
    return t ** 3 if t > 6 / 29 else 3 * (6 / 29) ** 2 * (t - 4 / 29)

# ---------- HCT: CAM16 hue and chroma, CIELAB L* tone (Material Color Utilities) ----------
# Follows the published algorithm of google/material-color-utilities (Apache-2.0):
# default viewing conditions, CAM16 forward and inverse, and the original HCT solver
# (search J for the tone, then reduce chroma until the colour fits sRGB).

M16 = ((0.401288, 0.650173, -0.051461), (-0.250268, 1.204414, 0.045854), (-0.002079, 0.048952, 0.953127))
M16_INV = ((1.8620678, -1.0112547, 0.14918678), (0.38752654, 0.62144744, -0.00897398),
           (-0.01584150, -0.03412294, 1.0499644))
XYZ_TO_LIN = ((3.2413774792388685, -1.5376652402851851, -0.49885366846268053),
              (-0.9691452513005321, 1.8758853451067872, 0.04156585616912061),
              (0.05562093689691305, -0.20395524564742123, 1.0571799111220335))

def mul(m, v):
    return tuple(sum(m[i][j] * v[j] for j in range(3)) for i in range(3))

def y_from_lstar(l):
    return 100 * finv((l + 16) / 116)

def _viewing_conditions():
    from types import SimpleNamespace
    white = (95.047, 100.0, 108.883)
    La = (200 / math.pi) * y_from_lstar(50) / 100
    f_ = 1.0
    rgbW = mul(M16, white)
    d = max(0.0, min(1.0, f_ * (1 - (1 / 3.6) * math.exp((-La - 42) / 92))))
    rgbD = tuple(d * (100 / w) + 1 - d for w in rgbW)
    k4 = (1 / (5 * La + 1)) ** 4
    fl = k4 * La + 0.1 * (1 - k4) ** 2 * (5 * La) ** (1 / 3)
    n = y_from_lstar(50) / white[1]
    nbb = 0.725 / n ** 0.2
    af = [(fl * rgbD[i] * rgbW[i] / 100) ** 0.42 for i in range(3)]
    a = [400 * x / (x + 27.13) for x in af]
    return SimpleNamespace(white=white, c=0.69, nc=f_, rgbD=rgbD, fl=fl, n=n, z=1.48 + math.sqrt(n),
                           nbb=nbb, ncb=nbb, aw=(2 * a[0] + a[1] + 0.05 * a[2]) * nbb)

VC = _viewing_conditions()

def cam16(hexcolor):
    """(J, chroma, hue, M) of an sRGB colour."""
    xyz = tuple(100 * v for v in rgb_to_xyz(hex_to_rgb(hexcolor)))
    rc = mul(M16, xyz)
    rd = tuple(VC.rgbD[i] * rc[i] for i in range(3))
    ra = []
    for v in rd:
        af = (VC.fl * abs(v) / 100) ** 0.42
        ra.append(math.copysign(400 * af / (af + 27.13), v))
    r, g, b = ra
    a = (11 * r - 12 * g + b) / 11
    bb = (r + g - 2 * b) / 9
    u = (20 * r + 20 * g + 21 * b) / 20
    p2 = (40 * r + 20 * g + b) / 20
    hue = math.degrees(math.atan2(bb, a)) % 360
    ac = p2 * VC.nbb
    J = 100 * (ac / VC.aw) ** (VC.c * VC.z) if ac > 0 else 0.0
    hp = hue + 360 if hue < 20.14 else hue
    ehue = 0.25 * (math.cos(math.radians(hp) + 2) + 3.8)
    p1 = 50000 / 13 * ehue * VC.nc * VC.ncb
    t = p1 * math.hypot(a, bb) / (u + 0.305)
    alpha = t ** 0.9 * (1.64 - 0.29 ** VC.n) ** 0.73
    C = alpha * math.sqrt(J / 100)
    return J, C, hue, C * VC.fl ** 0.25

def cam16_to_lin(J, C, H):
    """Linear sRGB (unclipped) of a CAM16 J, chroma, hue."""
    if J <= 0:
        return (0.0, 0.0, 0.0)
    alpha = 0 if C == 0 else C / math.sqrt(J / 100)
    t = (alpha / (1.64 - 0.29 ** VC.n) ** 0.73) ** (1 / 0.9)
    h = math.radians(H)
    ehue = 0.25 * (math.cos(h + 2) + 3.8)
    ac = VC.aw * (J / 100) ** (1 / VC.c / VC.z)
    p1 = ehue * (50000 / 13) * VC.nc * VC.ncb
    p2 = ac / VC.nbb
    hs, hc = math.sin(h), math.cos(h)
    gamma = 23 * (p2 + 0.305) * t / (23 * p1 + 11 * t * hc + 108 * t * hs)
    a, b = gamma * hc, gamma * hs
    ra = (460 * p2 + 451 * a + 288 * b) / 1403
    ga = (460 * p2 - 891 * a - 261 * b) / 1403
    ba = (460 * p2 - 220 * a - 6300 * b) / 1403
    out = []
    for v, dd in zip((ra, ga, ba), VC.rgbD):
        base = max(0.0, 27.13 * abs(v) / (400 - abs(v)))
        out.append(math.copysign(100 / VC.fl * base ** (1 / 0.42), v) / dd)
    xyz = mul(M16_INV, out)
    return mul(XYZ_TO_LIN, tuple(v / 100 for v in xyz))

def ucs(J, M, H):
    js = (1 + 100 * 0.007) * J / (1 + 0.007 * J)
    ms = 1 / 0.0228 * math.log(1 + 0.0228 * M)
    return js, ms * math.cos(math.radians(H)), ms * math.sin(math.radians(H))

def clip_hex(lin):
    return rgb_to_hex(tuple(lin_to_srgb(max(0.0, min(1.0, v))) for v in lin))

def lstar_hex(h):
    return 116 * f(luminance(h)) - 16

def _find_by_j(H, C, T):
    lo, hi, best, best_dl = 0.0, 100.0, None, 1000.0
    while abs(lo - hi) > 0.01:
        mid = lo + (hi - lo) / 2
        clipped = clip_hex(cam16_to_lin(mid, C, H))
        cl = lstar_hex(clipped)
        dl = abs(T - cl)
        if dl < 0.2:
            J2, C2, H2, M2 = cam16(clipped)
            jt, ct, ht = J2, C2, H
            # distance between the clipped colour and the same J and chroma at the target hue
            M_t = ct * VC.fl ** 0.25
            a1, a2 = ucs(J2, M2, H2), ucs(jt, M_t, ht)
            de = 1.41 * math.dist(a1, a2) ** 0.63
            if de <= 1 and dl <= best_dl:
                best, best_dl = clipped, dl
        if cl < T:
            lo = mid
        else:
            hi = mid
    return best

def tone(T, C, H):
    """HCT colour: hue H, chroma up to C, tone (L*) T."""
    if T <= 0.5:
        return '#000000'
    if T >= 99.5:
        return '#FFFFFF'
    gray = clip_hex(mul(XYZ_TO_LIN, tuple(v / 100 * y_from_lstar(T) / 100 for v in VC.white)))
    if C < 1:
        return gray
    hi, mid, lo, answer, first = C, C, 0.0, None, True
    while abs(lo - hi) >= 0.4:
        found = _find_by_j(H, mid, T)
        if first:
            if found is not None:
                return found
            first = False
            mid = lo + (hi - lo) / 2
            continue
        if found is None:
            hi = mid
        else:
            answer, lo = found, mid
        mid = lo + (hi - lo) / 2
    return answer or gray

def hct(hexcolor):
    J, C, H, M = cam16(hexcolor)
    return H, C, lstar_hex(hexcolor)

def luminance(hexcolor):
    return rgb_to_xyz(hex_to_rgb(hexcolor))[1]

def contrast(a, b):
    la, lb = sorted((luminance(a), luminance(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)

# ---------- Material 3 roles (tone of each palette) ----------

ROLES_LIGHT = {
    'primary': ('p', 40), 'onPrimary': ('p', 100), 'primaryContainer': ('p', 90), 'onPrimaryContainer': ('p', 10),
    'secondary': ('s', 40), 'onSecondary': ('s', 100), 'secondaryContainer': ('s', 90), 'onSecondaryContainer': ('s', 10),
    'tertiary': ('t', 40), 'onTertiary': ('t', 100), 'tertiaryContainer': ('t', 90), 'onTertiaryContainer': ('t', 10),
    'error': ('e', 40), 'onError': ('e', 100), 'errorContainer': ('e', 90), 'onErrorContainer': ('e', 10),
    'surface': ('n', 98), 'onSurface': ('n', 10), 'onSurfaceVariant': ('nv', 30),
    'surfaceContainerLowest': ('n', 100), 'surfaceContainerLow': ('n', 96), 'surfaceContainer': ('n', 94),
    'surfaceContainerHigh': ('n', 92), 'surfaceContainerHighest': ('n', 90), 'surfaceDim': ('n', 87), 'surfaceBright': ('n', 98),
    'outline': ('nv', 50), 'outlineVariant': ('nv', 80),
    'inverseSurface': ('n', 20), 'inverseOnSurface': ('n', 95), 'inversePrimary': ('p', 80), 'scrim': ('n', 0),
}
ROLES_DARK = {
    'primary': ('p', 80), 'onPrimary': ('p', 20), 'primaryContainer': ('p', 30), 'onPrimaryContainer': ('p', 90),
    'secondary': ('s', 80), 'onSecondary': ('s', 20), 'secondaryContainer': ('s', 30), 'onSecondaryContainer': ('s', 90),
    'tertiary': ('t', 80), 'onTertiary': ('t', 20), 'tertiaryContainer': ('t', 30), 'onTertiaryContainer': ('t', 90),
    'error': ('e', 80), 'onError': ('e', 20), 'errorContainer': ('e', 30), 'onErrorContainer': ('e', 90),
    'surface': ('n', 6), 'onSurface': ('n', 90), 'onSurfaceVariant': ('nv', 80),
    'surfaceContainerLowest': ('n', 4), 'surfaceContainerLow': ('n', 10), 'surfaceContainer': ('n', 12),
    'surfaceContainerHigh': ('n', 17), 'surfaceContainerHighest': ('n', 22), 'surfaceDim': ('n', 6), 'surfaceBright': ('n', 24),
    'outline': ('nv', 60), 'outlineVariant': ('nv', 30),
    'inverseSurface': ('n', 90), 'inverseOnSurface': ('n', 20), 'inversePrimary': ('p', 40), 'scrim': ('n', 0),
}

# Text pairs that must reach 4.5:1, and non-text pairs that must reach 3:1.
TEXT_PAIRS = [
    ('onPrimary', 'primary'), ('onPrimaryContainer', 'primaryContainer'),
    ('onSecondary', 'secondary'), ('onSecondaryContainer', 'secondaryContainer'),
    ('onTertiary', 'tertiary'), ('onTertiaryContainer', 'tertiaryContainer'),
    ('onError', 'error'), ('onErrorContainer', 'errorContainer'),
    ('onSurface', 'surface'), ('onSurfaceVariant', 'surface'), ('onSurface', 'surfaceContainerHighest'),
    ('onSurfaceVariant', 'surfaceContainerHighest'), ('primary', 'surface'), ('primary', 'surfaceContainerHigh'),
    ('inverseOnSurface', 'inverseSurface'),
]
NONTEXT_PAIRS = [('outline', 'surface'), ('primary', 'surfaceContainerHighest')]
TEXT_MIN, NONTEXT_MIN = 4.5, 3.0

# A grey seed (chroma under 16) keeps its own low chroma throughout.
def primary_chroma(C):
    return C if C < 16 else max(C, 48)

def palettes(seed):
    H, C, _ = hct(seed)
    grey = C < 16
    return {
        'p': (primary_chroma(C), H),
        's': (4 if grey else 16, H),
        't': (8 if grey else 24, (H + 60) % 360),
        'e': (84, 25),
        'n': (2 if grey else 6, H),
        'nv': (4 if grey else 8, H),
    }

def scheme(seed, roles):
    pal = palettes(seed)
    return {name: tone(t, *pal[p]) for name, (p, t) in roles.items()}

def worst(s, pairs):
    return min(contrast(s[a], s[b]) for a, b in pairs)

# The eight profile accents the owner approved on 6 November 2026. Iris is the default.
ACCENTS = [
    ('iris', '#5B3FC4'),
    ('cobalt', '#2F6BFF'),
    ('lagoon', '#007C89'),
    ('fern', '#2F7D3A'),
    ('amber', '#A15C00'),
    ('coral', '#C8442A'),
    ('rose', '#B8326A'),
    ('graphite', '#5E6670'),
]

def accents_group():
    """The color.accents group of tokens.json, in a fixed order."""
    group = {}
    for name, seed in ACCENTS:
        light, dark = scheme(seed, ROLES_LIGHT), scheme(seed, ROLES_DARK)
        text = min(worst(light, TEXT_PAIRS), worst(dark, TEXT_PAIRS))
        non_text = min(worst(light, NONTEXT_PAIRS), worst(dark, NONTEXT_PAIRS))
        if text < TEXT_MIN or non_text < NONTEXT_MIN:
            raise SystemExit(f'{name}: contrast {text:.2f} (text) / {non_text:.2f} (non-text) is too low')
        group[name] = {
            'seed': seed,
            'lowestTextContrast': math.floor(text * 100) / 100,
            'lowestNonTextContrast': math.floor(non_text * 100) / 100,
            'light': light,
            'dark': dark,
        }
    return group

def main(argv):
    if argv[:1] == ['tokens']:
        print(json.dumps(accents_group(), indent=1))
        return 0
    if len(argv) == 2 and argv[0] == 'check':
        with open(argv[1], encoding='utf-8') as fh:
            have = json.load(fh)['color']['accents']
        want = accents_group()
        if have != want:
            for name in want:
                for key, value in want[name].items():
                    if have.get(name, {}).get(key) != value:
                        print(f'color.accents.{name}.{key} differs from the generator', file=sys.stderr)
            print('Regenerate: python3 -I tools/palette/palette.py tokens', file=sys.stderr)
            return 1
        print(f'colour schemes are up to date ({len(want)} accents)')
        return 0
    print(__doc__, file=sys.stderr)
    return 2

if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
