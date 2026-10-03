"""emu.py : run the game's own save loader / writer under unicorn and log every field.

The v1.4 executable is relocated to BASE so a null pointer faults.  Manager objects are
allocated on demand: when an access faults near address 0, the slot the null pointer was
loaded from gets a fresh 16 MB object (pages mapped lazily) and the run restarts.

    emu.py load  SAVE START LEN ENTRY      run loader ENTRY(r1 = stream at START)
"""
import struct, sys, os, pickle, collections
from unicorn import *
from unicorn.arm_const import *
import capstone

ROOT = os.path.dirname(os.path.abspath(__file__)) + '/../../'
MEM = open(ROOT + 'scratch/mem.bin', 'rb').read()
BASE = 0x10000000
IMG_END = 0x2147000                       # bss end 0x2146788
STUB = 0x0E000000
OBJ = 0x40000000; OBJ_SZ = 0x1000000; OBJ_N = 160
FILEBUF = 0x20000000; OUTBUF = 0x28000000; BUF_SZ = 0x500000
STREAM = 0x30000000
STACK = 0x31000000; STACK_SZ = 0x100000
RET = 0x32000000
VALLOC = STUB + 0xff00              # null virtual -> allocate (size in r1)

u32 = lambda b, a: struct.unpack_from('<I', b, a)[0]
cs = capstone.Cs(capstone.CS_ARCH_ARM, capstone.CS_MODE_ARM); cs.detail = True


def build_image():
    img = bytearray(IMG_END); img[:len(MEM)] = MEM
    symtab, strtab = 0x1545720, 0x15487e0
    def sym(i):
        n, v, sz, info, oth, shn = struct.unpack_from('<IIIBBH', MEM, symtab + 16 * i)
        return MEM[strtab + n:MEM.index(b'\0', strtab + n)].decode(), v, shn
    stubs = {}; nxt = [STUB]
    def stub(name):
        if name not in stubs.values():
            stubs[nxt[0]] = name; nxt[0] += 8
        return [a for a, n in stubs.items() if n == name][0]
    def rel(tab, size):
        for i in range(0, size, 8):
            off, info = struct.unpack_from('<II', MEM, tab + i)
            t, s = info & 0xff, info >> 8
            if t == 23:
                struct.pack_into('<I', img, off, (u32(img, off) + BASE) & 0xffffffff)
            elif t in (2, 21, 22):
                name, v, shn = sym(s)
                tgt = BASE + v if shn else stub(name)
                a = u32(img, off) if t == 2 else 0
                struct.pack_into('<I', img, off, (a + tgt) & 0xffffffff)
    rel(0x13f206c, 0x150bf0); rel(0x1542c5c, 0x15b8)
    return bytes(img), stubs

IMG, STUBS = None, None
CACHE = ROOT + 'scratch/img_reloc.pkl'
if os.path.exists(CACHE):
    IMG, STUBS = pickle.load(open(CACHE, 'rb'))
else:
    IMG, STUBS = build_image(); pickle.dump((IMG, STUBS), open(CACHE, 'wb'))

_dec = {}
def dest_reg(uc, pc):
    if pc not in _dec:
        try:
            ins = next(cs.disasm(bytes(uc.mem_read(pc, 4)), pc))
            _dec[pc] = (ins.mnemonic, [o.reg for o in ins.operands if o.type == capstone.arm.ARM_OP_REG], ins)
        except StopIteration:
            _dec[pc] = ('?', [], None)
    return _dec[pc]

A = lambda x: BASE + x        # executable address -> emulated


class Emu:
    def __init__(self, save, assign, funcs=None):
        self.save = save
        self.assign = dict(assign)      # emulated slot address -> object index
        self.funcs = dict(funcs or {})  # emulated slot address -> 'alloc' (null virtual)
        self.log = []
        self.reads = collections.deque(maxlen=64)
        self.pending = None
        self.fault = None
        self.unknown = collections.Counter()
        self.nobj = max(self.assign.values(), default=-1) + 1

    def setup(self):
        uc = self.uc = Uc(UC_ARCH_ARM, UC_MODE_ARM)
        uc.ctl_set_cpu_model(UC_CPU_ARM_MAX)
        uc.reg_write(UC_ARM_REG_C1_C0_2, uc.reg_read(UC_ARM_REG_C1_C0_2) | (0xf << 20))
        uc.reg_write(UC_ARM_REG_FPEXC, 0x40000000)
        uc.mem_map(BASE, IMG_END); uc.mem_write(BASE, IMG)
        uc.mem_map(STUB, 0x10000); uc.mem_write(STUB, b'\x1e\xff\x2f\xe1' * 0x4000)
        uc.mem_map(FILEBUF, BUF_SZ); uc.mem_write(FILEBUF, self.save)
        uc.mem_map(OUTBUF, BUF_SZ)
        uc.mem_map(STREAM, 0x1000); uc.mem_map(STACK, STACK_SZ); uc.mem_map(RET, 0x1000)
        for slot, k in self.assign.items():
            if OBJ <= slot < OBJ + OBJ_N * OBJ_SZ:
                try: uc.mem_map(slot & ~0xfff, 0x1000)
                except UcError: pass
            uc.mem_write(slot, struct.pack('<I', OBJ + k * OBJ_SZ))
        self.regions = [(BASE, BASE + IMG_END), (STUB, STUB + 0x10000), (FILEBUF, FILEBUF + BUF_SZ), (OUTBUF, OUTBUF + BUF_SZ),
                        (STREAM, STREAM + 0x1000), (STACK, STACK + STACK_SZ)]
        for slot in self.funcs:
            if OBJ <= slot < OBJ + OBJ_N * OBJ_SZ:
                try: uc.mem_map(slot & ~0xfff, 0x1000)
                except UcError: pass
            uc.mem_write(slot, struct.pack('<I', VALLOC))
        self.bump = OBJ + 100 * OBJ_SZ      # objects 100..157: bump heap for allocations
        # fake allocator object: every virtual allocates (size in r1)
        self.falloc = OBJ + 158 * OBJ_SZ
        uc.mem_map(self.falloc, 0x1000)
        uc.mem_write(self.falloc, struct.pack('<I', self.falloc + 0x100) + b'\0' * 0xfc + struct.pack('<I', VALLOC) * 0x80)
        def valloc(uc2, a, s_, d):
            n = uc2.reg_read(UC_ARM_REG_R1)
            if n > 0x2000000: n = 0x1000          # not a size: a virtual that is not an allocator
            p = self.bump; self.bump += (max(n, 16) + 0xff) & ~0xff
            assert self.bump < OBJ + 158 * OBJ_SZ
            uc2.reg_write(UC_ARM_REG_R0, p); uc2.reg_write(UC_ARM_REG_PC, uc2.reg_read(UC_ARM_REG_LR))
        uc.hook_add(UC_HOOK_CODE, valloc, begin=VALLOC, end=VALLOC)
        uc.hook_add(UC_HOOK_MEM_UNMAPPED, self.on_unmapped)
        uc.hook_add(UC_HOOK_CODE, self.on_stub, begin=STUB, end=STUB + 0x10000)
        uc.hook_add(UC_HOOK_MEM_READ, self.on_read, begin=OBJ, end=OBJ + 100 * OBJ_SZ - 1)
        uc.hook_add(UC_HOOK_MEM_READ, self.on_read, begin=BASE + 0x172f000, end=BASE + IMG_END - 1)
        self.scratch = OBJ + (OBJ_N - 1) * OBJ_SZ      # zeroed dummy object for faked lookups
        self.whook = None
        self.hooks = {}
        return uc

    def fake(self, addr, ret):
        """replace function addr (executable address) by `return ret`"""
        def f(uc):
            uc.reg_write(UC_ARM_REG_R0, ret); uc.reg_write(UC_ARM_REG_PC, uc.reg_read(UC_ARM_REG_LR))
        self.hook_pc(A(addr), f)

    def set_pending(self, rec, val):
        self.pending = (rec, val)
        if self.whook is None:
            self.whook = self.uc.hook_add(UC_HOOK_MEM_WRITE, self.on_write)

    def hook_pc(self, addr, fn):
        self.hooks[addr] = fn
        self.uc.hook_add(UC_HOOK_CODE, lambda uc, a, s, d: fn(uc), begin=addr, end=addr)

    # --- memory model -------------------------------------------------
    def on_unmapped(self, uc, access, addr, size, value, ud):
        if OBJ <= addr < OBJ + OBJ_N * OBJ_SZ:
            uc.mem_map(addr & ~0xfff, 0x1000); return True
        self.fault = (access, addr, uc.reg_read(UC_ARM_REG_PC))
        return False

    def on_read(self, uc, access, addr, size, value, ud):
        if size == 4:
            v = u32(uc.mem_read(addr, 4), 0)
            if v == 0:
                pc = uc.reg_read(UC_ARM_REG_PC)
                self.reads.append((pc, addr))

    def on_write(self, uc, access, addr, size, value, ud):
        if self.pending is not None and not (STACK <= addr < STACK + STACK_SZ):
            rec, val = self.pending
            if (value & ((1 << (8 * size)) - 1)) == (val & ((1 << (8 * size)) - 1)):
                rec['dst'] = addr; rec['dsize'] = size; self.pending = None
                uc.hook_del(self.whook); self.whook = None

    # --- imports ------------------------------------------------------
    def on_stub(self, uc, addr, size, ud):
        name = STUBS.get(addr)
        if name is None: return
        r = [uc.reg_read(x) for x in (UC_ARM_REG_R0, UC_ARM_REG_R1, UC_ARM_REG_R2, UC_ARM_REG_R3)]
        lr = uc.reg_read(UC_ARM_REG_LR)
        def ok(a, n):
            if n == 0: return True
            if n > 0x2000000:                       # not a real length: report as a fault
                self.fault = ('bigcopy', a, addr, name, n, lr - BASE); uc.emu_stop(); return False
            for pg in range(a & ~0xfff, a + n, 0x1000):
                if OBJ <= pg < OBJ + OBJ_N * OBJ_SZ:
                    try: uc.mem_map(pg, 0x1000)
                    except UcError: pass
                elif not any(lo <= pg < hi for lo, hi in self.regions):
                    self.fault = ('stub', pg, addr, name); uc.emu_stop(); return False
            return True
        if name in ('__aeabi_memcpy', '__aeabi_memcpy4', '__aeabi_memcpy8', 'memcpy',
                    '__aeabi_memmove', '__aeabi_memmove4', '__aeabi_memmove8', 'memmove'):
            d, s, n = r[0], r[1], r[2]
            if not (ok(d, n) and ok(s, n)): return
            if n:
                uc.mem_write(d, bytes(uc.mem_read(s, n)))
            self.on_copy(d, s, n, lr)
        elif name in ('__aeabi_memclr', '__aeabi_memclr4', '__aeabi_memclr8'):
            if not ok(r[0], r[1]): return
            if r[1]: uc.mem_write(r[0], b'\0' * r[1])
        elif name in ('__aeabi_memset', '__aeabi_memset4', '__aeabi_memset8'):
            if not ok(r[0], r[1]): return
            if r[1]: uc.mem_write(r[0], bytes([r[2] & 0xff]) * r[1])
        elif name == 'memset':
            if not ok(r[0], r[2]): return
            if r[2]: uc.mem_write(r[0], bytes([r[1] & 0xff]) * r[2])
            uc.reg_write(UC_ARM_REG_R0, r[0])
        else:
            self.unknown[name] += 1
            uc.reg_write(UC_ARM_REG_R0, 0)

    def on_copy(self, d, s, n, lr):
        if FILEBUF <= s < FILEBUF + BUF_SZ:
            self.log.append({'k': 'R', 'off': s - FILEBUF, 'n': n, 'dst': d, 'pc': getattr(self, 'caller', None) or lr - BASE,
                             'mgr': getattr(self, 'mgr', None)})
        elif OUTBUF <= d < OUTBUF + BUF_SZ:
            self.log.append({'k': 'W', 'off': d - OUTBUF, 'n': n, 'src': s, 'pc': lr - BASE})

    # --- run ----------------------------------------------------------
    def stream(self, buf, pos):
        self.uc.mem_write(STREAM, struct.pack('<IIII', 0, 0, buf + pos, buf))

    def call(self, entry, r0, r1, limit=300_000_000, r2=0, until=None, regs=None):
        uc = self.uc
        uc.reg_write(UC_ARM_REG_SP, STACK + STACK_SZ - 0x100)
        uc.reg_write(UC_ARM_REG_LR, RET)
        uc.reg_write(UC_ARM_REG_R0, r0); uc.reg_write(UC_ARM_REG_R1, r1); uc.reg_write(UC_ARM_REG_R2, r2)
        for k, v in (regs or {}).items(): uc.reg_write(k, v)
        try:
            uc.emu_start(A(entry), A(until) if until else RET, count=limit)
        except UcError as e:
            return e
        return None

    def null_slot(self):
        """slot that held the null pointer of the last fault"""
        if self.fault[0] == UC_MEM_FETCH_UNMAPPED:
            lr = self.uc.reg_read(UC_ARM_REG_LR)
            mn, regs, ins = dest_reg(self.uc, lr - 4)
            reg = regs[0] if mn.startswith('blx') and regs else None
            for rpc, raddr in reversed(self.reads):
                rmn, rregs, rins = dest_reg(self.uc, rpc)
                if reg is None or (rregs and rregs[0] == reg):
                    return raddr, 'func', 0
            return None, 'func', 0
        if self.fault[0] == 'stub':
            return (self.reads[-1][1] if self.reads else None), None, 0
        access, addr, pc = self.fault
        mn, regs, ins = dest_reg(self.uc, pc)
        base = None
        if ins is not None:
            for o in ins.operands:
                if o.type == capstone.arm.ARM_OP_MEM:
                    base = o.mem.base
            if mn.startswith(('ldm', 'stm', 'pop', 'push')):
                base = regs[0] if regs else None
        bval = self.uc.reg_read(_REGMAP[base]) if base in _REGMAP else None
        # most recent zero read whose destination is the base register
        for rpc, raddr in reversed(self.reads):
            rmn, rregs, rins = dest_reg(self.uc, rpc)
            if rregs and rregs[0] == base and rmn.startswith('ldr'):
                return raddr, base, bval
        return (self.reads[-1][1] if self.reads else None), base, bval

_REGMAP = {getattr(capstone.arm, 'ARM_REG_R%d' % i): getattr(sys.modules['unicorn.arm_const'], 'UC_ARM_REG_R%d' % i) for i in range(13)}
_REGMAP[capstone.arm.ARM_REG_SB] = UC_ARM_REG_R9; _REGMAP[capstone.arm.ARM_REG_SL] = UC_ARM_REG_R10
_REGMAP[capstone.arm.ARM_REG_FP] = UC_ARM_REG_R11; _REGMAP[capstone.arm.ARM_REG_IP] = UC_ARM_REG_R12
