#!/usr/bin/env python3
"""
Generate the Intern app icon.

Replicates the sidebar logo exactly:
  - Rounded rectangle with indigo gradient (#818cf8 -> #6366f1)
  - White Unicode clockwise arrow (U+21BB) centered, heavy weight

Uses macOS system font (Apple Symbols / SF Pro) to render the glyph,
then sips + iconutil to produce AppIcon.icns.
"""

import math
import os
import shutil
import subprocess
import sys

try:
    from PIL import Image, ImageDraw, ImageFilter, ImageFont
except ImportError:
    print("Pillow not found. Install with: pip3 install Pillow")
    sys.exit(1)

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ASSETS_DIR = os.path.join(REPO_ROOT, "macos-app", "Assets")
PNG_PATH = os.path.join(ASSETS_DIR, "AppIcon.png")
ICONSET_DIR = os.path.join(ASSETS_DIR, "AppIcon.iconset")
ICNS_PATH = os.path.join(ASSETS_DIR, "AppIcon.icns")

os.makedirs(ASSETS_DIR, exist_ok=True)

# ---------------------------------------------------------------------------
# Design constants (all at 1024x1024)
# ---------------------------------------------------------------------------

SIZE = 1024

# Indigo gradient colours (matches Color+Intern.swift)
COLOR_TL = (129, 140, 248)       # #818cf8 inAccent
COLOR_BR = (99, 102, 241)        # #6366f1 inAccentDeep

# Rounded rect
INSET = 60
CORNER_RADIUS = 220

# The glyph
GLYPH = "\u21BB"  # clockwise open circle arrow

# macOS system fonts that contain U+21BB
FONT_CANDIDATES = [
    "/System/Library/Fonts/SFPro.ttf",
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    "/System/Library/Fonts/Supplemental/Apple Symbols.ttf",
    "/Library/Fonts/Arial Unicode.ttf",
]

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def make_gradient(w, h, c1, c2):
    """Diagonal linear gradient top-left to bottom-right."""
    img = Image.new("RGBA", (w, h))
    px = img.load()
    for y in range(h):
        for x in range(w):
            t = (x / (w - 1) + y / (h - 1)) / 2.0
            r = int(c1[0] + (c2[0] - c1[0]) * t)
            g = int(c1[1] + (c2[1] - c1[1]) * t)
            b = int(c1[2] + (c2[2] - c1[2]) * t)
            px[x, y] = (r, g, b, 255)
    return img


def find_font(size):
    """Find a system font that can render U+21BB."""
    for path in FONT_CANDIDATES:
        if os.path.exists(path):
            try:
                font = ImageFont.truetype(path, size)
                # Verify the font can render the glyph (bbox not None)
                bbox = font.getbbox(GLYPH)
                if bbox and (bbox[2] - bbox[0]) > 0:
                    return font
            except Exception:
                continue
    return None


# ---------------------------------------------------------------------------
# Render
# ---------------------------------------------------------------------------

def render_icon():
    w = h = SIZE
    canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))

    # --- Drop shadow ---
    shadow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow)
    sd.rounded_rectangle(
        [INSET + 8, INSET + 18, w - INSET + 8, h - INSET + 18],
        radius=CORNER_RADIUS,
        fill=(20, 20, 40, 180),
    )
    canvas = Image.alpha_composite(canvas, shadow.filter(ImageFilter.GaussianBlur(28)))

    # --- Gradient rounded rectangle ---
    gradient = make_gradient(w, h, COLOR_TL, COLOR_BR)
    mask = Image.new("L", (w, h), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        [INSET, INSET, w - INSET, h - INSET],
        radius=CORNER_RADIUS,
        fill=255,
    )
    gradient.putalpha(mask)
    canvas = Image.alpha_composite(canvas, gradient)

    # --- Subtle inner highlight ---
    hl = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    ImageDraw.Draw(hl).rounded_rectangle(
        [INSET + 3, INSET + 3, w - INSET - 3, h - INSET - 3],
        radius=CORNER_RADIUS - 3,
        outline=(255, 255, 255, 28),
        width=4,
    )
    canvas = Image.alpha_composite(canvas, hl)

    # --- White glyph (U+21BB) ---
    # The SwiftUI sidebar renders this at ~47% of the container size in heavy weight.
    # At 1024px icon with 60px inset each side, the container is 904px.
    # 904 * 0.47 ~ 425px font size.
    glyph_size = 425
    font = find_font(glyph_size)

    if font:
        print(f"  Using font: {font.path}")
        glyph_layer = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        gd = ImageDraw.Draw(glyph_layer)

        # Get glyph bounding box for centering
        bbox = font.getbbox(GLYPH)
        gw = bbox[2] - bbox[0]
        gh = bbox[3] - bbox[1]
        gx = (w - gw) / 2 - bbox[0]
        gy = (h - gh) / 2 - bbox[1]

        gd.text((gx, gy), GLYPH, fill=(255, 255, 255, 255), font=font)

        # Clip to rounded rect
        from PIL import ImageChops
        r, g, b, a = glyph_layer.split()
        clipped_a = ImageChops.multiply(a, mask)
        glyph_layer = Image.merge("RGBA", (r, g, b, clipped_a))

        canvas = Image.alpha_composite(canvas, glyph_layer)
    else:
        print("  WARNING: No suitable font found for U+21BB.")
        print("  Falling back to arc-based drawing.")
        canvas = render_arc_fallback(canvas, mask)

    return canvas


def render_arc_fallback(canvas, mask):
    """Fallback: draw a circular arrow programmatically if no font has the glyph."""
    from PIL import ImageChops

    w = h = SIZE
    cx, cy = w // 2, h // 2 - 10
    radius, stroke = 130, 42

    arrow = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    ad = ImageDraw.Draw(arrow)

    # Arc ring
    inner_r, outer_r = radius - stroke // 2, radius + stroke // 2
    outer_pts, inner_pts = [], []
    for i in range(241):
        angle = math.radians(200 + i * (310 / 240))
        outer_pts.append((cx + outer_r * math.cos(angle), cy + outer_r * math.sin(angle)))
        inner_pts.append((cx + inner_r * math.cos(angle), cy + inner_r * math.sin(angle)))
    ad.polygon(outer_pts + list(reversed(inner_pts)), fill=(255, 255, 255, 255))

    # Arrowhead
    tip_rad = math.radians(150)
    tip_x = cx + radius * math.cos(tip_rad)
    tip_y = cy + radius * math.sin(tip_rad)
    tang = tip_rad + math.pi / 2
    norm_x, norm_y = math.cos(tip_rad), math.sin(tip_rad)
    base_x = tip_x - math.cos(tang) * 56
    base_y = tip_y - math.sin(tang) * 56
    ad.polygon([
        (tip_x, tip_y),
        (base_x - norm_x * 25, base_y - norm_y * 25),
        (base_x + norm_x * 25, base_y + norm_y * 25),
    ], fill=(255, 255, 255, 255))

    r, g, b, a = arrow.split()
    clipped_a = ImageChops.multiply(a, mask)
    arrow = Image.merge("RGBA", (r, g, b, clipped_a))
    return Image.alpha_composite(canvas, arrow)


# ---------------------------------------------------------------------------
# Iconset specs
# ---------------------------------------------------------------------------

ICONSET_SPECS = [
    ("icon_16x16.png", 16, 1),
    ("icon_16x16@2x.png", 16, 2),
    ("icon_32x32.png", 32, 1),
    ("icon_32x32@2x.png", 32, 2),
    ("icon_64x64.png", 64, 1),
    ("icon_64x64@2x.png", 64, 2),
    ("icon_128x128.png", 128, 1),
    ("icon_128x128@2x.png", 128, 2),
    ("icon_256x256.png", 256, 1),
    ("icon_256x256@2x.png", 256, 2),
    ("icon_512x512.png", 512, 1),
    ("icon_512x512@2x.png", 512, 2),
]


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    print("Rendering 1024x1024 master PNG...")
    master = render_icon()
    master.save(PNG_PATH, "PNG")
    print(f"Saved: {PNG_PATH}")

    # Build iconset
    if os.path.isdir(ICONSET_DIR):
        shutil.rmtree(ICONSET_DIR)
    os.makedirs(ICONSET_DIR)

    print(f"\nGenerating iconset...")
    for filename, logical, scale in ICONSET_SPECS:
        px = logical * scale
        img = master.resize((px, px), Image.LANCZOS)
        img.save(os.path.join(ICONSET_DIR, filename), "PNG")
        print(f"  {filename:30s}  {px}x{px}")

    # 512@2x is the 1024 master
    shutil.copy(PNG_PATH, os.path.join(ICONSET_DIR, "icon_512x512@2x.png"))

    # Run iconutil
    print(f"\nRunning iconutil...")
    result = subprocess.run(
        ["iconutil", "-c", "icns", ICONSET_DIR, "-o", ICNS_PATH],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        print(f"iconutil failed: {result.stderr}")
        sys.exit(1)

    print(f"Success: {ICNS_PATH}")
    shutil.rmtree(ICONSET_DIR)
    print("Done.")


if __name__ == "__main__":
    main()
