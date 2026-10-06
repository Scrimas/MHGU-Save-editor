#!/usr/bin/env bash
# Seal a save folder (the one holding 0/ and 1/, each with system and system_backup) into
# app/assets/test-save.tar.xz.gpg, for the tests that need a real save (MHGU_TEST_SAVE) in
# CI. Same key and secret (ASSETS_PASSPHRASE) as seal-assets.sh; run that first once.
# The tests check values of the save it was made from: rerun only on purpose.
#
#   seal-test-save.sh ~/.config/Ryujinx/bis/user/save/0000000000000001
set -euo pipefail
app=$(cd "$(dirname "$0")/.." && pwd)
key=${ASSETS_PASSPHRASE_FILE:-${XDG_CONFIG_HOME:-$HOME/.config}/mhgu-save-editor/assets-passphrase}
out="$app/assets/test-save.tar.xz.gpg"
src=${1:?usage: seal-test-save.sh <save folder holding 0/ and 1/>}

for f in 0/system 0/system_backup 1/system 1/system_backup; do
    if [ ! -f "$src/$f" ]; then
        echo "no $f in $src" >&2
        exit 1
    fi
done
if [ ! -s "$key" ]; then
    echo "no passphrase in $key: run seal-assets.sh first" >&2
    exit 1
fi

tar -C "$src" --sort=name --owner=0 --group=0 --numeric-owner -cJf - 0/system 0/system_backup 1/system 1/system_backup |
    gpg --batch --yes --pinentry-mode loopback --passphrase-file "$key" \
        --symmetric --cipher-algo AES256 --compress-algo none -o "$out"
echo "sealed $(du -h "$out" | cut -f1) into $out"
