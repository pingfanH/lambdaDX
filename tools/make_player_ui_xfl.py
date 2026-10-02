#!/usr/bin/env python3
"""Generate the LambdaDX player UI as an Adobe Animate XFL project.

Reproduces the pure-macroquad player front-end (`src/player_ui/`) as an editable
Animate document: black-minimal surfaces, one cyan accent, sticker-sheet motion
(staggered reveal, spring pop, page sweep), split into modular movie clips.

Layering/reuse rules:
  * **Atoms are visual-only** (no baked labels): buttons, pills, tabs, sliders,
    toggles, panels, progress. Pages/compositions supply all text.
  * **Compositions** nest atoms as instances (topbar, pause panel, rows, pages).
  * State clips (``ui_row``/``ui_toggle``/``ui_pill``/``ui_tab``) hold one state
    per frame; the runtime picks a frame.
  * Every page's timeline is long enough for its animated children to advance
    (the runtime evaluates a child at ``parent_frame + firstFrame``).

Output: assets/player_ui/   (Animate opens this)

Run from the repo root:  python3 tools/make_player_ui_xfl.py
"""

import json
import math
import os
import shutil
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT_DIR = os.path.join(REPO, "assets", "player_ui")
TEMPLATES = os.path.join(REPO, "assets", "ui")

LAST = int(time.time())
TW = 20
STAGE_W, STAGE_H, FPS = 1280, 760, 60

VOID = "#171719"
PANEL = "#202022"
PANEL_ALT = "#242427"
RAISED = "#2A2A2E"
RAISED_HOVER = "#34343A"
BORDER = "#3A3A40"
BORDER_SOFT = "#2B2B2F"
TEXT = "#D6D6DA"
TEXT_DIM = "#9A9AA2"
TEXT_MUTED = "#6E6E78"
ACCENT = "#52D6E8"
ACCENT_DIM = "#2C7481"
DANGER = "#FF3B50"
SUCCESS = "#69D391"
LEVEL = "#C8FF2E"
FACE = "PingFangSC-Regular"
CUT = 14

_id = [0x100]


def uid():
    _id[0] += 1
    return "6abf0000-%08x" % _id[0]


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def tw(p):
    return int(round(p * TW))


# ── Element builders ────────────────────────────────────────────────────────

def _shape(edge_attr_edges, fills="", strokes=""):
    return f"<DOMShape>{fills}{strokes}<edges><Edge {edge_attr_edges}/></edges></DOMShape>"


def polygon(pts, fill=None, stroke=None, alpha=None):
    p = [(tw(x), tw(y)) for x, y in pts]
    e = f"!{p[0][0]} {p[0][1]}S2|{p[1][0]} {p[1][1]}"
    for i in range(1, len(p)):
        a, b = p[i], p[(i + 1) % len(p)]
        e += f"!{a[0]} {a[1]}|{b[0]} {b[1]}"
    fills = ""
    if fill:
        a = f' alpha="{alpha}"' if alpha is not None else ""
        fills = f'<fills><FillStyle index="1"><SolidColor color="{fill}"{a}/></FillStyle></fills>'
    strokes = ""
    if stroke:
        strokes = (
            f'<strokes><StrokeStyle index="1"><SolidStroke scaleMode="normal" '
            f'weight="{stroke[1]}" pixelHinting="true" sharpCorners="true">'
            f'<fill><SolidColor color="{stroke[0]}"/></fill></SolidStroke></StrokeStyle></strokes>'
        )
    at = []
    if fill:
        at.append('fillStyle1="1"')
    if stroke:
        at.append('strokeStyle="1"')
    return _shape(f'{" ".join(at)} edges="{e}"', fills, strokes)


def rect(x, y, w, h, fill=None, stroke=None, alpha=None):
    return polygon([(x, y), (x + w, y), (x + w, y + h), (x, y + h)], fill, stroke, alpha)


def cut_rect(x, y, w, h, cut=CUT, fill=None, stroke=None, alpha=None):
    return polygon([(x, y), (x + w - cut, y), (x + w, y + cut), (x + w, y + h), (x, y + h)], fill, stroke, alpha)


def circle(cx, cy, r, fill=None, stroke=None, n=28):
    return polygon([(cx + r * math.cos(2 * math.pi * i / n), cy + r * math.sin(2 * math.pi * i / n)) for i in range(n)], fill, stroke)


def hexagon(cx, cy, r, fill=None, stroke=None):
    return polygon([(cx + r * math.cos(-math.pi / 2 + math.pi / 3 * i), cy + r * math.sin(-math.pi / 2 + math.pi / 3 * i)) for i in range(6)], fill, stroke)


def line(x1, y1, x2, y2, color, weight=1):
    return _shape(
        f'strokeStyle="1" edges="!{tw(x1)} {tw(y1)}|{tw(x2)} {tw(y2)}"',
        strokes=(
            f'<strokes><StrokeStyle index="1"><SolidStroke scaleMode="normal" weight="{weight}" '
            f'pixelHinting="true" sharpCorners="true"><fill><SolidColor color="{color}"/></fill>'
            f"</SolidStroke></StrokeStyle></strokes>"
        ),
    )


def text(x, y, w, s, size, color, align="left", face=FACE, lh=None):
    lh = round(lh if lh is not None else size * 1.4, 3)
    return (
        f'<DOMStaticText fontRenderingMode="standard" width="{w}" height="{lh}" isSelectable="false">'
        f'<matrix><Matrix tx="{x}" ty="{y}"/></matrix>'
        f'<textRuns><DOMTextRun><characters>{esc(s)}</characters>'
        f'<textAttrs><DOMTextAttrs alignment="{align}" aliasText="false" autoKern="false" '
        f'lineHeight="{lh}" size="{size}" bitmapSize="{max(240, size * 20)}" face="{face}" '
        f'fillColor="{color}"/></textAttrs></DOMTextRun></textRuns></DOMStaticText>'
    )


def _num(v):
    v = round(v, 6)
    return str(int(v)) if v == int(v) else repr(v)


def inst(name, tx=0, ty=0, rot=0, sx=1, sy=1, alpha=None, firstFrame=0, tpx=0, tpy=0, loop="single frame"):
    th = math.radians(rot)
    a, b = sx * math.cos(th), sx * math.sin(th)
    c, d = -sy * math.sin(th), sy * math.cos(th)
    mat = f'<Matrix a="{_num(a)}" b="{_num(b)}" c="{_num(c)}" d="{_num(d)}" tx="{_num(tx)}" ty="{_num(ty)}"/>'
    col = f'<color><Color alphaMultiplier="{alpha}"/></color>' if alpha is not None else ""
    ff = f' firstFrame="{firstFrame}"' if firstFrame else ""
    tp = f'<Point x="{_num(tpx)}" y="{_num(tpy)}"/>' if (tpx or tpy) else "<Point/>"
    return (
        f'<DOMSymbolInstance libraryItemName="UI/{name}" symbolType="movieclip" loop="{loop}"{ff}>'
        f"<matrix>{mat}</matrix><transformationPoint>{tp}</transformationPoint>{col}</DOMSymbolInstance>"
    )


def frame(i, els=None, dur=1, tween=False):
    d = f' duration="{dur}"' if dur != 1 else ""
    t = ' tweenType="motion" motionTweenSnap="true"' if tween else ""
    km = 17921 if tween else 9728
    body = "<elements>" + "".join(els) + "</elements>" if els else "<elements/>"
    return f'<DOMFrame index="{i}"{d}{t} keyMode="{km}">{body}</DOMFrame>'


def layer(name, frames, locked=False):
    l = ' locked="true"' if locked else ""
    return f'<DOMLayer name="{name}" color="#9933CC"{l} autoNamed="false"><frames>{"".join(frames)}</frames></DOMLayer>'


def sym(name, layers, last_uid=1):
    return (
        f'<DOMSymbolItem xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" '
        f'xmlns="http://ns.adobe.com/xfl/2008/" name="UI/{name}" itemID="{uid()}" '
        f'symbolType="movieclip" lastModified="{LAST}" lastUniqueIdentifier="{last_uid}">'
        f'<timeline><DOMTimeline name="{name}" layerDepthEnabled="true">'
        f'<layers>{"".join(layers)}</layers></DOMTimeline></timeline></DOMSymbolItem>'
    )


SYMBOLS = {}


def add(name, xml):
    SYMBOLS[name] = xml


# ── Reusable surfaces ───────────────────────────────────────────────────────

def surface(name, w, h, fill=PANEL, border=None, cut=0):
    ls = []
    if border:
        ls.append(layer("border", [frame(0, [cut_rect(0, 0, w, h, cut, None, (border, 1))])]))
    ls.append(layer("fill", [frame(0, [cut_rect(0, 0, w, h, cut, fill)])]))
    return sym(name, ls)


# ── Atoms (visual only) ─────────────────────────────────────────────────────

def build_atoms():
    add("ui_panel", surface("ui_panel", 400, 240, PANEL_ALT, BORDER, CUT))
    add("ui_panel_list", surface("ui_panel_list", 420, 696, PANEL))
    add("ui_panel_side", surface("ui_panel_side", 240, 696, PANEL))
    add("ui_panel_pause", surface("ui_panel_pause", 420, 400, PANEL, BORDER, CUT))
    add("ui_header", surface("ui_header", 420, 92, PANEL))
    add("ui_hud_bar", sym("ui_hud_bar", [
        layer("edge", [frame(0, [rect(0, 65, 1280, 1, BORDER)])]),
        layer("bar", [frame(0, [rect(0, 0, 1280, 66, PANEL)])]),
    ]))
    add("ui_hairline", sym("ui_hairline", [layer("line", [frame(0, [rect(0, 0, 400, 1, BORDER)])])]))
    add("ui_shadow", sym("ui_shadow", [layer("shadow", [frame(0, [rect(0, 0, 400, 240, "#000000")])])]))

    def button(name, fill, over, down, border, border_over=None):
        ls = []
        if border:
            ls.append(layer("border", [
                frame(0, [cut_rect(0, 0, 180, 46, 8, None, (border, 1))]),
                frame(1, [cut_rect(0, 0, 180, 46, 8, None, (border_over or ACCENT, 1))]),
                frame(2, [cut_rect(0, 0, 180, 46, 8, None, (border, 1))]),
            ]))
        if fill:
            ls.append(layer("fill", [
                frame(0, [cut_rect(0, 0, 180, 46, 8, fill)]),
                frame(1, [cut_rect(0, 0, 180, 46, 8, over)]),
                frame(2, [cut_rect(0, 0, 180, 46, 8, down)]),
            ]))
        return sym(name, ls)

    add("ui_btn_primary", button("ui_btn_primary", ACCENT, "#6BDDED", "#43AFC0", None))
    add("ui_btn_secondary", button("ui_btn_secondary", RAISED, "#34343A", "#222226", BORDER))
    add("ui_btn_quiet", button("ui_btn_quiet", None, None, None, BORDER))
    add("ui_btn_danger", button("ui_btn_danger", DANGER, "#FF5063", "#D12F42", None))

    add("ui_badge", sym("ui_badge", [
        layer("glyph", [frame(0, [text(4, 15, 48, "DX", 20, ACCENT, "center")])]),
        layer("ring", [frame(0, [hexagon(28, 28, 28, None, (ACCENT, 2))])]),
        layer("fill", [frame(0, [hexagon(28, 28, 28, PANEL_ALT)])]),
    ]))
    add("ui_progress", sym("ui_progress", [
        layer("knob", [frame(0, [circle(300, 8, 8, ACCENT)])]),
        layer("fill", [frame(0, [rect(0, 0, 300, 8, ACCENT)])]),
        layer("track", [frame(0, [rect(0, 0, 510, 8, RAISED)])]),
    ]))
    add("ui_toggle_off", sym("ui_toggle_off", [
        layer("knob", [frame(0, [circle(11, 11, 9, TEXT_MUTED)])]),
        layer("track", [frame(0, [rect(0, 0, 40, 22, RAISED)])]),
    ]))
    add("ui_toggle_on", sym("ui_toggle_on", [
        layer("knob", [frame(0, [circle(29, 11, 9, ACCENT)])]),
        layer("track", [frame(0, [rect(0, 0, 40, 22, ACCENT_DIM)])]),
    ]))
    add("ui_toggle", sym("ui_toggle", [layer("state", [
        frame(0, [inst("ui_toggle_off")]),
        frame(1, [inst("ui_toggle_on")]),
    ])]))
    add("ui_slider", sym("ui_slider", [
        layer("knob", [frame(0, [rect(174, 18, 12, 18, TEXT)])]),
        layer("fill", [frame(0, [rect(0, 24, 180, 6, ACCENT)])]),
        layer("track", [frame(0, [rect(0, 24, 300, 6, RAISED)])]),
    ]))

    def row(name, fill, border, tick, badge):
        # top -> bottom: badge, tick, title, artist, thumb, border, bg
        ls = []
        if badge:
            ls.append(layer("badge", [frame(0, [circle(353, 33, 11, PANEL_ALT), circle(353, 33, 11, None, (ACCENT, 2)), text(344, 24, 18, "▶", 12, ACCENT, "center")])]))
        if tick:
            ls.append(layer("tick", [frame(0, [rect(0, 0, 3, 66, ACCENT)])]))
        ls.append(layer("title", [frame(0, [text(68, 14, 250, "曲目标题", 15, TEXT)])]))
        ls.append(layer("artist", [frame(0, [text(68, 38, 250, "艺术家", 12, TEXT_MUTED)])]))
        ls.append(layer("thumb", [frame(0, [rect(10, 10, 46, 46, RAISED_HOVER if tick else RAISED)])]))
        if border:
            ls.append(layer("border", [frame(0, [cut_rect(0, 0, 388, 66, 10, None, (border, 1))])]))
        ls.append(layer("bg", [frame(0, [cut_rect(0, 0, 388, 66, 10, fill)])]))
        return sym(name, ls)

    add("ui_song_row", row("ui_song_row", PANEL, BORDER_SOFT, False, False))
    add("ui_song_row_hover", row("ui_song_row_hover", RAISED, BORDER, True, False))
    add("ui_song_row_on", row("ui_song_row_on", PANEL_ALT, ACCENT, True, True))
    add("ui_row", sym("ui_row", [layer("state", [
        frame(0, [inst("ui_song_row")]),
        frame(1, [inst("ui_song_row_hover")]),
        frame(2, [inst("ui_song_row_on")]),
    ])]))

    def pill(name, fill, border):
        return sym(name, [
            layer("border", [frame(0, [cut_rect(0, 0, 78, 38, 8, None, (border, 1))])]),
            layer("fill", [frame(0, [cut_rect(0, 0, 78, 38, 8, fill)])]),
        ])

    add("ui_level_pill", pill("ui_level_pill", RAISED, BORDER))
    add("ui_level_pill_on", pill("ui_level_pill_on", ACCENT, ACCENT))
    add("ui_pill", sym("ui_pill", [layer("state", [
        frame(0, [inst("ui_level_pill")]),
        frame(1, [inst("ui_level_pill_on")]),
    ])]))

    def cat(name, fill, border, tick):
        ls = []
        if tick:
            ls.append(layer("tick", [frame(0, [rect(0, 0, 3, 60, ACCENT)])]))
        ls.append(layer("border", [frame(0, [cut_rect(0, 0, 208, 60, 8, None, (border, 1))])]))
        ls.append(layer("fill", [frame(0, [cut_rect(0, 0, 208, 60, 8, fill)])]))
        return sym(name, ls)

    add("ui_cat_tab", cat("ui_cat_tab", PANEL_ALT, BORDER, False))
    add("ui_cat_tab_on", cat("ui_cat_tab_on", RAISED, ACCENT, True))
    add("ui_tab", sym("ui_tab", [layer("state", [
        frame(0, [inst("ui_cat_tab")]),
        frame(1, [inst("ui_cat_tab_on")]),
    ])]))


# ── Compositions ────────────────────────────────────────────────────────────

def build_compositions():
    # Topbar: bar + bottom edge + quiet back button (an atom) + text.
    add("ui_topbar", sym("ui_topbar", [
        layer("title", [frame(0, [text(124, 20, 170, "选择歌曲", 22, TEXT),
                                  text(300, 26, 300, "PLAYER", 12, TEXT_MUTED)])]),
        layer("back", [frame(0, [inst("ui_btn_quiet", tx=20, ty=14, sx=84 / 180, sy=36 / 46),
                                 text(20, 23, 84, "返回", 14, TEXT_DIM, "center")])]),
        layer("edge", [frame(0, [rect(0, 63, 1280, 1, BORDER)])]),
        layer("bar", [frame(0, [rect(0, 0, 1280, 64, PANEL)])]),
    ]))
    add("ui_logo", sym("ui_logo", [
        layer("mark", [frame(0, [text(4, 4, 520, "LambdaDX", 58, ACCENT_DIM, lh=58),
                                 text(0, 0, 520, "LambdaDX", 58, TEXT, lh=58),
                                 text(2, 72, 400, "P L A Y E R", 16, ACCENT)])]),
    ]))
    add("ui_watermark", sym("ui_watermark", [layer("text", [frame(0, [text(16, 678, 700, "SELECT", 64, BORDER_SOFT)])])]))
    add("ui_cover_placeholder", sym("ui_cover_placeholder", [
        layer("badge", [frame(0, [circle(150, 150, 36, PANEL), circle(150, 150, 36, None, (BORDER_SOFT, 1)), text(114, 132, 72, "♪", 28, TEXT_MUTED, "center")])]),
        layer("border", [frame(0, [cut_rect(0, 0, 300, 300, 8, None, (BORDER_SOFT, 1))])]),
        layer("bg", [frame(0, [cut_rect(0, 0, 300, 300, 8, PANEL_ALT)])]),
    ]))
    add("ui_toast", sym("ui_toast", [
        layer("msg", [frame(0, [text(14, 8, 272, "提示信息", 13, DANGER, "center")])]),
        layer("border", [frame(0, [cut_rect(0, 0, 300, 32, 8, None, (DANGER, 1))])]),
        layer("bg", [frame(0, [cut_rect(0, 0, 300, 32, 8, PANEL_ALT)])]),
    ]))
    add("ui_load_bar", sym("ui_load_bar", [layer("bar", [frame(0, [rect(0, 0, 80, 4, ACCENT)])])]))
    add("ui_loading", sym("ui_loading", [
        layer("bar", [frame(0, [inst("ui_load_bar", tx=-40, ty=54, loop="loop")], dur=40, tween=True),
                      frame(40, [inst("ui_load_bar", tx=260, ty=54, loop="loop")])]),
        layer("track", [frame(0, [rect(40, 54, 220, 4, RAISED)])]),
        layer("msg", [frame(0, [text(20, 18, 260, "载入音频…", 16, TEXT, "center")])]),
        layer("border", [frame(0, [rect(0, 0, 300, 88, None, BORDER)])]),
        layer("dim", [frame(0, [rect(0, 0, 300, 88, "#101012")])]),
    ], last_uid=41))

    ticks = []
    for i in range(24):
        a = i * math.pi / 12
        lng = (i % 6 == 0)
        r0 = 136 if lng else 144
        ticks.append(line(math.cos(a) * r0, math.sin(a) * r0, math.cos(a) * 150, math.sin(a) * 150, ACCENT if lng else BORDER, 2 if lng else 1))
    add("ui_hero_ticks", sym("ui_hero_ticks", [layer("ticks", [frame(0, ticks)])]))

    sweep = []
    prev = None
    for j in range(51):
        a = j * (4.36 / 50)
        p = (math.cos(a) * 126, math.sin(a) * 126)
        if prev:
            sweep.append(line(prev[0], prev[1], p[0], p[1], ACCENT, 2))
        prev = p
    add("ui_hero_sweep", sym("ui_hero_sweep", [layer("arc", [frame(0, sweep)])]))

    add("ui_hero", sym("ui_hero", [
        layer("badge", [frame(0, [circle(150, 150, 38, PANEL_ALT), circle(150, 150, 38, None, (ACCENT, 2)), text(112, 130, 76, "DX", 24, ACCENT, "center")])]),
        layer("sweep", _spin("ui_hero_sweep", 120, 8, 150, 150, -1)),
        layer("ticks", _spin("ui_hero_ticks", 120, 8, 150, 150, 1)),
        layer("rings", [frame(0, [circle(150, 150, 150, None, (BORDER, 1), n=36),
                                  circle(150, 150, 108, None, (BORDER_SOFT, 1), n=36),
                                  circle(150, 150, 90, PANEL, n=36)])]),
    ], last_uid=121))

    # Pause panel composes the pause surface + buttons + text.
    labels = [("继续游玩", "ui_btn_primary"), ("重新开始", "ui_btn_secondary"),
              ("游玩设置", "ui_btn_quiet"), ("退出到选歌", "ui_btn_danger")]
    btn_els = []
    for i, (lab, symn) in enumerate(labels):
        y = 180 + i * 56
        btn_els.append(inst(symn, tx=28, ty=y))
        btn_els.append(text(28, y + 14, 364, lab, 16, VOID if symn in ("ui_btn_primary", "ui_btn_danger") else TEXT, "center"))
    add("ui_pause_panel", sym("ui_pause_panel", [
        layer("buttons", [frame(0, btn_els)]),
        layer("copy", [frame(0, [text(28, 38, 364, "PLAY SESSION", 11, ACCENT),
                                 text(28, 68, 364, "已暂停", 30, TEXT),
                                 text(28, 112, 364, "曲目标题", 14, TEXT_DIM),
                                 text(28, 134, 364, "当前时间  01:23", 12, TEXT_MUTED)])]),
        layer("surface", [frame(0, [inst("ui_panel_pause", tx=0, ty=0)])]),
    ]))


def _spin(symname, total, steps, tx, ty, direction):
    frames = []
    seg = total // steps
    for k in range(steps + 1):
        rot = direction * 360.0 * k / steps
        frames.append(frame(k * seg, [inst(symname, tx=tx, ty=ty, rot=rot, loop="loop")], dur=seg if k < steps else 1, tween=k < steps))
    return frames


# ── Pages (all content spans the page timeline) ─────────────────────────────

def _labelled_btn(symn, x, y, w, h, lab, color, dur):
    return layer(f"btn_{symn}_{int(x)}", [frame(0, [
        inst(symn, tx=x, ty=y, sx=w / 180, sy=h / 46),
        text(x, y + (h - 22.4) / 2, w, lab, 16, color, "center"),
    ], dur=dur)])


def build_pages():
    # Start page: hero animates, so the page spans the hero's 121 frames.
    add("page_start", sym("page_start", [
        layer("hero", [frame(0, [inst("ui_hero", tx=810, ty=230, loop="loop")], dur=121)]),
        _labelled_btn("ui_btn_primary", 56, 362, 360, 50, "开始 · 选歌", VOID, 121),
        _labelled_btn("ui_btn_secondary", 56, 424, 360, 50, "设置", TEXT, 121),
        layer("copy", [frame(0, [
            text(56, 198, 500, "ARCADE CHART PLAYER", 12, ACCENT),
            inst("ui_logo", tx=56, ty=230),
            text(56, 332, 520, "把节拍变成动作。选择谱面，设定难度，进入你的下一局。", 15, TEXT_DIM),
            text(56, 502, 400, "曲库 · 12 首", 12, TEXT_MUTED),
            text(56, 520, 500, "assets/charts", 11, TEXT_MUTED),
        ], dur=121)]),
        layer("bg", [frame(0, [rect(0, 0, 1280, 760, VOID)], dur=121)]),
    ], last_uid=122))

    # Song select: list panel + header + reused rows (staggered) + detail.
    rows = []
    row_states = [2, 1, 0, 0, 0, 0]
    for i in range(6):
        d = i * 2
        base = 164 + i * 76
        fs = []
        if d > 0:
            fs.append(frame(0, [], dur=d))
        fs.append(frame(d, [inst("ui_row", tx=16, ty=base + 10, alpha=0, firstFrame=row_states[i])], dur=14, tween=True))
        fs.append(frame(d + 14, [inst("ui_row", tx=16, ty=base, alpha=1, firstFrame=row_states[i])], dur=25 - (d + 14)))
        rows.append(layer(f"r{i}", fs))
    pills = []
    for i in range(4):
        x = 682 + i * 86
        pills.append(inst("ui_pill", tx=x, ty=514, firstFrame=(1 if i == 1 else 0)))
        pills.append(text(x, 523, 78, f"Lv.{12 + i}", 15, VOID if i == 1 else TEXT, "center"))
    add("page_song_select", sym("page_song_select", [
        layer("titles", [frame(0, [inst("ui_topbar", tx=0, ty=0)], dur=25)]),
        layer("detail", [frame(0, [
            inst("ui_cover_placeholder", tx=700, ty=98),
            text(440, 402, 820, "NOW SELECTING", 11, ACCENT, "center"),
            text(440, 430, 820, "曲目标题", 26, TEXT, "center"),
            text(440, 470, 820, "艺术家", 14, TEXT_DIM, "center"),
            text(440, 496, 820, "谱师 · 描述", 12, TEXT_MUTED, "center"),
            text(440, 574, 820, "128 notes · 180 BPM", 13, TEXT_DIM, "center"),
            inst("ui_btn_primary", tx=690, ty=608, sx=320 / 180, sy=50 / 46),
            text(690, 621, 320, "开始游玩", 16, VOID, "center"),
        ], dur=25)]),
        layer("pills", [frame(0, pills, dur=25)]),
        *rows,
        layer("list_header", [frame(0, [
            inst("ui_header", tx=0, ty=64),
            text(20, 80, 380, "曲库", 12, ACCENT),
            text(20, 98, 380, "选择一首歌开始", 20, TEXT),
        ], dur=25)]),
        layer("watermark", [frame(0, [inst("ui_watermark", tx=0, ty=0)], dur=25)]),
        layer("bg", [frame(0, [rect(0, 0, 1280, 760, VOID),
                               inst("ui_panel_list", tx=0, ty=64),
                               rect(419, 64, 1, 696, BORDER)], dur=25)]),
    ], last_uid=26))

    # Settings: side surface + reused tabs (labels in page) + toggle/sliders.
    cats = []
    cat_names = ["音频", "游玩", "显示"]
    cat_descs = ["音乐与判定音效", "速度与辅助", "布局与视觉"]
    for i in range(3):
        y = 112 + i * 68
        cats.append(layer(f"cat{i}", [frame(0, [
            inst("ui_tab", tx=16, ty=y, firstFrame=(1 if i == 1 else 0)),
            text(30, y + 12, 180, cat_names[i], 15, TEXT),
            text(30, y + 36, 180, cat_descs[i], 11, TEXT_MUTED),
        ])]))
    add("page_settings", sym("page_settings", [
        layer("top", [frame(0, [inst("ui_topbar", tx=0, ty=0)])]),
        layer("panel", [frame(0, [
            text(272, 94, 400, "设置项", 11, ACCENT),
            text(272, 124, 500, "音频", 26, TEXT),
            text(272, 176, 500, "音乐与判定音效", 15, TEXT),
            text(272, 196, 500, "关闭后游玩仍会继续，但不会播放声音。", 12, TEXT_MUTED),
            inst("ui_toggle", tx=788, ty=166, firstFrame=1),
            inst("ui_slider", tx=272, ty=240, sx=560 / 300), text(272, 246, 400, "流速 (Note Speed)", 15, TEXT), text(752, 246, 80, "7.50", 15, ACCENT, "right"),
            inst("ui_slider", tx=272, ty=312, sx=560 / 300), text(272, 318, 400, "Touch 流速", 15, TEXT), text(752, 318, 80, "7.50", 15, ACCENT, "right"),
            inst("ui_slider", tx=272, ty=384, sx=560 / 300), text(272, 390, 400, "Slide 提前量 (秒)", 15, TEXT), text(752, 390, 80, "0.52", 15, ACCENT, "right"),
            inst("ui_slider", tx=272, ty=456, sx=560 / 300), text(272, 462, 400, "播放速度", 15, TEXT), text(752, 462, 80, "1.00", 15, ACCENT, "right"),
            inst("ui_btn_quiet", tx=272, ty=548), text(272, 562, 180, "恢复默认", 16, TEXT_DIM, "center"),
            text(272, 612, 500, "已恢复默认设置", 12, TEXT_MUTED),
        ])]),
        *cats,
        layer("side", [frame(0, [inst("ui_panel_side", tx=0, ty=64),
                                  text(20, 90, 200, "PLAYER SETTINGS", 11, ACCENT)])]),
        layer("bg", [frame(0, [rect(0, 0, 1280, 760, VOID)])]),
    ]))

    add("page_gameplay_hud", sym("page_gameplay_hud", [
        layer("controls", [frame(0, [
            inst("ui_btn_secondary", tx=16, ty=13, sx=84 / 180, sy=40 / 46), text(16, 22, 84, "暂停", 16, TEXT, "center"),
            inst("ui_btn_quiet", tx=1124, ty=17, sx=70 / 180, sy=32 / 46), text(1124, 24, 70, "AUTO", 16, TEXT_DIM, "center"),
            inst("ui_btn_primary", tx=1202, ty=17, sx=62 / 180, sy=32 / 46), text(1202, 24, 62, "PLAY", 16, VOID, "center"),
        ])]),
        layer("progress", [frame(0, [inst("ui_progress", tx=502, ty=29)])]),
        layer("title", [frame(0, [text(118, 18, 500, "曲目标题", 17, TEXT),
                                  text(118, 42, 500, "Lv.12 · 1.0x", 11, TEXT_MUTED)])]),
        layer("hint", [frame(0, [text(700, 732, 564, "SPACE 暂停 · R 重播 · A 自动 · ←/→ 快退/快进 · ESC 暂停", 11, TEXT_MUTED, "right")])]),
        layer("bar", [frame(0, [inst("ui_hud_bar", tx=0, ty=0)])]),
    ]))

    add("page_pause", sym("page_pause", [
        layer("panel", [frame(0, [inst("ui_pause_panel", tx=430, ty=180, sx=0.9, sy=0.9, alpha=0, tpx=210, tpy=200)], dur=12, tween=True),
                        frame(12, [inst("ui_pause_panel", tx=430, ty=180, sx=1, sy=1, alpha=1, tpx=210, tpy=200)], dur=1)]),
        layer("dim", [frame(0, [rect(0, 0, 1280, 760, "#0B0B0C", alpha=0.72)], dur=13)]),
    ], last_uid=13))


def build_slots():
    """Dynamic-text slots: page-space rects the runtime fills with live data.

    Each `text` is the placeholder baked into the XFL (used to skip/erase it);
    the manifest is what lets the runtime draw real values without a font stack
    in macroanimate.
    """
    def slot(page, sid, text, x, y, w, size, color, align="left"):
        return {"page": f"UI/{page}", "id": sid, "text": text,
                "x": x, "y": y, "w": w, "size": size, "color": color, "align": align}

    out = []
    out += [
        slot("page_start", "start.count", "曲库 · 12 首", 56, 502, 400, 12, TEXT_MUTED),
        slot("page_start", "start.path", "assets/charts", 56, 520, 500, 11, TEXT_MUTED),
        slot("page_song_select", "ss.title", "曲目标题", 440, 430, 820, 26, TEXT, "center"),
        slot("page_song_select", "ss.artist", "艺术家", 440, 470, 820, 14, TEXT_DIM, "center"),
        slot("page_song_select", "ss.meta", "谱师 · 描述", 440, 496, 820, 12, TEXT_MUTED, "center"),
        slot("page_song_select", "ss.notes", "128 notes · 180 BPM", 440, 574, 820, 13, TEXT_DIM, "center"),
        slot("page_settings", "set.v0", "7.50", 752, 246, 80, 15, ACCENT, "right"),
        slot("page_settings", "set.v1", "7.50", 752, 318, 80, 15, ACCENT, "right"),
        slot("page_settings", "set.v2", "0.52", 752, 390, 80, 15, ACCENT, "right"),
        slot("page_settings", "set.v3", "1.00", 752, 462, 80, 15, ACCENT, "right"),
        slot("page_settings", "set.status", "已恢复默认设置", 272, 612, 500, 12, TEXT_MUTED),
        slot("page_gameplay_hud", "hud.title", "曲目标题", 118, 18, 500, 17, TEXT),
        slot("page_gameplay_hud", "hud.sub", "Lv.12 · 1.0x", 118, 42, 500, 11, TEXT_MUTED),
        slot("page_pause", "pause.title", "曲目标题", 458, 292, 364, 14, TEXT_DIM),
        slot("page_pause", "pause.time", "当前时间  01:23", 458, 314, 364, 12, TEXT_MUTED),
    ]
    for i in range(4):
        out.append(slot("page_song_select", f"ss.pill{i}", "Lv.12",
                        682 + i * 86, 523, 78, 15, VOID if i == 1 else TEXT, "center"))
    return out


def dom_document():
    inc = "\n".join(
        f'     <Include href="UI/{n}.xml" loadImmediate="false" itemID="{uid()}" lastModified="{LAST}"/>'
        for n in sorted(SYMBOLS)
    )
    pages = ["page_start", "page_song_select", "page_settings", "page_gameplay_hud", "page_pause"]
    page_layers = "\n".join(
        f'''     <DOMLayer name="P{i+1}_{p}" color="#9933CC">
      <frames>
       <DOMFrame index="0" keyMode="9728">
        <elements>{inst(p, tx=i*1340, ty=0)}</elements>
       </DOMFrame>
      </frames>
     </DOMLayer>'''
        for i, p in enumerate(pages)
    )
    return f'''<DOMDocument xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns="http://ns.adobe.com/xfl/2008/" backgroundColor="{VOID}" width="{STAGE_W}" height="{STAGE_H}" frameRate="{FPS}" currentTimeline="1" xflVersion="23.0" creatorInfo="Adobe Animate" platform="Macintosh" versionInfo="Saved by Animate Macintosh 24.0 build 305" majorVersion="24" buildNumber="305" nextSceneIdentifier="2" playOptionsPlayLoop="false" playOptionsPlayPages="false" playOptionsPlayFrameActions="false">
     <symbols>
{inc}
     </symbols>
     <timelines>
      <DOMTimeline name="Scene 1" layerDepthEnabled="true">
       <layers>
{page_layers}
        <DOMLayer name="00_BG" color="#00FFFF" current="true" isSelected="true">
         <frames>
          <DOMFrame index="0" keyMode="9728">
           <elements><DOMShape><fills><FillStyle index="1"><SolidColor color="{VOID}"/></FillStyle></fills><edges><Edge fillStyle1="1" edges="!0 0S2|140000 0!140000 0|140000 15200!140000 15200|0 15200!0 15200|0 0"/></edges></DOMShape></elements>
          </DOMFrame>
         </frames>
        </DOMLayer>
       </layers>
      </DOMTimeline>
     </timelines>
     <scripts/>
     <PrinterSettings platform="macintosh"/>
     <publishHistory/>
</DOMDocument>
'''


def main_xfl(files):
    entries = ['        <DOMFile path="DOMDocument.xml" type="application/vnd.adobe.xfl.document"/>']
    for rel in files:
        entries.append(f'        <DOMFile path="{rel}"/>')
    return (
        f'<DOMFlashFile xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" '
        f'xmlns="http://ns.adobe.com/xfl/2008/" version="2">\n     <files>\n' + "\n".join(entries) + "\n     </files>\n</DOMFlashFile>\n"
    )


def main():
    shutil.rmtree(OUT_DIR, ignore_errors=True)
    build_atoms()
    build_compositions()
    build_pages()
    files = {"DOMDocument.xml": dom_document()}
    for name, xml in SYMBOLS.items():
        files[f"LIBRARY/UI/{name}.xml"] = xml
    files["player_ui.xfl"] = "PROXY-CS5"
    for rel in ("PublishSettings.xml", "META-INF/metadata.xml", "MobileSettings.xml"):
        src = os.path.join(TEMPLATES, rel)
        if os.path.exists(src):
            with open(src, "rb") as f:
                files[rel] = f.read()
    files["main.xfl"] = main_xfl(sorted(k for k in files if k.startswith("LIBRARY/")))
    files["text_slots.json"] = json.dumps({"slots": build_slots()}, ensure_ascii=False, indent=1)
    for rel, data in files.items():
        path = os.path.join(OUT_DIR, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        mode = "wb" if isinstance(data, bytes) else "w"
        with open(path, mode) as f:
            f.write(data)
    print(f"wrote {OUT_DIR}: {len(SYMBOLS)} symbols")


if __name__ == "__main__":
    main()
