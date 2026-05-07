from __future__ import annotations

import json
import shutil
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

from generate_screenshot_ui_assets import IconPainter, alpha_composite_rect, load_font
from generate_reference_exact_ui_assets import ICON_ORDER, TOOLTIP


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "screenshot_ui_cute"
ICON_ROOT = OUT / "icons"
COMPONENT_ROOT = OUT / "components"
PREVIEW_ROOT = OUT / "preview"
STATES = ["normal", "hover", "down"]

INK = (74, 58, 86, 245)
BLUE = (0, 166, 241, 255)
BLUE_DARK = (0, 128, 205, 255)
RED = (244, 91, 103, 255)
GREEN = (68, 203, 142, 255)
YELLOW = (255, 202, 79, 255)
PINK = (255, 112, 177, 255)
HOT_PINK = (255, 78, 166, 255)
LILAC = (183, 137, 255, 255)
MINT = (127, 226, 194, 255)
PEACH = (255, 176, 128, 255)
CREAM = (255, 246, 252, 244)

PASTEL_BG = {
    "rectangle": (255, 218, 237, 112),
    "ellipse": (255, 228, 240, 108),
    "text": (255, 183, 218, 122),
    "number": (255, 224, 154, 118),
    "pen": (255, 169, 217, 135),
    "arrow": (203, 222, 255, 112),
    "line": (223, 200, 255, 112),
    "dash": (255, 207, 232, 108),
    "mosaic": (238, 202, 255, 128),
    "gif": (255, 190, 226, 118),
    "pin": (255, 180, 224, 132),
    "capture": (198, 231, 255, 128),
    "undo": (255, 194, 225, 132),
    "save": (225, 211, 255, 112),
    "cancel": (255, 186, 204, 122),
    "confirm": (210, 242, 233, 120),
}


def ensure_dirs() -> None:
    if ICON_ROOT.exists():
        shutil.rmtree(ICON_ROOT)
    for state in STATES:
        for size in ["24", "32", "48"]:
            (ICON_ROOT / state / size).mkdir(parents=True, exist_ok=True)
    for path in [COMPONENT_ROOT, PREVIEW_ROOT]:
        path.mkdir(parents=True, exist_ok=True)


def cute_backplate(p: IconPainter, name: str) -> None:
    fill = PASTEL_BG[name]
    p.rounded_rect((7.8, 8.6, 40.2, 39.4), 10, fill=fill, outline=None)


def cute_star(p: IconPainter, cx: float, cy: float, color: tuple[int, int, int, int]) -> None:
    p.line([(cx, cy - 3.4), (cx, cy + 3.4)], color, 1.5)
    p.line([(cx - 3.4, cy), (cx + 3.4, cy)], color, 1.5)


def cute_heart(
    p: IconPainter,
    cx: float,
    cy: float,
    scale: float,
    fill: tuple[int, int, int, int],
    outline: tuple[int, int, int, int] = INK,
) -> None:
    p.ellipse((cx - 6.2 * scale, cy - 5.4 * scale, cx - 0.6 * scale, cy + 0.2 * scale), fill=fill, outline=None)
    p.ellipse((cx + 0.6 * scale, cy - 5.4 * scale, cx + 6.2 * scale, cy + 0.2 * scale), fill=fill, outline=None)
    p.polygon(
        [
            (cx - 6.1 * scale, cy - 1.7 * scale),
            (cx + 6.1 * scale, cy - 1.7 * scale),
            (cx, cy + 7.0 * scale),
        ],
        fill=fill,
    )
    p.line(
        [
            (cx - 5.4 * scale, cy - 1.4 * scale),
            (cx - 2.8 * scale, cy - 5.4 * scale),
            (cx, cy - 3.2 * scale),
            (cx + 2.8 * scale, cy - 5.4 * scale),
            (cx + 5.4 * scale, cy - 1.4 * scale),
            (cx, cy + 6.6 * scale),
            (cx - 5.4 * scale, cy - 1.4 * scale),
        ],
        outline,
        1.4,
    )


def draw_cute_icon(name: str, size: int) -> Image.Image:
    p = IconPainter(size, aa=5)
    cute_backplate(p, name)

    if name == "rectangle":
        p.rounded_rect((12.0, 15.0, 36.0, 33.0), 4.0, fill=(255, 255, 255, 190), outline=INK, width=2.2)
    elif name == "ellipse":
        p.ellipse((10.0, 13.0, 38.0, 35.0), fill=(255, 255, 255, 176), outline=INK, width=2.2)
    elif name == "text":
        p.text((24, 25.8), "A", 28, INK, bold=True)
        p.line([(15.5, 35.5), (32.5, 35.5)], PINK, 2.0)
    elif name == "number":
        p.ellipse((11.2, 11.2, 36.8, 36.8), fill=(255, 250, 214, 210), outline=INK, width=2.2)
        p.text((24.1, 24.6), "1", 20, INK, bold=True)
    elif name == "pen":
        p.polygon([(16.6, 33.9), (14.0, 31.3), (31.0, 14.3), (33.6, 16.9)], fill=(255, 213, 76, 255))
        p.line([(16.6, 33.9), (14.0, 31.3), (31.0, 14.3), (33.6, 16.9), (16.6, 33.9)], INK, 1.5)
        p.polygon([(20.5, 37.8), (16.6, 33.9), (33.6, 16.9), (37.5, 20.8)], fill=(255, 186, 214, 255))
        p.line([(20.5, 37.8), (16.6, 33.9), (33.6, 16.9), (37.5, 20.8), (20.5, 37.8)], INK, 1.5)
        p.polygon([(10.5, 39.5), (14.0, 31.3), (20.5, 37.8)], fill=(244, 205, 144, 255))
        p.line([(10.5, 39.5), (14.0, 31.3), (20.5, 37.8), (10.5, 39.5)], INK, 1.4)
        p.polygon([(10.5, 39.5), (12.3, 35.2), (14.7, 37.6)], fill=(45, 43, 48, 255))
        p.polygon([(31.0, 14.3), (34.8, 10.5), (41.3, 17.0), (37.5, 20.8)], fill=(255, 139, 190, 255))
        p.line([(31.0, 14.3), (34.8, 10.5), (41.3, 17.0), (37.5, 20.8)], INK, 1.4)
        p.line([(29.4, 15.9), (35.9, 22.4)], (206, 136, 178, 255), 1.3)
    elif name == "arrow":
        p.line([(13.5, 35.0), (34.7, 13.8)], BLUE_DARK, 3.0)
        p.line([(34.7, 13.8), (33.8, 26.2)], BLUE_DARK, 3.0)
        p.line([(34.7, 13.8), (22.5, 14.7)], BLUE_DARK, 3.0)
    elif name == "line":
        p.line([(14.5, 34.6), (33.5, 15.4)], LILAC, 3.0)
        p.line([(14.5, 34.6), (33.5, 15.4)], INK, 1.2)
    elif name == "dash":
        for a, b in [((14.0, 35.0), (18.3, 30.7)), ((22.0, 27.0), (26.3, 22.7)), ((30.0, 19.0), (34.2, 14.8))]:
            p.line([a, b], MINT, 3.0)
            p.line([a, b], INK, 1.1)
    elif name == "mosaic":
        p.rounded_rect((10.8, 10.8, 37.2, 37.2), 5.0, fill=(255, 255, 255, 235), outline=INK, width=1.6)
        cells = [
            (14.0, 14.0, 19.6, 19.6, (24, 24, 27, 255)),
            (20.2, 14.0, 25.8, 19.6, (248, 248, 248, 255)),
            (26.4, 14.0, 32.0, 19.6, (66, 66, 70, 255)),
            (14.0, 20.2, 19.6, 25.8, (236, 236, 236, 255)),
            (20.2, 20.2, 25.8, 25.8, (12, 12, 14, 255)),
            (26.4, 20.2, 32.0, 25.8, (210, 210, 210, 255)),
            (14.0, 26.4, 19.6, 32.0, (78, 78, 82, 255)),
            (20.2, 26.4, 25.8, 32.0, (246, 246, 246, 255)),
            (26.4, 26.4, 32.0, 32.0, (26, 26, 28, 255)),
        ]
        for x0, y0, x1, y1, color in cells:
            p.rounded_rect((x0, y0, x1, y1), 0.9, fill=color, outline=(130, 130, 135, 110), width=0.6)
    elif name == "gif":
        p.rounded_rect((9.8, 13.6, 38.2, 34.4), 6.2, fill=(255, 255, 255, 198), outline=INK, width=2.0)
        p.rounded_rect((12.2, 16.1, 35.8, 31.9), 4.5, fill=(255, 212, 232, 205), outline=None)
        p.text((24, 24.5), "GIF", 8, INK, bold=True)
    elif name == "pin":
        p.ellipse((16.0, 10.0, 32.0, 20.0), fill=(232, 232, 238, 255), outline=INK, width=1.8)
        p.rounded_rect((18.0, 17.0, 30.0, 21.6), 1.8, fill=(205, 205, 214, 255), outline=INK, width=1.2)
        p.line([(24.0, 21.0), (24.0, 35.0)], INK, 3.4)
        p.line([(24.0, 21.0), (24.0, 35.0)], (235, 235, 242, 255), 1.9)
        p.polygon([(20.7, 34.0), (27.3, 34.0), (24.0, 40.2)], fill=(64, 64, 70, 255))
        p.line([(20.7, 34.0), (27.3, 34.0), (24.0, 40.2), (20.7, 34.0)], INK, 1.0)
        p.ellipse((19.5, 12.2, 23.5, 15.8), fill=(255, 255, 255, 135), outline=None)
    elif name == "capture":
        c = BLUE
        p.rounded_rect((12.0, 12.0, 36.0, 36.0), 6.0, fill=(255, 255, 255, 110), outline=None)
        p.line([(13.0, 21.5), (13.0, 13.0), (21.5, 13.0)], c, 3.0)
        p.line([(27.0, 13.0), (35.0, 13.0), (35.0, 21.0)], c, 3.0)
        p.line([(35.0, 27.0), (35.0, 35.0), (27.0, 35.0)], c, 3.0)
        p.line([(21.5, 35.0), (13.0, 35.0), (13.0, 26.5)], c, 3.0)
    elif name == "undo":
        box = p.box((10.5, 12.0, 38.5, 36.5))
        p.draw.arc(box, start=132, end=348, fill=INK, width=max(1, p.p(3.4)))
        p.draw.arc(box, start=132, end=348, fill=HOT_PINK, width=max(1, p.p(2.0)))
        p.polygon([(12.6, 20.6), (23.7, 13.7), (22.3, 27.5)], fill=INK)
        p.polygon([(14.3, 20.7), (22.0, 15.9), (21.0, 25.7)], fill=HOT_PINK)
    elif name == "save":
        p.rounded_rect((12.0, 10.0, 36.0, 38.0), 4.0, fill=(255, 255, 255, 195), outline=INK, width=2.1)
        p.rounded_rect((16.8, 13.5, 31.2, 22.0), 2.0, fill=(178, 216, 255, 230), outline=INK, width=1.5)
        p.rounded_rect((17.4, 28.2, 30.6, 35.8), 1.7, fill=(255, 246, 228, 235), outline=INK, width=1.5)
    elif name == "cancel":
        p.line([(15.0, 15.0), (33.0, 33.0)], RED, 3.4)
        p.line([(33.0, 15.0), (15.0, 33.0)], RED, 3.4)
    elif name == "confirm":
        p.line([(12.8, 25.8), (21.0, 33.5), (36.0, 14.5)], BLUE_DARK, 3.7)
        p.line([(14.0, 24.8), (21.0, 31.4), (34.2, 15.2)], (255, 255, 255, 84), 1.3)
    else:
        raise ValueError(name)
    return p.finish()


def draw_state_icon(name: str, size: int, state: str) -> Image.Image:
    if state not in STATES:
        raise ValueError(f"Unknown icon state: {state}")

    base = draw_cute_icon(name, size).convert("RGBA")
    if state == "normal":
        return base

    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    radius = max(5, round(size * 0.24))
    pad = max(1, round(size * 0.045))

    if state == "hover":
        draw.rounded_rectangle(
            (pad, pad, size - pad - 1, size - pad - 1),
            radius=radius,
            fill=(255, 230, 244, 210),
            outline=(255, 126, 198, 225),
            width=max(1, round(size * 0.025)),
        )
        img.alpha_composite(base)
        return img

    down_offset = max(1, round(size * 0.045))
    draw.rounded_rectangle(
        (pad, pad, size - pad - 1, size - pad - 1),
        radius=radius,
        fill=(255, 188, 224, 225),
        outline=(202, 78, 147, 230),
        width=max(1, round(size * 0.03)),
    )
    img.alpha_composite(base, (0, down_offset))
    return img


def export_icons() -> list[dict[str, str | int]]:
    manifest = []
    for name, label in ICON_ORDER:
        for state in STATES:
            for size in [24, 32, 48]:
                image = draw_state_icon(name, size, state)
                path = ICON_ROOT / state / str(size) / f"{name}.png"
                image.save(path)
                manifest.append(
                    {
                        "name": name,
                        "label": label,
                        "state": state,
                        "size": size,
                        "path": str(path.relative_to(ROOT)).replace("\\", "/"),
                    }
                )
    return manifest


def make_toolbar() -> Path:
    scale = 4
    width, height = 930, 74
    img = Image.new("RGBA", (width * scale, height * scale), (0, 0, 0, 0))
    alpha_composite_rect(
        img,
        (7 * scale, 5 * scale, (width - 7) * scale, (height - 9) * scale),
        15 * scale,
        CREAM,
        outline=(255, 210, 234, 232),
        width=1 * scale,
        shadow=(160, 72, 126, 66),
        shadow_radius=11 * scale,
        shadow_offset=(0, 6 * scale),
    )
    draw = ImageDraw.Draw(img)
    x0, step, y = 37, 57, 36
    for i, (name, _) in enumerate(ICON_ORDER):
        bx = x0 + i * step
        if name in {"capture", "confirm"}:
            draw.rounded_rectangle(
                ((bx - 21) * scale, (y - 21) * scale, (bx + 21) * scale, (y + 21) * scale),
                radius=12 * scale,
                fill=(255, 218, 237, 176),
                outline=(255, 153, 208, 172),
                width=1 * scale,
            )
        icon = draw_state_icon(name, 38, "normal").resize((38 * scale, 38 * scale), Image.Resampling.LANCZOS)
        img.alpha_composite(icon, ((bx - 19) * scale, (y - 19) * scale))
    img = img.resize((width, height), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "toolbar.png"
    img.save(out)
    return out


def make_dimension_badge() -> Path:
    scale = 4
    width, height = 184, 54
    img = Image.new("RGBA", (width * scale, height * scale), (0, 0, 0, 0))
    alpha_composite_rect(
        img,
        (5 * scale, 6 * scale, (width - 5) * scale, 46 * scale),
        12 * scale,
        (0, 174, 242, 250),
        outline=(117, 224, 255, 248),
        width=1 * scale,
        shadow=(0, 92, 143, 78),
        shadow_radius=5 * scale,
        shadow_offset=(0, 3 * scale),
    )
    draw = ImageDraw.Draw(img)
    draw.ellipse((16 * scale, 12 * scale, 39 * scale, 35 * scale), fill=(255, 255, 255, 46))
    draw.text((width * scale / 2 + 4 * scale, 27 * scale), "1246*742", font=load_font(23 * scale, mono=True), fill=(255, 255, 255, 255), anchor="mm")
    img = img.resize((width, height), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "dimension-badge-1246x742.png"
    img.save(out)
    return out


def make_tooltip() -> Path:
    scale = 4
    width, height = 348, 88
    img = Image.new("RGBA", (width * scale, height * scale), (0, 0, 0, 0))
    alpha_composite_rect(
        img,
        (8 * scale, 8 * scale, (width - 8) * scale, (height - 10) * scale),
        13 * scale,
        (255, 246, 252, 240),
        outline=(255, 211, 235, 236),
        width=1 * scale,
        shadow=(160, 72, 126, 60),
        shadow_radius=9 * scale,
        shadow_offset=(0, 5 * scale),
    )
    draw = ImageDraw.Draw(img)
    draw.ellipse((21 * scale, 19 * scale, 39 * scale, 37 * scale), fill=(255, 142, 195, 165))
    cute_font = load_font(17 * scale)
    draw.text((width * scale / 2 + 9 * scale, 35 * scale), TOOLTIP.split("\n")[0], font=cute_font, fill=(72, 76, 90, 245), anchor="mm")
    draw.text((width * scale / 2 + 9 * scale, 58 * scale), TOOLTIP.split("\n")[1], font=cute_font, fill=(72, 76, 90, 245), anchor="mm")
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
    gd.rectangle((5 * scale, 5 * scale, (width - 5) * scale, (height - 5) * scale), outline=(0, 168, 241, 150), width=6 * scale)
    glow = glow.filter(ImageFilter.GaussianBlur(2.2 * scale))
    img.alpha_composite(glow)
    draw = ImageDraw.Draw(img)
    draw.rectangle((5 * scale, 5 * scale, (width - 5) * scale, (height - 5) * scale), outline=BLUE, width=4 * scale)
    for x, y in [
        (5, 5),
        (width // 2, 5),
        (width - 5, 5),
        (5, height // 2),
        (width - 5, height // 2),
        (5, height - 5),
        (width // 2, height - 5),
        (width - 5, height - 5),
    ]:
        draw.rounded_rectangle(
            ((x - 5) * scale, (y - 5) * scale, (x + 5) * scale, (y + 5) * scale),
            radius=4 * scale,
            fill=(255, 255, 255, 255),
            outline=(64, 201, 252, 255),
            width=1 * scale,
        )
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
    sd.polygon([(x + 3 * scale, y + 4 * scale) for x, y in pts_s], fill=(0, 0, 0, 72))
    img.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(2 * scale)))
    colors = [
        ((12, 8), (28, 29), (14, 44), (255, 82, 143, 255)),
        ((12, 8), (47, 31), (28, 29), (255, 220, 72, 255)),
        ((14, 44), (28, 29), (33, 50), (79, 222, 139, 255)),
        ((28, 29), (47, 31), (41, 46), (73, 157, 255, 255)),
        ((28, 29), (41, 46), (33, 50), (169, 118, 255, 255)),
    ]
    for a, b, c, color in colors:
        draw.polygon([(a[0] * scale, a[1] * scale), (b[0] * scale, b[1] * scale), (c[0] * scale, c[1] * scale)], fill=color)
    draw.line(pts_s + [pts_s[0]], fill=INK, width=2 * scale, joint="curve")
    draw.ellipse((20 * scale, 16 * scale, 26 * scale, 22 * scale), fill=(255, 255, 255, 120))
    img = img.resize((size, size), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "rainbow-cursor.png"
    img.save(out)
    return out


def make_icon_sheet() -> Path:
    cell, label_h, cols = 70, 24, 8
    rows = 2
    img = Image.new("RGBA", (cols * cell, rows * (cell + label_h)), (251, 249, 245, 255))
    draw = ImageDraw.Draw(img)
    font = load_font(10)
    for index, (name, label) in enumerate(ICON_ORDER):
        col = index % cols
        row = index // cols
        x, y = col * cell, row * (cell + label_h)
        draw.rounded_rectangle((x + 6, y + 6, x + cell - 6, y + cell - 6), radius=13, fill=(255, 255, 255, 255), outline=(232, 224, 214, 255))
        img.alpha_composite(draw_state_icon(name, 36, "normal"), (x + 17, y + 17))
        draw.text((x + cell / 2, y + cell + 11), label, font=font, fill=(86, 88, 99, 255), anchor="mm")
    out = PREVIEW_ROOT / "icon-sheet.png"
    img.save(out)
    return out


def make_icon_states_sheet() -> Path:
    cell, label_w, cols = 58, 74, len(ICON_ORDER)
    header_h, row_h = 28, 62
    img = Image.new("RGBA", (label_w + cols * cell, header_h + len(STATES) * row_h), (255, 248, 253, 255))
    draw = ImageDraw.Draw(img)
    font = load_font(10)
    state_font = load_font(12, bold=True)

    for index, (name, _) in enumerate(ICON_ORDER):
        x = label_w + index * cell + cell / 2
        draw.text((x, 15), name, font=font, fill=(92, 76, 104, 255), anchor="mm")

    for row, state in enumerate(STATES):
        y0 = header_h + row * row_h
        draw.text((label_w / 2, y0 + row_h / 2), state, font=state_font, fill=(122, 74, 105, 255), anchor="mm")
        for index, (name, _) in enumerate(ICON_ORDER):
            x0 = label_w + index * cell
            draw.rounded_rectangle(
                (x0 + 5, y0 + 6, x0 + cell - 5, y0 + row_h - 6),
                radius=12,
                fill=(255, 255, 255, 255),
                outline=(241, 210, 228, 255),
            )
            img.alpha_composite(draw_state_icon(name, 36, state), (x0 + 11, y0 + 13))

    out = PREVIEW_ROOT / "icon-states-sheet.png"
    img.save(out)
    return out


def make_mockup(toolbar: Path, badge: Path, tooltip: Path, cursor: Path, frame: Path) -> Path:
    width, height = 1706, 1279
    img = Image.new("RGBA", (width, height), (151, 150, 144, 255))
    draw = ImageDraw.Draw(img)
    for y in range(height):
        t = y / (height - 1)
        draw.line((0, y, width, y), fill=(int(171 - 38 * t), int(158 - 33 * t), int(166 - 38 * t), 255))

    draw.polygon([(0, 0), (1706, 0), (1706, 112), (0, 82)], fill=(37, 26, 22, 225))
    draw.polygon([(0, 1005), (1706, 905), (1706, 1279), (0, 1279)], fill=(26, 19, 16, 242))
    draw.rounded_rectangle((126, 1036, 1600, 1120), radius=12, fill=(19, 19, 20, 218))
    draw.text((188, 1078), "2600 x 1414\u50cf\u7d20", fill=(236, 230, 220, 170), font=load_font(18), anchor="lm")
    draw.text((1548, 1058), "100%", fill=(236, 230, 220, 170), font=load_font(17), anchor="lm")

    sx, sy, sw, sh = 364, 178, 866, 540
    screen = Image.new("RGBA", (sw, sh), (255, 250, 254, 255))
    sd = ImageDraw.Draw(screen)
    sd.rounded_rectangle((54, 68, 244, 102), radius=17, fill=(255, 201, 229, 55))
    sd.rounded_rectangle((520, 390, 758, 430), radius=20, fill=(231, 199, 255, 48))
    sd.rounded_rectangle((286, 222, 576, 270), radius=24, fill=(255, 226, 242, 50))
    img.alpha_composite(screen, (sx, sy))
    img.alpha_composite(Image.open(frame).convert("RGBA"), (sx - 5, sy - 5))
    img.alpha_composite(Image.open(badge).convert("RGBA"), (sx - 8, sy - 54))
    img.alpha_composite(Image.open(toolbar).convert("RGBA"), (302, sy + sh - 4))
    img.alpha_composite(Image.open(cursor).convert("RGBA"), (1316, 478))
    img.alpha_composite(Image.open(tooltip).convert("RGBA"), (1334, 528))

    out = PREVIEW_ROOT / "screenshot-ui-cute.png"
    img.convert("RGB").save(out, quality=96)
    return out


def write_manifest(paths: dict[str, Path], icons: list[dict[str, str | int]]) -> Path:
    manifest = {
        "base": "assets/screenshot_ui_exact",
        "intent": "Cute cartoon variant that keeps the reference layout and tool order.",
        "style": {
            "accent": "#00A6F1",
            "toolbar": "pink translucent anime-style panel, rounded, soft shadow",
            "icon_stroke": "#363A48",
            "icon_treatment": "flat rounded icons with pink/purple pastel backplates and clear tool silhouettes",
            "states": STATES,
            "state_layout": "assets/screenshot_ui_cute/icons/{state}/{size}/{name}.png",
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
    states_sheet = make_icon_states_sheet()
    mockup = make_mockup(toolbar, badge, tooltip, cursor, frame)
    manifest = write_manifest(
        {
            "toolbar": toolbar,
            "dimension_badge": badge,
            "hint_tooltip": tooltip,
            "selection_frame": frame,
            "rainbow_cursor": cursor,
            "icon_sheet": sheet,
            "icon_states_sheet": states_sheet,
            "mockup": mockup,
        },
        icons,
    )
    print(f"Generated cute reference set: {OUT}")
    print(f"Icons: {len(icons)} PNG files")
    print(f"Manifest: {manifest}")
    print(f"Mockup: {mockup}")


if __name__ == "__main__":
    main()
