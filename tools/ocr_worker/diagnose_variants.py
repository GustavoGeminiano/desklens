"""Compare bounded pixel-only line variants; never silently correct model output."""
import argparse
import json
from pathlib import Path
import cv2
from inference import Inference
from corpus import ROOT


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--rect', type=int, nargs=4, required=True, metavar=('X', 'Y', 'WIDTH', 'HEIGHT'))
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    engine = Inference(ROOT / '.ocr-models')
    image = engine.engine.load_img(str(args.input.resolve()))
    x, y, w, h = args.rect
    crop = image[y:y+h, x:x+w]
    gray = cv2.cvtColor(crop, cv2.COLOR_BGR2GRAY)
    gray3 = lambda g: cv2.cvtColor(g, cv2.COLOR_GRAY2BGR)
    variants = {
        'original': crop,
        'cubic_150': cv2.resize(crop, None, fx=1.5, fy=1.5, interpolation=cv2.INTER_CUBIC),
        'area_80': cv2.resize(crop, None, fx=.8, fy=.8, interpolation=cv2.INTER_AREA),
        'cubic_80': cv2.resize(crop, None, fx=.8, fy=.8, interpolation=cv2.INTER_CUBIC),
        'gray': gray3(gray),
        'inverted_gray': gray3(255-gray),
        'clahe': gray3(cv2.createCLAHE(clipLimit=2, tileGridSize=(8, 8)).apply(gray)),
        'otsu': gray3(cv2.threshold(gray, 0, 255, cv2.THRESH_BINARY + cv2.THRESH_OTSU)[1]),
    }
    report = {'input': str(args.input), 'rect': args.rect, 'selection': None, 'variants': []}
    for name, value in variants.items():
        rec = engine.recognizer(engine.TextRecInput(img=[value], return_word_box=True))
        row = {'name': name, 'text': list(rec.txts), 'confidence': [float(s) for s in rec.scores]}
        report['variants'].append(row)
        print(json.dumps(row), flush=True)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__': main()
