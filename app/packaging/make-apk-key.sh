#!/usr/bin/env bash
# Make the Android release key once and give GitHub the keystore as the
# MHGU_APK_KEYSTORE_B64 secret (needs gh, keytool). The release workflow signs the APK with
# it (build-apk.sh --release). Rerun to refresh the secret; an existing key is kept.
#
# The key signs every APK ever published: Android installs an update only when it is
# signed with the same key, so a lost key means users must uninstall to get a newer
# version. Keep a copy of both files below somewhere safe (never commit them).
#
# The keystore and its key are protected by the asset pack's passphrase (seal-assets.sh),
# so the release workflow reads that one secret for both:
#   MHGU_APK_KEYSTORE      default ~/.config/mhgu-save-editor/apk-release.p12
#   ASSETS_PASSPHRASE_FILE default ~/.config/mhgu-save-editor/assets-passphrase
set -euo pipefail
app=$(cd "$(dirname "$0")/.." && pwd)
dir=${XDG_CONFIG_HOME:-$HOME/.config}/mhgu-save-editor
pass=${ASSETS_PASSPHRASE_FILE:-$dir/assets-passphrase}
ks=${MHGU_APK_KEYSTORE:-$dir/apk-release.p12}

if [ ! -s "$pass" ]; then
    echo "no passphrase in $pass: run seal-assets.sh first (it makes it)" >&2
    exit 1
fi
if [ ! -s "$ks" ]; then
    mkdir -p "$(dirname "$ks")"
    # PKCS12 keeps one password for the store and the key; 30 years of validity
    (umask 077 && keytool -genkeypair -storetype PKCS12 -keystore "$ks" -alias mhgu \
        -keyalg RSA -keysize 4096 -validity 10950 -dname "CN=Scrimas" \
        -storepass:file "$pass" -keypass:file "$pass" -noprompt)
    echo "made $ks"
fi

(cd "$app" && base64 -w0 "$ks" | gh secret set MHGU_APK_KEYSTORE_B64)
# the certificate's fingerprint (public): the same on every APK signed with this key
keytool -list -v -keystore "$ks" -storepass:file "$pass" -alias mhgu | grep -m1 "SHA256:"
