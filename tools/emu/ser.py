import struct,sys
import os
m=open(os.path.dirname(os.path.abspath(__file__))+'/../../scratch/mem.bin','rb').read()
u=lambda a: struct.unpack_from('<I',m,a)[0]
WR=0x3df378; WBITS=0x3df300
def rot(v,r): return ((v>>r)|(v<<(32-r)))&0xffffffff
def imm(w): return rot(w&0xff,((w>>8)&0xf)*2)
def walk(start,depth=0,maxn=400):
    reg={}
    a=start; out=[]
    for _ in range(maxn):
        w=u(a); c=w>>28
        if (w&0x0ff00000)==0x03000000: reg[(w>>12)&0xf]=((w>>4)&0xf000)|(w&0xfff)   # movw
        elif (w&0x0ff00000)==0x03400000: reg[(w>>12)&0xf]=reg.get((w>>12)&0xf,0)|((((w>>4)&0xf000)|(w&0xfff))<<16)
        elif (w&0x0fef0000)==0x03a00000: reg[(w>>12)&0xf]=imm(w)   # mov imm
        elif (w&0x0fe00000)==0x02800000: # add rd,rn,#imm
            rd=(w>>12)&0xf; rn=(w>>16)&0xf
            reg[rd]=('obj',imm(w)) if rn in(4,5,6,7,8,0) and not isinstance(reg.get(rn),int) else None
        elif (w&0x0ff00ff0)==0x00800000: # add rd,rn,rm
            rd=(w>>12)&0xf; rm=w&0xf
            reg[rd]=('obj',reg.get(rm)) if isinstance(reg.get(rm),int) else None
        elif (w&0x0f000000)==0x0b000000 or (w&0x0f000000)==0x0a000000:
            off=w&0xffffff
            if off&0x800000: off-=0x1000000
            t=(a+8+off*4)&0xffffffff
            link=(w&0x0f000000)==0x0b000000
            if t==WR: out.append((a,'W',reg.get(1),reg.get(2)))
            elif t==WBITS: out.append((a,'BITS',reg.get(1),reg.get(2)))
            elif link or (c==0xe and not (start<=t<a+0x1000)): out.append((a,'CALL' if link else 'TAIL',hex(t),None))
            elif not link: out.append((a,'BR',hex(t),c))
            if not link and c==0xe and t==WR: break
        elif (w&0x0fff0000)==0x08bd0000 and (w&0x8000) and c==0xe: break  # pop pc
        elif w==0xe12fff1e: break
        a+=4
    return out
if __name__=='__main__':
    for s in sys.argv[1:]:
        print('==',s)
        for o in walk(int(s,16)): print(' ',hex(o[0]),o[1],o[2] if not isinstance(o[2],tuple) else 'obj+%s'%(hex(o[2][1]) if isinstance(o[2][1],int) else o[2][1]),o[3])
