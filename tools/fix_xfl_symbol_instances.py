#!/usr/bin/env python3
"""Fix Animate XFL `<DOMSymbolInstance>` tags produced by converters.

Animate always writes `symbolType` (and `loop`/`firstFrame`) on a symbol
instance. Tools often omit them; Animate then treats the instance as a
movieclip, whose authoring-stage preview only shows its **first frame**, so a
frame-by-frame effect nested in a wrapper looks like it never animates.

This walks `LIBRARY/*.xml`, learns each symbol's type, and annotates every
instance missing `symbolType`:

    <DOMSymbolInstance libraryItemName="tapEffect">
  -> <DOMSymbolInstance libraryItemName="tapEffect" symbolType="graphic" loop="loop" firstFrame="0">

Usage: python3 tools/fix_xfl_symbol_instances.py [xfl_dir]   (default assets/ui)
"""

import glob
import os
import re
import sys

XFL = sys.argv[1] if len(sys.argv) > 1 else "assets/ui"
LIB = os.path.join(XFL, "LIBRARY")


def symbol_types():
    """symbol name -> symbolType (Animate defaults to movieclip)."""
    types = {}
    for path in glob.glob(os.path.join(LIB, "*.xml")):
        head = open(path, encoding="utf-8").read(4000)
        m = re.search(r"<DOMSymbolItem\b[^>]*\bname=\"([^\"]+)\"[^>]*>", head)
        if not m:
            continue
        t = re.search(r'symbolType="([^"]+)"', m.group(0))
        types[m.group(1)] = t.group(1) if t else "movieclip"
    return types


def annotate(tag, types):
    if "symbolType=" in tag:
        return tag
    name = re.search(r'libraryItemName="([^"]+)"', tag)
    if not name:
        return tag
    kind = types.get(name.group(1), "movieclip")
    extra = f' symbolType="{kind}"'
    if "loop=" not in tag:
        extra += ' loop="loop"'
    if kind == "graphic" and "firstFrame=" not in tag:
        extra += ' firstFrame="0"'
    return tag[:-1].rstrip() + extra + ">"


def main():
    types = symbol_types()
    total = fixed = 0
    for path in sorted(glob.glob(os.path.join(LIB, "*.xml"))):
        src = open(path, encoding="utf-8").read()

        def repl(m):
            nonlocal total, fixed
            total += 1
            out = annotate(m.group(0), types)
            if out != m.group(0):
                fixed += 1
            return out

        out = re.sub(r"<DOMSymbolInstance\b[^>]*>", repl, src)
        if out != src:
            open(path, "w", encoding="utf-8").write(out)
            print("patched", path)
    print(f"{fixed}/{total} instances annotated")


if __name__ == "__main__":
    main()
