#!/usr/bin/env python3
"""Export an Animation IR document to a Rive `.riv` via the OpenRive MCP server.

Phase 2 of the Flash/Animate -> IR -> Rive + macroquad migration. Reads the JSON
snapshot produced by `animation_ir` and drives OpenRive's MCP tools
(create_project / add_artboard / add_path / add_group / add_text / add_timeline /
add_keyframes / export_riv) to rebuild the document as a Rive file.

Usage (OpenRive must be running):

    cargo run -q -p animation_ir --example snapshot -- assets/effects/tap_hit --json > /tmp/tap_hit.ir.json
    python3 tools/ir_to_rive.py --ir /tmp/tap_hit.ir.json --name tap_hit --out assets/rive/vfx/tap_hit.riv

Scope of this first cut:

* One artboard per `movieclip` symbol (plus a `Scene` artboard when the scene has
  elements), sized to the document.
* `shape` elements -> `add_path` (arcs are already flattened to polygons by the
  XFL parser), with fill/stroke and straight alpha.
* `instance` elements -> a Rive group, its child geometry inlined from the
  referenced symbol, and x/y/rotation/scale/opacity keyframes taken from the XFL
  layer's motion tween.
* `text` -> `add_text`.
* `bitmap` elements are skipped (the MCP surface has no image import); they are
  counted and reported.
"""

from __future__ import annotations

import argparse
import base64
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from openrive_mcp import OpenRive  # noqa: E402


# ── Matrix / colour helpers ────────────────────────────────────────────────
# Flash column-major matrix [a, b, c, d, tx, ty]: (x,y) -> (a*x+c*y+tx, b*x+d*y+ty).
IDENTITY = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]


def mat_mul(parent: list[float], child: list[float]) -> list[float]:
    pa, pb, pc, pd, ptx, pty = parent
    ca, cb, cc, cd, ctx, cty = child
    return [
        pa * ca + pc * cb,
        pb * ca + pd * cb,
        pa * cc + pc * cd,
        pb * cc + pd * cd,
        pa * ctx + pc * cty + ptx,
        pb * ctx + pd * cty + pty,
    ]


def apply(m: list[float], pt: list[float]) -> list[float]:
    return [m[0] * pt[0] + m[2] * pt[1] + m[4], m[1] * pt[0] + m[3] * pt[1] + m[5]]


def decompose(m: list[float]) -> dict:
    a, b, c, d, tx, ty = m
    return {
        "x": tx,
        "y": ty,
        "rotationDegrees": math.degrees(math.atan2(b, a)),
        "scaleX": math.hypot(a, b),
        # Keep a mirror as negative scaleY rather than a 180° flip.
        "scaleY": math.hypot(c, d) * (-1.0 if a * d - b * c < 0 else 1.0),
    }


def css(color: dict) -> str:
    a = max(0, min(255, round(color["a"] * 255)))
    rgb = f"#{color['r']:02x}{color['g']:02x}{color['b']:02x}"
    return rgb if a >= 255 else f"{rgb}{a:02x}"


# ── Exporter ───────────────────────────────────────────────────────────────


class Exporter:
    def __init__(self, client: OpenRive, verbose: bool = False):
        self.client = client
        self.verbose = verbose
        self.symbols = {}
        self.skipped_bitmaps = 0
        self.artboards = 0

    def log(self, *a):
        if self.verbose:
            print(*a)

    def call(self, name: str, args: dict) -> dict:
        self.log(f"  -> {name} {json.dumps(args)[:120]}")
        return self.client.call(name, args)

    # ── geometry ──────────────────────────────────────────────────────────

    def emit_geometry(self, project, artboard, parent, elements, parent_mat, depth=0):
        """Emit symbol contents (shapes/text/nested instances) under `parent`."""
        for el in elements:
            kind = el["type"]
            if kind == "shape":
                self.emit_shape(project, artboard, parent, el)
            elif kind == "group":
                m = mat_mul(parent_mat, el["transform"])
                self.emit_geometry(
                    project, artboard, parent, el.get("members", []), m, depth + 1
                )
            elif kind == "instance":
                sym = self.symbols.get(el["symbol"])
                if sym is None:
                    continue
                m = mat_mul(parent_mat, el["transform"])
                self.emit_geometry(
                    project,
                    artboard,
                    parent,
                    self.first_frame_elements(sym),
                    m,
                    depth + 1,
                )
            elif kind == "text":
                self.emit_text(project, artboard, parent, el, parent_mat)
            elif kind == "bitmap":
                self.skipped_bitmaps += 1
            else:
                self.log(f"  (skip unknown element {kind})")

    def emit_shape(self, project, artboard, parent, el):
        m = el["transform"]
        for i, path in enumerate(el.get("paths", [])):
            pts = [apply(m, p) for p in path["points"]]
            if len(pts) < 2:
                continue
            args = {
                "project": project,
                "artboard": artboard,
                "name": f"path_{i}",
                "x": 0.0,
                "y": 0.0,
                "points": [{"x": round(p[0], 3), "y": round(p[1], 3)} for p in pts],
                "closed": True,
            }
            if parent:
                args["parent"] = parent
            fill = path.get("fill")
            stroke = path.get("stroke")
            if fill:
                args["fill"] = css(fill)
            elif stroke is None:
                args["noFill"] = True
            if stroke:
                args["stroke"] = css(stroke["color"])
                args["strokeWidth"] = stroke["width"]
            self.call("add_path", args)

    def emit_text(self, project, artboard, parent, el, parent_mat):
        m = mat_mul(parent_mat, el["transform"])
        args = {
            "project": project,
            "artboard": artboard,
            "text": el.get("text", ""),
            "x": round(m[4], 3),
            "y": round(m[5], 3),
            "fontSize": round(el.get("size", 16.0), 3),
            "color": css(el.get("color", {"r": 255, "g": 255, "b": 255, "a": 1.0})),
        }
        if parent:
            args["parent"] = parent
        if el.get("width"):
            args["width"] = el["width"]
        self.call("add_text", args)

    # ── timelines ─────────────────────────────────────────────────────────

    def export_symbol_artboard(self, project, doc, symbol):
        name = symbol["name"]
        ab = self.call(
            "add_artboard",
            {
                "project": project,
                "name": name,
                "width": doc["width"],
                "height": doc["height"],
            },
        )
        self.artboards += 1
        artboard = ab.get("id", name)
        frames = self.timeline_frames(symbol)

        timelines = symbol["timeline"]["layers"]
        has_motion = any(
            f.get("tween") == "Motion"
            for layer in timelines
            for f in layer["frames"]
        )
        timeline_name = None
        if has_motion and frames > 1:
            fps = doc["frame_rate"] or 60.0
            tl = self.call(
                "add_timeline",
                {
                    "project": project,
                    "artboard": artboard,
                    "name": "Timeline",
                    "fps": fps,
                    "duration": round(frames / fps, 4),
                    "loop": "oneShot",
                },
            )
            timeline_name = tl.get("name", "Timeline")

        for layer in timelines:
            self.export_layer(project, artboard, timeline_name, layer)

    def export_layer(self, project, artboard, timeline_name, layer):
        keyframes = layer["frames"]
        if not keyframes:
            return
        first = keyframes[0]
        elements = first.get("elements", [])
        # Track each object's keyframes by (element position in layer).
        for idx, el in enumerate(elements):
            if el["type"] == "instance":
                sym = self.symbols.get(el["symbol"])
                if sym is None:
                    continue
                trs = decompose(el["transform"])
                group = self.call(
                    "add_group",
                    {
                        "project": project,
                        "artboard": artboard,
                        "name": el["symbol"],
                        "x": round(trs["x"], 3),
                        "y": round(trs["y"], 3),
                    },
                )
                gid = group.get("id")
                props = {
                    "rotationDegrees": round(trs["rotationDegrees"], 3),
                    "scaleX": round(trs["scaleX"], 4),
                    "scaleY": round(trs["scaleY"], 4),
                    "opacity": el.get("alpha", 1.0),
                }
                self.call(
                    "set_properties",
                    {"project": project, "object": gid, "properties": props},
                )
                # Child geometry in the referenced symbol's own space.
                self.emit_geometry(
                    project, artboard, gid, self.first_frame_elements(sym), IDENTITY
                )
                if timeline_name:
                    self.key_layers(project, artboard, timeline_name, layer, idx, gid)
            else:
                self.emit_geometry(project, artboard, None, [el], IDENTITY)

    def key_layers(self, project, artboard, timeline_name, layer, idx, gid):
        """Key an instance group's TRS/opacity across the layer's keyframes."""
        for f in layer["frames"]:
            els = f.get("elements", [])
            if idx >= len(els) or els[idx].get("type") != "instance":
                continue
            el = els[idx]
            trs = decompose(el["transform"])
            frame = f["index"]
            for prop, value in (
                ("x", round(trs["x"], 3)),
                ("y", round(trs["y"], 3)),
                ("rotationDegrees", round(trs["rotationDegrees"], 3)),
                ("scaleX", round(trs["scaleX"], 4)),
                ("scaleY", round(trs["scaleY"], 4)),
                ("opacity", el.get("alpha", 1.0)),
            ):
                self.call(
                    "add_keyframes",
                    {
                        "project": project,
                        "artboard": artboard,
                        "timeline": timeline_name,
                        "object": gid,
                        "property": prop,
                        "keys": [{"frame": frame, "value": value, "ease": "easeInOut"}],
                    },
                )

    # ── helpers ───────────────────────────────────────────────────────────

    def first_frame_elements(self, symbol):
        for layer in symbol["timeline"]["layers"]:
            if layer["frames"]:
                return layer["frames"][0].get("elements", [])
        return []

    @staticmethod
    def timeline_frames(symbol) -> int:
        hi = 1
        for layer in symbol["timeline"]["layers"]:
            for f in layer["frames"]:
                hi = max(hi, f["index"] + f.get("duration", 1))
        return hi

    def export(self, doc, project_name, out_path):
        for s in doc["symbols"]:
            self.symbols[s["name"]] = s

        # Fresh project: drop any stale one with the same name.
        existing = self.client.call("list_projects")
        projects = existing if isinstance(existing, list) else existing.get("projects", [])
        for p in projects:
            if p.get("name") == project_name:
                self.client.call("delete_project", {"project": p.get("id", project_name)})

        created = self.call(
            "create_project", {"name": project_name, "template": "blank"}
        )
        project = created.get("id", project_name)
        editor = created.get("editor")
        print(f"project {project_name} -> {project}")
        if editor:
            print(f"editor  {editor}")

        roots = [s for s in doc["symbols"] if s["kind"].lower() == "movieclip"]
        for s in roots:
            self.export_symbol_artboard(project, doc, s)

        exported = self.call("export_riv", {"project": project})
        b64 = exported.get("dataBase64") if isinstance(exported, dict) else None
        if not b64:
            raise RuntimeError(f"export_riv returned no data: {exported}")
        raw = base64.b64decode(b64)
        out = Path(out_path)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_bytes(raw)
        print(
            f"wrote {out} ({len(raw)} bytes, {self.artboards} artboards, "
            f"{self.skipped_bitmaps} bitmaps skipped)"
        )
        return project


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--ir", required=True, help="IR JSON snapshot (animation_ir)")
    ap.add_argument("--name", required=True, help="Rive project name")
    ap.add_argument("--out", required=True, help="Output .riv path")
    ap.add_argument("--url", help="OpenRive MCP URL (else auto-discover)")
    ap.add_argument("-v", "--verbose", action="store_true")
    args = ap.parse_args(argv)

    doc = json.loads(Path(args.ir).read_text())
    client = OpenRive(args.url) if args.url else OpenRive()
    Exporter(client, verbose=args.verbose).export(doc, args.name, args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
