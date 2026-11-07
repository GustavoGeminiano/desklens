"""Pinned, CPU-only RapidOCR experiment. Normal inference never downloads models."""
from __future__ import annotations

import argparse
import hashlib
from importlib.metadata import version
import json
from pathlib import Path
import time
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_MODEL_DIR = ROOT / '.ocr-models'
MODELS = {
    'det': {
        'name': 'ch_PP-OCRv5_det_mobile.onnx',
        'sha256': '4d97c44a20d30a81aad087d6a396b08f786c4635742afc391f6621f5c6ae78ae',
    },
    'rec': {
        'name': 'latin_PP-OCRv5_rec_mobile.onnx',
        'sha256': 'b20bd37c168a570f583afbc8cd7925603890efbcdc000a59e22c269d160b5f5a',
    },
}
for _task, _model in MODELS.items():
    _model['url'] = f"https://www.modelscope.cn/models/RapidAI/RapidOCR/resolve/v3.10.0/onnx/PP-OCRv5/{_task}/{_model['name']}"


def sha256(path: Path) -> str:
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def verify_models(model_dir: Path) -> None:
    for model in MODELS.values():
        path = model_dir / model['name']
        if not path.is_file() or sha256(path) != model['sha256']:
            raise RuntimeError(f"Missing or invalid pinned model: {path}. Run rapidocr_engine.py prepare first.")


def prepare_models(model_dir: Path = DEFAULT_MODEL_DIR) -> None:
    """Explicit network-enabled setup; downloads checked against upstream SHA-256."""
    model_dir = Path(model_dir)
    model_dir.mkdir(parents=True, exist_ok=True)
    for model in MODELS.values():
        target = model_dir / model['name']
        if target.is_file() and sha256(target) == model['sha256']:
            continue
        temporary = target.with_suffix('.download')
        try:
            with urlopen(model['url'], timeout=120) as response, temporary.open('wb') as output:
                while chunk := response.read(1024 * 1024):
                    output.write(chunk)
            if sha256(temporary) != model['sha256']:
                raise RuntimeError(f"SHA-256 mismatch downloading {model['name']}")
            temporary.replace(target)
        finally:
            temporary.unlink(missing_ok=True)
    verify_models(model_dir)
    (model_dir / 'manifest.json').write_text(json.dumps(MODELS, indent=2) + '\n', encoding='utf-8')


def rectangle(polygon) -> dict:
    points = [[float(x), float(y)] for x, y in polygon]
    xs, ys = zip(*points)
    return dict(x=min(xs), y=min(ys), width=max(xs)-min(xs), height=max(ys)-min(ys), polygon=points)


class RapidOcrEngine:
    def __init__(self, model_dir: Path | None = None):
        from rapidocr import RapidOCR, ModelType, OCRVersion
        self.model_dir = Path(model_dir or DEFAULT_MODEL_DIR).resolve()
        verify_models(self.model_dir)
        if version('rapidocr') != '3.10.0':
            raise RuntimeError('This adapter requires rapidocr==3.10.0; install requirements.txt.')
        self.engine = RapidOCR(params={
            'Global.use_cls': False,
            'Global.return_word_box': True,
            'Global.text_score': 0.0,
            'Global.log_level': 'error',
            'Det.ocr_version': OCRVersion.PPOCRV5,
            'Det.model_type': ModelType.MOBILE,
            'Det.lang_type': 'ch',
            'Det.model_path': str(self.model_dir / MODELS['det']['name']),
            'Rec.ocr_version': OCRVersion.PPOCRV5,
            'Rec.model_type': ModelType.MOBILE,
            'Rec.lang_type': 'latin',
            'Rec.model_path': str(self.model_dir / MODELS['rec']['name']),
            'EngineConfig.onnxruntime.intra_op_num_threads': 4,
            'EngineConfig.onnxruntime.inter_op_num_threads': 1,
            'EngineConfig.onnxruntime.use_cuda': False,
            'EngineConfig.onnxruntime.use_dml': False,
        })

    def recognize(self, image_path: str | Path) -> dict:
        from PIL import Image
        start = time.perf_counter()
        result = self.engine(str(image_path), use_cls=False, return_word_box=True, text_score=0.0)
        elapsed_ms = (time.perf_counter() - start) * 1000
        lines = []
        texts = result.txts or ()
        word_results = result.word_results or ()
        for i, text in enumerate(texts):
            words = []
            native_words = word_results[i] if i < len(word_results) else ()
            for word_text, confidence, polygon in native_words:
                if polygon is not None and word_text:
                    words.append(dict(text=word_text, confidence=float(confidence)*100, **rectangle(polygon)))
            lines.append(dict(text=text, confidence=float(result.scores[i])*100,
                              **rectangle(result.boxes[i]), words=words))
        with Image.open(image_path) as image:
            width, height = image.size
        return {
            'engine': 'rapidocr', 'full_text': '\n'.join(texts), 'lines': lines,
            'metadata': {'rapidocr': version('rapidocr'), 'onnxruntime': version('onnxruntime'),
                         'models': MODELS, 'provider': 'CPUExecutionProvider',
                         'width': width, 'height': height, 'coordinate_space': 'original_image_pixels',
                         'confidence_scale': '0-100', 'word_boxes': 'native_ctc_alignment',
                         'preprocessing': 'RapidOCR built-in only; original RGB input; no DeskLens heuristics',
                         'threshold': 0.0, 'orientation_classifier': False, 'intra_op_threads': 4},
            'timing': {'inference_ms': elapsed_ms},
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['prepare', 'recognize'])
    parser.add_argument('image', nargs='?')
    parser.add_argument('--model-dir', type=Path, default=DEFAULT_MODEL_DIR)
    args = parser.parse_args()
    if args.command == 'prepare':
        prepare_models(args.model_dir)
        print(json.dumps({'prepared': str(args.model_dir), 'models': MODELS}, indent=2))
    else:
        if not args.image:
            parser.error('recognize requires an image path')
        print(json.dumps(RapidOcrEngine(args.model_dir).recognize(args.image), ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
