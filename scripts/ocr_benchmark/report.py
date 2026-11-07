"""Self-contained HTML comparison; all OCR content escaped."""
import base64
import html
import io
import json
from PIL import Image, ImageDraw


def render_image(path, result):
    im = Image.open(path).convert('RGB')
    draw = ImageDraw.Draw(im)
    for line in result['lines']:
        for w in line.get('words', []):
            x, y = w['x'], w['y']
            draw.rectangle((x, y, x+w['width'], y+w['height']), outline='#00bfff', width=1)
    data = io.BytesIO()
    im.save(data, format='PNG')
    return 'data:image/png;base64,' + base64.b64encode(data.getvalue()).decode()


def write_report(path, cases, partial=False, summary=None):
    esc = html.escape
    parts = ['<!doctype html><meta charset="utf-8"><title>DeskLens OCR comparison</title><style>body{font:15px system-ui;margin:24px;background:#17191d;color:#eee}.row{display:flex;gap:20px;overflow:auto}.card{min-width:380px;max-width:650px;background:#252830;padding:16px}img{max-width:100%}pre{white-space:pre-wrap}td,th{padding:5px;text-align:left}table{font-size:12px}</style><h1>DeskLens OCR comparison</h1><p>Local experiment. Blue boxes are output, not ground truth. Exact lines include whitespace. Box scores use whitespace-word annotations and one-to-one IoU ≥ 0.5; different word segmentation affects these scores.</p>']
    if partial:
        parts.append('<h2>PARTIAL RUN: RapidOCR not executed</h2>')
    parts.append('<p>Evidence limitation: small dataset, repeated Asimov content, existing overlays. This experiment cannot justify production engine promotion. Acquire clean held-out captures first.</p>')
    if summary:
        parts.append('<h2>Weighted aggregate (descriptive only)</h2><pre>' + esc(json.dumps(summary,indent=2)) + '</pre>')
    for case in cases:
        parts.append('<h2>' + esc(case['id']) + '</h2><p>' + esc(case['provenance']) + '</p><div class="row">')
        entries = [('Ground truth', {'result':case['truth']})] + list(case['runs'].items())
        for name, run in entries:
            parts.append('<section class="card"><h3>' + esc(name) + '</h3><img src="' + render_image(case['image_path'], run['result']) + '"><pre>' + esc(run['result']['full_text']) + '</pre>')
            if 'metrics' in run:
                parts.append('<table>' + ''.join('<tr><th>' + esc(k) + '</th><td>' + esc(str(round(v,4) if isinstance(v,float) else v)) + '</td></tr>' for k,v in run['metrics'].items()) + '</table><pre>' + esc(json.dumps(run['timing'], indent=2)) + '</pre>')
            parts.append('</section>')
        parts.append('</div>')
    path.write_text(''.join(parts), encoding='utf-8')
