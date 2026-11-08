"""Reproducible clean synthetic evaluation corpus. Never described as real captures.

20 development and 10 content-disjoint held-out sources are fixed in corpus.json.
Rendering/validation does not invoke OCR; evaluation of held-out cases is explicit.
"""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT=Path(__file__).resolve().parents[2]
SOURCE=Path(__file__).with_name('corpus.json')


def sources():
    data=json.loads(SOURCE.read_text(encoding='utf-8'))
    cases=data['cases']
    if len(cases)!=30 or sum(c['split']=='development' for c in cases)!=20 or sum(c['split']=='heldout' for c in cases)!=10:
        raise ValueError('Corpus requires exactly 20 development + 10 held-out sources')
    ids=[c['id'] for c in cases]
    texts=['\n'.join(l['text'] for l in c['lines']) for c in cases]
    if len(set(ids))!=len(ids) or len(set(texts))!=len(texts):raise ValueError('Duplicate identities or source contents across corpus')
    return cases


def render(case,output):
    import re
    mono=case['font']=='mono'
    font_path=ROOT/'scripts/ocr_benchmark/assets'/('DejaVuSansMono.ttf' if mono else 'DejaVuSans.ttf')
    font=ImageFont.truetype(str(font_path),case['font_size'])
    dark=case['theme']=='dark'
    image=Image.new('RGB',(case['width'],case['height']),'#191d24' if dark else '#ffffff')
    draw=ImageDraw.Draw(image);lines=[]
    for source in case['lines']:
        text=source['text'];x=source['x'];y=source['y']
        draw.text((x,y),text,font=font,fill='#e7eaf1' if dark else '#15181c')
        words=[]
        for match in re.finditer(r'\S+',text):
            left=x+draw.textlength(text[:match.start()],font=font)
            b=draw.textbbox((left,y),match.group(),font=font)
            if b[0]<0 or b[1]<0 or b[2]>image.width or b[3]>image.height:raise ValueError(f'Clipped word in {case["id"]}: {match.group()}')
            words.append(dict(text=match.group(),x=b[0],y=b[1],width=b[2]-b[0],height=b[3]-b[1]))
        lines.append(dict(text=text,words=words))
    output.mkdir(parents=True,exist_ok=True)
    path=output/(case['id']+'.png');image.save(path)
    return dict(id=case['id'],split=case['split'],category=case['category'],image_path=str(path),mode=case['mode'],provenance='Clean deterministic synthetic rendering from fixed literal source; not an acquired application screenshot.',font_sha256=hashlib.sha256(font_path.read_bytes()).hexdigest(),truth=dict(full_text='\n'.join(l['text'] for l in lines),lines=lines))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--split',choices=['development','heldout','all'],default='development')
    args=parser.parse_args()
    cases=[render(c,args.output) for c in sources() if args.split=='all' or c['split']==args.split]
    (args.output/'manifest.json').write_text(json.dumps({'cases':cases},ensure_ascii=False,indent=2)+'\n',encoding='utf-8')


if __name__=='__main__':main()
