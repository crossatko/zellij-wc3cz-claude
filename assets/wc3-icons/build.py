#!/usr/bin/env python3
"""Build the WC3 Icons CBDT colour bitmap font (used by the zellij claude-tabs plugin in kitty on Linux).

usage: [SBIX=1] python3 build.py [OUT.ttf [ADVANCE_UNITS [CROP_PX [BITMAP_HEIGHT]]]]
defaults: wc3-icons.ttf next to this script, 1212, 4, 144; env FAMILY="WC3 Icons" PPEM=96 BY=116 BX=0 ASPECT=0.7778 CROP_KEEP=<ASPECT>.
Design: each bitmap is a centre-crop of the icon to the aspect ASPECT (width/height, no squash), so it is never
distorted; terminals only scale a colour bitmap uniformly to fit the 2-cell box (kitty), so any cell size works,
with transparent margins on the longer side. kitty scales a strike by font_px/PPEM (font_size 18 = 24 px, PPEM 96:
x0.25) and shrinks it (by whole pixels) as soon as it exceeds the cell box, so the bitmap must scale to EXACTLY the
box and not a hair under: 144 x 112 strike px * 0.25 = 36 x 28 px = the 2-cell box of 14x36 cells. (The earlier
145 px / PPEM 97 gave 35.88 px, drawn as a half-covered top row = a thin background line above the icon.)
BY (116) fixes the sub-pixel vertical position (0.25 px per unit; 115 leaves a half-covered top row, 117 a
half-covered bottom row). Limits of the CBDT small metrics: height/width <= 255, BY <= 127. See README.md. Needs python3 with fonttools and Pillow.
UPM=1000, ascent 800, descent 200. Glyphs: the 7 race icons at U+1F7E0.. in ICONS order (env BASE, EXTRA_BASES)
plus one glyph per row of characters.tsv (codepoint column, icon_file column, relative to this dir; rows with an
empty icon_file get no glyph, the plugin then falls back to the race icon)."""
import sys, io, os, csv
from PIL import Image
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import newTable
from fontTools.ttLib.tables.BitmapGlyphMetrics import SmallGlyphMetrics, BigGlyphMetrics
from fontTools.ttLib.tables import E_B_D_T_ as EBDT, E_B_L_C_ as EBLC
ICONS=["BTNPeasant","BTNPeon","BTNAcolyte","BTNWisp","BTNNagaMyrmidon","BTNPitLord","BTNHeroTinker"]
BASE=int(os.environ.get("BASE","1F7E0"),16)
here=os.path.dirname(os.path.abspath(__file__))
a=sys.argv[1:]
out=a[0] if len(a)>0 else os.path.join(here,"wc3-icons.ttf")
adv=int(a[1]) if len(a)>1 else 1212
crop=int(a[2]) if len(a)>2 else 4
S=int(a[3]) if len(a)>3 else 144
ASPECT=float(os.environ.get("ASPECT","0.7778"))  # bitmap width/height; 0 = square
W=round(S*ASPECT) if ASPECT else S
CROP_KEEP=float(os.environ.get("CROP_KEEP") or (ASPECT or 1))  # < 1: fraction of the (frame-cropped, square) width kept (centre); > 1: the height kept is 1/CROP_KEEP; default = ASPECT, so no squash
def has_frame(im):
    """True if the 64x64 icon carries the Warcraft III button frame: the two outermost px rings on all four
    sides are grey (mean max-min channel spread < 15; portraits at the edge are coloured). All 99 shipped icons
    have it (checked), so everything gets the CROP px cut; a frameless icon would be kept whole."""
    px=im.convert("RGB").load(); w,h=im.size
    def spread(k):
        v=[]
        for t in range(8,w-8):
            for x,y in ((t,k),(t,h-1-k),(k,t),(w-1-k,t)):
                r,g,b=px[x,y]; v.append(max(r,g,b)-min(r,g,b))
        return sum(v)/len(v)
    return spread(0)<15 and spread(1)<15
dl=os.path.join(here,"icons")  # 64x64 BTN*.webp sources
# character glyphs from characters.tsv: race character codepoint clips icon_file
CHARS=[]  # (codepoint, path)
with open(os.path.join(here,"characters.tsv"),encoding="utf-8",newline="") as f:
    for r in csv.DictReader(f,delimiter="\t"):
        if r["icon_file"]: CHARS.append((int(r["codepoint"].replace("U+",""),16),os.path.join(here,r["icon_file"])))
SRC=[os.path.join(dl,n+".webp") for n in ICONS]+[p for _,p in CHARS]  # glyph i <- SRC[i]
UPM=1000; ASC=800; DESC=200
names=[".notdef"]+[f"wc3_{i}" for i in range(len(SRC))]
fb=FontBuilder(UPM,isTTF=True)
fb.setupGlyphOrder(names)
cm={BASE+i:f"wc3_{i}" for i in range(len(ICONS))}
for extra in os.environ.get("EXTRA_BASES","").split(","):
    if extra: cm.update({int(extra,16)+i:f"wc3_{i}" for i in range(len(ICONS))})
for k,(cp,_) in enumerate(CHARS): cm[cp]=f"wc3_{len(ICONS)+k}"
fb.setupCharacterMap(cm)
empty=TTGlyphPen(None).glyph()
fb.setupGlyf({n:empty for n in names})
fb.setupHorizontalMetrics({n:(adv if n!=".notdef" else 500,0) for n in names})
fb.setupHorizontalHeader(ascent=ASC,descent=-DESC)
fb.setupNameTable({"familyName":os.environ.get("FAMILY","WC3 Icons"),"styleName":"Regular"})
fb.setupOS2(sTypoAscender=ASC,sTypoDescender=-DESC,usWinAscent=ASC,usWinDescent=DESC,sTypoLineGap=0)
fb.setupPost()
font=fb.font
# bitmaps: S px strike scaled so S px == 1000 units * (S/ppem)
ppem=int(os.environ.get('PPEM',96))  # 1 em == S px, glyph square S x S fills 1em; sits from -DESC.. ASC-? bearingY
bearingY=int(os.environ.get("BY",116))
BX=int(os.environ.get('BX',0))  # left bearing in strike px (centres a bitmap narrower than the 2-cell box)  # top of bitmap at ascender line
bitmaps={}
for i,path in enumerate(SRC):
    im=Image.open(path).convert("RGBA")
    if crop and has_frame(im): im=im.crop((crop,crop,64-crop,64-crop))  # only icons that carry the button frame
    if 0<CROP_KEEP<1:  # partial centre crop of the sides, the rest is squashed to W x S
        w,h=im.size; nw=round(w*CROP_KEEP); l=(w-nw)//2; im=im.crop((l,0,l+nw,h))
    elif CROP_KEEP>1:  # wider than tall (ASPECT > 1): centre crop of top and bottom instead
        w,h=im.size; nh=round(h/CROP_KEEP); t=(h-nh)//2; im=im.crop((0,t,w,t+nh))
    im=im.resize((W,S),Image.LANCZOS)
    b=io.BytesIO()
    im.save(b,"PNG",optimize=True)
    bitmaps[f"wc3_{i}"]=b.getvalue()
if os.environ.get("SBIX"):  # Apple sbix colour bitmap (macOS CoreText); same glyphs/bitmaps, see README "macOS"
    from fontTools.ttLib.tables.sbixStrike import Strike as SbixStrike
    from fontTools.ttLib.tables.sbixGlyph import Glyph as SbixGlyph
    fam=os.environ.get("FAMILY","WC3 Icons"); nm=font["name"]  # CoreText wants full + PostScript names too
    for nid,val in ((3,fam+" sbix"),(4,fam+" Regular"),(5,"Version 1.0"),(6,fam.replace(" ","")+"-Regular")):
        for pl,en,lg in ((3,1,0x409),(1,0,0)): nm.setName(val,nid,pl,en,lg)
    if os.environ.get("SBIX_BOX"):  # outline = box of the bitmap: ghostty draws nothing for a glyph without outline; kitty then shrinks the icon, so only for the ghostty (Wide) build
        k=UPM/ppem; x0,x1,y0,y1=round(BX*k),round((BX+W)*k),round((bearingY-S)*k),round(bearingY*k)
        for n in names[1:]:  # one glyph object per name (a shared object is compiled once, the others come out empty)
            pen=TTGlyphPen(None); pen.moveTo((x0,y0)); pen.lineTo((x0,y1)); pen.lineTo((x1,y1)); pen.lineTo((x1,y0)); pen.closePath()
            font["glyf"][n]=pen.glyph()
    sb=newTable("sbix"); sb.version=1; sb.flags=1; sb.strikes={}
    stk=SbixStrike(ppem=ppem,resolution=72); stk.glyphs={}
    for gname,png in bitmaps.items():
        stk.glyphs[gname]=SbixGlyph(glyphName=gname,graphicType="png ",imageData=png,originOffsetX=BX,originOffsetY=bearingY-S)
    stk.glyphs[".notdef"]=SbixGlyph(glyphName=".notdef",graphicType=None,imageData=None)
    sb.strikes[ppem]=stk
    font["sbix"]=sb
    font.save(out); print("wrote",out,os.path.getsize(out)); sys.exit(0)
# build CBDT / CBLC
cbdt=newTable("CBDT"); cblc=newTable("CBLC")
cbdt.version=3.0; cblc.version=3.0
strikeData={}
from fontTools.ttLib.tables.C_B_D_T_ import cbdt_bitmap_format_17 as ebdt_bitmap_format_17
gl={}
for gname,png in bitmaps.items():
    g=ebdt_bitmap_format_17(b"",font)
    g.metrics=SmallGlyphMetrics(); g.metrics.height=S; g.metrics.width=W
    g.metrics.BearingX=BX; g.metrics.BearingY=bearingY; g.metrics.Advance=int(S*adv/UPM)
    g.imageData=png
    gl[gname]=g
cbdt.strikeData=[gl]
from fontTools.ttLib.tables.E_B_L_C_ import Strike, BitmapSizeTable, SbitLineMetrics, eblc_index_sub_table_1
st=Strike(); bst=BitmapSizeTable()
bst.hori=SbitLineMetrics(); bst.vert=SbitLineMetrics()
for m in (bst.hori,bst.vert):
    m.ascender=bearingY; m.descender=-(S-bearingY); m.widthMax=W; m.caretSlopeNumerator=0; m.caretSlopeDenominator=0
    m.caretOffset=0; m.minOriginSB=0; m.minAdvanceSB=0; m.maxBeforeBL=0; m.minAfterBL=0; m.pad1=0; m.pad2=0
bst.colorRef=0; bst.startGlyphIndex=1; bst.endGlyphIndex=len(SRC)
bst.ppemX=ppem; bst.ppemY=ppem; bst.bitDepth=32; bst.flags=1
ist=eblc_index_sub_table_1(b"",font)
ist.indexFormat=1; ist.imageFormat=17; ist.firstGlyphIndex=1; ist.lastGlyphIndex=len(SRC)
ist.names=list(bitmaps.keys())
ist.imageSize=None
st.bitmapSizeTable=bst; st.indexSubTables=[ist]
cblc.strikes=[st]
font["CBDT"]=cbdt; font["CBLC"]=cblc
# bitmap-only (no glyf/loca), like Noto Color Emoji: kitty only treats such fonts as colour
del font["glyf"]; del font["loca"]
font["maxp"].tableVersion=0x00005000
for a in ("maxPoints","maxContours","maxCompositePoints","maxCompositeContours","maxZones","maxTwilightPoints","maxStorage","maxFunctionDefs","maxInstructionDefs","maxStackElements","maxSizeOfInstructions","maxComponentElements","maxComponentDepth"):
    if hasattr(font["maxp"],a): delattr(font["maxp"],a)
font.save(out); print("wrote",out,os.path.getsize(out))
