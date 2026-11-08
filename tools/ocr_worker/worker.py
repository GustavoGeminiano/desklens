"""Offline JSON-lines OCR service. stdout is reserved for versioned protocol."""
import argparse
import contextlib
import json
from pathlib import Path
import sys
import time

PROTOCOL_VERSION = 1


def send(message):
    sys.stdout.write(json.dumps(message, ensure_ascii=False, allow_nan=False) + '\n')
    sys.stdout.flush()


def default_models():
    if getattr(sys, 'frozen', False):
        return Path(sys.executable).resolve().parent / 'models'
    return Path(__file__).resolve().parents[2] / '.ocr-models'


def serve(engine):
    for line in sys.stdin:
        request = {}
        try:
            request = json.loads(line)
            if not isinstance(request, dict):
                raise ValueError('Request must be a JSON object')
            if not isinstance(request.get('id'), str) or not isinstance(request.get('crop_id'), str):
                raise ValueError('id and crop_id must be strings')
            mode = request.get('mode', 'auto')
            if mode not in ('auto', 'text', 'code'):
                raise ValueError('mode must be auto, text, or code')
            path = Path(request['input_path'])
            if not path.is_absolute() or not path.is_file():
                raise ValueError('input_path must be an existing absolute image path')
            start = time.perf_counter()
            with contextlib.redirect_stdout(sys.stderr):
                result = engine.recognize(path, mode)
            result['elapsed_ms'] = (time.perf_counter() - start) * 1000
            send({'id': request['id'], 'crop_id': request['crop_id'], 'ok': True, 'result': result})
        except Exception as exc:
            identity = request if isinstance(request, dict) else {}
            send({'id': identity.get('id'), 'crop_id': identity.get('crop_id'), 'ok': False,
                  'error': str(exc)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model-dir', type=Path, default=default_models())
    parser.add_argument('--self-test', type=Path, help='Recognize one local image, emit protocol response, exit')
    args = parser.parse_args()
    try:
        with contextlib.redirect_stdout(sys.stderr):
            from inference import Inference, MODELS
            engine = Inference(args.model_dir)
        send({'type': 'ready', 'protocol_version': PROTOCOL_VERSION,
              'engine': 'rapidocr', 'models': MODELS, 'offline': True})
        if args.self_test:
            with contextlib.redirect_stdout(sys.stderr):
                result = engine.recognize(args.self_test.resolve(), 'auto')
            send({'id': 'self-test', 'crop_id': 'self-test', 'ok': True, 'result': result})
        else:
            serve(engine)
    except Exception as exc:
        send({'type': 'startup_error', 'protocol_version': PROTOCOL_VERSION, 'error': str(exc)})
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
