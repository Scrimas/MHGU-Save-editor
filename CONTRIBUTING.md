# Contributing

Bug reports, in-game checks, save-format findings, translations and code are all welcome.

## Reporting a bug

Open an [issue](https://github.com/Scrimas/MHGU-Save-editor/issues) with:

- the editor version (shown in **Settings…**) and your system (Windows / Linux);
- where the save comes from: Ryujinx, yuzu or a fork, or a console backup (JKSV, Checkpoint);
- the page and the change you made, and what the game showed afterwards.

If a write went wrong, the snapshot taken before it (**Snapshots**) puts the save back.

A save file holds your hunter's name, Guild Card and the cards of hunters you met
online. Attach one only if you are fine sharing that, and say which character slot is
concerned.

## Checking an edit in game

An edit is **Derived** (worked out from the format notes) until someone has made it with
the editor and seen the result in game; then it is **Confirmed**. Reports of either
outcome help: say which edit, what you wrote and what the game did, on which save.

## Save-format findings

The notes in [`docs/`](docs/README.md) only say CONFIRMED for what was seen in game: a
controlled before/after diff, a written value the game displayed or acted on, or a
check over a timeline of saves. Anything worked out from the game's code or tables
alone is DERIVED. [docs/06-methodology.md](docs/06-methodology.md) describes the
methods, the failures included. A finding is easiest to take in with:

- the offset (from the character slot base or absolute), size and meaning;
- what was done in game between the two saves, and the bytes that changed;
- what is still unknown.

A field tagged CONFIRMED in `tools/emu/savemap.py` needs a source that can be followed:
a check in `tools/evidence/run.py` or an entry in `tools/evidence/sources.py` naming the
section of `docs/` that holds the experiment. CI checks this.

## Code

[app/README.md](app/README.md) explains the build and the repository layout. A pull
request should:

- leave every byte it does not mean to change as it was;
- add a new edit as Derived (`Conf::Derived`), not Confirmed;
- come with a test that fails without the change, for a fix as for a feature;
- pass what CI runs:

```bash
cd app
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd ..
python3 tools/i18n.py --check
python3 tools/evidence/sources.py
```

The tests that need a real save are ignored by default; to run them, point
`MHGU_TEST_SAVE` at a copy of a `0/system` file:

```bash
MHGU_TEST_SAVE=/path/to/copy/system cargo test --workspace -- --include-ignored
```

## Translations

The interface comes in the game's own languages only: English, French, German, Italian,
Spanish, Traditional and Simplified Chinese. The catalogs are in
[`app/gui/lang`](app/gui/lang) (gettext `.po` files); use the game's own terms for its
items, places and menus. After changing strings in the source, `python3 tools/i18n.py`
updates the template and every catalog.

## Game assets (names, icons, text)

The game's names, icons and text are Capcom's. The editor embeds them at build time from
`app/assets/gen/`, which `tools/build_assets.py` extracts from your own dump of the game;
that folder is gitignored and never committed in readable form. The release builds get it
from `app/assets/gen.tar.gz.gpg`, the same pack encrypted with a passphrase that only the
maintainer and the repository's CI hold ([app/assets/README.md](app/assets/README.md)).

So a pull request that needs new assets (another icon set, more game text) does not
commit them:

- change `tools/build_assets.py` so that it extracts them, and say in the pull request
  which files of the RomFS they come from (and whether the 1.4 update is needed);
- make the editor read them from `gen/` like the existing ones, and keep it working
  without them: a placeholder icon, `#ID` instead of a name. CI on a fork's pull request
  has no pack, and neither has anyone building without a dump;
- check the result with a pack built from your own dump (`python3 tools/build_assets.py
  path/to/base_romfs.bin`); a screenshot in the pull request helps;
- leave `gen.tar.gz.gpg` alone: it cannot be resealed without the passphrase, and a pack
  sealed with another one breaks CI. Once the pull request is merged, the maintainer
  rebuilds the pack from their dump and reseals it.

The CSV tables in [`data/`](data) (quest, monster and recipe indexes and the like) are the
project's own write-up of what the game's data holds, and are committed; the list below
is about the game's own files.

## Never commit

- save files, snapshots or backups (they hold personal data); `app/assets/test-save.tar.xz.gpg`
  is the maintainer's save, sealed like the asset pack;
- `app/assets/gen/` or anything else copied from a game dump: archives, textures, text
  files, the executable.

## Licence

Contributions are accepted under the licence of the repository, the GNU General Public
License, version 3 or any later version ([`LICENSE`](LICENSE)).
