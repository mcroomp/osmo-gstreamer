#!/usr/bin/env bash
set -euo pipefail

version=${1:?Usage: bash scripts/publish-crates.sh X.Y.Z}
if [[ ! $version =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
    echo 'Release version must have the form X.Y.Z, without leading zeroes.' >&2
    exit 1
fi
if [[ ! ${CARGO_REGISTRY_TOKEN:-} =~ [^[:space:]] ]]; then
    echo 'Configure the CARGO_REGISTRY_TOKEN GitHub Actions secret before publishing.' >&2
    exit 1
fi
response=$(mktemp)
trap 'rm -f -- "$response"' EXIT
names=(unofficial-osmo-camera gst-plugin-unofficial-osmo)
checksums=()
published=()
for name in "${names[@]}"; do
    checksum=$(sha256sum "dist/$name-$version.crate")
    checksums+=("${checksum%% *}")
    status=$(curl --proto '=https' --tlsv1.2 --silent --show-error \
        --user-agent osmo-gstreamer-release --output "$response" --write-out '%{http_code}' \
        "https://crates.io/api/v1/crates/$name/$version")
    case "$status" in
        200)
            existing=$(jq -er '.version.checksum | strings | select(test("^[0-9a-f]{64}$"))' "$response")
            if [[ $existing != "${checksum%% *}" ]]; then
                echo "$name $version already exists with different contents. Crate versions cannot be overwritten." >&2
                exit 1
            fi
            echo "$name $version is already published with the expected checksum."
            published+=(true)
            ;;
        404) published+=(false) ;;
        *)
            echo "Registry lookup for $name $version failed with HTTP $status." >&2
            exit 1
            ;;
    esac
done
if [[ ${published[*]} == 'true true' ]]; then
    exit 0
fi
# Match the audited source job; publish dry runs do not retain archives here.
if ! cargo package --locked --workspace; then
    echo 'Pre-publication packaging and verification failed.' >&2
    exit 1
fi
for i in "${!names[@]}"; do
    name=${names[$i]}
    rebuilt="target/package/$name-$version.crate"
    if [[ ! -f $rebuilt ]]; then
        echo "Packaging did not produce $name archive; refusing to publish." >&2
        exit 1
    fi
    checksum=$(sha256sum "$rebuilt")
    if [[ ${checksum%% *} != "${checksums[$i]}" ]]; then
        echo "$name archive differs from the audited build artifact; refusing to publish." >&2
        exit 1
    fi
done
for i in "${!names[@]}"; do
    if [[ ${published[$i]} == true ]]; then
        continue
    fi
    name=${names[$i]}
    if ! cargo publish --locked --registry crates-io -p "$name"; then
        echo "Publishing $name failed. Check crates.io before retrying; an upload may have succeeded." >&2
        exit 1
    fi
done
