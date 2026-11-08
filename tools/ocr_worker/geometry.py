"""Tighten model character regions to observed ink, without changing OCR text.

CTC emits the full recognizer crop height for every character. Those regions are
alignment evidence, not tight glyph rectangles. Keep the original polygons and
record pixel bounds separately; empty/ambiguous regions retain model bounds.
"""
import math
import numpy as np


def tighten_document(document, image):
    from layout import bounds, rectangle
    height, width = image.shape[:2]
    changed = 0
    for line in document['lines']:
        for word in line['words']:
            for char in word.get('characters', []):
                box = rectangle(char)
                x0, y0 = max(0, math.floor(box['x'])), max(0, math.floor(box['y']))
                x1, y1 = min(width, math.ceil(box['x'] + box['width'])), min(height, math.ceil(box['y'] + box['height']))
                crop = image[y0:y1, x0:x1, :3]
                if not crop.size: continue
                # Top/bottom strips estimate the local backing surface. Never
                # infer a missing character or discard a small connected glyph.
                border = np.concatenate((crop[0], crop[-1]), axis=0).astype(float)
                background = np.median(border, axis=0)
                delta = np.max(np.abs(crop.astype(float) - background), axis=2)
                ys, xs = np.nonzero(delta > 35)
                if not len(xs) or len(xs) > crop.shape[0] * crop.shape[1] * .85:
                    continue
                tight = {'x': float(x0 + xs.min()), 'y': float(y0 + ys.min()),
                         'width': float(xs.max() - xs.min() + 1), 'height': float(ys.max() - ys.min() + 1)}
                char['model_polygon'] = char.pop('polygon', None)
                char.update(tight)
                char['geometry_source'] = 'observed_ink_inside_ctc_region'
                changed += 1
            if word.get('characters'): word.update(bounds(word['characters']))
        if line['words']: line.update(bounds(line['words']))
    document['words'] = [word for line in document['lines'] for word in line['words']]
    document['evidence']['geometry_refinement'] = {'characters': changed, 'method': 'local_background_contrast_within_model_region'}
    return document
