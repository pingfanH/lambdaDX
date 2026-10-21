#!/usr/bin/env python3
"""Source-derived SDEZ170 oracle for golden vectors.

This mirrors NoteJudge.cs / NoteScore.cs with explicit f32 rounding. It is an
independent implementation used to catch transcription errors in Rust; the C#
source remains the authority.
"""

from __future__ import annotations

import json
import math
import random
import struct
import sys
from pathlib import Path


def f32(value: float) -> float:
    return struct.unpack("f", struct.pack("f", float(value)))[0]


FRAME = f32(16.666666)
ADJUST = f32(50.0)

COEFFICIENTS = [
    [-9, -6, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 9],
    [-9, -6, -5, -4, -3, -2, -1, 0, 1, 4, 8, 9, 10, 11, 14],
    [-36, -26, -22, -18, -14, -14, -14, 0, 14, 14, 14, 16, 22, 26, 36],
    [-9, -9, -9, -9, -9, -9, -9, 0, 9, 10.5, 12, 13, 14, 15, 18],
    [-9, -9, -9, -9, -9, -9, -9, 0, 9, 9, 9, 9, 9, 9, 9],
    [-9, -6, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 9],
]

TIMINGS = [
    "TooFast", "FastGood", "FastGreat3rd", "FastGreat2nd", "FastGreat",
    "FastPerfect2nd", "FastPerfect", "Critical", "LatePerfect",
    "LatePerfect2nd", "LateGreat", "LateGreat2nd", "LateGreat3rd",
    "LateGood", "TooLate",
]

HOLD_PERCENT = [0, 33, 67, 95, 100]
HOLD_PARAM = [
    [1, 4, 4, 4, 4, 6, 6, 7, 8, 8, 10, 10, 10, 10, 13],
    [1, 4, 4, 4, 4, 6, 6, 8, 8, 8, 10, 10, 10, 10, 13],
    [1, 1, 4, 4, 4, 4, 4, 10, 10, 10, 10, 10, 10, 13, 13],
    [1, 1, 1, 1, 1, 1, 1, 13, 13, 13, 13, 13, 13, 13, 13],
    [0, 1, 1, 1, 1, 1, 1, 13, 13, 13, 13, 13, 13, 13, 14],
]

SCORES = [
    [0, 250, 400, 400, 400, 500, 500, 500, 500, 500, 400, 400, 400, 250, 0],
    [0, 500, 800, 800, 800, 1000, 1000, 1000, 1000, 1000, 800, 800, 800, 500, 0],
    [0, 750, 1200, 1200, 1200, 1500, 1500, 1500, 1500, 1500, 1200, 1200, 1200, 750, 0],
    [0, 1000, 1250, 1500, 2000, 2500, 2500, 2500, 2500, 2500, 2000, 1500, 1250, 1000, 0],
    [0, 250, 400, 400, 400, 500, 500, 500, 500, 500, 400, 400, 400, 250, 0],
    [0, 30, 40, 40, 40, 50, 75, 100, 75, 50, 40, 40, 40, 30, 0],
]


def threshold(kind: int, index: int) -> float:
    return f32(f32(COEFFICIENTS[kind][index] * FRAME) + ADJUST)


def get_judge_timing(diff: float, option: float, kind: int) -> tuple[int, float]:
    diff = f32(diff - f32(option * FRAME))
    if kind == 2:
        diff = f32(diff + ADJUST)
    d = diff
    if d < threshold(kind, 0):
        timing = 0
    elif d < threshold(kind, 1):
        timing = 1
    elif d < threshold(kind, 2):
        timing = 2
    elif d < threshold(kind, 3):
        timing = 3
    elif d < threshold(kind, 4):
        timing = 4
    elif d < threshold(kind, 5):
        timing = 5
    elif d < threshold(kind, 6):
        timing = 6
    elif d <= threshold(kind, 8):
        timing = 7
    elif d <= threshold(kind, 9):
        timing = 8
    elif d <= threshold(kind, 10):
        timing = 9
    elif d <= threshold(kind, 11):
        timing = 10
    elif d <= threshold(kind, 12):
        timing = 11
    elif d <= threshold(kind, 13):
        timing = 12
    elif not d <= threshold(kind, 14):
        timing = 14
    else:
        timing = 13
    diff = f32(diff - ADJUST)
    return timing, diff


def get_slide_judge_timing(
    diff: float, option: float, kind: int, last_wait: float
) -> tuple[int, float]:
    if kind != 2:
        return get_judge_timing(diff, option, 2)
    num = f32(last_wait / 4.0)
    num2 = f32(threshold(2, 6) - num)
    num3 = f32(threshold(2, 8) + num)
    num4 = threshold(2, 0)
    num5 = threshold(2, 14)
    num6 = f32(diff - f32(option * FRAME))
    if num2 <= num6 <= num3 and num4 <= num6 <= num5:
        return 7, diff
    return get_judge_timing(diff, option, 2)


def judge_hold_total(
    hold_time: float,
    release: float,
    timing: int,
    body_on: bool,
    touch: bool,
) -> int:
    head = 250.0 if touch else 100.0
    tail = 200.0
    if hold_time <= f32(head + tail):
        return timing
    hold_time = f32(hold_time - f32(head + tail))
    if not body_on:
        return HOLD_PARAM[4][timing]
    percent = math.ceil(f32(f32(release / hold_time) * 100.0))
    percent = min(percent, 100)
    for index, limit in enumerate(HOLD_PERCENT):
        if limit >= percent:
            return HOLD_PARAM[index][timing]
    return HOLD_PARAM[4][timing]


def main() -> None:
    output = Path(sys.argv[1] if len(sys.argv) > 1 else sys.argv[0])
    rng = random.Random(170)
    judge = []
    for kind in range(6):
        diffs = [rng.uniform(-450.0, 450.0) for _ in range(900)]
        for index in range(15):
            for delta in (-0.001, 0.0, 0.001):
                diffs.append(threshold(kind, index) + delta)
        for diff in diffs:
            option = rng.choice([0.0, 0.5, -0.5])
            timing, after = get_judge_timing(diff, option, kind)
            judge.append(
                {
                    "type": kind,
                    "diff": f32(diff),
                    "option": option,
                    "timing": TIMINGS[timing],
                    "after": after,
                }
            )

    slides = []
    for _ in range(1200):
        diff = rng.uniform(-750.0, 750.0)
        wait = rng.choice([0.0, 25.0, 100.0, 400.0, 1000.0, 2000.0])
        timing, after = get_slide_judge_timing(diff, 0.0, 2, wait)
        slides.append(
            {
                "diff": f32(diff),
                "wait": wait,
                "timing": TIMINGS[timing],
                "after": after,
            }
        )

    holds = []
    for _ in range(800):
        hold_time = rng.choice([200.0, 300.0, 400.0, 500.0, 700.0, 1000.0, 1500.0, 2500.0, 4000.0])
        release = rng.uniform(0.0, hold_time)
        timing = rng.randrange(15)
        body = rng.choice([True, False])
        touch = rng.choice([True, False])
        holds.append(
            {
                "holdTime": hold_time,
                "release": release,
                "timing": TIMINGS[timing],
                "bodyOn": body,
                "touch": touch,
                "result": TIMINGS[judge_hold_total(hold_time, release, timing, body, touch)],
            }
        )

    Path(output).write_text(
        json.dumps(
            {"judge": judge, "slide": slides, "hold": holds, "scores": SCORES},
            separators=(",", ":"),
        ),
        encoding="utf-8",
    )
    print(f"wrote {output}: judge={len(judge)} slide={len(slides)} hold={len(holds)}")


if __name__ == "__main__":
    main()
