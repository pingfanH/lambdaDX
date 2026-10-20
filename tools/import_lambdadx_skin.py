#!/usr/bin/env python3
"""Import design-00 note art and adapt its registration to LDX's skin slots."""

import json
import subprocess
from pathlib import Path

from PIL import Image, ImageOps

ROOT = Path(__file__).resolve().parents[1] / "assets/Skins/lambdadx"
BASE = "https://maiwiki.moe/gameresources/sprites"
SLOTS = {
    "tap": "tap/Tap_00", "tap_each": "tap/Tap_Each_00",
    "tap_break": "break/Break_00", "tap_ex": "tap/Tap_Ex_00",
    "tap_break_ex": "break/Break_Ex_00",
    "hold": "hold/Hold_00", "hold_each": "hold/Hold_Each_00",
    "hold_break": "breakhold/BreakHold_00", "hold_ex": "hold/Hold_Ex_00",
    "hold_break_ex": "breakhold/BreakHold_Ex_00",
    "Hold_End": "hold/Hold_End", "Hold_Each_End": "hold/Hold_Each_End",
    "Hold_Break_End": "breakhold/BreakHold_End",
    "star": "star/Star_00", "star_each": "star/Star_Each_00",
    "star_break": "breakstar/BreakStar_00", "star_ex": "star/Star_Ex_00",
    "star_break_ex": "breakstar/BreakStar_Ex_00",
    "star_double": "star/Star_Double_00", "star_double_each": "star/Star_Each_Double_00",
    "star_double_break": "breakstar/BreakStar_Double_00",
    "star_double_ex": "star/Star_Ex_Double_00",
    "star_double_break_ex": "breakstar/BreakStar_Ex_Double_00",
    "slide": "slide/Slide_00", "slide_each": "slide/Slide_Each_00",
    "slide_break": "breakslide/BreakSlide_00",
    "touch": "touchtap/UI_NOTES_Touch_01", "touch_each": "touchtap/UI_NOTES_Touch_Each_01",
    "touch_point": "touchtap/UI_NOTES_Touch_Point", "touch_point_each": "touchtap/UI_NOTES_Touch_Each_Point",
    "touchhold_border": "touchhold/UI_NOTES_Touch_Hold_hold",
    "tap_guide": "guide/Normal", "Each": "guide/Each",
    "Break": "guide/Break", "slide_guide": "guide/Slide",
    **{f"touchhold_{i}": f"touchhold/UI_NOTES_Touch_Hold_{i+1:02}" for i in range(4)},
    **{f"wifi_{i}": f"slidefan/MM_UI_Game_{53+i:03}" for i in range(11)},
}


def download(url, destination):
    if not destination.exists():
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(fetch(url))


def fetch(url):
    return subprocess.check_output(["curl", "-fsSL", "--retry", "3",
                                    "--max-time", "60", "-A", "Mozilla/5.0", url])


def main():
    ROOT.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(fetch(BASE + "/manifest.json"))
    for slot, key in SLOTS.items():
        source = ROOT / "source" / (key + ".png")
        download(BASE + "/" + key + ".png", source)
        art = Image.open(source).convert("RGBA")
        meta = manifest[key]
        if slot.startswith("tap") and slot != "tap_guide":
            art = centered(art, meta, 128)
        elif slot.startswith("star"):
            art = centered(art, meta, 132)
        elif slot.startswith("wifi_"):
            # Source fan strips are one half; the replay mirrors them at the seam.
            full = Image.new("RGBA", (art.width * 2, art.height))
            full.alpha_composite(art, (0, 0))
            full.alpha_composite(ImageOps.mirror(art), (art.width, 0))
            art = full
        art.save(ROOT / (slot + ".png"))
    (ROOT / "skin.json").write_text(json.dumps({
        "id": "lambdadx", "name": "LambdaDX", "design": "00",
        "source": BASE, "slots": {slot: {"source": key, "metadata": manifest[key]} for slot, key in SLOTS.items()},
    }, ensure_ascii=True, indent=2) + "\n")
    print(f"Imported {len(SLOTS)} LambdaDX note slots")


def centered(art, meta, size):
    x, y, w, h = meta["rect"]
    px, py = meta["pivot"]
    origin = (x + px * w, art.height - y - py * h)
    # Transparent margins belong to source registration, not the visible note size.
    left, top = round(origin[0] - size / 2), round(origin[1] - size / 2)
    return art.crop((left, top, left + size, top + size))


if __name__ == "__main__":
    main()
