"""Engine-independent OCR scoring; confidence values never compared across engines."""
import re


def distance(a, b):
    previous = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        current = [i]
        for j, cb in enumerate(b, 1):
            current.append(min(current[-1] + 1, previous[j] + 1, previous[j-1] + (ca != cb)))
        previous = current
    return previous[-1]


def iou(a, b):
    overlap = max(0, min(a['x']+a['width'], b['x']+b['width'])-max(a['x'], b['x'])) * max(0, min(a['y']+a['height'], b['y']+b['height'])-max(a['y'], b['y']))
    area = a['width']*a['height'] + b['width']*b['height'] - overlap
    return overlap / area if area > 0 else 0


def match_boxes(expected, predicted, threshold=.5, require_text=False):
    # Maximum-cardinality bipartite matching (not greedy): no double-counting.
    edges = [[j for j, p in sorted(enumerate(predicted), key=lambda jp: -iou(g, jp[1]))
              if iou(g, p) >= threshold and (not require_text or g['text'] == p['text'])] for g in expected]
    assigned = {}
    def augment(i, seen):
        for j in edges[i]:
            if j in seen:
                continue
            seen.add(j)
            if j not in assigned or augment(assigned[j], seen):
                assigned[j] = i
                return True
        return False
    for i in range(len(expected)):
        augment(i, set())
    return [(i, j) for j, i in assigned.items()]


def score(truth, result):
    expected = truth['full_text']
    actual = result['full_text']
    gt_lines = truth['lines']
    pred_lines = result['lines']
    gt_words = [w for l in gt_lines for w in l['words']]
    pred_words = [w for l in pred_lines for w in l.get('words', [])]
    matches = match_boxes(gt_words, pred_words)
    exact_matches = match_boxes(gt_words, pred_words, require_text=True)
    n = len(gt_words)
    m = len(pred_words)
    special = [(i, w) for i, w in enumerate(gt_words) if re.search(r'[0-9%=:<>+*/!(){}\[\];]', w['text'])]
    matched_exact = {i for i, _ in exact_matches}
    no_space = lambda t: re.sub(r'\s', '', t)
    ordered_gt = [l['text'] for l in gt_lines]
    ordered_pred = [l['text'] for l in pred_lines]
    return {
        'cer': distance(expected, actual) / max(1, len(expected)),
        'cer_no_whitespace': distance(no_space(expected), no_space(actual)) / max(1, len(no_space(expected))),
        'exact_lines': sum(a == b for a, b in zip(ordered_gt, ordered_pred)),
        'expected_lines': len(gt_lines), 'predicted_lines': len(pred_lines),
        'word_box_precision_iou50': len(matches) / m if m else 0,
        'word_box_recall_iou50': len(matches) / n if n else 0,
        'text_and_box_precision_iou50': len(exact_matches) / m if m else 0,
        'text_and_box_recall_iou50': len(exact_matches) / n if n else 0,
        'symbol_number_word_recall_iou50': sum(i in matched_exact for i, _ in special) / len(special) if special else None,
        'expected_words': n, 'predicted_words': m,
        'missing_or_wrong_words': [w['text'] for i, w in enumerate(gt_words) if i not in matched_exact],
    }
