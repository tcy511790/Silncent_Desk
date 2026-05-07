from pathlib import Path
import csv
import cv2
import numpy as np
from PIL import Image
from tqdm import tqdm


IMAGE_EXTS = {".jpg", ".jpeg", ".png", ".webp", ".bmp", ".tif", ".tiff"}


def cv2_imread_unicode(path: Path):
    data = np.fromfile(str(path), dtype=np.uint8)
    return cv2.imdecode(data, cv2.IMREAD_COLOR)


def calc_sharpness(path: Path) -> float:
    img = cv2_imread_unicode(path)
    if img is None:
        return 0.0

    gray = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)

    # 为了不同尺寸更公平，缩到一个中等尺寸再算清晰度
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


def classify(width: int, height: int, sharpness: float, target_w=2560, target_h=1440):
    mp = width * height / 1_000_000
    size_score = min(width / target_w, height / target_h)

    if size_score >= 0.75 and sharpness >= 90:
        return "高清：可做大图"
    if size_score >= 0.5 and sharpness >= 55:
        return "中清：建议适度缩放"
    if size_score >= 0.32:
        return "低清：建议画片模式"
    return "极低清：建议小画片模式"


def main():
    input_dir = input("请输入图片文件夹路径：").strip().strip('"')
    root = Path(input_dir)

    if not root.exists():
        print("文件夹不存在。")
        return

    files = [
        p for p in root.rglob("*")
        if p.is_file() and p.suffix.lower() in IMAGE_EXTS
    ]

    if not files:
        print("没有找到图片。")
        return

    output_csv = root / "image_audit_report.csv"

    rows = []
    for path in tqdm(files, desc="正在体检图片"):
        try:
            with Image.open(path) as im:
                width, height = im.size
                mode = im.mode

            sharpness = calc_sharpness(path)
            ratio = width / height if height else 0
            mp = width * height / 1_000_000
            tier = classify(width, height, sharpness)

            rows.append({
                "file": str(path),
                "width": width,
                "height": height,
                "megapixels": round(mp, 2),
                "ratio": round(ratio, 3),
                "sharpness": round(sharpness, 2),
                "mode": mode,
                "suggestion": tier,
            })

        except Exception as e:
            rows.append({
                "file": str(path),
                "width": "",
                "height": "",
                "megapixels": "",
                "ratio": "",
                "sharpness": "",
                "mode": "",
                "suggestion": f"读取失败：{e}",
            })

    with output_csv.open("w", newline="", encoding="utf-8-sig") as f:
        writer = csv.DictWriter(
            f,
            fieldnames=[
                "file",
                "width",
                "height",
                "megapixels",
                "ratio",
                "sharpness",
                "mode",
                "suggestion",
            ],
        )
        writer.writeheader()
        writer.writerows(rows)

    print()
    print(f"完成：{output_csv}")
    print()
    print("分类统计：")

    stats = {}
    for row in rows:
        key = row["suggestion"]
        stats[key] = stats.get(key, 0) + 1

    for key, count in sorted(stats.items(), key=lambda x: x[1], reverse=True):
        print(f"{key}: {count} 张")


if __name__ == "__main__":
    main()