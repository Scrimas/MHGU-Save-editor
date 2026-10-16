# Building and developing the editor

The player's side is in the [main README](../README.md). This page is for building the
editor and changing it.

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

then rebuild. [`assets/README.md`](assets/README.md) explains how the release builds get
them.

## Updates

On start the editor asks GitHub for a newer release. A downloaded file is checked against
the release's SHA-256 sums before it takes the place of the old one. The sums come from
the same release, so they catch a broken download, not a release published by someone
else.

## Translations

The interface text is in [`gui/lang`](gui/lang) (gettext catalogs). After changing
strings in the source, `python3 tools/i18n.py` updates the template and merges every
catalog; `python3 tools/i18n.py --check` is what CI runs.

## Colours from matugen

On Linux the editor follows the wallpaper's palette from
[matugen](https://github.com/InioX/matugen) when `~/.config/mhgu-save-editor/matugen.json`
exists, and picks up a new one within two seconds of matugen writing it. **Save template**
in Settings puts the template next to it; then add to `~/.config/matugen/config.toml`:

```toml
[templates.mhgu-save-editor]
input_path = '~/.config/mhgu-save-editor/matugen-template.json'
output_path = '~/.config/mhgu-save-editor/matugen.json'
```

Any template or tool will do that writes the same file: a `dark` and a `light` object,
each with the Material roles `primary`, `on_surface`, `on_surface_variant`,
`surface_container_lowest` … `surface_container_highest` and `error` as `#rrggbb`
([`packaging/matugen`](packaging/matugen/mhgu-save-editor.json)). Light, Dark and System
still choose which of the two is shown.

## Repository layout

| Path | |
|---|---|
| [`app/core`](core) | `mhgu-save`: the save format as a Rust library (read, edit, write every copy, snapshots) |
| [`app/gui`](gui) | The editor (Rust + [Slint](https://slint.dev)) |
| [`app/packaging`](packaging) | Release builds, asset pack sealing |
| [`docs/`](../docs/README.md) | **The save format notes**: container, quests, monsters, equipment, items, awards, NPC talk, a whole-file map |
| [`docs/screenshots`](../docs/screenshots) | The screenshots of the main README, light and dark |
| [`data/`](../data) | Machine-readable tables: quest, request, monster and NPC indexes, unlock rules, the whole-file map |
| [`tools/`](../tools) | Python scripts the notes use: readers, the quest unlock evaluator, the asset builder, the loader emulation, the evidence checks |

Start with [docs/README.md](../docs/README.md) for the format itself, its quick-reference
table and how each claim was established.
