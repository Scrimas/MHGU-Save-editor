"""runfull.py : run the game's full-save loader 0x3e7d10 on a save and log every field read.

    runfull.py SAVE [--resume]      -> scratch/fullmap.pkl  (load log + object assignment)

Loader order: block A at body+[body+8] (managers 0x52163c 0x263298 0x2257e4 0x16424c 0x3f8ef8),
block B at body+[body+0xc] (0x36bdc), then character slots 1-3 (0x3e0db8, body+[body+0x10+4i]).
body = file + 0x24.
"""
import sys, struct, pickle, time
from emu import *

save = open(sys.argv[1], 'rb').read()
BODY = 0x24
LOAD = 0x3e7d10
RBITS = 0x3df22c
FAKES = [0x143894, 0x3cebdc]      # message lookup (resource not loaded), zlib compress of a container element
STATE = ROOT + 'scratch/fullassign.pkl'

assign, funcs = {}, {}
if '--resume' in sys.argv:
    assign, funcs = pickle.load(open(STATE, 'rb'))

def add_bit_hooks(e):
    def rb(uc):
        st = uc.reg_read(UC_ARM_REG_R0)
        bitpos, cur = struct.unpack('<II', uc.mem_read(st + 4, 8))
        n = uc.reg_read(UC_ARM_REG_R1); lr = uc.reg_read(UC_ARM_REG_LR)
        rec = {'k': 'RB', 'off': cur - FILEBUF, 'bit': bitpos, 'n': n, 'pc': lr - BASE, 'mgr': e.mgr}
        e.log.append(rec)
        h = []
        def ret(uc2, a, s, d):
            v = uc2.reg_read(UC_ARM_REG_R0); rec['val'] = v; e.set_pending(rec, v); uc2.hook_del(h[0])
        h.append(uc.hook_add(UC_HOOK_CODE, ret, begin=lr, end=lr))
    e.hook_pc(A(RBITS), rb)

def bl_targets(start, end):
    out = []
    for a in range(start, end, 4):
        w = u32(MEM, a)
        if (w & 0x0f000000) in (0x0b000000, 0x0a000000) and (w >> 28) == 0xe:
            off = w & 0xffffff
            if off & 0x800000: off -= 0x1000000
            out.append(a + 8 + off * 4)
    return out
# manager loaders: char chain 0x3e77b0 (to 0x3e7938), common chain 0x3e7d10 (to 0x3e7f1c)
MANAGERS = [t for t in bl_targets(0x3e77b0, 0x3e7938) + bl_targets(0x3e7d10, 0x3e7f1c) if t not in (0x3e0db8,)]

def add_attrib(e):
    e.mgr = None; e.caller = None
    for t in MANAGERS:
        e.hook_pc(A(t), (lambda t: lambda uc: setattr(e, 'mgr', t))(t))
    e.hook_pc(A(0x3df2a8), lambda uc: setattr(e, 'caller', uc.reg_read(UC_ARM_REG_LR) - BASE))
    e.hook_pc(A(0x51dbb4), seed_size_records)

def seed_size_records(uc):
    """S+0x950/+0x954: list of (ptr, n u16) filled at startup; the monster size records are 137 x (min, max)"""
    s = uc.reg_read(UC_ARM_REG_R0)
    if struct.unpack('<I', uc.mem_read(s + 0x954, 4))[0] == 0:
        lst = STREAM + 0xc00; buf = OBJ + 99 * OBJ_SZ
        uc.mem_map(buf, 0x1000) if not any(r[0] <= buf < r[1] + 1 for r in uc.mem_regions()) else None
        uc.mem_write(lst, struct.pack('<II', buf, 274))
        uc.mem_write(s + 0x950, struct.pack('<II', lst, 1))

THIS = STREAM + 0x800
# save managers: (site of `bl new` in the main init 0x3d8b10, object size); ctor runs with r0 = new object
CTORS = [(0x3d90d4, 176), (0x3d90fc, None), (0x3d923c, None), (0x3d9250, None), (0x3d9264, None), (0x3d927c, None),
         (0x3d92f4, None), (0x3d93d8, None), (0x3d93f4, None), (0x3d9410, None), (0x3d942c, None), (0x3d9484, 0xbca4),
         (0x3d94a0, None), (0x3d94bc, None), (0x3d9534, None)]
def r0_before(site):
    """constant loaded into r0 just before `bl new` at site"""
    v = None
    for a in range(site - 32, site, 4):
        w = u32(MEM, a)
        if (w & 0x0ff0f000) == 0x03000000: v = ((w >> 4) & 0xf000) | (w & 0xfff)                     # movw r0
        elif (w & 0x0ff0f000) == 0x03400000 and v is not None: v |= (((w >> 4) & 0xf000) | (w & 0xfff)) << 16   # movt r0
        elif (w & 0x0fff0000) == 0x03a00000 and (w >> 12) & 0xf == 0:                                  # mov r0,#imm
            r = ((w >> 8) & 0xf) * 2; x = w & 0xff; v = ((x >> r) | (x << (32 - r))) & 0xffffffff
    return v
CTORS = [(s, z or r0_before(s)) for s, z in CTORS]
MAINOBJ = STREAM + 0x400

def construct(e):
    for site, size in CTORS:
        t = time.time()
        err = e.call(site, size, 16, until=site + 8, regs={UC_ARM_REG_R7: MAINOBJ}, limit=int(os.environ.get('CLIMIT', 2_000_000_000)))
        if os.environ.get('VERBOSE'): print('  ctor', hex(site), '%.1fs' % (time.time() - t), err, flush=True)
        pc = e.uc.reg_read(UC_ARM_REG_PC)
        if e.fault is not None or pc != A(site + 8):
            return site, err
    return None
t0 = time.time()
for it in range(400):
    e = Emu(save, assign, funcs); uc = e.setup(); add_bit_hooks(e); add_attrib(e)
    for fa in FAKES: e.fake(fa, e.scratch)
    e.fake(0x7aa47c, e.falloc)      # allocator lookup -> fake allocator
    bad = construct(e)
    if bad and e.fault is None:
        print('ctor stopped', hex(bad[0]), bad[1], hex(uc.reg_read(UC_ARM_REG_PC) - BASE), flush=True); break
    err = None if bad else e.call(LOAD, THIS, FILEBUF + BODY, limit=int(os.environ.get('LIMIT', 200_000_000)), r2=0x4eb898)
    pc = uc.reg_read(UC_ARM_REG_PC)
    if e.fault is None and pc == RET and not bad:
        break
    if e.fault is None:
        print('stopped', err, 'at', hex(pc - BASE), 'lr', hex(uc.reg_read(UC_ARM_REG_LR) - BASE), flush=True); break
    if e.fault[0] == 'bigcopy':
        f = e.fault
        print('bigcopy', hex(f[1]), f[3], hex(f[4]), 'lr', hex(f[5]), 'caller', e.caller and hex(e.caller), 'mgr', e.mgr and hex(e.mgr),
              'file', hex(e.log[-1]['off']) if e.log else None, flush=True); break
    slot, base, bval = e.null_slot()
    acc, addr, fpc = e.fault[:3]
    last = e.log[-1]['off'] if e.log else 0
    print('%4d %5.0fs fault %s %s pc %s lr %s slot %s  at file %s' % (it, time.time() - t0, e.fault[3:], hex(addr), hex(fpc - BASE),
          hex(uc.reg_read(UC_ARM_REG_LR) - BASE), slot and hex(slot), hex(last)), flush=True)
    if slot is None or slot in assign or slot in funcs:
        print('cannot resolve', flush=True); break
    if base == 'func': funcs[slot] = 'alloc'
    else: assign[slot] = len(set(assign.values()))
    pickle.dump((assign, funcs), open(STATE, 'wb'))
pickle.dump((assign, funcs), open(STATE, 'wb'))
print('done', 'objects', len(set(assign.values())), 'funcs', len(funcs), 'unknown imports', dict(e.unknown), flush=True)
reads = [r for r in e.log if r['k'] in ('R', 'RB')]
cov = bytearray(len(save))
for r in reads:
    if r['k'] == 'R':
        cov[r['off']:r['off'] + r['n']] = b'\1' * r['n']
    else:
        cov[r['off']:r['off'] + (r['bit'] + r['n'] + 7) // 8] = b'\1' * ((r['bit'] + r['n'] + 7) // 8)
print('bytes read', sum(cov), 'of', len(save), flush=True)
pickle.dump({'log': e.log, 'assign': assign, 'funcs': funcs, 'cov': bytes(cov)}, open(ROOT + 'scratch/fullmap.pkl', 'wb'))
