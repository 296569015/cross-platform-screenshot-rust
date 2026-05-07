from __future__ import annotations

import json
import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "assets" / "screenshot_ui"
ICON_ROOT = OUT / "icons"
COMPONENT_ROOT = OUT / "components"
PREVIEW_ROOT = OUT / "preview"

ACCENT = (0, 166, 241, 255)
ACCENT_DARK = (0, 132, 211, 255)
INK = (38, 39, 44, 238)
INK_SOFT = (54, 56, 64, 220)
RED = (225, 84, 87, 255)
WHITE = (255, 255, 255, 255)

TOOLTIP_TEXT = "\u8bf7\u5728\u622a\u56fe\u533a\u57df\u5185\u53cc\u51fb\u5b8c\u6210\u622a\u56fe\n\u70b9\u51fb\u53f3\u952e\u6216\u8005\u6309ESC\u9000\u51fa"


def ensure_dirs() -> None:
    for path in [
        ICON_ROOT / "24",
        ICON_ROOT / "32",
        ICON_ROOT / "48",
        COMPONENT_ROOT,
        PREVIEW_ROOT,
    ]:
        path.mkdir(parents=True, exist_ok=True)


def load_font(size: int, bold: bool = False, mono: bool = False) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    fonts = []
    if mono:
        fonts.extend(
            [
                "C:/Windows/Fonts/consola.ttf",
                "C:/Windows/Fonts/seguisym.ttf",
            ]
        )
    if bold:
        fonts.extend(
            [
                "C:/Windows/Fonts/msyhbd.ttc",
                "C:/Windows/Fonts/segoeuib.ttf",
                "C:/Windows/Fonts/arialbd.ttf",
            ]
        )
    fonts.extend(
        [
            "C:/Windows/Fonts/msyh.ttc",
            "C:/Windows/Fonts/segoeui.ttf",
            "C:/Windows/Fonts/arial.ttf",
        ]
    )
    for path in fonts:
        try:
            return ImageFont.truetype(path, size=size)
        except OSError:
            continue
    return ImageFont.load_default()


def alpha_composite_rect(
    image: Image.Image,
    xy: tuple[int, int, int, int],
    radius: int,
    fill: tuple[int, int, int, int],
    outline: tuple[int, int, int, int] | None = None,
    width: int = 1,
    shadow: tuple[int, int, int, int] | None = None,
    shadow_radius: int = 16,
    shadow_offset: tuple[int, int] = (0, 6),
) -> None:
    if shadow:
        sx0, sy0, sx1, sy1 = xy
        sx0 += shadow_offset[0]
        sx1 += shadow_offset[0]
        sy0 += shadow_offset[1]
        sy1 += shadow_offset[1]
        layer = Image.new("RGBA", image.size, (0, 0, 0, 0))
        sdraw = ImageDraw.Draw(layer)
        sdraw.rounded_rectangle((sx0, sy0, sx1, sy1), radius=radius, fill=shadow)
        layer = layer.filter(ImageFilter.GaussianBlur(shadow_radius))
        image.alpha_composite(layer)
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle(xy, radius=radius, fill=fill, outline=outline, width=width)


def text_size(draw: ImageDraw.ImageDraw, text: str, font: ImageFont.ImageFont) -> tuple[int, int]:
    box = draw.textbbox((0, 0), text, font=font)
    return box[2] - box[0], box[3] - box[1]


class IconPainter:
    def __init__(self, size: int, aa: int = 4):
        self.size = size
        self.aa = aa
        self.canvas_size = size * aa
        self.scale = self.canvas_size / 48.0
        self.image = Image.new("RGBA", (self.canvas_size, self.canvas_size), (0, 0, 0, 0))
        self.draw = ImageDraw.Draw(self.image)

    def p(self, value: float) -> int:
        return int(round(value * self.scale))

    def box(self, xy: tuple[float, float, float, float]) -> tuple[int, int, int, int]:
        return tuple(self.p(v) for v in xy)  # type: ignore[return-value]

    def line(
        self,
        points: list[tuple[float, float]],
        fill: tuple[int, int, int, int] = INK,
        width: float = 2.7,
        joint: str = "curve",
    ) -> None:
        pts = [(self.p(x), self.p(y)) for x, y in points]
        self.draw.line(pts, fill=fill, width=max(1, self.p(width)), joint=joint)
        radius = max(1, self.p(width) // 2)
        for x, y in pts:
            self.draw.ellipse((x - radius, y - radius, x + radius, y + radius), fill=fill)

    def rounded_rect(
        self,
        xy: tuple[float, float, float, float],
        radius: float,
        fill: tuple[int, int, int, int] | None = None,
        outline: tuple[int, int, int, int] | None = INK,
        width: float = 2.6,
    ) -> None:
        self.draw.rounded_rectangle(
            self.box(xy),
            radius=self.p(radius),
            fill=fill,
            outline=outline,
            width=max(1, self.p(width)),
        )

    def ellipse(
        self,
        xy: tuple[float, float, float, float],
        fill: tuple[int, int, int, int] | None = None,
        outline: tuple[int, int, int, int] | None = INK,
        width: float = 2.6,
    ) -> None:
        self.draw.ellipse(self.box(xy), fill=fill, outline=outline, width=max(1, self.p(width)))

    def polygon(self, points: list[tuple[float, float]], fill: tuple[int, int, int, int]) -> None:
        self.draw.polygon([(self.p(x), self.p(y)) for x, y in points], fill=fill)

    def text(
        self,
        xy: tuple[float, float],
        value: str,
        size: int,
        fill: tuple[int, int, int, int] = INK,
        bold: bool = False,
        anchor: str = "mm",
        mono: bool = False,
    ) -> None:
        font = load_font(self.p(size), bold=bold, mono=mono)
        self.draw.text((self.p(xy[0]), self.p(xy[1])), value, font=font, fill=fill, anchor=anchor)

    def finish(self) -> Image.Image:
        return self.image.resize((self.size, self.size), Image.Resampling.LANCZOS)


def draw_icon(name: str, size: int) -> Image.Image:
    p = IconPainter(size)

    if name == "rectangle":
        p.rounded_rect((10.5, 13.5, 37.5, 34.5), 4, outline=INK)
    elif name == "ellipse":
        p.ellipse((9.5, 12.5, 38.5, 35.5), outline=INK)
    elif name == "text":
        p.text((24, 24.5), "A", 30, INK, bold=False)
        p.line([(15, 36), (33, 36)], INK, 2.2)
    elif name == "number":
        p.ellipse((10.5, 10.5, 37.5, 37.5), outline=INK)
        p.text((24, 24.5), "1", 22, INK, bold=True)
    elif name == "pen":
        p.line([(14, 34), (29.5, 18.5)], INK, 3.0)
        p.polygon([(29.5, 18.5), (34.8, 13.2), (37.2, 15.6), (32.0, 21.0)], INK_SOFT)
        p.line([(11.5, 37), (18, 35.7)], INK, 2.2)
    elif name == "arrow":
        p.line([(13.5, 34.5), (35, 13)], INK, 2.7)
        p.line([(35, 13), (34, 25.5)], INK, 2.7)
        p.line([(35, 13), (22.5, 14)], INK, 2.7)
    elif name == "line":
        p.line([(14, 35), (34, 13)], INK, 2.7)
    elif name == "dash":
        for a, b in [((14, 35), (18, 31)), ((22, 27), (26, 23)), ((30, 19), (34, 15))]:
            p.line([a, b], INK, 2.7)
    elif name == "mosaic":
        p.rounded_rect((12.5, 12.5, 35.5, 35.5), 4, outline=INK, width=2.4)
        p.rounded_rect((16, 16, 23.2, 23.2), 1.5, fill=INK_SOFT, outline=None)
        p.rounded_rect((24.8, 16, 32, 23.2), 1.5, fill=(0, 0, 0, 0), outline=INK_SOFT, width=1.8)
        p.rounded_rect((16, 24.8, 23.2, 32), 1.5, fill=(0, 0, 0, 0), outline=INK_SOFT, width=1.8)
        p.rounded_rect((24.8, 24.8, 32, 32), 1.5, fill=INK_SOFT, outline=None)
    elif name == "gif":
        p.rounded_rect((9.5, 13.5, 38.5, 34.5), 5, outline=INK, width=2.3)
        p.text((24, 24.7), "GIF", 10, INK, bold=True)
    elif name == "pin":
        p.polygon([(21.5, 10.5), (35.5, 24.5), (31.8, 28.2), (17.8, 14.2)], INK_SOFT)
        p.line([(16, 16), (32, 32)], INK, 2.5)
        p.line([(22.5, 27.5), (13.5, 36.5)], INK, 2.5)
        p.line([(28, 20), (20, 28)], WHITE, 1.5)
    elif name == "focus":
        color = ACCENT
        p.line([(13, 22), (13, 13), (22, 13)], color, 2.8)
        p.line([(26, 13), (35, 13), (35, 22)], color, 2.8)
        p.line([(35, 26), (35, 35), (26, 35)], color, 2.8)
        p.line([(22, 35), (13, 35), (13, 26)], color, 2.8)
        p.line([(24, 17), (24, 31)], color, 1.6)
        p.line([(17, 24), (31, 24)], color, 1.6)
    elif name == "undo":
        box = p.box((12, 13, 38, 35))
        p.draw.arc(box, start=142, end=348, fill=INK, width=max(1, p.p(2.6)))
        p.polygon([(14, 21), (23, 14.5), (23, 27.5)], INK)
    elif name == "redo":
        box = p.box((10, 13, 36, 35))
        p.draw.arc(box, start=192, end=38, fill=INK, width=max(1, p.p(2.6)))
        p.polygon([(34, 21), (25, 14.5), (25, 27.5)], INK)
    elif name == "save":
        p.rounded_rect((12, 10, 36, 38), 3.5, outline=INK, width=2.5)
        p.rounded_rect((17, 13.5, 31, 21.5), 1.5, outline=INK, width=2.1)
        p.rounded_rect((17, 27, 31, 36), 1.4, outline=INK, width=2.0)
        p.line([(29, 13.5), (29, 20.5)], INK, 2.1)
    elif name == "cancel":
        p.line([(15, 15), (33, 33)], RED, 2.9)
        p.line([(33, 15), (15, 33)], RED, 2.9)
    elif name == "confirm":
        p.line([(13, 25.5), (21.2, 33.5), (35.5, 15)], ACCENT_DARK, 3.2)
    elif name == "copy":
        p.rounded_rect((17, 12, 36, 31), 3, outline=INK, width=2.2)
        p.rounded_rect((12, 17, 31, 36), 3, outline=INK, width=2.2)
    elif name == "longshot":
        p.rounded_rect((15, 8.5, 33, 39.5), 4, outline=INK, width=2.4)
        p.line([(20, 16), (28, 16)], INK, 2.2)
        p.line([(20, 22), (28, 22)], INK, 2.2)
        p.line([(24, 27), (24, 34), (20, 30)], ACCENT_DARK, 2.4)
        p.line([(24, 34), (28, 30)], ACCENT_DARK, 2.4)
    else:
        raise ValueError(f"Unknown icon: {name}")

    return p.finish()


ICON_ORDER = [
    ("rectangle", "Rectangle annotation"),
    ("ellipse", "Ellipse annotation"),
    ("text", "Text annotation"),
    ("number", "Number marker"),
    ("pen", "Freehand pen"),
    ("arrow", "Arrow annotation"),
    ("line", "Straight line"),
    ("dash", "Dotted line"),
    ("mosaic", "Mosaic blur"),
    ("gif", "GIF capture"),
    ("pin", "Pin to screen"),
    ("focus", "Selection focus"),
    ("undo", "Undo"),
    ("redo", "Redo"),
    ("save", "Save"),
    ("copy", "Copy"),
    ("longshot", "Long screenshot"),
    ("cancel", "Cancel"),
    ("confirm", "Confirm"),
]


def export_icons() -> list[dict[str, str | int]]:
    manifest = []
    for name, label in ICON_ORDER:
        for size in [24, 32, 48]:
            image = draw_icon(name, size)
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


def make_dimension_badge() -> Path:
    scale = 3
    w, h = 186 * scale, 48 * scale
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    alpha_composite_rect(
        img,
        (8 * scale, 7 * scale, 178 * scale, 42 * scale),
        7 * scale,
        (0, 174, 244, 248),
        outline=(50, 206, 255, 255),
        width=1 * scale,
        shadow=(0, 80, 130, 80),
        shadow_radius=5 * scale,
        shadow_offset=(0, 2 * scale),
    )
    draw = ImageDraw.Draw(img)
    font = load_font(22 * scale, bold=False, mono=True)
    draw.text((93 * scale, 24 * scale), "1246*742", fill=WHITE, font=font, anchor="mm")
    img = img.resize((186, 48), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "dimension-badge-1246x742.png"
    img.save(out)
    return out


def make_tooltip() -> Path:
    scale = 3
    w, h = 366 * scale, 92 * scale
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    alpha_composite_rect(
        img,
        (10 * scale, 10 * scale, 356 * scale, 78 * scale),
        8 * scale,
        (246, 246, 246, 236),
        outline=(255, 255, 255, 210),
        width=1 * scale,
        shadow=(0, 0, 0, 70),
        shadow_radius=11 * scale,
        shadow_offset=(0, 5 * scale),
    )
    draw = ImageDraw.Draw(img)
    font = load_font(20 * scale)
    lines = TOOLTIP_TEXT.split("\n")
    y = 31 * scale
    for line in lines:
        draw.text((183 * scale, y), line, fill=(76, 76, 76, 242), font=font, anchor="mm")
        y += 24 * scale
    img = img.resize((366, 92), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "hint-tooltip.png"
    img.save(out)
    return out


def make_cursor() -> Path:
    scale = 4
    size = 64 * scale
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    pts = [(12, 8), (14, 44), (25, 34), (33, 50), (41, 46), (33, 31), (47, 31)]
    pts = [(x * scale, y * scale) for x, y in pts]
    shadow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow)
    sd.polygon([(x + 3 * scale, y + 4 * scale) for x, y in pts], fill=(0, 0, 0, 70))
    shadow = shadow.filter(ImageFilter.GaussianBlur(2 * scale))
    img.alpha_composite(shadow)
    colors = [
        ((12, 8), (28, 29), (14, 44), (255, 43, 126, 255)),
        ((12, 8), (47, 31), (28, 29), (251, 216, 36, 255)),
        ((14, 44), (28, 29), (33, 50), (73, 226, 109, 255)),
        ((28, 29), (47, 31), (41, 46), (50, 141, 255, 255)),
        ((28, 29), (41, 46), (33, 50), (156, 73, 255, 255)),
    ]
    for a, b, c, color in colors:
        draw.polygon([(a[0] * scale, a[1] * scale), (b[0] * scale, b[1] * scale), (c[0] * scale, c[1] * scale)], fill=color)
    draw.line(pts + [pts[0]], fill=(30, 30, 35, 255), width=2 * scale, joint="curve")
    img = img.resize((64, 64), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "rainbow-cursor.png"
    img.save(out)
    return out


def make_selection_frame() -> Path:
    scale = 2
    w, h = 900 * scale, 540 * scale
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    glow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    gd.rectangle((8 * scale, 8 * scale, w - 8 * scale, h - 8 * scale), outline=(0, 166, 241, 150), width=5 * scale)
    glow = glow.filter(ImageFilter.GaussianBlur(2.2 * scale))
    img.alpha_composite(glow)
    draw = ImageDraw.Draw(img)
    draw.rectangle((8 * scale, 8 * scale, w - 8 * scale, h - 8 * scale), outline=ACCENT, width=3 * scale)
    handle_fill = (255, 255, 255, 245)
    handle_outline = (0, 150, 224, 255)
    for x, y in [
        (8, 8),
        (450, 8),
        (892, 8),
        (8, 270),
        (892, 270),
        (8, 532),
        (450, 532),
        (892, 532),
    ]:
        draw.rounded_rectangle(
            ((x - 4) * scale, (y - 4) * scale, (x + 4) * scale, (y + 4) * scale),
            radius=2 * scale,
            fill=handle_fill,
            outline=handle_outline,
            width=1 * scale,
        )
    img = img.resize((900, 540), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "selection-frame.png"
    img.save(out)
    return out


def make_toolbar() -> Path:
    icon_names = [
        "rectangle",
        "ellipse",
        "text",
        "number",
        "pen",
        "arrow",
        "line",
        "dash",
        "mosaic",
        "gif",
        "pin",
        "focus",
        "undo",
        "save",
        "cancel",
        "confirm",
    ]
    button = 44
    gap = 2
    pad_x = 18
    pad_y = 10
    separators_after = {7, 10, 13}
    sep_w = 12
    width = pad_x * 2 + len(icon_names) * button + (len(icon_names) - 1) * gap + len(separators_after) * sep_w
    height = 68
    scale = 3
    img = Image.new("RGBA", (width * scale, height * scale), (0, 0, 0, 0))
    alpha_composite_rect(
        img,
        (7 * scale, 6 * scale, (width - 7) * scale, (height - 8) * scale),
        10 * scale,
        (247, 246, 243, 234),
        outline=(255, 255, 255, 235),
        width=1 * scale,
        shadow=(0, 0, 0, 80),
        shadow_radius=11 * scale,
        shadow_offset=(0, 5 * scale),
    )
    draw = ImageDraw.Draw(img)
    x = pad_x
    for i, name in enumerate(icon_names):
        bx = x * scale
        by = pad_y * scale
        if name == "focus":
            draw.rounded_rectangle(
                (bx + 4 * scale, by + 3 * scale, bx + (button - 4) * scale, by + (button - 3) * scale),
                radius=8 * scale,
                fill=(223, 246, 255, 180),
                outline=(98, 206, 248, 180),
                width=1 * scale,
            )
        if name in {"cancel", "confirm"}:
            draw.rounded_rectangle(
                (bx + 5 * scale, by + 4 * scale, bx + (button - 5) * scale, by + (button - 4) * scale),
                radius=7 * scale,
                fill=(255, 255, 255, 72),
            )
        icon = draw_icon(name, 30).resize((30 * scale, 30 * scale), Image.Resampling.LANCZOS)
        img.alpha_composite(icon, (int(bx + 7 * scale), int(by + 7 * scale)))
        x += button + gap
        if i in separators_after:
            sx = x + 4
            draw.line(
                [(sx * scale, 19 * scale), (sx * scale, 49 * scale)],
                fill=(180, 180, 180, 120),
                width=max(1, scale),
            )
            x += sep_w
    img = img.resize((width, height), Image.Resampling.LANCZOS)
    out = COMPONENT_ROOT / "toolbar.png"
    img.save(out)
    return out


def make_icon_sheet() -> Path:
    size = 64
    cols = 7
    rows = math.ceil(len(ICON_ORDER) / cols)
    label_h = 22
    w = cols * size
    h = rows * (size + label_h)
    img = Image.new("RGBA", (w, h), (248, 249, 250, 255))
    draw = ImageDraw.Draw(img)
    label_font = load_font(10)
    for index, (name, _) in enumerate(ICON_ORDER):
        col = index % cols
        row = index // cols
        x = col * size
        y = row * (size + label_h)
        draw.rounded_rectangle((x + 6, y + 6, x + size - 6, y + size - 6), radius=8, fill=(255, 255, 255, 255), outline=(223, 226, 230, 255))
        icon = draw_icon(name, 32)
        img.alpha_composite(icon, (x + 16, y + 16))
        draw.text((x + size / 2, y + size + 10), name, font=label_font, fill=(82, 85, 92, 255), anchor="mm")
    out = PREVIEW_ROOT / "icon-sheet.png"
    img.save(out)
    return out


def make_preview(toolbar: Path, badge: Path, tooltip: Path, cursor: Path) -> Path:
    w, h = 1600, 1000
    img = Image.new("RGBA", (w, h), (0, 0, 0, 255))
    draw = ImageDraw.Draw(img)

    for y in range(h):
        t = y / max(1, h - 1)
        r = int(206 - 22 * t)
        g = int(204 - 21 * t)
        b = int(198 - 18 * t)
        draw.line([(0, y), (w, y)], fill=(r, g, b, 255))

    # Subtle desktop hints under the dimming layer.
    desktop = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    desktop_draw = ImageDraw.Draw(desktop)
    for i in range(18):
        x = 80 + (i * 137) % 1460
        y = 82 + (i * 79) % 790
        color = (255, 255, 255, 18 + (i % 4) * 5)
        desktop_draw.rounded_rectangle((x, y, x + 180 + (i % 3) * 80, y + 68), radius=12, fill=color)
    img.alpha_composite(desktop)

    dim = Image.new("RGBA", (w, h), (0, 0, 0, 78))
    img.alpha_composite(dim)

    sx, sy, sw, sh = 300, 178, 820, 536
    # Simulated selected screenshot content.
    selected = Image.new("RGBA", (sw, sh), (248, 247, 243, 255))
    sd = ImageDraw.Draw(selected)
    for y in range(sh):
        t = y / max(1, sh - 1)
        shade = 248 - int(5 * t)
        sd.line([(0, y), (sw, y)], fill=(shade, shade, 244, 255))
    detail = Image.new("RGBA", (sw, sh), (0, 0, 0, 0))
    dd = ImageDraw.Draw(detail)
    for i in range(20):
        x = (i * 151) % sw
        y = (i * 73) % sh
        dd.rounded_rectangle((x, y, x + 120 + (i % 5) * 24, y + 36), radius=7, fill=(150, 150, 146, 38))
    selected.alpha_composite(detail)
    img.alpha_composite(selected, (sx, sy))

    frame_glow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    fg = ImageDraw.Draw(frame_glow)
    fg.rectangle((sx, sy, sx + sw, sy + sh), outline=(0, 166, 241, 150), width=7)
    frame_glow = frame_glow.filter(ImageFilter.GaussianBlur(2.5))
    img.alpha_composite(frame_glow)
    draw = ImageDraw.Draw(img)
    draw.rectangle((sx, sy, sx + sw, sy + sh), outline=ACCENT, width=4)

    # Handles on the outer capture canvas, as seen in desktop screenshot tools.
    for x, y in [
        (sx, sy),
        (sx + sw // 2, sy),
        (sx + sw, sy),
        (sx, sy + sh // 2),
        (sx + sw, sy + sh // 2),
        (sx, sy + sh),
        (sx + sw // 2, sy + sh),
        (sx + sw, sy + sh),
    ]:
        draw.rounded_rectangle((x - 5, y - 5, x + 5, y + 5), radius=2, fill=(255, 255, 255, 250), outline=ACCENT_DARK)

    badge_img = Image.open(badge).convert("RGBA")
    img.alpha_composite(badge_img, (sx - 12, sy - 48))

    toolbar_img = Image.open(toolbar).convert("RGBA")
    tx = sx + (sw - toolbar_img.width) // 2
    img.alpha_composite(toolbar_img, (tx, sy + sh - 4))

    tooltip_img = Image.open(tooltip).convert("RGBA")
    img.alpha_composite(tooltip_img, (sx + sw + 68, sy + 300))

    cursor_img = Image.open(cursor).convert("RGBA")
    img.alpha_composite(cursor_img, (sx + sw + 44, sy + 272))

    # Bottom status strip hint, kept quiet so the capture UI remains primary.
    status = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    status_draw = ImageDraw.Draw(status)
    status_draw.rounded_rectangle((42, h - 78, w - 42, h - 24), radius=14, fill=(0, 0, 0, 145))
    img.alpha_composite(status)
    draw = ImageDraw.Draw(img)
    font = load_font(18)
    draw.text((72, h - 51), "2600 x 1414", fill=(255, 255, 255, 155), font=font, anchor="lm")
    draw.text((w - 154, h - 51), "100%", fill=(255, 255, 255, 155), font=font, anchor="lm")

    out = PREVIEW_ROOT / "screenshot-ui-mockup.png"
    img.convert("RGB").save(out, quality=96)
    return out


def write_manifest(paths: dict[str, Path], icons: list[dict[str, str | int]]) -> Path:
    manifest = {
        "style": {
            "accent": "#00A6F1",
            "accent_dark": "#0084D3",
            "toolbar_background": "rgba(247,246,243,0.92)",
            "corner_radius_px": 8,
            "reference": "wechat screenshot reference supplied by user",
        },
        "components": {name: str(path.relative_to(ROOT)).replace("\\", "/") for name, path in paths.items()},
        "icons": icons,
    }
    out = OUT / "manifest.json"
    out.write_text(json.dumps(manifest, indent=2, ensure_ascii=True), encoding="utf-8")
    return out


def main() -> None:
    from generate_reference_cute_ui_assets import main as generate_cute_assets

    generate_cute_assets()


if __name__ == "__main__":
    main()
