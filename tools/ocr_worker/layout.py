"""Spatial OCR reconstruction. Characters come exclusively from model output.

Coordinates and UTF-16 offsets refer to the original image and canonical full_text.
No dictionary, language parser or source-code repair is used.
"""
import math
import re
import statistics


def utf16(text):
    return len(text.encode('utf-16-le')) // 2


def rectangle(item):
    if 'polygon' in item:
        points=item['polygon']
        xs=[float(p[0]) for p in points]; ys=[float(p[1]) for p in points]
        values=(min(xs),min(ys),max(xs)-min(xs),max(ys)-min(ys))
    else:
        values=tuple(float(item[k]) for k in ('x','y','width','height'))
    if not all(math.isfinite(v) for v in values) or values[2]<=0 or values[3]<=0:
        raise ValueError('OCR coordinates must be finite with positive extents')
    return dict(zip(('x','y','width','height'),values))


def bounds(items):
    boxes=[rectangle(i) for i in items]
    x=min(b['x'] for b in boxes); y=min(b['y'] for b in boxes)
    return dict(x=x,y=y,width=max(b['x']+b['width'] for b in boxes)-x,height=max(b['y']+b['height'] for b in boxes)-y)


def confidence(item):
    value=item.get('confidence',0)
    if value is None: return 0
    return float(value)/100 if value>1 else float(value)


def group_visual_lines(fragments):
    """Group baseline-compatible fragments, then split large column gutters."""
    rows=[]
    for fragment in sorted(fragments,key=lambda a:(rectangle(a)['y']+rectangle(a)['height']/2,rectangle(a)['x'])):
        b=rectangle(fragment); center=b['y']+b['height']/2
        compatible=[]
        for row in rows:
            rb=bounds(row)
            overlap=max(0,min(rb['y']+rb['height'],b['y']+b['height'])-max(rb['y'],b['y']))
            delta=abs(center-(rb['y']+rb['height']/2))
            if overlap/min(rb['height'],b['height'])>=.5 and delta<=.6*max(rb['height'],b['height']):
                compatible.append((delta,row))
        if compatible: min(compatible,key=lambda pair:pair[0])[1].append(fragment)
        else: rows.append([fragment])
    split=[]
    for row in rows:
        row.sort(key=lambda a:rectangle(a)['x'])
        height=statistics.median(rectangle(a)['height'] for a in row)
        current=[]
        for fragment in row:
            if current:
                a=rectangle(current[-1]); b=rectangle(fragment)
                if b['x']-(a['x']+a['width'])>4*height:
                    split.append(current);current=[]
            current.append(fragment)
        if current:split.append(current)
    return sorted(split,key=lambda row:(bounds(row)['y'],bounds(row)['x']))


def _blocks(rows):
    """Separate simultaneous columns; nearby vertically successive rows share block."""
    blocks=[]
    for row in rows:
        b=bounds(row); candidates=[]
        for block in blocks:
            previous=bounds(block[-1]); total=bounds([f for r in block for f in r])
            overlap=min(total['x']+total['width'],b['x']+b['width'])-max(total['x'],b['x'])
            vertical=b['y']-(previous['y']+previous['height'])
            # Rows at the same visual height, split by a gutter, must remain columns.
            if overlap>0 and vertical>=-.35*min(previous['height'],b['height']) and vertical<3*max(previous['height'],b['height']):
                candidates.append((max(0,vertical),block))
        if candidates:min(candidates,key=lambda a:a[0])[1].append(row)
        else:blocks.append([row])
    # Column-major for vertically overlapping blocks, y-major otherwise.
    ordered=[]
    while blocks:
        first=min(blocks,key=lambda block:bounds([f for r in block for f in r])['y'])
        fb=bounds([f for r in first for f in r])
        peers=[block for block in blocks if bounds([f for r in block for f in r])['y']<fb['y']+fb['height']]
        peers.sort(key=lambda block:bounds([f for r in block for f in r])['x'])
        ordered.extend(peers)
        blocks=[block for block in blocks if not any(block is p for p in peers)]
    return ordered


def _ink_bounds(row):
    chars=[c for f in row for c in f.get('characters',[]) if c.get('text','').strip()]
    return bounds(chars if chars else row)


def _pitch(rows):
    """Robust character-center slope against literal source character positions.

    Longer spans suppress quantized CTC jitter; whitespace positions count as
    columns. The stability check rejects strongly proportional advances.
    """
    slopes=[];tracks=[]
    for row in rows:
        for fragment in row:
            text=fragment['text'];cursor=0;track=[]
            for char in fragment.get('characters',[]):
                token=char.get('text','')
                if not token.strip():continue
                index=text.find(token,cursor)
                if index<0:continue
                b=rectangle(char);track.append((index,b['x']+b['width']/2));cursor=index+len(token)
            local=[(y2-y1)/(i2-i1) for n,(i1,y1) in enumerate(track) for i2,y2 in track[n+1:] if 3<=i2-i1<=12 and y2>y1]
            if local:slopes.append(statistics.median(local));tracks.append(track)
    if len(slopes)<2 or sum(len(t) for t in tracks)<12:return None
    pitch=statistics.median(slopes)
    if pitch<=0:return None
    consistent=sum(abs(p/pitch-1)<.15 for p in slopes)/len(slopes)
    residuals=[]
    for track in tracks:
        anchor=statistics.median(y-i*pitch for i,y in track)
        residuals.extend(abs(y-(anchor+i*pitch))/pitch for i,y in track)
    return pitch if consistent>=.75 and sum(r<.35 for r in residuals)/len(residuals)>=.75 else None


def _join(row):
    text=''; characters=[]
    for fragment in sorted(row,key=lambda f:rectangle(f)['x']):
        fragment_text=fragment['text']
        if text and not text[-1].isspace() and fragment_text and not fragment_text[0].isspace():text+=' '
        offset=len(text)
        for c in fragment.get('characters',[]):
            item=dict(c)
            # Inference supplies Python-codepoint offsets into fragment text.
            start=c.get('start')
            if start is not None:item['start']=offset+start;item['end']=offset+c.get('end',start+len(c['text']))
            characters.append(item)
        text+=fragment_text
    return text,characters


def _words(text,characters,offset):
    """Word boxes are unions of CTC characters, never uniform-width guesses."""
    words=[]
    positioned=[]
    search=0
    for char in characters:
        start=char.get('start')
        if start is None:
            start=text.find(char['text'],search)
        if start<0:continue
        end=char.get('end',start+len(char['text']))
        search=end
        positioned.append((start,end,char))
    for match in re.finditer(r'\S+',text):
        members=[char for start,end,char in positioned if start<match.end() and end>match.start() and char.get('text','').strip()]
        if not members:continue
        words.append(dict(text=match.group(),**bounds(members),confidence=statistics.mean(confidence(c) for c in members)*100,start_utf16=offset+utf16(text[:match.start()]),end_utf16=offset+utf16(text[:match.end()]),characters=[dict(c,**rectangle(c),start_utf16=offset+utf16(text[:start]),end_utf16=offset+utf16(text[:end])) for start,end,c in positioned if c in members]))
    return words


def process(raw_lines,width,height,mode='auto',rerecognize=None):
    if mode not in ('auto','text','code'):raise ValueError('mode must be auto, text, or code')
    for line in raw_lines: rectangle(line)
    rows=group_visual_lines(raw_lines)
    blocks=_blocks(rows)
    evidence={'raw_lines':raw_lines,'line_recognition':[]}
    uncertainties=[]; canonical=[]; block_results=[]; output=''
    for block_id,block in enumerate(blocks):
        block_bounds=bounds([f for row in block for f in row])
        recovered=[]
        for row in block:
            original,chars=_join(row); chosen=row
            if rerecognize is not None:
                b=bounds(row);margin=max(2,round(b['height']*.15))
                x=max(0,math.floor(block_bounds['x']-margin));y=max(0,math.floor(b['y']-margin))
                right=min(width,math.ceil(block_bounds['x']+block_bounds['width']+margin));bottom=min(height,math.ceil(b['y']+b['height']+margin))
                candidate=rerecognize(x,y,right-x,bottom-y)
                record={'bounds':dict(x=x,y=y,width=right-x,height=bottom-y),'original_text':original,'candidate':candidate,'selected':False}
                if candidate and candidate.get('text','').strip():
                    prior=statistics.mean(confidence(f) for f in row)
                    score=confidence(candidate)
                    old=re.sub(r'\s','',original);new=re.sub(r'\s','',candidate['text'])
                    subsequence=iter(new)
                    retains=all(any(c==n for n in subsequence) for c in old)
                    if new==old or (retains and score>=.75) or score>=prior+.03:
                        if new==old:
                            candidate=dict(candidate, text=original)
                        chosen=[candidate];record['selected']=True
                    if new!=old:uncertainties.append({'kind':'recognition_disagreement','original_text':original,'candidate_text':candidate['text'],'selected':'whole_line' if record['selected'] else 'fragments','bounds':b})
                evidence['line_recognition'].append(record)
            recovered.append(chosen)
        pitch=_pitch(recovered)
        looks_code=mode=='code' or (mode=='auto' and pitch is not None and sum(bool(re.search(r'[(){}=;%]|:\s*$',f['text'])) for row in recovered for f in row)>=2)
        left=min(_ink_bounds(row)['x'] for row in block)
        if looks_code and pitch is None:uncertainties.append({'kind':'indentation_unresolved','block_id':block_id})
        block_start=len(canonical)
        for row,original_row in zip(recovered,block):
            text,characters=_join(row)
            leading=round(max(0,_ink_bounds(original_row)['x']-left)/pitch) if looks_code and pitch else 0
            prefix=' '*leading
            for char in characters:
                if 'start' in char:char['start']+=leading;char['end']+=leading
            text=prefix+text
            if output:output+='\n'
            offset=utf16(output)
            words=_words(text,characters,offset)
            line=dict(text=text,**_ink_bounds(row),words=words,block_id=block_id,line_id=len(canonical),start_utf16=offset,end_utf16=offset+utf16(text))
            if not words:uncertainties.append({'kind':'character_geometry_unavailable','line_id':line['line_id']})
            canonical.append(line);output+=text
        block_results.append(dict(block_id=block_id,**block_bounds,line_start=block_start,line_end=len(canonical),monospace_pitch=pitch if looks_code else None))
    return {'full_text':output,'lines':canonical,'words':[w for line in canonical for w in line['words']],'blocks':block_results,'evidence':evidence,'uncertainties':uncertainties,'uncertain':bool(uncertainties),'mode':mode,'coordinate_space':'original_image_pixels','offset_encoding':'utf16'}
