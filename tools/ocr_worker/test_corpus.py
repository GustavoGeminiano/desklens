import tempfile
import unittest
from pathlib import Path
from corpus import sources, render


class CorpusTests(unittest.TestCase):
    def test_content_disjoint_split_and_all_geometry_valid(self):
        cases=sources()
        self.assertEqual(len(cases),30)
        self.assertEqual(sum(c['split']=='development' for c in cases),20)
        self.assertEqual(sum(c['split']=='heldout' for c in cases),10)
        with tempfile.TemporaryDirectory() as folder:
            for case in cases:
                rendered=render(case,Path(folder))
                self.assertTrue(Path(rendered['image_path']).is_file())
                self.assertEqual(rendered['truth']['full_text'],'\n'.join(l['text'] for l in case['lines']))
                self.assertIn('synthetic',rendered['provenance'])
                self.assertEqual(len(rendered['font_sha256']),64)
        self.assertTrue(any(c['category']=='columns' and c['split']=='heldout' for c in cases))


if __name__=='__main__':unittest.main()
