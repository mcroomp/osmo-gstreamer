#!/usr/bin/env bash
set -euo pipefail

version=${1:?Usage: bash scripts/package-native-release.sh X.Y.Z TARGET}
target=${2:?Usage: bash scripts/package-native-release.sh X.Y.Z TARGET}
if [[ ! $version =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
    echo 'Release version must have the form X.Y.Z, without leading zeroes.' >&2
    exit 1
fi
case "$target" in
    x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu) library=libgstosmo.so ;;
    x86_64-apple-darwin|aarch64-apple-darwin) library=libgstosmo.dylib ;;
    *)
        echo "Unsupported native target: $target. Windows packaging uses package-windows-release.ps1." >&2
        exit 1
        ;;
esac
name="osmo-gstreamer-$version-$target"
stage="dist/$name"
if [[ -e $stage ]]; then
    echo "Release staging directory already exists: $stage" >&2
    exit 1
fi
mkdir -p "$stage/lib" "$stage/bin"
cp "target/$target/release/$library" "$stage/lib/"
cp "target/$target/release/osmo-pair" "$stage/bin/"
cp LICENSE README.md PROTOCOL.md dist/THIRD-PARTY-LICENSES.txt "$stage/"
export GST_PLUGIN_PATH
GST_PLUGIN_PATH=$(cd "$stage/lib" && pwd -P)
registry_dir=$(mktemp -d)
export GST_REGISTRY="$registry_dir/registry.bin"
trap 'rm -f -- "$GST_REGISTRY"; rmdir -- "$registry_dir"' EXIT
if ! inspection=$(gst-inspect-1.0 osmosrc 2>&1); then
    echo "Packaged plugin inspection failed: $inspection" >&2
    exit 1
fi
if [[ $inspection != *"$GST_PLUGIN_PATH/$library"* ]]; then
    echo 'Inspection did not load the staged plugin.' >&2
    exit 1
fi
printf '%s\n' "$inspection"
runtime=$(gst-inspect-1.0 --version)
rust_version=$(rustc --version)
{
    printf 'Version: %s\nCommit: %s\nTarget: %s\nRunner: %s %s\nRust: %s\n%s\n' \
        "$version" "${GITHUB_SHA:-}" "$target" "${ImageOS:-}" "${ImageVersion:-}" "$rust_version" "$runtime"
    echo 'GStreamer and system libraries are not bundled. Use the recorded runtime version or a separately validated compatible runtime.'
    echo 'Hardware compatibility is not certified. See README.md for authorization and model limitations.'
} > "$stage/BUILD-INFO.txt"
tar -czf "dist/$name.tar.gz" -C dist "$name"
