"""Evaluate frozen synthetic splits and the literal user regression against native OCR.

Held-out evaluation is explicit. Results never automatically promote the engine.
"""
import argparse
import json
import platform
from pathlib import Path
import statistics
import sys
import time

from corpus import render, sources, ROOT
from inference import Inference
sys.path.insert(0, str(ROOT / 'scripts/ocr_benchmark'))
from evaluate import score
from fixtures import materialize
from report import write_report
from run import run_native


def aggregate(cases, name):
    chars = sum(len(c['truth']['full_text']) for c in cases)
    words = sum(c['runs'][name]['metrics']['expected_words'] for c in cases)
    return {
        'weighted_cer': sum(len(c['truth']['full_text']) * c['runs'][name]['metrics']['cer'] for c in cases) / max(1, chars),
        'weighted_cer_no_whitespace': sum(len(''.join(c['truth']['full_text'].split())) * c['runs'][name]['metrics']['cer_no_whitespace'] for c in cases) / max(1, sum(len(''.join(c['truth']['full_text'].split())) for c in cases)),
        'weighted_text_and_box_recall_iou50': sum(c['runs'][name]['metrics']['expected_words'] * c['runs'][name]['metrics']['text_and_box_recall_iou50'] for c in cases) / max(1, words),
        'exact_documents': sum(c['truth']['full_text'] == c['runs'][name]['result']['full_text'] for c in cases),
        'documents': len(cases),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model-dir', type=Path, default=ROOT / '.ocr-models')
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--split', choices=['development', 'heldout'], default='development')
    parser.add_argument('--diagnostic', type=Path)
    parser.add_argument('--repeats', type=int, default=5)
    args = parser.parse_args()
    if args.repeats < 1: parser.error('--repeats must be positive')
    args.output.mkdir(parents=True, exist_ok=True)
    cases = [render(c, args.output / 'inputs') for c in sources() if c['split'] == args.split]
    regressions = materialize(ROOT / 'scripts/ocr_benchmark/manifest.json', args.output / 'regressions')
    mandatory = next(c for c in regressions if c['id'] == 'user_report_2026_11_08_code')
    mandatory.update(mode='auto', category='user_regression')
    cases.insert(0, mandatory)
    start = time.perf_counter()
    engine = Inference(args.model_dir)
    startup_ms = (time.perf_counter() - start) * 1000
    for case in cases:
        samples = []
        for repeat in range(args.repeats + 1):
            start = time.perf_counter()
            value = engine.recognize(case['image_path'], case['mode'])
            elapsed = (time.perf_counter() - start) * 1000
            if repeat == 0: canonical, first_ms = value, elapsed
            else: samples.append(elapsed)
        raw = canonical['raw_detector_lines']
        # Raw line character polygons are retained as evidence; no fabricated word boxes.
        baseline = {'full_text': '\n'.join(l['text'] for l in raw), 'lines': [dict(l, words=[]) for l in raw]}
        case['runs'] = {
            'raw_detector_fragments': {'result': baseline, 'metrics': score(case['truth'], baseline), 'timing': {'scope': 'intermediate evidence from canonical pipeline, not a separately timed engine'}},
            'canonical_layout': {'result': canonical, 'metrics': score(case['truth'], canonical), 'timing': {'model_initialization_ms': startup_ms, 'first_ms': first_ms, 'warm_ms': samples, 'warm_median_ms': statistics.median(samples), 'scope': 'persistent detection + whole-line recognition + layout'}},
        }
        if args.diagnostic:
            native_runs = {}
            for repeat in range(args.repeats + 1):
                output = args.output / f'{case["id"]}.native.{repeat}.json'
                run_native([str(args.diagnostic.resolve()), '--input', str(Path(case['image_path']).resolve()), '--output', str(output.resolve())])
                for variant in json.loads(output.read_text(encoding='utf-8'))['variants']:
                    name = variant['name']
                    if repeat == 0:
                        native_runs[name] = {'result': variant['result'], 'metrics': score(case['truth'], variant['result']), 'timing': {'first_ms': variant['elapsed_ms'], 'warm_ms': [], 'scope': 'native diagnostic subprocess; OS OCR process initialization included'}}
                    else: native_runs[name]['timing']['warm_ms'].append(variant['elapsed_ms'])
            for run in native_runs.values(): run['timing']['warm_median_ms'] = statistics.median(run['timing']['warm_ms'])
            case['runs'].update(native_runs)
        print(case['id'], json.dumps(case['runs']['canonical_layout']['metrics']), flush=True)
    exact = mandatory['runs']['canonical_layout']['result']['full_text'] == mandatory['truth']['full_text']
    clean = cases[1:]
    summary = {name: aggregate(clean, name) for name in cases[0]['runs']}
    families = {family: {name: aggregate([c for c in clean if c['category'] == family], name) for name in summary} for family in sorted({c['category'] for c in clean})}
    numerical = False
    if args.diagnostic:
        old, new = summary['current_pipeline'], summary['canonical_layout']
        numerical = exact and new['weighted_cer'] <= .8 * old['weighted_cer'] and all(
            f['canonical_layout']['weighted_cer_no_whitespace'] <= f['current_pipeline']['weighted_cer_no_whitespace']
            and f['canonical_layout']['weighted_text_and_box_recall_iou50'] >= f['current_pipeline']['weighted_text_and_box_recall_iou50'] for f in families.values())
    samples = sorted(ms for c in clean for ms in c['runs']['canonical_layout']['timing']['warm_ms'])
    data = {'schema_version': 2, 'platform': platform.platform(), 'split': args.split,
            'mandatory_exact_5line_gate': exact, 'numerical_promotion_gates': numerical,
            'production_promotion': False, 'warm_p95_ms': samples[max(0, __import__('math').ceil(.95 * len(samples)) - 1)],
            'evidence_limit': 'Synthetic clean corpus; user image contains overlay. Real clean held-out captures and both native platforms are required for production promotion.',
            'summary': summary, 'families': families, 'cases': cases}
    (args.output / 'results.json').write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    write_report(args.output / 'report.html', cases, summary=summary)
    if not exact: raise SystemExit('Mandatory literal five-line reproduction failed')


if __name__ == '__main__': main()
