"""Render the maintained project report as a standalone PDF.

Requires reportlab. Run from any directory; the source is repository-owned.
"""
from pathlib import Path
import argparse
import re
from xml.sax.saxutils import escape
from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT
from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle, PageBreak

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT/'output/pdf/desklens-relatorio-tecnico-2026-11-09.pdf')
    args = parser.parse_args()
    font = ROOT/'scripts/ocr_benchmark/assets/DejaVuSans.ttf'
    pdfmetrics.registerFont(TTFont('ReportSans', str(font)))
    pdfmetrics.registerFontFamily('ReportSans',normal='ReportSans',bold='ReportSans',italic='ReportSans')
    styles = getSampleStyleSheet()
    for name,size,leading,color in [('Body',9.4,14,'#283548'),('TitleReport',27,33,'#11233e'),('Chapter',19,25,'#11233e'),('Sub',11.5,16,'#16709c'),('Cell',8.2,11.5,'#283548'),('CodeReport',8.0,12,'#203a54')]:
        styles.add(ParagraphStyle(name,fontName='ReportSans',fontSize=size,leading=leading,textColor=colors.HexColor(color),spaceAfter=(6 if name=='Body' else 8) if name!='Cell' else 0,alignment=TA_LEFT,keepWithNext=name in ('TitleReport','Chapter','Sub')))
    def inline(text):
        text=escape(text)
        text=re.sub(r'\*\*(.+?)\*\*',r'<b>\1</b>',text)
        return re.sub(r'`([^`]+)`',r'<font color="#16709c">\1</font>',text)
    story=[]
    source=(ROOT/'docs/relatorio-tecnico.md').read_text(encoding='utf-8')
    pages=source.split('<!-- page -->')
    for page_number,chunk in enumerate(pages):
        if page_number:story.append(PageBreak())
        lines=chunk.strip().splitlines();i=0
        while i<len(lines):
            line=lines[i].strip()
            if not line:i+=1;continue
            if line.startswith('```'):
                code=[];i+=1
                while i<len(lines) and not lines[i].strip().startswith('```'):
                    code.append(escape(lines[i]));i+=1
                box=Table([[Paragraph('<br/>'.join(code),styles['CodeReport'])]],colWidths=[495])
                box.setStyle(TableStyle([('BACKGROUND',(0,0),(-1,-1),colors.HexColor('#edf4fa')),('BOX',(0,0),(-1,-1),.5,colors.HexColor('#d6e2ed')),('LEFTPADDING',(0,0),(-1,-1),11),('RIGHTPADDING',(0,0),(-1,-1),11),('TOPPADDING',(0,0),(-1,-1),9),('BOTTOMPADDING',(0,0),(-1,-1),9)]))
                story.extend([box,Spacer(1,10)])
            elif line.startswith('|'):
                rows=[]
                while i<len(lines) and lines[i].strip().startswith('|'):
                    cells=[x.strip() for x in lines[i].strip().strip('|').split('|')]
                    if not all(re.fullmatch(r'[- :]+',x) for x in cells):rows.append(cells)
                    i+=1
                n=len(rows[0]); widths=[160,335] if n==2 else ([110,110,95,180] if n==4 else [495/n]*n)
                table=Table([[Paragraph(inline(c),styles['Cell']) for c in r] for r in rows],colWidths=widths,repeatRows=1,hAlign='LEFT')
                table.setStyle(TableStyle([('BACKGROUND',(0,0),(-1,0),colors.HexColor('#e0edf7')),('ROWBACKGROUNDS',(0,1),(-1,-1),[colors.HexColor('#f5f8fb'),colors.white]),('LINEBELOW',(0,0),(-1,0),.8,colors.HexColor('#4b89b4')),('LINEBELOW',(0,1),(-1,-1),.3,colors.HexColor('#d8e2eb')),('VALIGN',(0,0),(-1,-1),'TOP'),('LEFTPADDING',(0,0),(-1,-1),8),('RIGHTPADDING',(0,0),(-1,-1),8),('TOPPADDING',(0,0),(-1,-1),6),('BOTTOMPADDING',(0,0),(-1,-1),6)]))
                story.extend([table,Spacer(1,12)]);continue
            elif line.startswith('# '):story.append(Paragraph(inline(line[2:]),styles['TitleReport' if page_number==0 else 'Chapter']))
            elif line.startswith('## '):story.append(Paragraph(inline(line[3:]),styles['Chapter']))
            elif line.startswith('### '):story.append(Paragraph(inline(line[4:]),styles['Sub']))
            else:story.append(Paragraph(inline(line),styles['Body']))
            i+=1
    def furniture(canvas,doc):
        canvas.setStrokeColor(colors.HexColor('#2b85b4'));canvas.setLineWidth(1)
        canvas.line(50,802,545,802)
        canvas.setFont('ReportSans',8);canvas.setFillColor(colors.HexColor('#53677e'))
        canvas.drawString(50,813,'DESKLENS  /  DOCUMENTAÇÃO TÉCNICA')
        canvas.drawRightString(545,813,'09 NOV 2026')
        canvas.setStrokeColor(colors.HexColor('#d8e2eb'));canvas.line(50,42,545,42)
        canvas.drawString(50,28,'Gustavo Geminiano  |  Referência: 5d480f6')
        canvas.drawRightString(545,28,f'{doc.page}')
    args.output.parent.mkdir(parents=True,exist_ok=True)
    doc=SimpleDocTemplate(str(args.output),pagesize=A4,leftMargin=50,rightMargin=50,topMargin=60,bottomMargin=56,title='DeskLens - Relatório técnico e arquitetural atualizado',author='Gustavo Geminiano',subject='Estrutura, funcionalidades, tecnologias e validação - 09/11/2026')
    doc.build(story,onFirstPage=furniture,onLaterPages=furniture)
    print(args.output)


if __name__=='__main__':main()
