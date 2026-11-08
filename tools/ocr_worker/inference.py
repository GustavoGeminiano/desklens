"""Shared persistent neural inference; all coordinates refer to the source crop."""
from pathlib import Path
import sys

BENCHMARK_MODULES = Path(__file__).resolve().parents[2] / 'scripts' / 'ocr_benchmark'
if not getattr(sys, 'frozen', False):
    sys.path.insert(0, str(BENCHMARK_MODULES))
from rapidocr_engine import RapidOcrEngine, MODELS


class Inference:
    def __init__(self, model_dir):
        import numpy as np
        import onnxruntime as ort
        ort.disable_telemetry_events()
        from rapidocr.ch_ppocr_rec.typings import TextRecInput
        self.np = np
        self.adapter = RapidOcrEngine(Path(model_dir))
        self.engine = self.adapter.engine
        # Force lazy loading before ready; reuse these exact sessions for line reads.
        self.engine._load_det_model()
        self.recognizer = self.engine._load_rec_model()
        self.TextRecInput = TextRecInput

    @staticmethod
    def _line(text, confidence, polygon, chars):
        return {'text': str(text), 'confidence': float(confidence) * 100,
                'polygon': [[float(x), float(y)] for x, y in polygon],
                'characters': [{'text': str(t), 'confidence': float(c) * 100,
                                'polygon': [[float(x), float(y)] for x, y in p]}
                               for t, c, p in chars if p is not None and t]}

    def recognize(self, path, mode):
        from layout import process
        image = self.engine.load_img(str(path))
        h, w = image.shape[:2]
        result = self.engine(image, use_det=True, use_cls=False, use_rec=True,
                             return_word_box=True, return_single_char_box=True, text_score=0.)
        raw = []
        if result.txts:
            for text, score, quad, chars in zip(result.txts, result.scores, result.boxes, result.word_results):
                raw.append(self._line(text, score, quad, chars))

        def rerecognize(x, y, width, height):
            import math
            x0, y0 = max(0, math.floor(x)), max(0, math.floor(y))
            x1, y1 = min(w, math.ceil(x + width)), min(h, math.ceil(y + height))
            if x1 <= x0 or y1 <= y0:
                return None
            crop = image[y0:y1, x0:x1].copy()
            rec = self.recognizer(self.TextRecInput(img=[crop], return_word_box=True))
            if not rec.txts:
                return None
            quad = self.np.array([[x0, y0], [x1, y0], [x1, y1], [x0, y1]], dtype=self.np.float32)
            rec = self.engine.cal_rec_boxes([crop], self.np.array([quad]), rec, True)
            return self._line(rec.txts[0], rec.scores[0], quad, rec.word_results[0])

        canonical = process(raw, w, h, mode, rerecognize)
        from geometry import tighten_document
        canonical = tighten_document(canonical, image)
        canonical['engine'] = 'rapidocr'
        canonical['raw_detector_lines'] = raw
        canonical['model_manifest'] = MODELS
        return canonical
