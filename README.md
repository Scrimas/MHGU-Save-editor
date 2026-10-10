# MHGU Save Editor

A desktop save editor for **Monster Hunter Generations Ultimate** (Nintendo Switch,
emulator saves), and the reverse-engineering notes and data it is built on.

![Overview: progress of the character and one-click goals, two of them staged for Review](docs/screenshots/01-overview-dark.png#gh-dark-mode-only)
![Overview: progress of the character and one-click goals, two of them staged for Review](docs/screenshots/01-overview-light.png#gh-light-mode-only)

Every value it shows is read where the game itself keeps it, and every change is
described in game terms ("Mega Potion ×10", "HR 999", "every quest cleared") before
anything touches the file. Every edit the editor makes has been checked in game
(**Confirmed**); an edit added later that has not been yet is marked **Derived** (from
the format notes) until it is.

## Download

Single-file builds are attached to each [release](https://github.com/Scrimas/MHGU-Save-editor/releases):

| File | Runs on |
|---|---|
| `MHGU-Save-Editor-<version>-x86_64.AppImage` | Linux x86-64, glibc 2.28 or newer (distros from 2019 on) |
| `MHGU-Save-Editor-<version>-x86_64.exe` | Windows 10 / 11, nothing to install |

The `.exe` is not code-signed, so Windows SmartScreen warns the first time: **More info → Run anyway**.

The AppImage adds itself to the application menu when started (`~/.local/share/applications`);
the newest version on disk keeps the entry, and menus hide it once that AppImage is deleted.

Both builds update themselves: on start they ask GitHub for a newer release (Settings →
Updates turns this off, or checks now). Nothing is downloaded until you choose to; the new
file is checked against the release's SHA-256 sums and takes the place of the old one.
The sums come from the same release, so they catch a broken download, not a release
published by someone else; a download can be cancelled.

Saves are found automatically for Ryujinx (`~/.config/Ryujinx`, `%APPDATA%\Ryujinx`, the
Flatpak folders) and for yuzu and its forks: suyu, Sudachi, Citron, Eden, torzu
(`~/.local/share/<name>`, `%APPDATA%\<name>`, `~/.var/app/<any app>/data/<name>`).
For a portable install or another emulator, add its folder in **Settings…** or use
**Open save…** on its `system` file. Title ID `0100770008DD8000` (EU / western release).

## What it edits

| Page | |
|---|---|
| Overview | One-click goals: complete every quest, all Hunter Arts, Canteen dishes and ingredients, all Guild Card awards, Hunter's Notes, every crown, HR 999, max zenny and points, every item obtained, every Smithy entry listed, every Guild Card title word, scene and pose; and, Derived: every Soaratorium Lab upgrade, Poogie and Moofy costume, Jukebox song, Trader ware, Horns Coin trade, combination recipe and Gallery movie |
| Character | Name, Hunter Rank and HR points, zenny, Wycademy points, Village and Hub star levels, play time, appearance: body type, face, hairstyle, voice, features, clothing, skin and clothing colours; Guild Card title, scene, pose and greeting, Guild Card weapon usage, quests completed per hunting style; and, Derived: the hunting style, Hunter Arts (SP too) and armor pigment you have on, the Courier's quest counter and points, the village a load starts in and the Room Service's Housekeeper (only those the game offers) |
| Items | Item box, pouch and loadouts (name and pouch layout); add, sort and merge, max counts. A pouch stack above the item's carry limit is written with a warning |
| Equipment | Hunter and Palico equipment boxes: add, replace and remove pieces, levels, decorations (those that do not fit the free slots with a warning), transmog, talismans (checked against the game's charm tables); My Sets: pigment, and, Derived: name, gear, hunting style and Hunter Arts (checked against the weapon, the style's slots and the arts unlocked), a set made from what you have on; Palico equipment sets (Derived): name and gear |
| Palicoes | Name, level and experience, forte, target, greeting, original owner; support moves and skills: the list and what is equipped, checked against the forte's innate entries and the level's slots; looks: coat, eyes, ears, tail, voice, clothing and their colours from the game's palettes; and, Derived: a Palico copied with the equipment it wears to another character, or exported to a file and imported into any save; the Palicoes for hire (same editor); the Palico played as Prowler and the two hunting buddies, Dojo sessions completed and Palicoes hired; the Palico Scout's conditions; StreetPass Palicoes (remove from the inbox); the Dojo, Palico Board and Meownster Hunters read-only |
| Quests | Every quest cleared or not (seen and quest sets follow, as in game, and so do the Guild Card's quests-completed counts); says what unlocks a quest that is not on the board yet. Arena records: best time and equipment set per Arena quest, ranked by the quest's times |
| Requests | Villager requests: accepted (quest posted on the board) and reported |
| Collections | Hunter Arts, Canteen dishes and ingredients, Guild Card awards, Deviants (Special Permits and levels cleared; says when G-rank levels still wait for a G-rank hunt; Derived: Special Permit points and those waiting at the Courier) |
| Unlocks | Derived, entry by entry, each with what unlocks it in game: Soaratorium Lab upgrades (the supply drop sets follow), Jukebox songs, supply drop sets, Horns Coin trades, Poogie and Moofy costumes; the village pets (names, costumes worn, adoption), Moofah affection and Moofah gifts (with their award); what the Trader sells (items, title words, Guild Card scenes, pet costumes, delivery requests); the Housekeeper's Gallery, the Hunter's Notes second list and tips read, the Combination List recipes combined, one-time event scenes and the milestones the award checks read. The Market, Guild Store and Armory lists are shown read-only: the game sells by star level, whatever the save holds |
| Monsters | Hunted and captured counts, smallest and largest sizes (checked against the sizes the game's quests give), Hunter's Notes |
| Guild Cards | Derived: the Guild Cards stored (HR, type, Unity, title, weapon, greeting) and the Guild Card Inbox: remove a card, move an inbox card to the Card List as the Post Office does; the blocked-user list (unblock); the Hunters for Hire offered and hired, read-only |
| Options | Derived: the Start Menu's Options and Multiplayer Settings, the shoutouts and auto-shoutouts of the three chat groups and the group in use; the title screen's TV brightness, rumble and language (shared by the characters) |
| Database | The game's own data next to the save, read-only: monsters, quests, items, skills and equipment ([below](#database)) |
| Save map | Every byte range of the save, named from the game's own loader (read-only) |

All three character slots are supported. **Characters…** (Derived) copies, swaps and
deletes whole characters, and exports one to a file to import it into a slot of any save.

**Values the game cannot produce** (a stack above its limit, a decoration that does not
fit, a talisman outside the charm tables, a weapon above its top level, a locked Guild
Card title…) are never refused: they are written as they are, with a warning next to the
value and in Review and Write. They work offline; online, other hunters can see them.

<table>
  <tr>
    <td width="50%">
      <img alt="Items: the item box with game icons" src="docs/screenshots/02-items-dark.png#gh-dark-mode-only">
      <img alt="Items: the item box with game icons" src="docs/screenshots/02-items-light.png#gh-light-mode-only">
    </td>
    <td width="50%">
      <img alt="Equipment: the hunter box and a talisman with its skills, slots and decorations" src="docs/screenshots/03-equipment-dark.png#gh-dark-mode-only">
      <img alt="Equipment: the hunter box and a talisman with its skills, slots and decorations" src="docs/screenshots/03-equipment-light.png#gh-light-mode-only">
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

## Database

The **Database** page puts the game's own tables next to the save, so you can see what is
left to get and where to get it. Nothing on it is edited; its links lead to the page that
edits the value, and the Monsters, Quests and Items pages link back to it.

- **Monsters**: the size record against the mini and gold crown sizes and the sizes its
  quests give; the chance of a crown in each quest, best first (Derived); carves per rank,
  with what the item box holds.
- **Quests**: what the quest board shows (objective, subquest, locale, reward, HRP, fee),
  the monsters with their sizes and crown chances (Derived), and the reward tables: the
  Base rows A to D, with the item A and B always give, and the subquest's.
- **Items**: every carve, quest reward and combination that gives the item, with its chance;
  every Smithy recipe and combination that takes it; its value as Smithy provisions.
- **Skills**: the skills a tree activates and at how many points, and the decorations,
  talismans and armor pieces that give those points. Search finds a tree by its own name,
  the skills it activates or its decorations ("Razor Sharp", "Razor Jwl 3", "Sharpness").
- **Equipment**: for every weapon, armor piece and decoration, what forging, upgrading and
  each level take against the item box ("ready" when the box holds enough), and the
  weapon's upgrade tree.

![Database: Rathalos's size record against its crown sizes and the chance of a gold crown in each quest](docs/screenshots/08-database-monster-dark.png#gh-dark-mode-only)
![Database: Rathalos's size record against its crown sizes and the chance of a gold crown in each quest](docs/screenshots/08-database-monster-light.png#gh-light-mode-only)

<table>
  <tr>
    <td width="50%">
      <img alt="Database: where to get a Rathalos Ruby, carves and quest rewards with their chances" src="docs/screenshots/09-database-item-dark.png#gh-dark-mode-only">
      <img alt="Database: where to get a Rathalos Ruby, carves and quest rewards with their chances" src="docs/screenshots/09-database-item-light.png#gh-light-mode-only">
    </td>
    <td width="50%">
      <img alt="Database: the Sharpness skill tree, its skills and the talismans, decorations and armor that give its points" src="docs/screenshots/10-database-skill-dark.png#gh-dark-mode-only">
      <img alt="Database: the Sharpness skill tree, its skills and the talismans, decorations and armor that give its points" src="docs/screenshots/10-database-skill-light.png#gh-light-mode-only">
    </td>
  </tr>
  <tr>
    <td align="center">Items: where to get it</td>
    <td align="center">Skills: what gives the points</td>
  </tr>
</table>

## Keeping your save safe

- **Nothing is written until you press Write.** Edits are staged; Review lists them
  in game terms, each one can be undone, and the page shows what each value was.
- **Write refuses while an emulator is running**: Ryujinx (and others) overwrite the
  save when they exit.
- **Write refuses a save the game saved again after it was opened**, so that play is
  not lost; Reload reads it again and keeps the staged changes.
- **A snapshot is taken before every write** (and before every restore). Snapshots
  restores the whole save folder from any of them, and only into the save it was
  taken of.
- The emulator keeps two copies of the save, each with a backup; all four files get
  the same bytes and keep their own headers. Every copy is checked before any is
  written, and each write is checked by reading it back.

Keyboard: Ctrl+S Write…, Ctrl+Z undo the latest change, Ctrl+F search the page, Ctrl+O
open a save, Ctrl+Up/Down the previous/next page, Ctrl+Left/Right the previous/next tab of
the page, Escape closes a dialog or Review. The arrows move between the entries of the page
(a list's rows are picked as you go, Enter opens or edits an entry); a number box being
edited steps with Up/Down until Escape. In the sidebar, Up/Down/Home/End switch pages. The
mouse's Back/Forward buttons go back and forth through the pages and tabs shown.

Keep your own backup anyway. This is an unofficial tool.

![Write dialog: the changes in game terms, the emulator check and the snapshot](docs/screenshots/06-write-dark.png#gh-dark-mode-only)
![Write dialog: the changes in game terms, the emulator check and the snapshot](docs/screenshots/06-write-light.png#gh-light-mode-only)

## Settings

**Settings…** in the sidebar: language, theme and interface size; snapshot folder, how many
snapshots to keep per save, and whether Write takes one by default; extra folders to
search for saves (portable installs, other folders); recent saves and reopening the
last one; extra emulator process names for the running check; and **Confirmed changes
only**, which refuses every Derived change (none since 2.0; it guards edits added later).
They are kept in
`~/.config/mhgu-save-editor/settings.json` (Windows: `%APPDATA%\mhgu-save-editor`).

**Language**: English, French, German, Italian or Spanish, the game's own languages; by
default the system's. Item, equipment, skill and monster names are the game's own in that
language. The interface text is in
[`app/gui/lang`](app/gui/lang) (gettext catalogs); after changing strings in the source,
`python3 tools/i18n.py` updates the template and merges every catalog.

**Colours from [matugen](https://github.com/InioX/matugen)** (Linux): the editor follows
the wallpaper's palette when `~/.config/mhgu-save-editor/matugen.json` exists, and picks
up a new one within two seconds of matugen writing it. **Save template** in Settings puts
the template next to it; then add to `~/.config/matugen/config.toml`:

```toml
[templates.mhgu-save-editor]
input_path = '~/.config/mhgu-save-editor/matugen-template.json'
output_path = '~/.config/mhgu-save-editor/matugen.json'
```

Any template or tool will do that writes the same file: a `dark` and a `light` object,
each with the Material roles `primary`, `on_surface`, `on_surface_variant`,
`surface_container_lowest` … `surface_container_highest` and `error` as `#rrggbb`
([`app/packaging/matugen`](app/packaging/matugen/mhgu-save-editor.json)). Light, Dark
and System still choose which of the two is shown.

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
