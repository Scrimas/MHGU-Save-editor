# MHGU Save Editor

A desktop save editor for **Monster Hunter Generations Ultimate** (Nintendo Switch,
emulator saves), and the reverse-engineering notes and data it is built on.

Every value it shows is read where the game itself keeps it, and every change is
described in game terms ("Mega Potion ×10", "HR 999", "every quest cleared") before
anything touches the file. Fields are marked **Confirmed** (checked in game) or
**Derived** (from the format notes, not yet checked in game).

![Overview: progress of the character and one-click goals, two of them staged for Review](docs/screenshots/01-overview-dark.png#gh-dark-mode-only)
![Overview: progress of the character and one-click goals, two of them staged for Review](docs/screenshots/01-overview-light.png#gh-light-mode-only)

## Download

Single-file builds are attached to each [release](https://github.com/Scrimas/MHGU-Save-editor/releases):

| File | Runs on |
|---|---|
| `MHGU-Save-Editor-<version>-x86_64.AppImage` | Linux x86-64, glibc 2.28 or newer (distros from 2019 on) |
| `MHGU-Save-Editor-<version>-x86_64.exe` | Windows 10 / 11, nothing to install |

Ryujinx saves are found automatically (`~/.config/Ryujinx`, `%APPDATA%\Ryujinx`, the
Flatpak folders). For another emulator, use **Open save…** on its
`…/save/<id>/0/system` file. Title ID `0100770008DD8000` (EU / western release).

## What it edits

| Page | |
|---|---|
| Overview | One-click goals: complete every quest, all Hunter Arts, Canteen dishes and ingredients, all Guild Card awards, Hunter's Notes, every crown, HR 999, max zenny and points |
| Character | Name, Hunter Rank and HR points, zenny, Wycademy points, Village and Hub star levels, play time |
| Items | Item box, pouch and loadouts; sort and merge, max counts. Pouch stacks stop at each item's carry limit |
| Equipment | Hunter and Palico equipment boxes: add pieces, levels, decorations, talismans |
| Palicoes | Name, level and experience, forte, greeting, original owner |
| Quests | Every quest cleared or not (seen and quest sets follow, as in game); says what unlocks a quest that is not on the board yet |
| Requests | Villager requests: accepted (quest posted on the board) and completed |
| Collections | Hunter Arts, Canteen dishes and ingredients, Guild Card awards |
| Monsters | Hunted and captured counts, smallest and largest sizes, Hunter's Notes |
| Save map | Every byte range of the save, named from the game's own loader (read-only) |

All three character slots are supported.

<table>
  <tr>
    <td width="50%">
      <img alt="Items: the item box with game icons" src="docs/screenshots/02-items-dark.png#gh-dark-mode-only">
      <img alt="Items: the item box with game icons" src="docs/screenshots/02-items-light.png#gh-light-mode-only">
    </td>
    <td width="50%">
      <img alt="Equipment: the hunter box and the selected piece" src="docs/screenshots/03-equipment-dark.png#gh-dark-mode-only">
      <img alt="Equipment: the hunter box and the selected piece" src="docs/screenshots/03-equipment-light.png#gh-light-mode-only">
    </td>
  </tr>
  <tr>
    <td align="center">Items</td>
    <td align="center">Equipment</td>
  </tr>
  <tr>
    <td>
      <img alt="Monsters: hunts, captures, sizes and crowns" src="docs/screenshots/04-monsters-dark.png#gh-dark-mode-only">
      <img alt="Monsters: hunts, captures, sizes and crowns" src="docs/screenshots/04-monsters-light.png#gh-light-mode-only">
    </td>
    <td>
      <img alt="Quests: the Hub quests, cleared, seen and failed" src="docs/screenshots/05-quests-dark.png#gh-dark-mode-only">
      <img alt="Quests: the Hub quests, cleared, seen and failed" src="docs/screenshots/05-quests-light.png#gh-light-mode-only">
    </td>
  </tr>
  <tr>
    <td align="center">Monsters</td>
    <td align="center">Quests</td>
  </tr>
</table>

## Keeping your save safe

- **Nothing is written until you press Write.** Edits are staged; Review lists them
  in game terms, each one can be undone, and the page shows what each value was.
- **Write refuses while an emulator is running**: Ryujinx (and others) overwrite the
  save when they exit.
- **A snapshot is taken before every write** (and before every restore). Snapshots
  restores the whole save folder from any of them.
- The emulator keeps two copies of the save, each with a backup; all four files get
  the same bytes and keep their own headers. Each write is checked by reading it back.

Keep your own backup anyway. This is an unofficial tool.

![Write dialog: the changes in game terms, the emulator check and the snapshot](docs/screenshots/06-write-dark.png#gh-dark-mode-only)
![Write dialog: the changes in game terms, the emulator check and the snapshot](docs/screenshots/06-write-light.png#gh-light-mode-only)

## Settings

**Settings…** in the sidebar: theme and interface size; snapshot folder, how many
snapshots to keep per save, and whether Write takes one by default; extra folders to
search for saves (yuzu, Eden, Citron, portable installs); recent saves and reopening the
last one; extra emulator process names for the running check; and **Confirmed changes
only**, which refuses every Derived change. They are kept in
`~/.config/mhgu-save-editor/settings.json` (Windows: `%APPDATA%\mhgu-save-editor`).

![Settings dialog](docs/screenshots/07-settings-dark.png#gh-dark-mode-only)
![Settings dialog](docs/screenshots/07-settings-light.png#gh-light-mode-only)

## Building from source

Needs a stable Rust toolchain (edition 2024).

```bash
cd app
cargo build --release
```

The binary is `app/target/release/mhgu-editor`. `app/packaging/build.sh` makes the
AppImage and the Windows `.exe` (see its header for the toolchain).

**Game assets.** Item, equipment, skill and monster names and icons come from the
game's own files. They are Capcom's and are not in this repository in readable form.
Without them the editor still works, with `#ID` names and placeholder icons. With
your own dump of the game:

```bash
python3 tools/build_assets.py path/to/base_romfs.bin
```

then rebuild. [`app/assets/README.md`](app/assets/README.md) explains how the release
builds get them.

## Repository layout

| Path | |
|---|---|
| [`app/core`](app/core) | `mhgu-save`: the save format as a Rust library (read, edit, write every copy, snapshots) |
| [`app/gui`](app/gui) | The editor (Rust + [Slint](https://slint.dev)) |
| [`app/packaging`](app/packaging) | Release builds, asset pack sealing |
| [`docs/`](docs/README.md) | **The save format notes**: container, quests, monsters, equipment, items, awards, NPC talk, a whole-file map |
| [`docs/screenshots`](docs/screenshots) | The editor's screenshots in this README, light and dark |
| [`data/`](data) | Machine-readable tables: quest, request, monster and NPC indexes, unlock rules, the whole-file map |
| [`tools/`](tools) | Python scripts the notes use: readers, the quest unlock evaluator, the asset builder, the loader emulation |

Start with [docs/README.md](docs/README.md) for the format itself, its quick-reference
table and how each claim was established.

## Licence

Copyright © 2026 Scrimas.

This program is free software: you can redistribute it and/or modify it under the
terms of the GNU General Public License as published by the Free Software Foundation,
either version 3 of the License, or (at your option) any later version. It is
distributed WITHOUT ANY WARRANTY; see [`LICENSE`](LICENSE). This covers the whole
repository: the editor, the tools, the notes and the data.

Monster Hunter and its assets are © CAPCOM. This is an unofficial fan project, not
affiliated with or endorsed by Capcom or Nintendo. The editor's UI toolkit,
[Slint](https://slint.dev), is used under its Royalty-free 2.0 licence.
