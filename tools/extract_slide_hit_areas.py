#!/usr/bin/env python3
"""Extract SDEZ170 SlideManager HitArea tables into JSON."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


AREA_NAMES = [
    "A1", "A2", "A3", "A4", "A5", "A6", "A7", "A8",
    "B1", "B2", "B3", "B4", "B5", "B6", "B7", "B8",
    "C1", "C2",
    "D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8",
    "E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8",
    "Blank",
]
AREA_INDEX = {name: index for index, name in enumerate(AREA_NAMES)}


def matching(text: str, start: int, opening: str, closing: str) -> int:
    depth = 0
    for index in range(start, len(text)):
        char = text[index]
        if char == opening:
            depth += 1
        elif char == closing:
            depth -= 1
            if depth == 0:
                return index
    raise ValueError(f"unbalanced {opening}{closing}")


def extract_initializer(text: str, name: str) -> str:
    match = re.search(
        rf"\b{re.escape(name)}\s*=\s*new\s+List<List<HitArea>>",
        text,
    )
    if not match:
        raise ValueError(f"missing {name}")
    opening = text.find("{", match.end())
    if opening < 0:
        raise ValueError(f"missing initializer for {name}")
    closing = matching(text, opening, "{", "}")
    return text[opening + 1 : closing]


def split_top_level(text: str) -> list[str]:
    items: list[str] = []
    start = 0
    braces = parentheses = brackets = 0
    for index, char in enumerate(text):
        if char == "{":
            braces += 1
        elif char == "}":
            braces -= 1
        elif char == "(":
            parentheses += 1
        elif char == ")":
            parentheses -= 1
        elif char == "[":
            brackets += 1
        elif char == "]":
            brackets -= 1
        elif char == "," and braces == 0 and parentheses == 0 and brackets == 0:
            items.append(text[start:index])
            start = index + 1
    tail = text[start:]
    if tail.strip():
        items.append(tail)
    return items


def parse_hit_area(block: str) -> dict:
    points_match = re.search(
        r"HitPoints\s*=\s*new\s+List<InputManager\.TouchPanelArea>\s*\{",
        block,
    )
    if not points_match:
        raise ValueError("missing HitPoints")
    points_open = block.find("{", points_match.end() - 1)
    points_close = matching(block, points_open, "{", "}")
    points_text = block[points_open + 1 : points_close]
    points = []
    for piece in points_text.split(","):
        name = piece.strip().rsplit(".", 1)[-1]
        if name:
            points.append(AREA_INDEX[name])
    push = float(re.search(r"PushDistance\s*=\s*([-+0-9.eE]+)", block).group(1))
    release = float(
        re.search(r"ReleaseDistance\s*=\s*([-+0-9.eE]+)", block).group(1)
    )
    return {"points": points, "push": push, "release": release}


def parse_shape(text: str, name: str) -> list[list[dict]]:
    initializer = extract_initializer(text, name)
    outer = split_top_level(initializer)
    if len(outer) != 8:
        raise ValueError(f"{name}: expected 8 lengths, got {len(outer)}")
    result: list[list[dict]] = []
    for element in outer:
        if "new HitArea" not in element:
            result.append([])
            continue
        areas = []
        cursor = 0
        while True:
            match = re.search(r"new\s+HitArea\s*\{", element[cursor:])
            if not match:
                break
            opening = cursor + match.end() - 1
            closing = matching(element, opening, "{", "}")
            areas.append(parse_hit_area(element[opening + 1 : closing]))
            cursor = closing + 1
        result.append(areas)
    return result


def parse_flip(text: str) -> list[int]:
    match = re.search(
        r"TouchAreaFlip\s*=\s*new\s+InputManager\.TouchPanelArea\[35\]\s*\{",
        text,
    )
    if not match:
        raise ValueError("missing TouchAreaFlip")
    opening = text.find("{", match.end() - 1)
    closing = matching(text, opening, "{", "}")
    names = [
        piece.strip().rsplit(".", 1)[-1]
        for piece in text[opening + 1 : closing].split(",")
        if piece.strip()
    ]
    if len(names) != 35:
        raise ValueError(f"TouchAreaFlip: expected 35 entries, got {len(names)}")
    return [AREA_INDEX[name] for name in names]


def rotate_area(area: int, rotation: int) -> int:
    rotation %= 8
    for base in (0, 8, 18, 26):
        if base <= area <= base + 7:
            if area + rotation > base + 7:
                rotation -= 8
            return area + rotation
    return area


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "source",
        nargs="?",
        default="/home/pingfanh/project/JudgementCode/src/Assembly-CSharp/Manager/SlideManager.cs",
    )
    parser.add_argument("output", nargs="?", default="slide_hit_areas.json")
    args = parser.parse_args()

    text = Path(args.source).read_text(encoding="utf-8")
    flip = parse_flip(text)
    sources = [
        ("line", "_lineHitAreaList", False),
        ("circle_l", "_circleHitAreaListL", False),
        ("circle_r", "_circleHitAreaListL", True),
        ("u_l", "_uHitAreaListL", False),
        ("u_r", "_uHitAreaListL", True),
        ("thunder_l", "_ThunderHitAreaListL", False),
        ("thunder_r", "_ThunderHitAreaListL", True),
        ("v", "_vHitAreaList", False),
        ("cup_l", "_cupHitAreaListL", False),
        ("cup_r", "_cupHitAreaListL", True),
        ("l_l", "_lHitAreaListL", False),
        ("l_r", "_lHitAreaListL", True),
        ("fan", "_fanHitAreaList", False),
    ]

    names = ["blank"]
    all_shapes = [[[[] for _ in range(8)] for _ in range(8)]]
    for name, variable, do_flip in sources:
        names.append(name)
        parsed = parse_shape(text, variable)
        per_rotation = []
        for rotation in range(8):
            per_length = []
            for length in range(8):
                rotated = []
                for area in parsed[length]:
                    points = [
                        rotate_area(flip[point] if do_flip else point, rotation)
                        for point in area["points"]
                    ]
                    rotated.append(
                        {
                            "points": points,
                            "push": area["push"],
                            "release": area["release"],
                        }
                    )
                per_length.append(rotated)
            per_rotation.append(per_length)
        all_shapes.append(per_rotation)

    print(f"shapes={len(all_shapes)} names={names}")
    print(f"line[0][0]={json.dumps(all_shapes[1][0][0])}")
    Path(args.output).write_text(
        json.dumps({"shapeNames": names, "shapes": all_shapes}, separators=(",", ":")),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
