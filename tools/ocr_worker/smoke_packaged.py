"""Exercise the frozen worker with an empty PATH and the mandatory literal crop.

Network isolation is supplied by the CI runner (Linux network namespace or
Windows firewall). Python here is the test driver, never a worker dependency.
"""
import argparse
import json
import os
from pathlib import Path
import queue
import statistics
import subprocess
import sys
import threading
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts/ocr_benchmark'))
from fixtures import materialize


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--app', type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    case = next(c for c in materialize(ROOT / 'scripts/ocr_benchmark/manifest.json', args.output / 'inputs') if c['id'] == 'user_report_2026_11_08_code')
    executable = args.runtime.resolve() / ('desklens-ocr.exe' if os.name == 'nt' else 'desklens-ocr')
    env = dict(os.environ, PATH='', PYTHONPATH='', PYTHONHOME='')
    started = time.perf_counter()
    rows = queue.Queue()
    with (args.output / 'worker-stderr.log').open('w', encoding='utf-8') as errors:
        process = subprocess.Popen([str(executable)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errors, text=True, encoding='utf-8', env=env)
        def reader():
            for line in process.stdout:
                try: rows.put(json.loads(line))
                except Exception as error: rows.put({'protocol_error': str(error)})
        threading.Thread(target=reader, daemon=True).start()
        try:
            ready = rows.get(timeout=30)
            startup = (time.perf_counter() - started) * 1000
            assert ready.get('type') == 'ready', ready
            timings, replies = [], []
            for index in range(6):
                request = {'id': str(index), 'crop_id': 'mandatory', 'mode': 'auto', 'input_path': str(Path(case['image_path']).resolve())}
                start = time.perf_counter()
                process.stdin.write(json.dumps(request) + '\n'); process.stdin.flush()
                response = rows.get(timeout=15)
                timings.append((time.perf_counter() - start) * 1000)
                assert response.get('ok') and response.get('id') == str(index), response
                assert response['result']['full_text'] == case['truth']['full_text'], response['result']['full_text']
                replies.append(response)
            report = {'ok': True, 'startup_ms': startup, 'first_ms': timings[0], 'warm_ms': timings[1:], 'warm_median_ms': statistics.median(timings[1:]), 'empty_path': True, 'result': replies[0]['result']}
        finally:
            process.kill(); process.wait(timeout=10)
    if args.app:
        output = (args.output / 'app.json').resolve()
        subprocess.run([str(args.app.resolve()), '--ocr-smoke', str(Path(case['image_path']).resolve()), str(output)], env=env, check=True, timeout=90)
        application = json.loads(output.read_text(encoding='utf-8'))
        assert application.get('ok') and application['result']['engine'] == 'rapidocr', application
        assert application['result']['full_text'] == case['truth']['full_text'], application
        report['installed_app'] = application
    (args.output / 'smoke.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k not in ('result', 'installed_app')}))


if __name__ == '__main__': main()
