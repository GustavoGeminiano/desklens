import unittest
from layout import process, group_visual_lines, utf16


def fragment(text,x,y,pitch=10,score=.95):
    chars=[]
    for index,char in enumerate(text):
        chars.append(dict(text=char,confidence=score,start=index,end=index+1,x=x+index*pitch,y=y,width=pitch,height=18))
    return dict(text=text,confidence=score,x=x,y=y,width=len(text)*pitch,height=18,characters=chars)


class LayoutTests(unittest.TestCase):
    def test_split_rows_rejoin_without_inserting_glyphs(self):
        result=process([fragment('for',0,0),fragment('i in',40,0),fragment('range(10):',90,0),fragment('else:',30,35)],300,100,'code')
        self.assertEqual(result['full_text'],'for i in range(10):\n   else:')
        self.assertEqual(len(result['lines']),2)

    def test_literal_invalid_source_and_three_six_spaces(self):
        result=process([fragment('for i in range(10):',0,0),fragment('if i % 2 == 0:',30,30),fragment('print(f("%i is even")',60,60)],300,100,'auto')
        self.assertEqual(result['full_text'],'for i in range(10):\n   if i % 2 == 0:\n      print(f("%i is even")')

    def test_utf16_ranges_include_surrogate_pairs(self):
        result=process([fragment('Olá 😀 café',0,0)],300,50,'text')
        self.assertEqual(result['lines'][0]['end_utf16'],11)
        for word in result['words']:
            encoded=result['full_text'].encode('utf-16-le')
            text=encoded[word['start_utf16']*2:word['end_utf16']*2].decode('utf-16-le')
            self.assertEqual(text,word['text'])
        self.assertEqual(utf16('😀'),2)

    def test_columns_are_separate_blocks_in_reading_order(self):
        result=process([fragment('left a',0,0),fragment('right a',230,0),fragment('left b',0,30),fragment('right b',230,30)],400,100,'text')
        self.assertEqual(result['full_text'],'left a\nleft b\nright a\nright b')
        self.assertEqual(len(result['blocks']),2)

    def test_whole_line_recognition_can_recover_literal_glyph(self):
        calls=[]
        original=fragment('is even',30,0,score=.95)
        def recognize(x,y,w,h):
            calls.append((x,y,w,h))
            return fragment('0 is even',0,0,score=.96)
        result=process([original],200,50,'text',recognize)
        self.assertEqual(result['full_text'],'0 is even')
        self.assertEqual(len(calls),1)
        self.assertTrue(result['uncertain'])
        self.assertTrue(result['evidence']['line_recognition'][0]['selected'])

    def test_low_confidence_disagreement_keeps_observed_text(self):
        result=process([fragment('Olá',0,0,score=.99)],100,30,'text',lambda *args:fragment('0lá',0,0,score=.8))
        self.assertEqual(result['full_text'],'Olá')
        self.assertTrue(result['uncertain'])

    def test_no_fabricated_word_geometry(self):
        raw=dict(text='a b',x=0,y=0,width=30,height=18,confidence=.9)
        result=process([raw],100,50)
        self.assertEqual(result['words'],[])
        self.assertTrue(result['uncertain'])

    def test_rerecognition_crop_box_does_not_erase_indent(self):
        raw=[fragment('for i in range(10):',0,0),fragment('if i:',30,30),fragment('print("a")',60,60)]
        def recognize(x,y,w,h):
            original=raw[min(range(3),key=lambda i:abs(raw[i]['y']-y))]
            candidate=dict(original)
            candidate.update(x=x,y=y,width=w,height=h)
            return candidate
        result=process(raw,300,100,'code',recognize)
        self.assertEqual(result['full_text'],'for i in range(10):\n   if i:\n      print("a")')

    def test_invalid_geometry_fails(self):
        with self.assertRaises(ValueError):process([dict(text='bad',x=float('nan'),y=0,width=4,height=4)],10,10)

    def test_quantized_ctc_advances_do_not_erase_indent(self):
        raw=[fragment('for i in range(10):',0,0,pitch=20),fragment('if i % 2 == 0:',60,35,pitch=20),fragment('print(f("%i is odd")',120,70,pitch=20)]
        for line in raw:
            for i,c in enumerate(line['characters']):
                c['x'] += [-2,1,0,3,-1][i%5]
        result=process(raw,600,120,'auto')
        self.assertEqual(result['full_text'],'for i in range(10):\n   if i % 2 == 0:\n      print(f("%i is odd")')

    def test_full_line_same_glyphs_preserves_existing_whitespace(self):
        raw=fragment('for i in range(10):',0,0)
        candidate=fragment('for iinrange(10):',0,0)
        result=process([raw],300,40,'text',lambda *args:candidate)
        self.assertEqual(result['full_text'],raw['text'])

    def test_missing_suffix_recovered_without_syntax_rules(self):
        raw=fragment('receipt total',0,0,score=.99)
        candidate=fragment('receipt total 83.25',0,0,score=.79)
        result=process([raw],300,40,'text',lambda *args:candidate)
        self.assertEqual(result['full_text'],'receipt total 83.25')
        self.assertTrue(result['uncertain'])


if __name__=='__main__':unittest.main()
