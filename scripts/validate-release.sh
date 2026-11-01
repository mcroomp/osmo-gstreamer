#!/usr/bin/env bash
set -euo pipefail

tag=${1:?Usage: bash scripts/validate-release.sh vX.Y.Z}
if [[ ! $tag =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
    echo "Release tags must have the form vX.Y.Z, without leading zeroes: '$tag'." >&2
    exit 1
fi
version=${tag#v}
if ! metadata=$(cargo metadata --locked --no-deps --format-version 1); then
    echo 'Could not read locked Cargo metadata.' >&2
    exit 1
fi
for name in osmo-camera gst-plugin-osmo; do
    if ! package=$(jq -ce --arg name "$name" '[.packages[] | select(.name == $name)] | select(length == 1) | .[0]' <<< "$metadata"); then
        echo "Tag '$tag' must match exactly one $name package." >&2
        exit 1
    fi
    if [[ $(jq -r '.version' <<< "$package") != "$version" ]]; then
        echo "Tag '$tag' must match the $name package version." >&2
        exit 1
    fi
    if [[ $(jq -r '.license' <<< "$package") != MIT ]]; then
        echo "$name must declare the MIT SPDX license." >&2
        exit 1
    fi
    for field in description readme homepage repository documentation; do
        if ! jq -e --arg field "$field" '.[$field] | strings | test("\\S")' <<< "$package" >/dev/null; then
            echo "$name is missing publication metadata: $field." >&2
            exit 1
        fi
    done
    for field in authors keywords categories; do
        if ! jq -e --arg field "$field" '.[$field] | arrays | length > 0' <<< "$package" >/dev/null; then
            echo "$name is missing publication metadata: $field." >&2
            exit 1
        fi
    done
    manifest=$(jq -r '.manifest_path' <<< "$package")
    if command -v cygpath >/dev/null 2>&1; then
        manifest=$(cygpath -u "$manifest")
    fi
    crate_license=$(tr -d '\r' < "$(dirname "$manifest")/LICENSE")
    root_license=$(tr -d '\r' < LICENSE)
    if [[ $crate_license != "$root_license" ]]; then
        echo "$name LICENSE must match the repository MIT license." >&2
        exit 1
    fi
done
if ! jq -e --arg version "=$version" '
    [.packages[] | select(.name == "gst-plugin-osmo") |
        .dependencies[] | select(.name == "osmo-camera") | .req] == [$version]
' <<< "$metadata" >/dev/null; then
    echo "gst-plugin-osmo must depend on osmo-camera with version '=$version'." >&2
    exit 1
fi
if [[ -n ${GITHUB_OUTPUT:-} ]]; then
    printf 'version=%s\n' "$version" >> "$GITHUB_OUTPUT"
fi
echo "Validated release $tag."
