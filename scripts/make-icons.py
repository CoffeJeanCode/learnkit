"""Generate the app icon source (1024x1024 RGBA) that `bunx tauri icon` fans out.

Kept in the repo so the icon can be regenerated instead of edited by hand:
    python scripts/make-icons.py
    bunx tauri icon src-tauri/icons/source-1024.png

RGBA matters: `tauri::generate_context!` refuses RGB-only PNGs on Linux
("icon ... is not RGBA"), which is what the previous placeholder icons were.
Colors come from src/styles.css (paper-deep, chalk, sky).
"""

from PIL import Image, ImageDraw

SIZE = 1024
OUT = "src-tauri/icons/source-1024.png"

PAPER_DEEP = (18, 20, 23, 255)  # --paper-deep
CHALK = (232, 226, 212, 255)  # body text
SKY = (56, 189, 248, 255)  # --marker / arrow-sky


def main() -> None:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    margin = 56
    d.rounded_rectangle(
        [margin, margin, SIZE - margin, SIZE - margin],
        radius=200,
        fill=PAPER_DEEP,
        outline=CHALK,
        width=40,
    )

    # Notebook: cover, spine, three ruled lines.
    left, top, right, bottom = 300, 236, 724, 788
    d.rounded_rectangle([left, top, right, bottom], radius=56, outline=CHALK, width=40)
    d.line([(left + 96, top + 8), (left + 96, bottom - 8)], fill=CHALK, width=40)
    for y in (388, 512, 636):
        d.line([(left + 176, y), (right - 72, y)], fill=SKY, width=34)

    img.save(OUT)
    print(f"wrote {OUT} {img.size} {img.mode}")


if __name__ == "__main__":
    main()
