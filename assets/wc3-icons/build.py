#!/usr/bin/env python3
"""Build the WC3 Icons CBDT colour bitmap font (used by the zellij claude-tabs plugin in kitty on Linux).

usage: python3 build.py [OUT.ttf [ADVANCE_UNITS [CROP_PX [BITMAP_HEIGHT]]]]
defaults: wc3-icons.ttf next to this script, 1212, 4, 145; env FAMILY="WC3 Icons" PPEM=97 BY=116 ASPECT=0.7778.
The defaults reproduce the shipped font, tuned for kitty font_size 18 + modify_font cell_height 110%
(cells 14x36 px): glyph height = BITMAP_HEIGHT*24/PPEM must stay just below the cell height, other
sizes need retuning (see README.md). Needs python3 with fonttools and Pillow.
UPM=1000, ascent 800, descent 200. Glyphs U+1F7E0.. in ICONS order (env BASE, EXTRA_BASES)."""
import sys, io, os
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
S=int(a[3]) if len(a)>3 else 145
ASPECT=float(os.environ.get("ASPECT","0.7778"))  # glyph width/height; 0 = square
W=round(S*ASPECT) if ASPECT else S
dl=os.path.join(here,"icons")  # 64x64 BTN*.webp sources
UPM=1000; ASC=800; DESC=200
names=[".notdef"]+[f"wc3_{i}" for i in range(len(ICONS))]
fb=FontBuilder(UPM,isTTF=True)
fb.setupGlyphOrder(names)
cm={BASE+i:f"wc3_{i}" for i in range(len(ICONS))}
for extra in os.environ.get("EXTRA_BASES","").split(","):
    if extra: cm.update({int(extra,16)+i:f"wc3_{i}" for i in range(len(ICONS))})
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
ppem=int(os.environ.get('PPEM',97))  # 1 em == S px, glyph square S x S fills 1em; sits from -DESC.. ASC-? bearingY
bearingY=int(os.environ.get('BY',116))  # top of bitmap at ascender line
bitmaps={}
for i,n in enumerate(ICONS):
    im=Image.open(f"{dl}/{n}.webp").convert("RGBA")
    if crop: im=im.crop((crop,crop,64-crop,64-crop))
    if ASPECT:  # crop sides (centre) to the cell-box aspect instead of stretching
        w,h=im.size; nw=round(h*ASPECT); l=(w-nw)//2; im=im.crop((l,0,l+nw,h))
    im=im.resize((W,S),Image.LANCZOS)
    b=io.BytesIO(); im.save(b,"PNG",optimize=True)
    bitmaps[f"wc3_{i}"]=b.getvalue()
# build CBDT / CBLC
cbdt=newTable("CBDT"); cblc=newTable("CBLC")
cbdt.version=3.0; cblc.version=3.0
strikeData={}
from fontTools.ttLib.tables.C_B_D_T_ import cbdt_bitmap_format_17 as ebdt_bitmap_format_17
gl={}
for gname,png in bitmaps.items():
    g=ebdt_bitmap_format_17(b"",font)
    g.metrics=SmallGlyphMetrics(); g.metrics.height=S; g.metrics.width=W
    g.metrics.BearingX=0; g.metrics.BearingY=bearingY; g.metrics.Advance=int(S*adv/UPM)
    g.imageData=png
    gl[gname]=g
cbdt.strikeData=[gl]
from fontTools.ttLib.tables.E_B_L_C_ import Strike, BitmapSizeTable, SbitLineMetrics, eblc_index_sub_table_1
st=Strike(); bst=BitmapSizeTable()
bst.hori=SbitLineMetrics(); bst.vert=SbitLineMetrics()
for m in (bst.hori,bst.vert):
    m.ascender=bearingY; m.descender=-(S-bearingY); m.widthMax=W; m.caretSlopeNumerator=0; m.caretSlopeDenominator=0
    m.caretOffset=0; m.minOriginSB=0; m.minAdvanceSB=0; m.maxBeforeBL=0; m.minAfterBL=0; m.pad1=0; m.pad2=0
bst.colorRef=0; bst.startGlyphIndex=1; bst.endGlyphIndex=len(ICONS)
bst.ppemX=ppem; bst.ppemY=ppem; bst.bitDepth=32; bst.flags=1
ist=eblc_index_sub_table_1(b"",font)
ist.indexFormat=1; ist.imageFormat=17; ist.firstGlyphIndex=1; ist.lastGlyphIndex=len(ICONS)
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
