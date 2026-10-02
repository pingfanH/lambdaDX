#!/usr/bin/env python3
"""Generate the LambdaDX note textures as an Adobe Animate XFL project.

For every note image under `assets/Skins/classic/` whose name starts with one
of tap / hold / slide / star / touch, this builds **two** library items:

  * `<name>_shape` — a **graphic** symbol (a "shape") wrapping the bitmap, and
  * `<name>`        — a **movie clip** holding an instance of that shape.

So each image is first wrapped in a shape, then placed inside its movie clip,
exactly as required for further editing in Animate.

Output: assets/notes/   (Animate opens this)

Run from the repo root:  python3 tools/make_notes_xfl.py
"""

import os
import shutil
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SKIN_DIR = os.path.join(REPO, "assets", "Skins", "classic")
OUT_DIR = os.path.join(REPO, "assets", "notes")
TEMPLATES = os.path.join(REPO, "assets", "ui")

PREFIXES = ("tap", "hold", "slide", "star", "touch")
CATEGORY_ORDER = {p: i for i, p in enumerate(PREFIXES)}
LAST = int(time.time())

_id = [0x2000]


def uid():
    _id[0] += 1
    return "6ac00000-%08x" % _id[0]


def png_size(path):
    with open(path, "rb") as f:
        head = f.read(24)
    if head[:8] != b"\x89PNG\r\n\x1a\n":
        return (0, 0)
    return int.from_bytes(head[16:20], "big"), int.from_bytes(head[20:24], "big")


def category(stem):
    s = stem.lower()
    for p in PREFIXES:
        if s.startswith(p):
            return p
    return None


def shape_symbol(name, bitmap, w, h):
    tx, ty = -w / 2.0, -h / 2.0
    return f'''<DOMSymbolItem xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/" name="notes/{name}" itemID="{uid()}" symbolType="graphic" lastModified="{LAST}">
  <timeline>
    <DOMTimeline name="{name}" layerDepthEnabled="true">
      <layers>
        <DOMLayer name="bitmap" color="#4FFF4F" current="true" isSelected="true" autoNamed="false">
          <frames>
            <DOMFrame index="0" keyMode="9728">
              <elements>
                <DOMBitmapInstance libraryItemName="{bitmap}">
                  <matrix><Matrix tx="{tx:g}" ty="{ty:g}"/></matrix>
                  <transformationPoint><Point/></transformationPoint>
                </DOMBitmapInstance>
              </elements>
            </DOMFrame>
          </frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timeline>
</DOMSymbolItem>
'''


def clip_symbol(name, shape):
    return f'''<DOMSymbolItem xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/" name="notes/{name}" itemID="{uid()}" symbolType="movieclip" lastModified="{LAST}">
  <timeline>
    <DOMTimeline name="{name}" layerDepthEnabled="true">
      <layers>
        <DOMLayer name="shape" color="#9933CC" current="true" isSelected="true" autoNamed="false">
          <frames>
            <DOMFrame index="0" keyMode="9728">
              <elements>
                <DOMSymbolInstance libraryItemName="notes/{shape}" symbolType="graphic" loop="single frame">
                  <matrix><Matrix/></matrix>
                  <transformationPoint><Point/></transformationPoint>
                </DOMSymbolInstance>
              </elements>
            </DOMFrame>
          </frames>
        </DOMLayer>
      </layers>
    </DOMTimeline>
  </timeline>
</DOMSymbolItem>
'''


def dom_document(items, layout):
    media = "\n".join(
        f'    <DOMBitmapItem name="{bmp}" itemID="{uid()}" href="{bmp}" '
        f'sourceExternalFilepath="../Skins/classic/{bmp}" sourceLastImported="{LAST}" '
        f'useImportedJPEGData="false" compressionType="lossless" '
        f'originalCompressionType="lossless" quality="50" '
        f'frameRight="{w * 20}" frameBottom="{h * 20}"/>'
        for bmp, w, h in items
    )
    syms = "\n".join(
        f'    <Include href="notes/{n}.xml" loadImmediate="false" itemID="{uid()}" lastModified="{LAST}"/>'
        for n in layout_names(items, layout)
    )
    clip_layers = "\n".join(
        f'''     <DOMLayer name="{stem}" color="#9933CC">
      <frames>
       <DOMFrame index="0" keyMode="9728">
        <elements>
         <DOMSymbolInstance libraryItemName="notes/{stem}" symbolType="movieclip" loop="loop">
          <matrix><Matrix tx="{x:g}" ty="{y:g}"/></matrix>
          <transformationPoint><Point/></transformationPoint>
         </DOMSymbolInstance>
        </elements>
       </DOMFrame>
      </frames>
     </DOMLayer>'''
        for stem, x, y in layout
    )
    return f'''<DOMDocument xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/" width="1920" height="1080" frameRate="60" currentTimeline="1" xflVersion="23.0" creatorInfo="Adobe Animate" platform="Macintosh" versionInfo="Saved by Animate Macintosh 24.0 build 305" majorVersion="24" buildNumber="305" nextSceneIdentifier="2" playOptionsPlayLoop="false" playOptionsPlayPages="false" playOptionsPlayFrameActions="false">
     <media>
{media}
     </media>
     <symbols>
{syms}
     </symbols>
     <timelines>
      <DOMTimeline name="Scene 1" layerDepthEnabled="true">
       <layers>
{clip_layers}
       </layers>
      </DOMTimeline>
     </timelines>
     <scripts/>
     <PrinterSettings platform="macintosh"/>
     <publishHistory/>
</DOMDocument>
'''


def layout_names(items, layout):
    stems = [s for s, _, _ in layout]
    out = []
    for s in stems:
        out.append(f"{s}_shape")
        out.append(s)
    return out


def main_xfl(files):
    entries = ['        <DOMFile path="DOMDocument.xml" type="application/vnd.adobe.xfl.document"/>']
    for rel in files:
        entries.append(f'        <DOMFile path="{rel}"/>')
    body = "\n".join(entries)
    return (
        '<DOMFlashFile xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" '
        'xmlns="http://ns.adobe.com/xfl/2008/" version="2">\n     <files>\n'
        f"{body}\n     </files>\n</DOMFlashFile>\n"
    )


def main():
    files = {}
    items = []   # (bitmap file, w, h)
    layout = []  # (stem, x, y)
    library = {}  # relative path -> xml

    entries = sorted(os.listdir(SKIN_DIR))
    notes = []
    for fn in entries:
        if not fn.lower().endswith(".png"):
            continue
        stem = fn[:-4]
        cat = category(stem)
        if cat is None:
            continue
        notes.append((cat, stem, fn))

    # Stable, grouped order: tap, hold, slide, star, touch; then name.
    notes.sort(key=lambda t: (CATEGORY_ORDER[t[0]], t[1].lower()))

    x, y, row_h = 120.0, 200.0, 0.0
    for cat, stem, fn in notes:
        src = os.path.join(SKIN_DIR, fn)
        w, h = png_size(src)
        with open(src, "rb") as f:
            png = f.read()
        library[f"LIBRARY/{fn}"] = png  # stored under LIBRARY to match href
        library[f"LIBRARY/notes/{stem}_shape.xml"] = shape_symbol(f"{stem}_shape", fn, w, h)
        library[f"LIBRARY/notes/{stem}.xml"] = clip_symbol(stem, f"{stem}_shape")
        items.append((fn, w, h))
        layout.append((stem, x, y))
        x += max(w, 96) + 60.0
        row_h = max(row_h, h)
        if x > 1700.0:
            x = 120.0
            y += row_h + 120.0
            row_h = 0.0

    files["DOMDocument.xml"] = dom_document(items, layout)
    files.update(library)
    files["notes.xfl"] = "PROXY-CS5"
    for rel in ("PublishSettings.xml", "META-INF/metadata.xml", "MobileSettings.xml"):
        src = os.path.join(TEMPLATES, rel)
        if os.path.exists(src):
            with open(src, "rb") as f:
                files[rel] = f.read()
    files["main.xfl"] = main_xfl(sorted(k for k in files if k.startswith("LIBRARY/")))

    shutil.rmtree(OUT_DIR, ignore_errors=True)
    for rel, data in files.items():
        path = os.path.join(OUT_DIR, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        mode = "wb" if isinstance(data, bytes) else "w"
        with open(path, mode) as f:
            f.write(data)
    print(f"wrote {OUT_DIR}: {len(notes)} notes, {len(notes) * 2} symbols")


if __name__ == "__main__":
    main()
