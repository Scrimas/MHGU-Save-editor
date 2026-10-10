# 01 — Container

## File layout

On Switch the MHGU save lives in the title's savedata volume. Under Ryujinx this
surfaces as a directory tree:

```
<ryujinx>/bis/user/save/<save-id>/
├── 0/                      commit slot 0
│   ├── system              5,159,100 bytes — the save
│   └── system_backup       5,159,100 bytes — in-game backup copy
└── 1/                      commit slot 1
    ├── system
    └── system_backup
```

**CONFIRMED — both commit slots must be written.** Slots `0/` and `1/` are a
double-commit scheme. The game reads whichever slot it considers current; an editor
that patches only one will see its changes appear or vanish depending on which slot
wins. Write identical bytes to both.

**CONFIRMED — the emulator holds the save in memory.** Ryujinx writes its in-memory
copy back on exit, silently discarding external edits made while it is running. Any
tool must require the emulator be fully closed before patching.

`system` and `system_backup` differ only in the entry key at `0x14` described below; they
are otherwise byte-identical. `system` is the live file.

## No encryption

**CONFIRMED — the file is plaintext.** Two independent measures:

- Shannon entropy ≈ 3.0 bits/byte across the whole file. Encrypted or compressed
  data sits near 8.0.
- 71.5% of all bytes are `0x00`.

Strings are stored readable — quest names appear as UTF-16LE in the quest history
log and were located by plain substring search.

This is the single most useful property of the format: values can be found by
searching for them directly, and edits need no re-encryption step.

## No content checksum

**CONFIRMED — there is no integrity check over the save body.**

The proof is direct. `system` and `system_backup` were compared byte for byte
immediately after a normal in-game save. They differed in exactly **four bytes**, at
offset `0x14`. Had any checksum, hash, or MAC covered the body, the two files —
which carry identical body content — could not have differed in only a field that is
itself part of the header.

Every edit described in these documents was subsequently applied by raw byte write
and loaded by the game without complaint, across many separate sessions. No
recalculation of any kind was ever required.

### Container header — `0x00`–`0x23`

**CONFIRMED** (2026-10-10, writer `0x877c20`, loader `0x8778cc`, all 46 saves of the
timeline). The first 36 bytes are not a Switch header but the game's own `sSavedata`
key/value container: a 16-byte header, a table of entry offsets, then one 16-byte record
per entry followed by its blob. The save holds one entry, the body.

| Offset | Size | Field |
|---|---|---|
| `0x00` | u32 | 0; the loader rejects anything else (error 6) |
| `0x04` | u32 | container format version, 0; the loader rejects another (error 7) |
| `0x08` | u32 | size of the offset table = 4 × entry count (4) |
| `0x0C` | u32 | 0, not checked |
| `0x10` | u32 | offset of entry 0's record (`0x14`); skipped by the loader |
| `0x14` | u32 | entry key: JAMCRC (CRC-32 without the final xor, `0x7cbdb4`) of the entry name. `0x36B2EE74` = "system" in `system`, `0xD0819B92` = "system_backup" in `system_backup` |
| `0x18` | u32 | entry type, 12 = raw blob |
| `0x1C` | u64 | blob size, `0x4EB898` (file size − `0x24`); the blob is loaded only when it equals the registered size |

Earlier revisions called `0x14` a nonce that changes on every save. It does not: it is
constant within each file, and `system` and `system_backup` differ there only because
their entry names differ. An editor leaves the header alone.

## Practical consequences for an editor

1. Read `system`, patch bytes, write back. No crypto, no checksum, no framing.
2. Write the same bytes to both `0/system` and `1/system`.
3. Refuse to run while the game or emulator is live.
4. Back up first. The format has no self-repair and no validation — a bad write is
   only detectable by playing.
5. Leave `0x14` untouched.

## Regions and builds

Only the EU/western build (`0100770008DD8000`) was examined. The 3DS release of
MHGU's predecessor (MHXX) uses a different, encrypted container and none of this
applies to it.

## Open questions

- **Character slots — answered.** The game's loader reads three slots of
  `0x11F8C4` bytes with the same chain, so the per-character structures repeat at the
  slot base given by the pointer table ([11](11-save-map.md#method)). Confirmed with two
  more real characters ([07](07-equipment.md#character-slots)).
- **The bulk of the file — mapped.** [11 — Whole-file map](11-save-map.md) assigns
  every byte to the game object that reads it. Most of the 5 MB is per-slot Guild Card
  lists (0.98 MB per slot, mostly padding) and the downloaded event quests (1.5 MB,
  shared). Many fields inside those objects are bounded but not yet named.
