from __future__ import annotations

import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

from generate_screenshot_ui_assets import IconPainter, alpha_composite_rect, load_font


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "screenshot_ui_exact"
ICON_ROOT = OUT / "icons"
COMPONENT_ROOT = OUT / "components"
PREVIEW_ROOT = OUT / "preview"

INK = (35, 36, 40, 242)
INK_LIGHT = (48, 49, 54, 232)
BLUE = (0, 163, 235, 255)
BLUE_DARK = (0, 128, 205, 255)
RED = (215, 70, 72, 255)
WHITE = (255, 255, 255, 255)
TOOLTIP = "\u8bf7\u5728\u622a\u56fe\u533a\u57df\u5185\u53cc\u51fb\u5b8c\u6210\u622a\u56fe\n\u70b9\u51fb\u53f3\u952e\u6216\u8005\u6309ESC\u9000\u51fa"


ICON_ORDER = [
    ("rectangle", "\u77e9\u5f62"),
    ("ellipse", "\u692d\u5706"),
    ("text", "\u6587\u5b57"),
    ("number", "\u5e8f\u53f7"),
    ("pen", "\u753b\u7b14"),
    ("arrow", "\u7bad\u5934"),
    ("line", "\u76f4\u7ebf"),
    ("dash", "\u865a\u7ebf"),
    ("mosaic", "\u9a6c\u8d5b\u514b"),
    ("gif", "GIF"),
    ("pin", "\u9489\u5728\u5c4f\u5e55"),
    ("capture", "\u9009\u533a"),
    ("undo", "\u64a4\u9500"),
    ("save", "\u4fdd\u5b58"),
    ("cancel", "\u53d6\u6d88"),
    ("confirm", "\u5b8c\u6210"),
]


def ensure_dirs() -> None:
    for path in [ICON_ROOT / "24", ICON_ROOT / "32", ICON_ROOT / "48", COMPONENT_ROOT, PREVIEW_ROOT]:
        path.mkdir(parents=True, exist_ok=True)


def draw_reference_icon(name: str, size: int) -> Image.Image:
    p = IconPainter(size, aa=5)
    if name == "rectangle":
        p.rounded_rect((10, 14, 38, 34), 2.2, outline=INK, width=2.6)
    elif name == "ellipse":
        p.ellipse((9, 13, 39, 35), outline=INK, width=2.6)
    elif name == "text":
        p.text((24, 26), "A", 30, INK, bold=False)
    elif name == "number":
        p.ellipse((11, 11, 37, 37), outline=INK, width=2.5)
        p.text((24.2, 24.8), "1", 21, INK, bold=False)
    elif name == "pen":
        p.line([(14.2, 34.8), (28.8, 20.2)], INK, 2.4)
        p.line([(19.1, 37.0), (34.0, 22.1)], INK, 2.4)
        p.line([(28.8, 20.2), (33.3, 15.7), (37.5, 19.9), (34.0, 22.1)], INK, 2.4)
        p.line([(14.2, 34.8), (12.2, 38.4), (19.1, 37.0)], INK, 2.4)
        p.line([(16.6, 34.2), (31.0, 19.8)], INK, 1.3)
    elif name == "arrow":
        p.line([(13.5, 35.5), (35.2, 13.8)], INK, 2.5)
        p.line([(35.2, 13.8), (34.3, 26)], INK, 2.5)
        p.line([(35.2, 13.8), (23, 14.7)], INK, 2.5)
    elif name == "line":
        p.line([(13.8, 35.3), (34.2, 13.7)], INK, 2.45)
    elif name == "dash":
        p.line([(14, 35), (18.2, 30.8)], INK, 2.5)
        p.line([(22.1, 26.9), (26.3, 22.7)], INK, 2.5)
        p.line([(30.2, 18.8), (34.2, 14.8)], INK, 2.5)
    elif name == "mosaic":
        p.rounded_rect((12.6, 11.6, 35.4, 36.4), 4.2, fill=INK, outline=None)
        p.rounded_rect((16.4, 16.0, 22.6, 22.2), 1.4, fill=(255, 255, 255, 238), outline=None)
        p.rounded_rect((26.0, 15.2, 31.5, 20.8), 1.3, fill=(255, 255, 255, 238), outline=None)
        p.rounded_rect((25.2, 27.0, 31.2, 32.8), 1.3, fill=(255, 255, 255, 238), outline=None)
    elif name == "gif":
        p.rounded_rect((9.7, 13.7, 38.3, 34.3), 4.2, outline=INK, width=2.4)
        p.text((24, 24.3), "GIF", 9, INK, bold=True)
    elif name == "pin":
        p.line([(17.4, 16.7), (31.6, 30.9)], INK, 2.5)
        p.line([(24.3, 10.8), (37.2, 23.7)], INK, 2.5)
        p.line([(22, 26.2), (13.6, 34.6)], INK, 2.5)
        p.line([(24.3, 10.8), (17.4, 16.7)], INK, 2.5)
        p.line([(37.2, 23.7), (31.6, 30.9)], INK, 2.5)
    elif name == "capture":
        c = BLUE
        p.line([(13.5, 22), (13.5, 13.5), (22, 13.5)], c, 2.6)
        p.line([(27, 13.5), (34.5, 13.5), (34.5, 21)], c, 2.6)
        p.line([(34.5, 27), (34.5, 34.5), (27, 34.5)], c, 2.6)
        p.line([(22, 34.5), (13.5, 34.5), (13.5, 26)], c, 2.6)
        p.line([(38.5, 10.5), (38.5, 17.5)], c, 1.9)
        p.line([(35, 14), (42, 14)], c, 1.9)
    elif name == "undo":
        box = p.box((12, 12.5, 38, 35.5))
        p.draw.arc(box, start=138, end=348, fill=INK, width=max(1, p.p(2.4)))
        p.polygon([(14, 21), (23.4, 14.7), (22.5, 27.5)], INK)
    elif name == "save":
        p.rounded_rect((12, 10, 36, 38), 2.2, outline=INK, width=2.5)
        p.rounded_rect((17, 13.5, 30.8, 21.8), 1.2, outline=INK, width=2.1)
        p.rounded_rect((17.4, 28.2, 30.6, 36), 1.1, outline=INK, width=2.1)
        p.line([(28.8, 13.8), (28.8, 20.4)], INK, 2.1)
    elif name == "cancel":
        p.line([(15.2, 15.2), (32.8, 32.8)], RED, 2.6)
        p.line([(32.8, 15.2), (15.2, 32.8)], RED, 2.6)
    elif name == "confirm":
        p.line([(13.2, 25.6), (21.2, 33.2), (35.8, 14.8)], BLUE_DARK, 2.9)
    else:
        raise ValueError(name)
    return p.finish()


def export_icons() -> list[dict[str, str | int]]:
    manifest = []
    for name, label in ICON_ORDER:
        for size in [24, 32, 48]:
            image = draw_reference_icon(name, size)
            path = ICON_ROOT / str(size) / f"{name}.png"
            image.save(path)
            manifest.append(
                {
                    "name": name,
                    "label": label,
                    "size": size,
                    "path": str(path.relative_to(ROOT)).replace("\\", "/"),
                }
            )
    return manifest


def make_toolbar() -> Path:
    scale = 4
    width, height = 930, 68
    img = Image.new("RGBA", (width * scale, height * scale), (0, 0, 0, 0))
    alpha_composite_rect(
        img,
        (7 * scale, 5 * scale, (width - 7) * scale, (height - 8) * scale),
        9 * scale,
        (245, 244, 240, 236),
        outline=(255, 255, 255, 224),
        width=1 * scale,
        shadow=(0, 0, 0, 86),
        shadow_radius=9 * scale,
        shadow_offset=(0, 5 * scale),
    )
    x0, step, y = 37, 57, 34
    for i, (name, _) in enumerate(ICON_ORDER):
        icon = draw_reference_icon(name, 36).resize((36 * scale, 36 * scale), Image.Resampling.LANCZOS)
        img.alpha_composite(icon, ((x0 + i * step - 18) * scale, (y - 18) * scale))
    img = img.resize((width, height), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "toolbar.png"
    img.save(out)
    return out


def make_dimension_badge() -> Path:
    scale = 4
    width, height = 174, 49
    img = Image.new("RGBA", (width * scale, height * scale), (0, 0, 0, 0))
    alpha_composite_rect(
        img,
        (5 * scale, 6 * scale, (width - 5) * scale, 42 * scale),
        6 * scale,
        (0, 171, 239, 250),
        outline=(24, 195, 255, 250),
        width=1 * scale,
        shadow=(0, 72, 118, 86),
        shadow_radius=4 * scale,
        shadow_offset=(0, 2 * scale),
    )
    draw = ImageDraw.Draw(img)
    draw.text((width * scale / 2, 24 * scale), "1246*742", font=load_font(24 * scale, mono=True), fill=WHITE, anchor="mm")
    img = img.resize((width, height), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "dimension-badge-1246x742.png"
    img.save(out)
    return out


def make_tooltip() -> Path:
    scale = 4
    width, height = 332, 78
    img = Image.new("RGBA", (width * scale, height * scale), (0, 0, 0, 0))
    alpha_composite_rect(
        img,
        (8 * scale, 7 * scale, (width - 8) * scale, (height - 9) * scale),
        5 * scale,
        (244, 244, 242, 232),
        outline=(255, 255, 255, 214),
        width=1 * scale,
        shadow=(0, 0, 0, 82),
        shadow_radius=8 * scale,
        shadow_offset=(0, 4 * scale),
    )
    draw = ImageDraw.Draw(img)
    font = load_font(17 * scale)
    draw.text((width * scale / 2, 30 * scale), TOOLTIP.split("\n")[0], font=font, fill=(74, 74, 74, 240), anchor="mm")
    draw.text((width * scale / 2, 52 * scale), TOOLTIP.split("\n")[1], font=font, fill=(74, 74, 74, 240), anchor="mm")
    img = img.resize((width, height), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "hint-tooltip.png"
    img.save(out)
    return out


def make_selection_frame() -> Path:
    scale = 3
    width, height = 866, 540
    img = Image.new("RGBA", (width * scale, height * scale), (0, 0, 0, 0))
    glow = Image.new("RGBA", img.size, (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    gd.rectangle((5 * scale, 5 * scale, (width - 5) * scale, (height - 5) * scale), outline=(0, 168, 241, 150), width=5 * scale)
    glow = glow.filter(ImageFilter.GaussianBlur(1.8 * scale))
    img.alpha_composite(glow)
    draw = ImageDraw.Draw(img)
    draw.rectangle((5 * scale, 5 * scale, (width - 5) * scale, (height - 5) * scale), outline=BLUE, width=3 * scale)
    img = img.resize((width, height), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "selection-frame.png"
    img.save(out)
    return out


def make_cursor() -> Path:
    scale = 4
    size = 64
    img = Image.new("RGBA", (size * scale, size * scale), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    pts = [(12, 8), (14, 44), (25, 34), (33, 50), (41, 46), (33, 31), (47, 31)]
    pts_s = [(x * scale, y * scale) for x, y in pts]
    shadow = Image.new("RGBA", img.size, (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow)
    sd.polygon([(x + 3 * scale, y + 4 * scale) for x, y in pts_s], fill=(0, 0, 0, 78))
    img.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(2 * scale)))
    colors = [
        ((12, 8), (28, 29), (14, 44), (255, 43, 126, 255)),
        ((12, 8), (47, 31), (28, 29), (255, 216, 35, 255)),
        ((14, 44), (28, 29), (33, 50), (49, 223, 87, 255)),
        ((28, 29), (47, 31), (41, 46), (43, 139, 255, 255)),
        ((28, 29), (41, 46), (33, 50), (150, 72, 255, 255)),
    ]
    for a, b, c, color in colors:
        draw.polygon([(a[0] * scale, a[1] * scale), (b[0] * scale, b[1] * scale), (c[0] * scale, c[1] * scale)], fill=color)
    draw.line(pts_s + [pts_s[0]], fill=(27, 27, 31, 255), width=2 * scale, joint="curve")
    img = img.resize((size, size), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "rainbow-cursor.png"
    img.save(out)
    return out


def make_icon_sheet() -> Path:
    cell, label_h, cols = 68, 24, 8
    rows = 2
    img = Image.new("RGBA", (cols * cell, rows * (cell + label_h)), (246, 247, 248, 255))
    draw = ImageDraw.Draw(img)
    font = load_font(10)
    for index, (name, label) in enumerate(ICON_ORDER):
        col = index % cols
        row = index // cols
        x, y = col * cell, row * (cell + label_h)
        draw.rounded_rectangle((x + 7, y + 7, x + cell - 7, y + cell - 7), radius=7, fill=(255, 255, 255, 255), outline=(222, 224, 227, 255))
        img.alpha_composite(draw_reference_icon(name, 32), (x + 18, y + 18))
        draw.text((x + cell / 2, y + cell + 11), label, font=font, fill=(80, 82, 88, 255), anchor="mm")
    out = PREVIEW_ROOT / "icon-sheet.png"
    img.save(out)
    return out


def make_mockup(toolbar: Path, badge: Path, tooltip: Path, cursor: Path) -> Path:
    width, height = 1706, 1279
    img = Image.new("RGBA", (width, height), (144, 143, 138, 255))
    draw = ImageDraw.Draw(img)
    for y in range(height):
        t = y / (height - 1)
        draw.line((0, y, width, y), fill=(int(161 - 42 * t), int(160 - 43 * t), int(154 - 45 * t), 255))

    # A quiet screen/photo-like environment, keeping the screenshot UI as the subject.
    draw.polygon([(0, 0), (1706, 0), (1706, 112), (0, 82)], fill=(34, 25, 21, 225))
    draw.polygon([(0, 1005), (1706, 905), (1706, 1279), (0, 1279)], fill=(24, 18, 16, 242))
    draw.rounded_rectangle((126, 1036, 1600, 1120), radius=6, fill=(18, 18, 19, 226))
    draw.text((188, 1078), "2600 x 1414\u50cf\u7d20", fill=(220, 220, 220, 150), font=load_font(18), anchor="lm")
    draw.text((1548, 1058), "100%", fill=(220, 220, 220, 150), font=load_font(17), anchor="lm")

    sx, sy, sw, sh = 364, 178, 866, 540
    screen = Image.new("RGBA", (sw, sh), (246, 245, 241, 255))
    img.alpha_composite(screen, (sx, sy))
    frame = Image.open(COMPONENT_ROOT / "selection-frame.png").convert("RGBA")
    img.alpha_composite(frame, (sx - 5, sy - 5))
    img.alpha_composite(Image.open(badge).convert("RGBA"), (sx - 8, sy - 50))
    img.alpha_composite(Image.open(toolbar).convert("RGBA"), (302, sy + sh - 4))
    img.alpha_composite(Image.open(cursor).convert("RGBA"), (1316, 478))
    img.alpha_composite(Image.open(tooltip).convert("RGBA"), (1334, 528))

    out = PREVIEW_ROOT / "screenshot-ui-reference-exact.png"
    img.convert("RGB").save(out, quality=96)
    return out


def write_manifest(paths: dict[str, Path], icons: list[dict[str, str | int]]) -> Path:
    manifest = {
        "reference": "C:/Users/Administrator/xwechat_files/wxid_e66oiwpww9pn21_b80a/temp/RWTemp/2026-05/9fed38e8d17506b0d5d1d7f565406c6b.jpg",
        "intent": "Reference-matched screenshot toolbar UI assets.",
        "style": {
            "accent": "#00A3EB",
            "toolbar": "pale translucent white, 9px radius, soft shadow, no separators",
            "icon_stroke": "#232428",
            "icon_order": [name for name, _ in ICON_ORDER],
        },
        "components": {k: str(v.relative_to(ROOT)).replace("\\", "/") for k, v in paths.items()},
        "icons": icons,
    }
    out = OUT / "manifest.json"
    out.write_text(json.dumps(manifest, indent=2, ensure_ascii=True), encoding="utf-8")
    return out


def main() -> None:
    ensure_dirs()
    icons = export_icons()
    toolbar = make_toolbar()
    badge = make_dimension_badge()
    tooltip = make_tooltip()
    frame = make_selection_frame()
    cursor = make_cursor()
    sheet = make_icon_sheet()
    mockup = make_mockup(toolbar, badge, tooltip, cursor)
    manifest = write_manifest(
        {
            "toolbar": toolbar,
            "dimension_badge": badge,
            "hint_tooltip": tooltip,
            "selection_frame": frame,
            "rainbow_cursor": cursor,
            "icon_sheet": sheet,
            "mockup": mockup,
        },
        icons,
    )
    print(f"Generated exact reference set: {OUT}")
    print(f"Icons: {len(icons)} PNG files")
    print(f"Manifest: {manifest}")
    print(f"Mockup: {mockup}")


if __name__ == "__main__":
    main()
