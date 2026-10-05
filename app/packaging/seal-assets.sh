#!/usr/bin/env bash
# Seal the game-asset pack (app/assets/gen) into app/assets/gen.tar.gz.gpg for the release
# workflow, and give GitHub its passphrase as the ASSETS_PASSPHRASE secret (needs gh).
# Rerun after rebuilding the pack. See app/assets/README.md.
#
# The passphrase is made once and kept outside the repo (never commit it):
#   ASSETS_PASSPHRASE_FILE, default ~/.config/mhgu-save-editor/assets-passphrase
set -euo pipefail
app=$(cd "$(dirname "$0")/.." && pwd)
key=${ASSETS_PASSPHRASE_FILE:-${XDG_CONFIG_HOME:-$HOME/.config}/mhgu-save-editor/assets-passphrase}
out="$app/assets/gen.tar.gz.gpg"

if [ ! -f "$app/assets/gen/names.json" ]; then
    echo "no asset pack in $app/assets/gen: run tools/build_assets.py first" >&2
    exit 1
fi
if [ ! -s "$key" ]; then
    mkdir -p "$(dirname "$key")"
    (umask 077 && head -c 48 /dev/urandom | base64 -w0 > "$key")
fi

tar -C "$app/assets" --sort=name --owner=0 --group=0 --numeric-owner -czf - gen |
    gpg --batch --yes --pinentry-mode loopback --passphrase-file "$key" \
        --symmetric --cipher-algo AES256 -o "$out"
(cd "$app" && gh secret set ASSETS_PASSPHRASE < "$key")
echo "sealed $(du -h "$out" | cut -f1) into $out"
