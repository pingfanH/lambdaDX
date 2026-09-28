#!/usr/bin/env python3
"""Generate the tap-hit effect as an Adobe Animate XFL project + .fla archive.

The effect matches the in-game procedural hit fx: a gold ring expands and fades,
a central flash shrinks, and 8 sparks fly outward. It is authored with classic
motion tweens (transform + alpha) so it stays editable in Animate.

Output:
  assets/effects/tap_hit/      XFL project directory
  assets/effects/tap_hit.fla   ZIP of the XFL (what Animate opens)

Run from the repo root:  python3 tools/make_tap_hit_xfl.py
"""

import math
import os
import shutil
import time
import zipfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT_DIR = os.path.join(REPO, "assets", "effects", "tap_hit")
OUT_FLA = os.path.join(REPO, "assets", "effects", "tap_hit.fla")
TEMPLATES = os.path.join(REPO, "assets", "ui")

LAST_MODIFIED = int(time.time())

# XFL shape coordinates are twips (1/20 px); matrices are in px.
TWIPS = 20
FPS = 24
FRAMES = 8  # 0.333 s

# Authored gold so it is visible/editable in Animate; the game recolors it
# per judge grade at runtime.
COLOR = "#FFD64C"
STAGE_W, STAGE_H = 256, 256

# Geometry in px.
RING_R, RING_W = 44, 6
FLASH_R = 30
SPARK_INNER, SPARK_OUTER, SPARK_HALF = 14, 30, 3
SPARKS = 8


def uid(n: int) -> str:
    return f"00000000-0000{n:04x}"


def png_circle(radius_px: float, segments: int = 32):
    pts = []
    for i in range(segments):
        a = 2 * math.pi * i / segments
        pts.append((round(radius_px * TWIPS * math.cos(a)),
                    round(radius_px * TWIPS * math.sin(a))))
    return pts


def edge_path(pts):
    """Closed polygon in Animate's edge-string grammar."""
    s = f"!{pts[0][0]} {pts[0][1]}S2|{pts[1][0]} {pts[1][1]}"
    for i in range(1, len(pts)):
        a, b = pts[i], pts[(i + 1) % len(pts)]
        s += f"!{a[0]} {a[1]}|{b[0]} {b[1]}"
    return s


def shape_symbol(name, item_id, edges, *, fill=None, stroke=None):
    if fill:
        paint = (
            f'              <fills><FillStyle index="1">'
            f'<SolidColor color="{fill}" alpha="1"/></FillStyle></fills>\n'
        )
        edge_attr = 'fillStyle1="1"'
    else:
        paint = (
            f'              <strokes><StrokeStyle index="1">'
            f'<SolidStroke weight="{stroke[1]}" scaleMode="normal">'
            f'<fill><SolidColor color="{stroke[0]}" alpha="1"/></fill>'
            f'</SolidStroke></StrokeStyle></strokes>\n'
        )
        edge_attr = 'strokeStyle="1"'
    return f'''<DOMSymbolItem xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/" name="{name}" itemID="{item_id}" symbolType="graphic" lastModified="{LAST_MODIFIED}" lastUniqueIdentifier="1">
  <timeline>
    <DOMTimeline name="{name}" layerDepthEnabled="true">
      <layers>
        <DOMLayer name="Layer_1" color="#00FFFF" current="true" isSelected="true">
          <frames>
            <DOMFrame index="0" duration="1" keyMode="9728">
              <elements>
                <DOMShape>
                  <matrix><Matrix/></matrix>
{paint}              <edges>
                <Edge {edge_attr} edges="{edges}"/>
              </edges>
                </DOMShape>
              </elements>
            </DOMFrame>
          </frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timeline>
</DOMSymbolItem>
'''


def instance(name, a, b, c, d, tx, ty, alpha):
    return f'''<DOMSymbolInstance libraryItemName="{name}" symbolType="graphic" loop="loop">
                  <matrix><Matrix a="{a:.6f}" b="{b:.6f}" c="{c:.6f}" d="{d:.6f}" tx="{tx:.6f}" ty="{ty:.6f}"/></matrix>
                  <transformationPoint><Point/></transformationPoint>
                  <color><Color alphaMultiplier="{alpha}"/></color>
                </DOMSymbolInstance>'''


def tween_layer(layer, symbol, start, end):
    """A classic motion tween: keyframe 0 (duration FRAMES) -> keyframe FRAMES."""
    return f'''        <DOMLayer name="{layer}" color="#9933CC">
          <frames>
            <DOMFrame index="0" duration="{FRAMES}" tweenType="motion" motionTweenSnap="true" keyMode="22017">
              <elements>
                {instance(symbol, *start)}
              </elements>
            </DOMFrame>
            <DOMFrame index="{FRAMES}" tweenType="motion" motionTweenSnap="true" keyMode="22017">
              <elements>
                {instance(symbol, *end)}
              </elements>
            </DOMFrame>
          </frames>
        </DOMLayer>'''


def scale(s, alpha):
    return (s, 0.0, 0.0, s, 0.0, 0.0, alpha)


def rot_scale(theta, s, alpha):
    cos, sin = math.cos(theta), math.sin(theta)
    return (s * cos, s * sin, -s * sin, s * cos, 0.0, 0.0, alpha)


def tap_hit_symbol(item_id):
    layers = [
        tween_layer("flash", "flash_shape", scale(1.0, 1), scale(0.30, 0)),
        tween_layer("ring", "ring_shape", scale(0.35, 1), scale(1.70, 0)),
    ]
    for i in range(SPARKS):
        theta = 2 * math.pi * i / SPARKS
        layers.append(
            tween_layer(
                f"spark{i}", "spark_shape",
                rot_scale(theta, 0.45, 1),
                rot_scale(theta, 1.70, 0),
            )
        )
    body = "\n".join(layers)
    return f'''<DOMSymbolItem xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/" name="tap_hit" itemID="{item_id}" symbolType="movieclip" lastModified="{LAST_MODIFIED}" lastUniqueIdentifier="{FRAMES + 1}">
  <timeline>
    <DOMTimeline name="tap_hit" layerDepthEnabled="true">
      <layers>
{body}
      </layers>
    </DOMTimeline>
  </timeline>
</DOMSymbolItem>
'''


def dom_document(includes):
    # NB: `<Include href>` is resolved relative to the LIBRARY folder, so the
    # href is just the file name (the files live in LIBRARY/).
    inc = "\n".join(
        f'          <Include href="{href}" loadImmediate="true" itemID="{uid(i)}" lastModified="{LAST_MODIFIED}"/>'
        for i, href in enumerate(includes)
    )
    return f'''<DOMDocument xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/" width="{STAGE_W}" height="{STAGE_H}" frameRate="{FPS}" currentTimeline="1" xflVersion="23.0" creatorInfo="Adobe Animate" platform="Macintosh" versionInfo="Saved by Animate Macintosh 24.0 build 305" majorVersion="24" buildNumber="305" nextSceneIdentifier="2" playOptionsPlayLoop="false" playOptionsPlayPages="false" playOptionsPlayFrameActions="false">
     <folders/>
     <media/>
     <symbols>
{inc}
     </symbols>
     <timelines>
          <DOMTimeline name="Scene 1" layerDepthEnabled="true">
               <layers>
                    <DOMLayer name="Layer_1" color="#00FFFF" current="true" isSelected="true">
                         <frames>
                              <DOMFrame index="0" keyMode="9728">
                                   <elements>
                                        <DOMSymbolInstance libraryItemName="tap_hit" symbolType="movieclip" loop="loop">
                                             <matrix><Matrix tx="{STAGE_W // 2}" ty="{STAGE_H // 2}"/></matrix>
                                             <transformationPoint><Point/></transformationPoint>
                                        </DOMSymbolInstance>
                                   </elements>
                              </DOMFrame>
                         </frames>
                    </DOMLayer>
               </layers>
          </DOMTimeline>
     </timelines>
     <scripts>
          <GlobalScripts language="Javascript"/>
     </scripts>
     <PrinterSettings platform="macintosh"/>
     <publishHistory/>
</DOMDocument>
'''


def main_xfl(library_files):
    """`main.xfl` manifest: Animate uses it to enumerate the library reliably."""
    entries = ['        <DOMFile path="DOMDocument.xml" type="application/vnd.adobe.xfl.document"/>']
    for rel in library_files:
        entries.append(f'        <DOMFile path="{rel}"/>')
    body = "\n".join(entries)
    return f'''<DOMFlashFile xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/" version="2">
     <files>
{body}
     </files>
</DOMFlashFile>
'''


def build_files():
    circle = png_circle(RING_R)
    flash = png_circle(FLASH_R)
    spark = [(SPARK_INNER * TWIPS, -SPARK_HALF * TWIPS),
             (SPARK_INNER * TWIPS, SPARK_HALF * TWIPS),
             (SPARK_OUTER * TWIPS, 0)]

    library = {
        "LIBRARY/tap_hit.xml": tap_hit_symbol(uid(1)),
        "LIBRARY/ring_shape.xml": shape_symbol("ring_shape", uid(2), edge_path(circle), stroke=(COLOR, RING_W)),
        "LIBRARY/flash_shape.xml": shape_symbol("flash_shape", uid(3), edge_path(flash), fill=COLOR),
        "LIBRARY/spark_shape.xml": shape_symbol("spark_shape", uid(4), edge_path(spark), fill=COLOR),
    }
    files = {
        # Include hrefs are relative to the LIBRARY folder (basenames).
        "DOMDocument.xml": dom_document([
            "tap_hit.xml", "ring_shape.xml", "flash_shape.xml", "spark_shape.xml",
        ]),
        **library,
        "main.xfl": main_xfl(sorted(library.keys())),
    }
    # Copy Animate's scaffolding so the .fla opens cleanly.
    for rel in ("PublishSettings.xml", "META-INF/metadata.xml", "MobileSettings.xml"):
        src = os.path.join(TEMPLATES, rel)
        if os.path.exists(src):
            with open(src, "rb") as f:
                files[rel] = f.read()
    return files


def main():
    files = build_files()
    if os.path.isdir(OUT_DIR):
        shutil.rmtree(OUT_DIR)
    for rel, data in files.items():
        path = os.path.join(OUT_DIR, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        mode = "wb" if isinstance(data, bytes) else "w"
        with open(path, mode) as f:
            f.write(data)

    # .fla = ZIP with `mimetype` first (stored) then the XFL entries.
    with zipfile.ZipFile(OUT_FLA, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("mimetype", b"application/vnd.adobe.xfl", compress_type=zipfile.ZIP_STORED)
        for rel, data in files.items():
            z.writestr(rel, data)
    print(f"wrote {OUT_DIR}")
    print(f"wrote {OUT_FLA}")


if __name__ == "__main__":
    main()
