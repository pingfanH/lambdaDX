#!/usr/bin/env python3
"""Bake the MajdataPlay slide prefabs into ``assets/slide.svg``.

MajdataPlay (``unity-flash-tools/MajdataPlay``) positions every slide's trail
bars and judgment ("slide OK") sprite inside a per-type Unity prefab
(``Star_Line_3.prefab``, ``Star_Circle_5.prefab``, …). This tool walks each
prefab's child hierarchy in **path order** and writes a compact SVG so the Rust
runtime can read the same geometry without Unity.

Each prefab is authored in a canonical frame (pad centre at the origin, button 1
at the top-right) and rotated at runtime by ``-45°·(start-1)``; mirror variants
are the prefab reflected across the Y axis. The SVG keeps that frame.

Coordinate conventions
----------------------
* Prefab uses Unity: y **up**, rotation CCW positive, unit = 4.8 (the A-ring
  radius, ``SlideGeo.MainRadius``); button 1 is canonical.
* The SVG keeps those Unity coordinates verbatim. The Rust loader reflects /
  rotates them by the note's start button and only then converts to screen
  space (y down, CW+) — keeping all the transform math in one frame.

Output::

    <g id="line3" data-kind="line">
      <polyline class="path" points="x,y x,y …" data-rots="deg,deg,…"/>
      <circle class="ok" cx="…" cy="…" data-rot="deg"/>
    </g>

Run:  python3 tools/extract_slide_prefabs.py [--src DIR] [--out FILE]
"""

import argparse
import math
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_SRC = os.path.join(
    os.path.expanduser("~"),
    "project",
    "unity-flash-tools",
    "MajdataPlay",
    "Assets",
    "Prefabs",
    "Game",
    "Slides",
)
DEFAULT_OUT = os.path.join(REPO, "assets", "slide.svg")

# type string -> prefab file stem. Mirrors NoteLoader.SLIDE_PREFAB_MAP.
def type_to_file(t: str) -> str:
    if t.startswith("line"):
        return "Star_Line_" + t[4:]
    if t.startswith("circle"):
        return "Star_Circle_" + t[6:]
    if t.startswith("ppqq"):
        return "Star_ppqq_" + t[4:]
    if t.startswith("pq"):
        return "Star_pq_" + t[2:]
    if t.startswith("v"):
        return "Star_V_" + t[1:]
    if t.startswith("L"):
        return "Star_L_" + t[1:]
    if t == "s":
        return "Star_S"
    if t == "wifi":
        return "Slide_Wifi"
    raise ValueError(t)


TYPES = (
    ["line3", "line4", "line5", "line6", "line7"]
    + [f"circle{i}" for i in range(1, 9)]
    + ["v1", "v2", "v3", "v4", "v6", "v7", "v8"]
    + [f"ppqq{i}" for i in range(1, 9)]
    + [f"pq{i}" for i in range(1, 9)]
    + ["s", "L2", "L3", "L4", "L5", "wifi"]
)

# The `slideok` sprite's natural world size (Unity units), from the base
# `Just_curv` / `Just_str` / `Just_wifi` prefabs' SpriteRenderer `m_Size`.
JUST_SIZE = {
    "Just_curv": (4.1, 1.4),
    "Just_str": (3.79, 1.08),
    "Just_wifi": (6.68, 1.88),
}


def _num(v: float) -> str:
    v = round(v, 4)
    if v == int(v):
        return str(int(v))
    return ("%g" % v)


def _norm_deg(a: float) -> float:
    """Normalize an angle to (-180, 180]."""
    a = math.fmod(a, 360.0)
    if a <= -180.0:
        a += 360.0
    elif a > 180.0:
        a -= 360.0
    return a


def _sub(a, b):
    return (a[0] - b[0], a[1] - b[1])


def _angle_between(a, b) -> float:
    la = math.hypot(*a)
    lb = math.hypot(*b)
    if la < 1e-9 or lb < 1e-9:
        return 0.0
    c = max(-1.0, min(1.0, (a[0] * b[0] + a[1] * b[1]) / (la * lb)))
    return math.degrees(math.acos(c))


def _densify(bars, subdiv=8, corner_deg=50.0):
    """Fit a smooth (Catmull-Rom/Hermite) curve through the tile centres.

    The prefab stores bars one *tile spacing* apart, so a curved slide becomes a
    coarse polygon (5–15° per corner) and the star visibly ratchets. Hermite
    interpolation through the same centres — using the neighbouring centres as
    tangents — yields a smooth curve that still passes exactly through them.
    Pairs whose tangents differ by more than ``corner_deg`` (a real V/L cusp) are
    left as straight lines so sharp vertices stay sharp.

    Returns ``(points, rots)``; rotations are the local path direction (stored in
    the prefab's anti-tangent convention) of the densified polyline.
    """
    P = [p for p, _ in bars]
    n = len(P)
    if n < 3:
        return P, [r for _, r in bars]

    dense = []
    for i in range(n - 1):
        p0, p1 = P[i], P[i + 1]
        m0 = _sub(p1, P[i - 1]) if i > 0 else _sub(p1, p0)
        m0 = (m0[0] * 0.5, m0[1] * 0.5)
        m1 = _sub(P[i + 2], p0) if i + 2 < n else _sub(p1, p0)
        m1 = (m1[0] * 0.5, m1[1] * 0.5)
        corner = _angle_between(m0, m1) > corner_deg
        for k in range(subdiv):
            t = k / subdiv
            if corner:
                dense.append((p0[0] + (p1[0] - p0[0]) * t, p0[1] + (p1[1] - p0[1]) * t))
            else:
                t2, t3 = t * t, t * t * t
                h00 = 2 * t3 - 3 * t2 + 1
                h10 = t3 - 2 * t2 + t
                h01 = -2 * t3 + 3 * t2
                h11 = t3 - t2
                dense.append(
                    (
                        h00 * p0[0] + h10 * m0[0] + h01 * p1[0] + h11 * m1[0],
                        h00 * p0[1] + h10 * m0[1] + h01 * p1[1] + h11 * m1[1],
                    )
                )
    dense.append(P[-1])

    # Rotations = local tangent, in the prefab's anti-tangent (bar) convention.
    rots = []
    for i in range(len(dense)):
        if i + 1 < len(dense):
            d = _sub(dense[i + 1], dense[i])
        else:
            d = _sub(dense[i], dense[i - 1])
        rots.append(_norm_deg(math.degrees(math.atan2(d[1], d[0])) + 180.0))
    return dense, rots


def _quat_z_angle(body: str) -> float:
    """Local Z rotation (deg, CCW+) from ``m_LocalRotation`` (q ≡ -q safe)."""
    qm = re.search(
        r"m_LocalRotation:\s*\{x:\s*([-\d.eE]+),\s*y:\s*([-\d.eE]+),"
        r"\s*z:\s*([-\d.eE]+),\s*w:\s*([-\d.eE]+)\}",
        body,
    )
    if not qm:
        return 0.0
    _, _, z, w = (float(qm.group(k)) for k in (1, 2, 3, 4))
    return math.degrees(2.0 * math.atan2(z, w))


def parse_prefab(path: str):
    """Parse a Unity prefab into ``(names, transforms, stripped, instances, root)``.

    * ``transforms``: fileID → (gameObject, x, y, angle, children)
    * ``stripped``:   fileID → prefabInstance fileID
    * ``instances``:  prefabInstance fileID → {propertyPath: value}
    """
    txt = open(path, encoding="utf-8-sig").read()
    parts = re.split(r"^--- !u!(\d+) &(-?\d+).*?$", txt, flags=re.M)
    names: dict[str, str] = {}
    transforms: dict[str, tuple] = {}
    stripped: dict[str, str] = {}
    instances: dict[str, dict] = {}
    i = 1
    while i + 2 <= len(parts):
        typ, fid, body = parts[i], parts[i + 1], parts[i + 2]
        i += 3
        if typ == "1":
            m = re.search(r"m_Name:\s*(.*)", body)
            names[fid] = m.group(1).strip() if m else ""
        elif typ == "4":
            gm = re.search(r"m_GameObject:\s*\{fileID:\s*(-?\d+)\}", body)
            if not gm:
                # A "stripped" prefab-instance transform: its pose lives in the
                # PrefabInstance overrides, not in this document.
                pi = re.search(r"m_PrefabInstance:\s*\{fileID:\s*(-?\d+)\}", body)
                if pi:
                    stripped[fid] = pi.group(1)
                continue
            pm = re.search(
                r"m_LocalPosition:\s*\{x:\s*([-\d.eE]+),\s*y:\s*([-\d.eE]+)", body
            )
            cm = re.search(r"m_Children:\n((?:\s*-\s*\{fileID:\s*-?\d+\}\n?)+)", body)
            ch = re.findall(r"-\s*\{fileID:\s*(-?\d+)\}", cm.group(1)) if cm else []
            x, y = (float(pm.group(1)), float(pm.group(2))) if pm else (0.0, 0.0)
            transforms[fid] = (gm.group(1), x, y, _quat_z_angle(body), ch)
        elif typ == "1001":
            mods: dict[str, str] = {}
            for tm in re.finditer(
                r"propertyPath:\s*(\S+)\n\s*value:\s*(.*)\n", body
            ):
                mods.setdefault(tm.group(1), tm.group(2).strip())
            instances[fid] = mods
    root = next(
        (fid for fid, (g, *_) in transforms.items() if names.get(g) == root_name(path)),
    )
    return names, transforms, stripped, instances, root


def root_name(path: str) -> str:
    return os.path.basename(path).replace(".prefab", "")


def _instance_pose(mods: dict):
    """(x, y, angle) from a prefab instance's overrides."""
    x = float(mods.get("m_LocalPosition.x", 0.0))
    y = float(mods.get("m_LocalPosition.y", 0.0))
    z = float(mods.get("m_LocalRotation.z", 0.0))
    w = float(mods.get("m_LocalRotation.w", 1.0))
    return x, y, _norm_deg(math.degrees(2.0 * math.atan2(z, w)))


def extract(path: str):
    """Return ``(bars, ok, just_name)`` in the Unity prefab frame."""
    names, transforms, stripped, instances, root = parse_prefab(path)
    _, _, _, _, children = transforms[root]
    bars = []
    ok = None
    just_name = ""
    for cid in children:
        if cid in transforms:
            gx, x, y, ang, _ = transforms[cid]
            name = names.get(gx, "")
        elif cid in stripped:
            mods = instances.get(stripped[cid], {})
            x, y, ang = _instance_pose(mods)
            name = mods.get("m_Name", "Just")
        else:
            continue
        # Keep Unity coordinates (y up, CCW+); the loader converts to screen.
        pos = (x, y)
        rot = _norm_deg(ang)
        if name.startswith("Just"):
            ok = (pos[0], pos[1], rot)
            just_name = name
        elif name.startswith(("Slide", "wifi")):
            # Path order is the child order; the fractional name is only a label.
            bars.append((pos, rot))
    return bars, ok, just_name


def emit_svg(entries, out_path: str):
    lines = [
        '<?xml version="1.0" encoding="UTF-8"?>',
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="-6 -6 12 12" '
        'data-unit="4.8" data-frame="unity-y-up-ccw">',
        "  <!-- Generated by tools/extract_slide_prefabs.py. Do not edit. -->",
        "  <!-- Frame: pad centre at origin, button 1 canonical, y up, rotation "
        "CCW+; unit = 4.8 (A-ring radius). -->",
    ]
    for t, pts, rots, ok, size in entries:
        ptxt = " ".join(f"{_num(x)},{_num(y)}" for x, y in pts)
        rtxt = ",".join(_num(r) for r in rots)
        lines.append(f'  <g id="{t}">')
        lines.append(
            f'    <polyline class="path" points="{ptxt}" data-rots="{rtxt}"/>'
        )
        if ok is not None:
            ox, oy, orot = ok
            sw, sh = size
            lines.append(
                f'    <circle class="ok" cx="{_num(ox)}" cy="{_num(oy)}" '
                f'data-rot="{_num(orot)}" data-size="{_num(sw)},{_num(sh)}"/>'
            )
        lines.append("  </g>")
    lines.append("</svg>")
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    with open(out_path, "w") as f:
        f.write("\n".join(lines) + "\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--src", default=DEFAULT_SRC)
    ap.add_argument("--out", default=DEFAULT_OUT)
    args = ap.parse_args()

    entries = []
    missing = []
    for t in TYPES:
        path = os.path.join(args.src, type_to_file(t) + ".prefab")
        if not os.path.exists(path):
            missing.append((t, path))
            continue
        bars, ok, just_name = extract(path)
        if not bars:
            missing.append((t, path + " (no bars)"))
            continue
        size = JUST_SIZE.get(just_name, (4.1, 1.4))
        pts, rots = _densify(bars)
        entries.append((t, pts, rots, ok, size))
    if missing:
        for t, p in missing:
            print(f"warn: missing {t}: {p}", file=sys.stderr)
    emit_svg(entries, args.out)
    print(f"wrote {args.out}: {len(entries)} slide types")


if __name__ == "__main__":
    main()
