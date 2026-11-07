#!/usr/bin/env python3
"""Run actual engines against a small annotated dataset, never mocked OCR."""
import argparse
import json
import importlib.metadata
import importlib.util
import platform
import sys
import statistics
import subprocess
import time
from pathlib import Path

from evaluate import score
from fixtures import materialize
from report import write_report


def timed(call):
    start = time.perf_counter()
    result = call()
    return result, (time.perf_counter()-start)*1000


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--diagnostic', required=True, type=Path, help='Built Rust ocr_diagnostic executable')
    parser.add_argument('--output',type=Path,default=Path('benchmark-results'))
    parser.add_argument('--manifest',type=Path,default=Path(__file__).with_name('manifest.json'))
    parser.add_argument('--repeats',type=int,default=5)
    parser.add_argument('--font',type=Path,help='Local monospace TTF for reproducible synthetic cases; font SHA recorded')
    parser.add_argument('--model-dir',type=Path)
    parser.add_argument('--skip-rapidocr',action='store_true',help='Explicit partial run; report must not claim comparative engine winner')
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error('--repeats must be positive')
    diagnostic = args.diagnostic.resolve()
    if not diagnostic.is_file():
        parser.error(f'Diagnostic executable missing: {diagnostic}')
    args.output.mkdir(parents=True,exist_ok=True)
    cases = materialize(args.manifest,args.output/'inputs',args.font)
    rapid = None
    startup = None
    if not args.skip_rapidocr:
        from rapidocr_engine import RapidOcrEngine
        rapid,startup = timed(lambda: RapidOcrEngine(model_dir=args.model_dir))
    for case in cases:
        print('Benchmarking',case['id'],flush=True)
        case['runs'] = {}
        native_samples = {}
        wall_samples = []
        for repeat in range(args.repeats+1):
            native_path = args.output / f'{case["id"]}.native.{repeat}.json'
            _, wall = timed(lambda: subprocess.run([str(diagnostic),'--input',case['image_path'],'--output',str(native_path)],check=True))
            native = json.loads(native_path.read_text(encoding='utf-8'))
            wall_samples.append(wall)
            for variant in native['variants']:
                name = variant['name']
                if name not in case['runs']:
                    case['runs'][name] = {'engine':variant['engine'],'result':variant['result'],'metadata':{'confidence_scale':variant.get('confidence_scale'),'confidence_origin':variant.get('confidence_origin'),'coordinates':native.get('coordinates'),'platform':native.get('platform'),'timing_notes':native.get('timing_notes')},'timing':{'first_ms':variant['elapsed_ms'],'preprocessing_ms':variant.get('preprocessing_ms')}}
                    native_samples[name] = []
                else:
                    native_samples[name].append(variant['elapsed_ms'])
        for name,samples in native_samples.items():
            case['runs'][name]['timing'].update(warm_ms=samples,warm_median_ms=statistics.median(samples),scope='engine elapsed reported by fresh diagnostic subprocess; Tesseract process startup included',diagnostic_wall_ms=wall_samples)
        if rapid:
            first,first_ms = timed(lambda:rapid.recognize(Path(case['image_path'])))
            samples = []
            for _ in range(args.repeats):
                _,ms = timed(lambda:rapid.recognize(Path(case['image_path'])))
                samples.append(ms)
            case['runs']['rapidocr_original_raw'] = {'engine':'rapidocr','result':first,'metadata':first.get('metadata',{}),'timing':{'model_initialization_ms':startup,'first_ms':first_ms,'warm_ms':samples,'warm_median_ms':statistics.median(samples),'scope':'persistent model inference, initialization excluded'}}
        for run in case['runs'].values():
            run['metrics'] = score(case['truth'],run['result'])
    summary = {}
    for name in cases[0]['runs']:
        entries = [(case['truth'],case['runs'][name]) for case in cases]
        chars = sum(len(truth['full_text']) for truth,_ in entries)
        words = sum(run['metrics']['expected_words'] for _,run in entries)
        summary[name] = {'weighted_cer':sum(run['metrics']['cer']*len(truth['full_text']) for truth,run in entries)/max(1,chars),'weighted_text_and_box_recall_iou50':sum(run['metrics']['text_and_box_recall_iou50']*run['metrics']['expected_words'] for _,run in entries)/max(1,words),'expected_characters':chars,'expected_words':words}
    def command_info(command):
        try:
            output = subprocess.run(command,capture_output=True,text=True,check=True)
            return (output.stdout+output.stderr).strip()
        except (OSError, subprocess.CalledProcessError) as exc:
            return {'unavailable':str(exc)}
    environment = {'platform':platform.platform(),'python':sys.version,'packages':{name:importlib.metadata.version(name) for name in ('Pillow','rapidocr','onnxruntime') if importlib.util.find_spec({'Pillow':'PIL'}.get(name,name)) is not None},'tesseract_version':command_info(['tesseract','--version']),'tesseract_languages':command_info(['tesseract','--list-langs'])}
    data = {'schema_version':1, 'environment':environment,'summary':summary,'partial_run':args.skip_rapidocr,'repeats':args.repeats,'selection_policy':'Insufficient evidence for production promotion: small, repeated-content and overlay-contaminated dataset. Acquire clean held-out captures before any global engine decision.','cases':cases}
    (args.output/'results.json').write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    write_report(args.output/'report.html',cases,partial=args.skip_rapidocr,summary=summary)
    print(args.output/'report.html')


if __name__ == '__main__':
    main()
