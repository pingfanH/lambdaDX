#!/usr/bin/env python3
"""Package extracted maimai replay textures as an editable Adobe Animate project."""

import html
import math
import os
import sys
import time
import zipfile
import xml.etree.ElementTree as ET

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SOURCE_DIR = os.path.join(REPO, "assets", "extracted", "lambdadx_replay_1600_master")
OUT_DIR = os.path.join(REPO, "assets", "effects", "lambdadx_replay_1600_master")
OUT_FLA = os.path.join(REPO, "assets", "effects", "lambdadx_replay_1600_master.fla")
TEMPLATES = os.path.join(REPO, "assets", "ui")

LAST_MODIFIED = int(time.time())
STAGE_W, STAGE_H = 1920, 1080
XFL_NS = "http://ns.adobe.com/xfl/2008/"
ET.register_namespace("", XFL_NS)
ET.register_namespace("xsi", "http://www.w3.org/2001/XMLSchema-instance")
_next_id = 0x1600


def uid():
    global _next_id
    _next_id += 1
    return "6ac16000-%08x" % _next_id


def attr(value):
    return html.escape(str(value), quote=True)


def png_size(path):
    with open(path, "rb") as f:
        header = f.read(24)
    if header[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("Not a PNG: " + path)
    return int.from_bytes(header[16:20], "big"), int.from_bytes(header[20:24], "big")


def classify(relpath):
    parts = relpath.split("/")
    if parts[0] == "judgement" and len(parts) == 3:
        return "Judgement", "Judgement/" + parts[1]
    if parts[:2] == ["effects", "sprite_effects"] and len(parts) == 4:
        return "Break Effects", "BreakEffects/" + parts[2]
    if parts[:2] == ["effects", "particle_textures"] and len(parts) == 3:
        return "Particle Textures", "ParticleTextures"
    return None


def collect_assets():
    assets = []
    for root, _, filenames in os.walk(SOURCE_DIR):
        for filename in sorted(filenames):
            if not filename.lower().endswith(".png"):
                continue
            source_path = os.path.join(root, filename)
            relpath = os.path.relpath(source_path, SOURCE_DIR).replace(os.sep, "/")
            classification = classify(relpath)
            if classification is None:
                continue
            group, folder = classification
            width, height = png_size(source_path)
            stem = filename[:-4]
            assets.append({
                "source": source_path,
                "source_rel": relpath,
                "filename": filename,
                "stem": stem,
                "group": group,
                "folder": folder,
                # Keep raw bitmaps in LIBRARY's root, matching assets/notes.
                # The wrapper symbols carry the category folders.
                "bitmap": filename,
                "shape": folder + "/" + stem + "_shape",
                "clip": folder + "/" + stem,
                "width": width,
                "height": height,
            })
    order = {"Judgement": 0, "Break Effects": 1, "Particle Textures": 2}
    assets.sort(key=lambda item: (order[item["group"]], item["source_rel"].lower()))
    if not assets:
        raise RuntimeError("No extracted PNG assets found under " + SOURCE_DIR)
    return assets


def bitmap_shape(asset):
    name = attr(asset["shape"])
    bitmap = attr(asset["bitmap"])
    return f'''<DOMSymbolItem xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="{XFL_NS}" name="{name}" itemID="{uid()}" symbolType="graphic" lastModified="{LAST_MODIFIED}" lastUniqueIdentifier="1">
  <timeline>
    <DOMTimeline name="{attr(asset['stem'])}_shape" layerDepthEnabled="true">
      <layers>
        <DOMLayer name="bitmap" color="#4FFF4F" current="true" isSelected="true" autoNamed="false">
          <frames>
            <DOMFrame index="0" keyMode="9728">
              <elements>
                <DOMBitmapInstance libraryItemName="{bitmap}">
                  <matrix><Matrix tx="{-asset['width'] / 2:g}" ty="{-asset['height'] / 2:g}"/></matrix>
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


def bitmap_clip(asset):
    return f'''<DOMSymbolItem xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="{XFL_NS}" name="{attr(asset['clip'])}" itemID="{uid()}" symbolType="movieclip" lastModified="{LAST_MODIFIED}" lastUniqueIdentifier="1">
  <timeline>
    <DOMTimeline name="{attr(asset['stem'])}" layerDepthEnabled="true">
      <layers>
        <DOMLayer name="shape" color="#9933CC" current="true" isSelected="true">
          <frames>
            <DOMFrame index="0" keyMode="9728">
              <elements>
                <DOMSymbolInstance libraryItemName="{attr(asset['shape'])}" symbolType="graphic" loop="single frame">
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


def layout_instances(assets, cols):
    rows = math.ceil(len(assets) / cols)
    margin_x, margin_y = 64.0, 72.0
    usable_w = STAGE_W - margin_x * 2
    usable_h = STAGE_H - margin_y * 2
    cell_w, cell_h = usable_w / cols, usable_h / rows
    max_w, max_h = cell_w * 0.86, cell_h * 0.78
    instances = []
    for index, asset in enumerate(assets):
        row, col = divmod(index, cols)
        scale = min(2.0, max_w / asset["width"], max_h / asset["height"])
        tx = margin_x + (col + 0.5) * cell_w
        ty = margin_y + (row + 0.5) * cell_h
        instances.append(f'''                <DOMSymbolInstance libraryItemName="{attr(asset['clip'])}" symbolType="movieclip" loop="single frame">
                  <matrix><Matrix a="{scale:.6f}" d="{scale:.6f}" tx="{tx:.3f}" ty="{ty:.3f}"/></matrix>
                  <transformationPoint><Point/></transformationPoint>
                </DOMSymbolInstance>''')
    return "\n".join(instances)


def dom_document(assets, folders):
    media = []
    includes = []
    for asset in assets:
        media.append(
            f'''          <DOMBitmapItem name="{attr(asset['bitmap'])}" itemID="{uid()}" href="{attr(asset['bitmap'])}" sourceExternalFilepath="../../extracted/lambdadx_replay_1600_master/{attr(asset['source_rel'])}" sourceLastImported="{LAST_MODIFIED}" useImportedJPEGData="false" compressionType="lossless" originalCompressionType="lossless" quality="50" frameRight="{asset['width'] * 20}" frameBottom="{asset['height'] * 20}"/>'''
        )
        includes.extend([
            f'''          <Include href="{attr(asset['shape'])}.xml" loadImmediate="false" itemID="{uid()}" lastModified="{LAST_MODIFIED}"/>''',
            f'''          <Include href="{attr(asset['clip'])}.xml" loadImmediate="false" itemID="{uid()}" lastModified="{LAST_MODIFIED}"/>''',
        ])

    folder_xml = "\n".join(
        f'''          <DOMFolderItem name="{attr(folder)}" itemID="{uid()}"/>'''
        for folder in folders
    )
    frames = []
    groups = [
        ("Judgement", [a for a in assets if a["group"] == "Judgement"], 4),
        ("Break_Effects", [a for a in assets if a["group"] == "Break Effects"], 3),
        ("Particle_Textures", [a for a in assets if a["group"] == "Particle Textures"], 5),
    ]
    for index, (label, group_assets, cols) in enumerate(groups):
        if not group_assets:
            continue
        frames.append(f'''            <DOMFrame index="{index}" name="{label}" labelType="name" keyMode="9728">
              <elements>
{layout_instances(group_assets, cols)}
              </elements>
            </DOMFrame>''')

    return f'''<DOMDocument xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="{XFL_NS}" width="{STAGE_W}" height="{STAGE_H}" frameRate="24" currentTimeline="1" xflVersion="23.0" creatorInfo="Adobe Animate" platform="Macintosh" versionInfo="Saved by Animate Macintosh 24.0 build 305" majorVersion="24" buildNumber="305" nextSceneIdentifier="2" playOptionsPlayLoop="false" playOptionsPlayPages="false" playOptionsPlayFrameActions="false">
     <folders>
{folder_xml}
     </folders>
     <media>
{chr(10).join(media)}
     </media>
     <symbols>
{chr(10).join(includes)}
     </symbols>
     <timelines>
          <DOMTimeline name="Scene 1" layerDepthEnabled="true">
               <layers>
                    <DOMLayer name="Asset Preview" color="#00FFFF" current="true" isSelected="true">
                         <frames>
{chr(10).join(frames)}
                         </frames>
                    </DOMLayer>
               </layers>
          </DOMTimeline>
     </timelines>
     <scripts><GlobalScripts language="Javascript"/></scripts>
     <PrinterSettings platform="macintosh"/>
     <publishHistory/>
</DOMDocument>
'''


def main_xfl(library_files):
    entries = ['        <DOMFile path="DOMDocument.xml" type="application/vnd.adobe.xfl.document"/>']
    entries.extend(f'        <DOMFile path="{attr(path)}"/>' for path in sorted(library_files))
    return f'''<DOMFlashFile xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="{XFL_NS}" version="2">
     <files>
{chr(10).join(entries)}
     </files>
</DOMFlashFile>
'''


def build_files():
    assets = collect_assets()
    folder_set = set()
    for asset in assets:
        parts = asset["folder"].split("/")
        for depth in range(1, len(parts) + 1):
            folder_set.add("/".join(parts[:depth]))
    folders = sorted(folder_set, key=lambda value: (value.count("/"), value.lower()))

    files = {}
    library_files = []
    for asset in assets:
        with open(asset["source"], "rb") as f:
            files["LIBRARY/" + asset["bitmap"]] = f.read()
        shape_path = "LIBRARY/" + asset["shape"] + ".xml"
        clip_path = "LIBRARY/" + asset["clip"] + ".xml"
        files[shape_path] = bitmap_shape(asset)
        files[clip_path] = bitmap_clip(asset)
        library_files.extend(["LIBRARY/" + asset["bitmap"], shape_path, clip_path])

    files["DOMDocument.xml"] = dom_document(assets, folders)
    files["main.xfl"] = main_xfl(library_files)
    files["lambdadx_replay_1600_master.xfl"] = "PROXY-CS5"
    for rel in ("PublishSettings.xml", "META-INF/metadata.xml", "MobileSettings.xml"):
        source = os.path.join(TEMPLATES, rel)
        if os.path.exists(source):
            with open(source, "rb") as f:
                files[rel] = f.read()
    return assets, files


def validate(files, fla_path):
    document = ET.fromstring(files["DOMDocument.xml"])
    ns = {"xfl": XFL_NS}
    for item in document.findall("xfl:media/xfl:DOMBitmapItem", ns):
        href = item.attrib["href"]
        if "LIBRARY/" + href not in files:
            raise RuntimeError("Missing bitmap referenced by XFL: " + href)
    for include in document.findall("xfl:symbols/xfl:Include", ns):
        href = include.attrib["href"]
        if "LIBRARY/" + href not in files:
            raise RuntimeError("Missing symbol referenced by XFL: " + href)
    manifest = ET.fromstring(files["main.xfl"])
    for entry in manifest.findall(".//xfl:DOMFile", ns):
        path = entry.attrib["path"]
        if path not in files:
            raise RuntimeError("Missing file listed by main.xfl: " + path)
    with zipfile.ZipFile(fla_path) as archive:
        if archive.testzip() is not None:
            raise RuntimeError("FLA archive failed CRC validation")
        if archive.namelist()[0] != "mimetype":
            raise RuntimeError("FLA archive must begin with mimetype")


def main():
    if "--judgments-only" in sys.argv:
        refresh_judgments()
        return
    assets, files = build_files()
    os.makedirs(OUT_DIR, exist_ok=True)
    for rel, data in files.items():
        path = os.path.join(OUT_DIR, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        mode = "wb" if isinstance(data, bytes) else "w"
        with open(path, mode) as f:
            f.write(data)

    with zipfile.ZipFile(OUT_FLA, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        archive.writestr("mimetype", b"application/vnd.adobe.xfl", compress_type=zipfile.ZIP_STORED)
        for rel in sorted(files):
            archive.writestr(rel, files[rel])

    validate(files, OUT_FLA)
    counts = {}
    for asset in assets:
        counts[asset["group"]] = counts.get(asset["group"], 0) + 1
    print("wrote " + OUT_DIR)
    print("wrote " + OUT_FLA)
    print("included %d PNG assets: %s" % (len(assets), ", ".join(
        "%s %d" % (group, count) for group, count in counts.items()
    )))
    print("validated XFL references and FLA ZIP integrity")


def refresh_judgments():
    """Add grade symbols without rewriting the existing authored symbols."""
    global _next_id
    _next_id = 0x8000
    with zipfile.ZipFile(OUT_FLA) as archive:
        files = {name: archive.read(name) for name in archive.namelist() if name != "mimetype"}
    document = ET.fromstring(files["DOMDocument.xml"])
    manifest = ET.fromstring(files["main.xfl"])
    q = lambda name: "{" + XFL_NS + "}" + name
    media = document.find(q("media"))
    symbols = document.find(q("symbols"))
    listed = manifest.find(q("files"))
    changed = {}
    count = 0
    for asset in collect_assets():
        if asset["group"] != "Judgement":
            continue
        clip = "LIBRARY/" + asset["clip"] + ".xml"
        if clip in files:
            continue
        image = "LIBRARY/" + asset["bitmap"]
        shape = "LIBRARY/" + asset["shape"] + ".xml"
        changed[image] = open(asset["source"], "rb").read()
        changed[shape] = bitmap_shape(asset).encode()
        changed[clip] = bitmap_clip(asset).encode()
        ET.SubElement(media, q("DOMBitmapItem"), {
            "name": asset["bitmap"], "itemID": uid(), "href": asset["bitmap"],
            "useImportedJPEGData": "false", "compressionType": "lossless",
            "frameRight": str(asset["width"] * 20), "frameBottom": str(asset["height"] * 20),
        })
        for path in [shape, clip]:
            ET.SubElement(symbols, q("Include"), {"href": path.removeprefix("LIBRARY/"), "loadImmediate": "false", "itemID": uid()})
        for path in [image, shape, clip]:
            ET.SubElement(listed, q("DOMFile"), {"path": path})
        count += 1
    if not count:
        print("Judgement symbols already up to date")
        return
    changed["DOMDocument.xml"] = ET.tostring(document, encoding="utf-8")
    changed["main.xfl"] = ET.tostring(manifest, encoding="utf-8")
    for path, data in changed.items():
        destination = os.path.join(OUT_DIR, path)
        os.makedirs(os.path.dirname(destination), exist_ok=True)
        with open(destination, "wb") as out:
            out.write(data)
    files.update(changed)
    with zipfile.ZipFile(OUT_FLA, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        archive.writestr("mimetype", b"application/vnd.adobe.xfl", compress_type=zipfile.ZIP_STORED)
        for path in sorted(files):
            archive.writestr(path, files[path])
    validate(files, OUT_FLA)
    print(f"Added {count} Judgement symbols; existing authored symbols preserved")


if __name__ == "__main__":
    main()
