"""Dataset materialization. Synthetic labels derive from font geometry, never OCR."""
import hashlib
import json
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[2]


def box(text, x, y, w, h):
    return dict(text=text, x=x, y=y, width=w, height=h)


def materialize(manifest_path, output, font_override=None):
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    cases = []
    output.mkdir(parents=True, exist_ok=True)
    for entry in manifest['cases']:
        original = ROOT / entry['image']
        if not original.is_file():
            raise FileNotFoundError(f'Missing required fixture: {original}')
        image = Image.open(original).convert('RGB')
        if 'crop' in entry:
            image = image.crop(tuple(entry['crop']))
        image_path = output / (entry['id'] + '.png')
        image.save(image_path)
        truth = entry['truth']
        if not truth.get('lines') or truth.get('full_text') != '\n'.join(line['text'] for line in truth['lines']):
            raise ValueError(f'Missing or inconsistent literal annotations: {entry["id"]}')
        if any(not line.get('words') for line in truth['lines']):
            raise ValueError(f'Missing word annotations: {entry["id"]}')
        for line in truth['lines']:
            for w in line['words']:
                if not (w['width'] > 0 and w['height'] > 0 and w['x'] >= 0 and w['y'] >= 0 and w['x']+w['width'] <= image.width and w['y']+w['height'] <= image.height):
                    raise ValueError(f'Annotation out of bounds: {entry["id"]}: {w}')
        cases.append({**entry, 'image_path':str(image_path), 'source_sha256':hashlib.sha256(original.read_bytes()).hexdigest()})
    font_path = Path(font_override) if font_override else ROOT / manifest['synthetic']['font']
    if not font_path.is_file():
        raise FileNotFoundError(f'Synthetic font missing: {font_path}; supply --font to generate fixtures with a local monospace TTF')
    return cases + synthetic_cases(font_path, output)


def synthetic_cases(font_path, output):
    font = ImageFont.truetype(str(font_path), 23)
    texts = {
        'pt': ['A tradução mantém acentos: ação, órgão, café.', 'Número do pedido: 2026; preço R$ 19,90.', 'Olá, você consegue ler cada palavra?'],
        'en': ['Read every word without changing the source.', 'Invoice number: 2026; total $19.90.', 'Code: if i % 2 == 0: print("even")'],
    }
    cases = []
    for lang, lines in texts.items():
        for theme in ('dark','light'):
            image = Image.new('RGB',(850,180),'#181b20' if theme == 'dark' else '#ffffff')
            draw = ImageDraw.Draw(image)
            truth_lines = []
            for row, text in enumerate(lines):
                x, y = 20, 15+row*48
                draw.text((x,y),text,font=font,fill='#e6e8ed' if theme == 'dark' else '#17191d')
                words = []
                import re
                for match in re.finditer(r'\S+',text):
                    left = x + draw.textlength(text[:match.start()],font=font)
                    b = draw.textbbox((left,y),match.group(),font=font)
                    words.append(box(match.group(),b[0],b[1],b[2]-b[0],b[3]-b[1]))
                truth_lines.append({'text':text,'words':words})
            case_id = f'synthetic_{lang}_{theme}'
            path = output / (case_id+'.png')
            image.save(path)
            cases.append({'id':case_id,'image_path':str(path),'provenance':'Synthetic, generated with Pillow; exact font-metric boxes (not manually annotated screenshots).','truth':{'full_text':'\n'.join(lines),'lines':truth_lines},'font_sha256':hashlib.sha256(font_path.read_bytes()).hexdigest()})
    return cases
