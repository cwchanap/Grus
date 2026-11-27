#!/usr/bin/env python3
"""HPA-1210: turn the raw Codex outputs into runtime art, verify it, and draw contact sheets.

Run from the repository root (needs Pillow + numpy):

    python3 art-src/2026-11-26/process.py            # build runtime copies, verify, contact sheets
    python3 art-src/2026-11-26/process.py --verify   # verify the committed runtime files only

Units: raw RGBA -> premultiplied Lanczos downscale to 512x512, both poses of a unit at ONE scale, feet
re-anchored to the bottom centre, and the same soft contact shadow baked under both poses.
Panels: raw -> Lanczos cover-resize + centre crop to 1920x1080, saved opaque RGB.
Exits non-zero if any check fails.
"""

import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
UNIT_OUT = REPO / "godot/assets/units/painted"
INTRO_OUT = REPO / "godot/assets/ui/frostline/intro"
PORTRAITS = REPO / "godot/assets/ui/icons/units"
KEY_ART = REPO / "godot/assets/ui/frostline/screens/start_key_art.png"
SHEETS = HERE / "contact_sheets"

KINDS = ["worker", "guard", "rifleman", "swarmer", "spitter", "brute", "behemoth"]
PANELS = [1, 2, 3, 5]
SIZE = 512
FEET_Y = SIZE - 24  # bottom of the feet; room below for the shadow
MARGIN = 8
# Calibration knob: per-image scale multiplier if a raw pose came back at a different body scale.
SCALE_FIX = {}
# Calibration knob: per-image x offset (output px) applied at paste time. feet() anchors the lowest 8% band,
# which for the Brute and Behemoth BASE poses is a forward claw, not the planted stance; these shift the body
# back over the shadow so it sits where the action pose's body sits (tuned by eye on anchor_check.png).
OFFSET_FIX = {"brute": -71, "behemoth": -77}


def alpha(im):
    return np.asarray(im)[:, :, 3]


def feet(a):
    """(bottom y, centre x, width) of the solid pixels in the lowest 8% of the figure."""
    ys, xs = np.nonzero(a > 128)
    bottom, top = ys.max(), ys.min()
    band = ys >= bottom - 0.08 * (bottom - top)
    return bottom, (xs[band].min() + xs[band].max()) / 2, xs[band].max() - xs[band].min()


def fit_scale(a, cx, bottom):
    ys, xs = np.nonzero(a > 16)
    half = max(cx - xs.min(), xs.max() - cx)
    return min((FEET_Y - MARGIN) / (bottom - ys.min()), (SIZE / 2 - MARGIN) / half)


def shadow(width):
    w, h = int(width), min(int(width * 0.3), 40)  # flat ellipse; stays inside the canvas after the blur
    layer = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    ImageDraw.Draw(layer).ellipse(
        [SIZE / 2 - w / 2, FEET_Y - h * 0.7, SIZE / 2 + w / 2, FEET_Y + h * 0.3], fill=(14, 22, 34, 120)
    )
    return layer.filter(ImageFilter.GaussianBlur(5))


def load_raw(path):
    px = np.array(Image.open(path).convert("RGBA"))
    px[px[:, :, 3] < 4] = 0  # drop near-invisible generator noise (raw corners carry alpha=1 specks)
    return Image.fromarray(px)


def build_unit(kind):
    raws = {p: load_raw(HERE / "units" / f"{kind}{p}.png") for p in ("", "_action")}
    for p, im in raws.items():
        if alpha(im)[[0, 0, -1, -1], [0, -1, 0, -1]].max() > 0:
            raise SystemExit(f"{kind}{p}: raw has an opaque background; key it out first")
    info = {p: feet(alpha(im)) for p, im in raws.items()}
    scale = min(fit_scale(alpha(raws[p]), info[p][1], info[p][0]) for p in raws)
    base_shadow = shadow(max(70, min(300, info[""][2] * scale * 1.15)))
    for p, im in raws.items():
        s = scale * SCALE_FIX.get(kind + p, 1.0)
        bottom, cx, _ = info[p]
        small = im.convert("RGBa").resize((round(im.width * s), round(im.height * s)), Image.LANCZOS).convert("RGBA")
        # paste (not alpha_composite) accepts negative offsets; onto an empty canvas it is exact
        canvas = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
        canvas.paste(small, (round(SIZE / 2 - cx * s) + OFFSET_FIX.get(kind + p, 0), round(FEET_Y - bottom * s)))
        out = base_shadow.copy()
        out.alpha_composite(canvas)
        out.save(UNIT_OUT / f"{kind}{p}.png", optimize=True)


def build_panel(n):
    im = Image.open(HERE / "intro" / f"panel_{n}.png").convert("RGB")
    s = max(1920 / im.width, 1080 / im.height)
    im = im.resize((round(im.width * s), round(im.height * s)), Image.LANCZOS)
    x, y = (im.width - 1920) // 2, (im.height - 1080) // 2
    im.crop((x, y, x + 1920, y + 1080)).save(INTRO_OUT / f"panel_{n}.png", optimize=True)


def verify():
    errors = []
    anchors = {}
    for kind in KINDS:
        for p in ("", "_action"):
            path = UNIT_OUT / f"{kind}{p}.png"
            try:
                im = Image.open(path)
                im.load()
            except Exception as e:  # missing or undecodable
                errors.append(f"{path.name}: {e}")
                continue
            a = alpha(im.convert("RGBA"))
            bottom, cx, _ = feet(a)
            anchors[kind + p] = (bottom, cx)
            checks = {
                "size 512x512": im.size == (SIZE, SIZE),
                "RGBA": im.mode == "RGBA",
                "real alpha (0 and 255)": a.min() == 0 and a.max() == 255,
                "transparent corners": a[[0, 0, -1, -1], [0, -1, 0, -1]].max() == 0,
                "transparent border": max(a[0].max(), a[-1].max(), a[:, 0].max(), a[:, -1].max()) < 8,
                "feet near bottom": SIZE - 40 <= bottom <= SIZE - 8,
                "feet at anchor": abs(cx - SIZE / 2 - OFFSET_FIX.get(kind + p, 0)) <= 12,
            }
            bad = [k for k, ok in checks.items() if not ok]
            print(f"{'FAIL' if bad else 'ok  '} units/painted/{path.name:22} feet y={bottom} x={cx:.0f} {bad or ''}")
            errors += [f"{path.name}: {k}" for k in bad]
        if kind in anchors and kind + "_action" in anchors:
            (b0, c0), (b1, c1) = anchors[kind], anchors[kind + "_action"]
            c0, c1 = c0 - OFFSET_FIX.get(kind, 0), c1 - OFFSET_FIX.get(kind + "_action", 0)
            if abs(b0 - b1) > 4 or abs(c0 - c1) > 8:
                errors.append(f"{kind}: base/action feet anchors differ ({b0},{c0:.0f}) vs ({b1},{c1:.0f})")
    for n in PANELS:
        path = INTRO_OUT / f"panel_{n}.png"
        try:
            im = Image.open(path)
            im.load()
        except Exception as e:
            errors.append(f"{path.name}: {e}")
            continue
        opaque = im.mode == "RGB" or (im.mode == "RGBA" and alpha(im).min() == 255)
        bad = [k for k, ok in {"size 1920x1080": im.size == (1920, 1080), "opaque": opaque}.items() if not ok]
        print(f"{'FAIL' if bad else 'ok  '} ui/frostline/intro/{path.name:12} {im.size} {im.mode} {bad or ''}")
        errors += [f"{path.name}: {k}" for k in bad]
    return errors


def on(bg, im):
    out = Image.new("RGBA", im.size, bg)
    out.alpha_composite(im.convert("RGBA"))
    return out


def unit_sheet():
    """Per unit: portrait | base | action | both poses at ~20 px tall (true size and x4 nearest zoom) on snow."""
    cell, small_h = 256, 20
    sheet = Image.new("RGBA", (cell * 5 + 40, cell * len(KINDS) + 40), (34, 42, 56, 255))
    d = ImageDraw.Draw(sheet)
    for i, h in enumerate(["portrait", "base", "action", "~20 px (1:1)", "~20 px (x4)"]):
        d.text((20 + i * cell + 8, 14), h, fill=(220, 230, 240, 255))
    for r, kind in enumerate(KINDS):
        y = 40 + r * cell
        d.text((24, y + 6), kind, fill=(255, 170, 90, 255))
        sheet.alpha_composite(on((92, 104, 122, 255), Image.open(PORTRAITS / f"{kind}.png").resize((cell, cell))), (20, y))
        snow_true = Image.new("RGBA", (cell, cell), (214, 222, 230, 255))
        snow_zoom = Image.new("RGBA", (cell, cell), (214, 222, 230, 255))
        for c, p in enumerate(("", "_action")):
            im = Image.open(UNIT_OUT / f"{kind}{p}.png").convert("RGBA")
            sheet.alpha_composite(on((92, 104, 122, 255), im.resize((cell, cell), Image.LANCZOS)), (20 + (c + 1) * cell, y))
            ys = np.nonzero(alpha(im) > 128)[0]
            s = small_h / (ys.max() - ys.min())
            tiny = im.convert("RGBa").resize((max(1, round(SIZE * s)),) * 2, Image.LANCZOS).convert("RGBA")
            snow_true.alpha_composite(tiny, (cell // 2 - tiny.width - 4 + c * (tiny.width + 8), cell // 2 - tiny.height // 2))
            zoom = tiny.resize((tiny.width * 4, tiny.height * 4), Image.NEAREST)
            snow_zoom.alpha_composite(zoom, (max(0, cell // 2 - zoom.width + c * zoom.width), cell // 2 - zoom.height // 2))
        sheet.alpha_composite(snow_true, (20 + 3 * cell, y))
        sheet.alpha_composite(snow_zoom, (20 + 4 * cell, y))
    sheet.convert("RGB").save(SHEETS / "units.png", optimize=True)


def anchor_sheet():
    """Per unit at ONE canvas scale and origin: base | action | overlay (base orange, action cyan).
    The vertical line marks the canvas centre, where the baked shadow sits."""
    cell = 256
    sheet = Image.new("RGBA", (cell * 3 + 40, cell * len(KINDS) + 40), (34, 42, 56, 255))
    d = ImageDraw.Draw(sheet)
    for i, h in enumerate(["base", "action", "overlay: base orange / action cyan"]):
        d.text((20 + i * cell + 8, 14), h, fill=(220, 230, 240, 255))
    for r, kind in enumerate(KINDS):
        y = 40 + r * cell
        ims = [Image.open(UNIT_OUT / f"{kind}{p}.png").convert("RGBA").resize((cell, cell), Image.LANCZOS)
               for p in ("", "_action")]
        tints = []
        for im, rgb in zip(ims, [(255, 150, 60), (80, 220, 255)]):
            t = Image.new("RGBA", im.size, rgb + (0,))
            t.putalpha(im.getchannel("A").point(lambda v: v * 0.6))
            tints.append(t)
        for c, im in enumerate([*ims, None]):
            tile = Image.new("RGBA", (cell, cell), (92, 104, 122, 255))
            for layer in ([im] if im else tints):
                tile.alpha_composite(layer)
            ImageDraw.Draw(tile).line([(cell // 2, 0), (cell // 2, cell)], fill=(255, 255, 255, 110))
            sheet.alpha_composite(tile, (20 + c * cell, y))
        d.text((24, y + 6), kind, fill=(255, 170, 90, 255))
    sheet.convert("RGB").save(SHEETS / "anchor_check.png", optimize=True)


def intro_sheet():
    """Panels 1-5 in story order (4 = Start key art, cropped 16:9). Dashed line marks the caption band."""
    w, h = 768, 432
    tiles = []
    for n in [1, 2, 3, 4, 5]:
        im = Image.open(KEY_ART if n == 4 else INTRO_OUT / f"panel_{n}.png").convert("RGB")
        s = max(w / im.width, h / im.height)
        im = im.resize((round(im.width * s), round(im.height * s)), Image.LANCZOS)
        im = im.crop(((im.width - w) // 2, (im.height - h) // 2, (im.width - w) // 2 + w, (im.height - h) // 2 + h))
        d = ImageDraw.Draw(im)
        for x in range(0, w, 16):
            d.line([(x, h * 2 // 3), (x + 8, h * 2 // 3)], fill=(255, 255, 255), width=1)
        d.text((10, 8), f"panel {n}" + (" (start_key_art)" if n == 4 else ""), fill=(255, 220, 160))
        tiles.append(im)
    sheet = Image.new("RGB", (3 * w + 80, 2 * h + 60), (20, 24, 32))
    for i, t in enumerate(tiles):
        sheet.paste(t, (20 + (i % 3) * (w + 20), 20 + (i // 3) * (h + 20)))
    sheet.save(SHEETS / "intro.png", optimize=True)


def main():
    if "--verify" not in sys.argv:
        UNIT_OUT.mkdir(parents=True, exist_ok=True)
        INTRO_OUT.mkdir(parents=True, exist_ok=True)
        SHEETS.mkdir(exist_ok=True)
        for kind in KINDS:
            build_unit(kind)
        for n in PANELS:
            build_panel(n)
    errors = verify()
    if "--verify" not in sys.argv:
        unit_sheet()
        anchor_sheet()
        intro_sheet()
    print("\n".join(["", *errors, f"{len(errors)} problem(s)"]) if errors else "\nall 18 runtime files OK")
    sys.exit(1 if errors else 0)


if __name__ == "__main__":
    main()
