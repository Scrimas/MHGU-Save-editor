# Game-asset pack

The editor shows the game's own item, equipment, skill and monster names, and its item,
monster and award icons. They are embedded in the binary at build time from `gen/`, which
`tools/build_assets.py` extracts from a RomFS dump of the game (see the header of that script).

## Why `gen.tar.gz.gpg` is an encrypted blob

`gen/` is Capcom's copyrighted artwork and text. This repository is GPL-licensed and
cannot relicense or host it, so `gen/` is gitignored and never committed in readable form. (The
release binaries embed it, like every local build; they are drafts until published.)

The release workflow (`.github/workflows/release.yml`) still has to build the same binaries
as a local build, and GitHub's servers have no game dump. A repository secret is limited to
48 KB, so the pack is committed encrypted instead (GitHub's documented method for larger
secrets): `gen.tar.gz.gpg` is `gen/` as a tar.gz, encrypted with GPG (AES-256, symmetric).
Its passphrase exists only as the repository secret `ASSETS_PASSPHRASE` and on the
maintainer's machine. The workflow decrypts it into `gen/` before building.

Without the passphrase the blob is useless; nothing else in the repository depends on it.

## Building without it

Everything works without `gen/`: the build prints a warning and the editor shows
placeholder icons and `#ID` instead of names. With your own dump of the game:

    python3 tools/build_assets.py path/to/base_romfs.bin

## Updating it (maintainer)

After rebuilding `gen/`, reseal it and refresh the secret, then commit the new blob:

    app/packaging/seal-assets.sh

## `test-save.tar.xz.gpg`

A frozen copy of a played save folder (`0/` and `1/`, each `system` and `system_backup`),
sealed with the same passphrase. CI (`.github/workflows/ci.yml`) unpacks it with the asset
pack and points `MHGU_TEST_SAVE` at it, so the tests that need a real save run on every
push; without the
secret (a fork's pull request) they are skipped. It holds the maintainer's character, so
it is encrypted like the pack. The tests check values of this save: reseal it only on
purpose, from a save folder:

    app/packaging/seal-test-save.sh path/to/save/0000000000000001
