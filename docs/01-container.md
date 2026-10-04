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

`system` and `system_backup` differ only in the header nonce described below; they
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

### Header nonce — `0x000014`, u32

**DERIVED.** Changes on every save. Not a checksum: the game loads correctly when it
is left untouched after arbitrary body edits. Most plausibly a save counter or
generation marker used to decide which commit slot is newer. An editor should leave
it alone.

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

- **Character slots — answered from code.** The game's loader reads three slots of
  `0x11F8C4` bytes with the same chain, so the per-character structures repeat at the
  slot base given by the pointer table ([11](11-save-map.md#method)). This has not been
  tested with a second real character.
- **The bulk of the file — mapped.** [11 — Whole-file map](11-save-map.md) assigns
  every byte to the game object that reads it. Most of the 5 MB is per-slot Guild Card
  lists (0.98 MB per slot, mostly padding) and the downloaded event quests (1.5 MB,
  shared). Many fields inside those objects are bounded but not yet named.
