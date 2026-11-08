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

        alternate_budget = 3
        alternate_choices = []
        def rerecognize(x, y, width, height):
            nonlocal alternate_budget
            import math
            x0, y0 = max(0, math.floor(x)), max(0, math.floor(y))
            x1, y1 = min(w, math.ceil(x + width)), min(h, math.ceil(y + height))
            if x1 <= x0 or y1 <= y0:
                return None
            crop = image[y0:y1, x0:x1].copy()
            rec = self.recognizer(self.TextRecInput(img=[crop], return_word_box=True))
            if not rec.txts:
                return None
            chosen_crop = crop
            original_text = rec.txts[0]
            alternatives = []
            if float(rec.scores[0]) < .90 and alternate_budget > 0:
                import cv2
                alternate_budget -= 1
                gray = cv2.cvtColor(crop, cv2.COLOR_BGR2GRAY)
                threshold = cv2.threshold(gray, 0, 255, cv2.THRESH_BINARY + cv2.THRESH_OTSU)[1]
                variants = [('cubic_150', cv2.resize(crop, None, fx=1.5, fy=1.5, interpolation=cv2.INTER_CUBIC)),
                            ('otsu', cv2.cvtColor(threshold, cv2.COLOR_GRAY2BGR))]
                for name, pixels in variants:
                    candidate = self.recognizer(self.TextRecInput(img=[pixels], return_word_box=True))
                    if not candidate.txts: continue
                    alternatives.append({'variant': name, 'text': candidate.txts[0], 'confidence': float(candidate.scores[0])})
                    # Agreement is not evidence to replace already aligned geometry.
                    differs = ''.join(candidate.txts[0].split()) != ''.join(rec.txts[0].split())
                    if differs and float(candidate.scores[0]) > float(rec.scores[0]) + .02:
                        rec, chosen_crop = candidate, pixels
                if rec.txts[0] != original_text:
                    alternate_choices.append({'kind': 'pixel_variant_disagreement', 'original_text': original_text, 'selected_text': rec.txts[0]})
            quad = self.np.array([[x0, y0], [x1, y0], [x1, y1], [x0, y1]], dtype=self.np.float32)
            rec = self.engine.cal_rec_boxes([chosen_crop], self.np.array([quad]), rec, True)
            line = self._line(rec.txts[0], rec.scores[0], quad, rec.word_results[0])
            line['pixel_variants'] = alternatives
            return line

        canonical = process(raw, w, h, mode, rerecognize)
        from geometry import tighten_document
        canonical = tighten_document(canonical, image)
        canonical['uncertainties'].extend(alternate_choices)
        canonical['uncertain'] = bool(canonical['uncertainties'])
        canonical['engine'] = 'rapidocr'
        canonical['raw_detector_lines'] = raw
        canonical['model_manifest'] = MODELS
        return canonical
