#!/bin/sh
set -eu

# The release workflow validates this tag against the workspace version first.
version=${RELEASE_TAG:?set RELEASE_TAG to the validated release tag}
version=${version#v}

for package in quasar-derive quasar-lang quasar-spl quasar-metadata quasar-test quasar-schema quasar-idl-schema quasar-idl quasar-profile quasar-cli; do
    status=$(curl --silent --show-error --retry 3 --output /dev/null \
        --write-out '%{http_code}' "https://crates.io/api/v1/crates/$package/$version")
    case "$status" in
        200) echo "$package $version is already published; skipping" ;;
        404) cargo publish --locked --registry crates-io --package "$package" ;;
        *) echo "Cannot check $package $version: crates.io returned HTTP $status" >&2; exit 1 ;;
    esac
done
