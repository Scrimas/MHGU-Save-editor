"""runchar.py : character block round trip.  load chain 0x3e77b0, then write chain 0x3e75f8.

    runchar.py SAVE [SLOTBASE]     -> scratch/charmap.pkl
"""
import sys, struct, pickle
from emu import *

save = open(sys.argv[1], 'rb').read()
CB = int(sys.argv[2], 16) if len(sys.argv) > 2 and not sys.argv[2].startswith('-') else 0x18CC9C
START = CB + 632
LOAD, WRITE = 0x3e77b0, 0x3e75f8
RBITS, WBITS = 0x3df22c, 0x3df300

FAKES = [0x143894]      # message lookup (resource not loaded) -> dummy object
assign, funcs = {}, {}
if '--resume' in sys.argv:
    assign, funcs = pickle.load(open(ROOT + 'scratch/charassign.pkl', 'rb'))

def add_bit_hooks(e):
    def rb(uc):
        st = STREAM
        bitpos, cur = struct.unpack('<II', uc.mem_read(st + 4, 8))
        n = uc.reg_read(UC_ARM_REG_R1); lr = uc.reg_read(UC_ARM_REG_LR)
        rec = {'k': 'RB', 'off': cur - FILEBUF, 'bit': bitpos, 'n': n, 'pc': lr - BASE}
        e.log.append(rec)
        def ret(uc2, a, s, d):
            e.pending = (rec, uc2.reg_read(UC_ARM_REG_R0)); rec['val'] = uc2.reg_read(UC_ARM_REG_R0)
        h = []
        h.append(uc.hook_add(UC_HOOK_CODE, lambda uc2, a, s, d: (ret(uc2, a, s, d), uc2.hook_del(h[0])), begin=lr, end=lr))
    def wb(uc):
        bitpos, cur = struct.unpack('<II', uc.mem_read(STREAM + 4, 8))
        e.log.append({'k': 'WB', 'off': cur - OUTBUF, 'bit': bitpos, 'n': uc.reg_read(UC_ARM_REG_R2),
                      'val': uc.reg_read(UC_ARM_REG_R1), 'pc': uc.reg_read(UC_ARM_REG_LR) - BASE})
    e.hook_pc(A(RBITS), rb); e.hook_pc(A(WBITS), wb)

for it in range(200):
    e = Emu(save, assign, funcs); uc = e.setup(); add_bit_hooks(e)
    for fa in FAKES: e.fake(fa, e.scratch)
    e.stream(FILEBUF, START)
    err = e.call(LOAD, 0, STREAM, limit=20_000_000)
    pc = uc.reg_read(UC_ARM_REG_PC)
    if e.fault is None and pc == RET:
        break
    if e.fault is None and err is None:
        print('stopped at', hex(pc - BASE), 'lr', hex(uc.reg_read(UC_ARM_REG_LR) - BASE)); break
    if e.fault is None:
        print('error without fault', err, hex(uc.reg_read(UC_ARM_REG_PC) - BASE)); break
    slot, base, bval = e.null_slot()
    acc, addr, pc = e.fault[:3]
    print('fault', e.fault[3:] , hex(addr), 'pc', hex(pc - BASE), 'lr', hex(uc.reg_read(UC_ARM_REG_LR) - BASE), 'base', bval and hex(bval), 'slot', slot and hex(slot))
    if slot is None or slot in assign or slot in funcs or addr >= 0x01000000 and bval not in (0, None):
        print('cannot resolve'); break
    if base == 'func': funcs[slot] = 'alloc'
    else: assign[slot] = len(set(assign.values()))
pickle.dump((assign, funcs), open(ROOT + 'scratch/charassign.pkl', 'wb'))
end = struct.unpack('<I', uc.mem_read(STREAM + 8, 4))[0] - FILEBUF
print('load done: stream end', hex(end), 'len', hex(end - CB), 'objects', len(set(assign.values())), 'unknown imports', dict(e.unknown))
loadlog = e.log

# writer, same objects
e.log = []; e.pending = None
uc.mem_write(OUTBUF, save[:BUF_SZ])     # pre-fill so untouched bytes compare equal
uc.mem_write(OUTBUF + START, b'\0' * (end - START))
e.stream(OUTBUF, START)
err = e.call(WRITE, 0, STREAM)
wend = struct.unpack('<I', uc.mem_read(STREAM + 8, 4))[0] - OUTBUF
print('write:', err, 'end', hex(wend))
out = bytes(uc.mem_read(OUTBUF, len(save)))
diff = [i for i in range(START, end) if out[i] != save[i]]
print('round-trip diffs', len(diff), [hex(i - CB) for i in diff[:20]])
pickle.dump({'load': loadlog, 'write': e.log, 'end': end, 'diff': diff, 'assign': assign}, open(ROOT + 'scratch/charmap.pkl', 'wb'))
