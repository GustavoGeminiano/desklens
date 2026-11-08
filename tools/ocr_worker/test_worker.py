"""Protocol tests use a fake engine so malformed input cannot initialize models."""
import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from worker import serve


class FakeEngine:
    def recognize(self, path, mode):
        return {'full_text': mode, 'lines': []}


class ProtocolTests(unittest.TestCase):
    def test_persistent_requests_and_invalid_request_recovery(self):
        with tempfile.TemporaryDirectory(prefix='OCR espaço ') as directory:
            image = Path(directory) / 'captura.png'
            image.write_bytes(b'fake')
            request = {'id': '1', 'crop_id': 'crop-a', 'mode': 'code', 'input_path': str(image)}
            second = dict(request, id='2', crop_id='crop-b', mode='text')
            text = json.dumps(request) + '\nnot-json\n[]\n' + json.dumps(second) + '\n'
            output = io.StringIO()
            with patch('sys.stdin', io.StringIO(text)), contextlib.redirect_stdout(output):
                serve(FakeEngine())
            rows = [json.loads(line) for line in output.getvalue().splitlines()]
            self.assertEqual([r['ok'] for r in rows], [True, False, False, True])
            self.assertEqual(rows[0]['result']['full_text'], 'code')
            self.assertEqual(rows[3]['crop_id'], 'crop-b')

    def test_entrypoint_forces_utf8_under_legacy_pipe_encoding(self):
        import os
        import subprocess
        script = """
import sys, types
sys.path.insert(0, sys.argv[1])
class Engine:
    def __init__(self, path): pass
    def recognize(self, path, mode): return {'full_text': 'Olá 😀', 'lines': []}
sys.modules['inference'] = types.SimpleNamespace(Inference=Engine, MODELS={})
import worker
sys.argv = ['worker.py']
raise SystemExit(worker.main())
"""
        with tempfile.TemporaryDirectory(prefix='captura espaço ') as directory:
            source = Path(directory) / 'ação.png'
            source.write_bytes(b'fake')
            request = {'id': 'unicode', 'crop_id': 'crop', 'mode': 'text', 'input_path': str(source)}
            result = subprocess.run([sys.executable, '-c', script, str(Path(__file__).resolve().parent)],
                input=(json.dumps(request, ensure_ascii=False)+'\n').encode('utf-8'), capture_output=True,
                env=dict(os.environ, PYTHONIOENCODING='cp1252'), check=True, timeout=15)
            rows = [json.loads(line) for line in result.stdout.decode('utf-8').splitlines()]
            self.assertTrue(rows[1]['ok'])
            self.assertEqual(rows[1]['result']['full_text'], 'Olá 😀')

    def test_invalid_mode_and_relative_path_are_rejected(self):
        for mode, path in [('bad', '/image.png'), ('auto', 'image.png')]:
            request = {'id': '1', 'crop_id': 'c', 'mode': mode, 'input_path': path}
            output = io.StringIO()
            with patch('sys.stdin', io.StringIO(json.dumps(request)+'\n')), contextlib.redirect_stdout(output):
                serve(FakeEngine())
            self.assertFalse(json.loads(output.getvalue())['ok'])


if __name__ == '__main__':
    unittest.main()
