"""Adapter contracts that do not need ONNX models or optional dependencies."""
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from rapidocr_engine import MODELS, rectangle, verify_models


class AdapterContractTests(unittest.TestCase):
    def test_polygon_envelope_preserves_original_coordinates(self):
        box = rectangle([[11, 4], [32, 5], [30, 18], [10, 17]])
        self.assertEqual((box['x'], box['y'], box['width'], box['height']), (10, 4, 22, 14))
        self.assertEqual(box['polygon'], [[11., 4.], [32., 5.], [30., 18.], [10., 17.]])

    def test_missing_model_fails_without_download(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, 'prepare first'):
                verify_models(Path(directory))

    def test_present_but_corrupt_model_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for model in MODELS.values():
                (root / model['name']).write_bytes(b'corrupt')
            with self.assertRaisesRegex(RuntimeError, 'invalid pinned model'):
                verify_models(root)

    def test_every_model_is_verified(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            models = {
                'det': {'name': 'det.onnx', 'sha256': hashlib.sha256(b'det').hexdigest()},
                'rec': {'name': 'rec.onnx', 'sha256': hashlib.sha256(b'rec').hexdigest()},
            }
            (root / 'det.onnx').write_bytes(b'det')
            (root / 'rec.onnx').write_bytes(b'rec')
            with patch('rapidocr_engine.MODELS', models):
                verify_models(root)
                (root / 'rec.onnx').write_bytes(b'tampered')
                with self.assertRaisesRegex(RuntimeError, 'rec.onnx'):
                    verify_models(root)


if __name__ == '__main__':
    unittest.main()
