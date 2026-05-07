from __future__ import annotations

import argparse
import csv
from pathlib import Path

import cv2
import numpy as np
from PIL import (
    Image,
    ImageDraw,
    ImageEnhance,
    ImageFilter,
    ImageOps,
)
from tqdm import tqdm


IMAGE_EXTS = {".jpg", ".jpeg", ".png", ".webp", ".bmp", ".tif", ".tiff"}


def smoothstep(edge0: float, edge1: float, x: np.ndarray) -> np.ndarray:
    t = np.clip((x - edge0) / max(edge1 - edge0, 1e-6), 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def load_image(path: Path) -> Image.Image:
    img = Image.open(path)
    img = ImageOps.exif_transpose(img)

    if img.mode in ("RGBA", "LA"):
        base = Image.new("RGBA", img.size, (22, 20, 15, 255))
        base.alpha_composite(img.convert("RGBA"))
        return base.convert("RGB")

    return img.convert("RGB")


def cv2_imread_unicode(path: Path):
    data = np.fromfile(str(path), dtype=np.uint8)
    return cv2.imdecode(data, cv2.IMREAD_COLOR)


def calc_sharpness(path: Path) -> float:
    img = cv2_imread_unicode(path)
    if img is None:
        return 0.0

    gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
    h, w = gray.shape[:2]

    max_side = 1200
    scale = min(1.0, max_side / max(w, h))
    if scale < 1:
        gray = cv2.resize(
            gray,
            (int(w * scale), int(h * scale)),
            interpolation=cv2.INTER_AREA,
        )

    return float(cv2.Laplacian(gray, cv2.CV_64F).var())


def fit_cover(img: Image.Image, target_w: int, target_h: int) -> Image.Image:
    w, h = img.size
    scale = max(target_w / w, target_h / h)
    new_w = max(1, int(round(w * scale)))
    new_h = max(1, int(round(h * scale)))

    resized = img.resize((new_w, new_h), Image.Resampling.LANCZOS)

    left = (new_w - target_w) // 2
    top = (new_h - target_h) // 2

    return resized.crop((left, top, left + target_w, top + target_h))


def decide_preset(
    width: int,
    height: int,
    sharpness: float,
    target_w: int,
    target_h: int,
):
    quality_score = min(width / target_w, height / target_h)

    # 高清图，可以稍微大一些
    if quality_score >= 0.72 and sharpness >= 80:
        return {
            "name": "large",
            "box_w": 0.92,
            "box_h": 0.92,
            "max_upscale": 1.35,
            "bg_blur": 34,
            "bg_brightness": 0.72,
            "main_shadow": 95,
        }

    # 中等图，做成主视觉画片
    if quality_score >= 0.45 or sharpness >= 55:
        return {
            "name": "poster",
            "box_w": 0.78,
            "box_h": 0.78,
            "max_upscale": 1.22,
            "bg_blur": 42,
            "bg_brightness": 0.70,
            "main_shadow": 110,
        }

    # 低清图，不要硬拉大
    return {
        "name": "card",
        "box_w": 0.62,
        "box_h": 0.68,
        "max_upscale": 1.08,
        "bg_blur": 54,
        "bg_brightness": 0.66,
        "main_shadow": 120,
    }


def fit_main_image(
    img: Image.Image,
    target_w: int,
    target_h: int,
    preset: dict,
) -> Image.Image:
    w, h = img.size

    max_w = target_w * preset["box_w"]
    max_h = target_h * preset["box_h"]

    scale = min(
        max_w / w,
        max_h / h,
        preset["max_upscale"],
    )

    new_w = max(1, int(round(w * scale)))
    new_h = max(1, int(round(h * scale)))

    main = img.resize((new_w, new_h), Image.Resampling.LANCZOS)

    return main


def enhance_main(main: Image.Image, sharpness: float) -> Image.Image:
    # 只做轻微增强，不做暴力锐化
    main = ImageEnhance.Contrast(main).enhance(1.03)
    main = ImageEnhance.Color(main).enhance(1.02)

    if sharpness < 70:
        main = main.filter(
            ImageFilter.UnsharpMask(
                radius=0.8,
                percent=42,
                threshold=6,
            )
        )

    return main


def make_feather_mask(size: tuple[int, int], preset_name: str) -> Image.Image:
    w, h = size
    short_side = max(1, min(w, h))

    if preset_name == "large":
        feather = max(16, short_side // 55)
    elif preset_name == "poster":
        feather = max(18, short_side // 45)
    else:
        feather = max(20, short_side // 34)

    mask = Image.new("L", (w, h), 0)
    draw = ImageDraw.Draw(mask)

    inset = feather
    draw.rounded_rectangle(
        (inset, inset, w - inset, h - inset),
        radius=max(18, feather * 2),
        fill=255,
    )

    return mask.filter(ImageFilter.GaussianBlur(feather))


def make_overlay(target_w: int, target_h: int) -> Image.Image:
    y = np.linspace(0, 1, target_h, dtype=np.float32)[:, None]
    x = np.linspace(-1, 1, target_w, dtype=np.float32)[None, :]

    abs_x = np.abs(x)
    yy = np.repeat(y, target_w, axis=1)
    xx = np.repeat(x, target_h, axis=0)

    # 边缘暗角
    radial = np.sqrt((xx * 0.92) ** 2 + ((yy - 0.48) * 1.12) ** 2)
    radial_alpha = smoothstep(0.48, 1.08, radial) * 92

    # 左右柔雾暗幕
    side_alpha = smoothstep(0.52, 1.0, np.repeat(abs_x, target_h, axis=0)) * 76

    # 底部压暗，给音乐条和 UI 留空间
    bottom_alpha = smoothstep(0.50, 1.0, yy) * 82

    # 顶部暖光
    top_warm_alpha = (1.0 - yy) ** 2 * 24

    overlay = Image.new("RGBA", (target_w, target_h), (0, 0, 0, 0))

    def add_layer(color: tuple[int, int, int], alpha_arr: np.ndarray):
        alpha = np.clip(alpha_arr, 0, 255).astype(np.uint8)
        rgba = np.zeros((target_h, target_w, 4), dtype=np.uint8)
        rgba[..., 0] = color[0]
        rgba[..., 1] = color[1]
        rgba[..., 2] = color[2]
        rgba[..., 3] = alpha
        layer = Image.fromarray(rgba, "RGBA")
        overlay.alpha_composite(layer)

    add_layer((30, 26, 18), radial_alpha)
    add_layer((34, 29, 20), side_alpha)
    add_layer((0, 0, 0), bottom_alpha)
    add_layer((245, 225, 188), top_warm_alpha)

    return overlay


def add_noise(img: Image.Image, strength: float = 2.4) -> Image.Image:
    arr = np.asarray(img).astype(np.int16)
    noise = np.random.normal(0, strength, arr.shape).astype(np.int16)
    arr = np.clip(arr + noise, 0, 255).astype(np.uint8)
    return Image.fromarray(arr, "RGB")


def process_one(
    input_path: Path,
    output_path: Path,
    target_w: int,
    target_h: int,
):
    src = load_image(input_path)
    src_w, src_h = src.size
    sharpness = calc_sharpness(input_path)

    preset = decide_preset(src_w, src_h, sharpness, target_w, target_h)

    # 底层：放大模糊氛围背景
    bg = fit_cover(src, target_w, target_h)
    bg = ImageEnhance.Brightness(bg).enhance(preset["bg_brightness"])
    bg = ImageEnhance.Color(bg).enhance(0.88)
    bg = bg.filter(ImageFilter.GaussianBlur(preset["bg_blur"]))

    canvas = bg.convert("RGBA")

    # 中层：主图，不强行铺满
    main = fit_main_image(src, target_w, target_h, preset)
    main = enhance_main(main, sharpness)
    mask = make_feather_mask(main.size, preset["name"])

    main_w, main_h = main.size
    x = (target_w - main_w) // 2
    y = (target_h - main_h) // 2

    # 主图轻阴影
    shadow_layer = Image.new("RGBA", (target_w, target_h), (0, 0, 0, 0))
    shadow_mask = Image.new("L", (target_w, target_h), 0)
    shadow_mask.paste(mask, (x, y))
    shadow_mask = shadow_mask.filter(ImageFilter.GaussianBlur(24))

    shadow_color = Image.new(
        "RGBA",
        (target_w, target_h),
        (0, 0, 0, preset["main_shadow"]),
    )
    shadow_layer = Image.composite(shadow_color, shadow_layer, shadow_mask)
    canvas.alpha_composite(shadow_layer)

    # 贴主图
    main_layer = Image.new("RGBA", (target_w, target_h), (0, 0, 0, 0))
    main_layer.paste(main.convert("RGBA"), (x, y), mask)
    canvas.alpha_composite(main_layer)

    # 顶层：柔雾羽化边缘
    overlay = make_overlay(target_w, target_h)
    canvas.alpha_composite(overlay)

    final = canvas.convert("RGB")
    final = add_noise(final, strength=1.6)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    final.save(output_path, quality=95, subsampling=0, optimize=True)

    return {
        "file": str(input_path),
        "output": str(output_path),
        "src_width": src_w,
        "src_height": src_h,
        "sharpness": round(sharpness, 2),
        "preset": preset["name"],
        "out_width": target_w,
        "out_height": target_h,
    }


def main():
    parser = argparse.ArgumentParser(
        description="批量生成柔雾羽化壁纸，适合低像素图片。"
    )
    parser.add_argument("--input", required=True, help="输入图片文件夹")
    parser.add_argument("--output", required=True, help="输出文件夹")
    parser.add_argument("--width", type=int, default=1920, help="输出宽度")
    parser.add_argument("--height", type=int, default=1080, help="输出高度")
    parser.add_argument("--limit", type=int, default=0, help="只处理前 N 张，0 表示全部")
    parser.add_argument("--overwrite", action="store_true", help="覆盖已有输出")

    args = parser.parse_args()

    input_dir = Path(args.input)
    output_dir = Path(args.output)

    if not input_dir.exists():
        raise FileNotFoundError(f"输入文件夹不存在：{input_dir}")

    files = [
        p for p in input_dir.rglob("*")
        if p.is_file() and p.suffix.lower() in IMAGE_EXTS
    ]

    files.sort()

    if args.limit > 0:
        files = files[: args.limit]

    if not files:
        print("没有找到图片。")
        return

    report_rows = []

    for path in tqdm(files, desc="正在生成柔雾壁纸"):
        relative = path.relative_to(input_dir)
        out_path = output_dir / relative.with_suffix(".jpg")

        if out_path.exists() and not args.overwrite:
            continue

        try:
            row = process_one(
                input_path=path,
                output_path=out_path,
                target_w=args.width,
                target_h=args.height,
            )
            report_rows.append(row)
        except Exception as e:
            report_rows.append({
                "file": str(path),
                "output": "",
                "src_width": "",
                "src_height": "",
                "sharpness": "",
                "preset": f"失败：{e}",
                "out_width": args.width,
                "out_height": args.height,
            })

    report_path = output_dir / "soft_mist_report.csv"
    output_dir.mkdir(parents=True, exist_ok=True)

    with report_path.open("w", newline="", encoding="utf-8-sig") as f:
        fieldnames = [
            "file",
            "output",
            "src_width",
            "src_height",
            "sharpness",
            "preset",
            "out_width",
            "out_height",
        ]
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(report_rows)

    print()
    print(f"完成，输出目录：{output_dir}")
    print(f"报告文件：{report_path}")


if __name__ == "__main__":
    main()
