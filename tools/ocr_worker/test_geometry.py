import unittest
import numpy as np
from geometry import tighten_document


class GeometryTests(unittest.TestCase):
    def test_tightens_ink_and_keeps_single_pixel_symbol_and_text(self):
        image = np.zeros((30, 30, 3), dtype=np.uint8)
        image[15, 12] = [180, 240, 90]
        char = {'text': '.', 'polygon': [[10, 5], [20, 5], [20, 25], [10, 25]]}
        word = {'text': '.', 'characters': [char]}
        document = {'full_text': '.', 'lines': [{'text': '.', 'words': [word]}], 'evidence': {}}
        result = tighten_document(document, image)
        self.assertEqual(result['full_text'], '.')
        self.assertEqual((word['x'], word['y'], word['width'], word['height']), (12, 15, 1, 1))
        self.assertIn('model_polygon', char)

    def test_no_ink_keeps_model_geometry(self):
        image = np.full((30, 30, 3), 255, dtype=np.uint8)
        char = {'text': '?', 'polygon': [[10, 5], [20, 5], [20, 25], [10, 25]]}
        word = {'text': '?', 'characters': [char]}
        result = tighten_document({'full_text': '?', 'lines': [{'text': '?', 'words': [word]}], 'evidence': {}}, image)
        self.assertEqual(word['height'], 20)
        self.assertEqual(result['full_text'], '?')


if __name__ == '__main__': unittest.main()
