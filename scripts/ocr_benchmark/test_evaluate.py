import unittest
from evaluate import distance, iou, match_boxes, score


def w(text='a',x=0):
    return dict(text=text,x=x,y=0,width=10,height=10)


class EvaluationTests(unittest.TestCase):
    def test_unicode_edits(self):
        self.assertEqual(distance('ação','acao'),2)
        self.assertEqual(distance('','abc'),3)

    def test_duplicate_predictions_do_not_double_count(self):
        self.assertEqual(len(match_boxes([w()],[w(),w()])),1)
        self.assertEqual(len(match_boxes([w(),w()],[w()])),1)

    def test_matching_is_maximum_cardinality(self):
        # First expected can use either prediction, second only the first.
        self.assertEqual(len(match_boxes([w(x=2),w(x=0)],[w(x=0),w(x=4)])),2)

    def test_exact_text_required_for_joint_match(self):
        self.assertEqual(len(match_boxes([w('0')],[w('O')],require_text=True)),0)
        self.assertEqual(iou(w(),w()),1)

    def test_missing_symbols_and_empty_predictions(self):
        truth={'full_text':'0 ==','lines':[{'text':'0 ==','words':[w('0'),w('==',15)]}]}
        result={'full_text':'','lines':[]}
        result=score(truth,result)
        self.assertEqual(result['cer'],1)
        self.assertEqual(result['symbol_number_word_recall_iou50'],0)
        self.assertEqual(result['word_box_precision_iou50'],0)

    def test_identical_result_scores_perfectly(self):
        truth={'full_text':'ação','lines':[{'text':'ação','words':[w('ação')]}]}
        result=score(truth,truth)
        self.assertEqual(result['cer'],0)
        self.assertEqual(result['text_and_box_recall_iou50'],1)

class DatasetTests(unittest.TestCase):
    def test_missing_fixture_is_an_error(self):
        import json
        import tempfile
        from pathlib import Path
        from fixtures import materialize
        with tempfile.TemporaryDirectory() as directory:
            base=Path(directory)
            manifest=base/'manifest.json'
            manifest.write_text(json.dumps({'cases':[{'image':'missing-ocr-fixture.png'}]}))
            with self.assertRaises(FileNotFoundError):
                materialize(manifest,base/'inputs')

    def test_committed_annotations_and_portable_font_materialize(self):
        import tempfile
        from pathlib import Path
        from fixtures import materialize
        with tempfile.TemporaryDirectory() as directory:
            cases=materialize(Path(__file__).with_name('manifest.json'),Path(directory))
            self.assertEqual(len(cases),10)
            self.assertTrue(any(case['id']=='user_report_2026_11_07' for case in cases))
            self.assertEqual(sum(case['id'].startswith('synthetic_') for case in cases),4)


    def test_missing_annotations_are_an_error(self):
        import json
        import tempfile
        from pathlib import Path
        from fixtures import materialize
        with tempfile.TemporaryDirectory() as directory:
            base=Path(directory)
            manifest=base/'manifest.json'
            manifest.write_text(json.dumps({'cases':[{'id':'unlabelled','image':'tests/fixtures/real_editor_asimov_crop.png','truth':{'full_text':'','lines':[]}}]}))
            with self.assertRaises(ValueError):
                materialize(manifest,base/'inputs')

    def test_report_escapes_ocr_and_case_content(self):
        import tempfile
        from pathlib import Path
        from PIL import Image
        from report import write_report
        with tempfile.TemporaryDirectory() as directory:
            base=Path(directory)
            image=base/'input.png'
            Image.new('RGB',(20,20),'white').save(image)
            truth={'full_text':'<script>alert(1)</script>','lines':[]}
            case={'id':'<b>case</b>','provenance':'<img onerror="boom">','image_path':str(image),'truth':truth,'runs':{}}
            page=base/'report.html'
            write_report(page,[case],partial=True)
            contents=page.read_text()
            self.assertNotIn('<script>',contents)
            self.assertNotIn('<b>case</b>',contents)
            self.assertIn('&lt;script&gt;',contents)
            self.assertIn('PARTIAL RUN',contents)


if __name__ == '__main__':
    unittest.main()
