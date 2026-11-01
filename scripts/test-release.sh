#!/usr/bin/env bash
set -euo pipefail

scripts=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
metadata=$(cargo metadata --locked --no-deps --format-version 1)
version=$(jq -er '.packages[] | select(.name == "unofficial-osmo-camera") | .version' <<< "$metadata")
temp=$(mktemp -d)
trap 'rm -f -- "$temp"/*.log "$temp/output"; rm -rf -- "$temp/fixtures"; rmdir -- "$temp"' EXIT
mkdir -p "$temp/fixtures/dist" "$temp/fixtures/target/package"

assert_fails() {
    local expected=$1
    shift
    if "$@" > "$temp/failure.log" 2>&1; then
        echo "Expected failure: $expected" >&2
        exit 1
    fi
    if ! grep -Fq -- "$expected" "$temp/failure.log"; then
        cat "$temp/failure.log" >&2
        exit 1
    fi
}

GITHUB_OUTPUT="$temp/output" bash "$scripts/validate-release.sh" "v$version"
grep -Fxq "version=$version" "$temp/output"
for tag in 1.2.3 v01.2.3 v1.2.3-rc.1 'v1.2.3;echo bad'; do
    assert_fails 'Release tags must have the form' bash "$scripts/validate-release.sh" "$tag"
done
assert_fails 'must match' bash "$scripts/validate-release.sh" v999999.0.0

cd "$temp/fixtures"
export CARGO_REGISTRY_TOKEN=''
assert_fails 'Configure the CARGO_REGISTRY_TOKEN' bash "$scripts/publish-crates.sh" "$version"
export CARGO_REGISTRY_TOKEN=synthetic-test-token
export TEST_VERSION="$version" TEST_EXISTING_CAMERA='' TEST_EXISTING_PLUGIN=''
export TEST_ALTER_ARCHIVE='' TEST_OMIT_ARCHIVE='' TEST_PACKAGE_EXIT=0 TEST_PUBLISH_EXIT=0 TEST_HTTP_STATUS=404
for name in unofficial-osmo-camera gst-plugin-unofficial-osmo; do
    printf 'synthetic archive for %s\n' "$name" > "dist/$name-$version.crate"
done

curl() {
    local response='' uri='' name checksum=''
    while (($#)); do
        case "$1" in
            --output) response=$2; shift 2 ;;
            https:*) uri=$1; shift ;;
            *) shift ;;
        esac
    done
    name=${uri%/*}
    name=${name##*/}
    case "$name" in
        unofficial-osmo-camera) checksum=$TEST_EXISTING_CAMERA ;;
        gst-plugin-unofficial-osmo) checksum=$TEST_EXISTING_PLUGIN ;;
        *) echo 'Unexpected registry lookup.' >&2; return 1 ;;
    esac
    if [[ -n $checksum ]]; then
        printf '{"version":{"checksum":"%s"}}\n' "$checksum" > "$response"
        printf 200
    else
        printf '{}\n' > "$response"
        printf '%s' "$TEST_HTTP_STATUS"
    fi
}
cargo() {
    case "$1" in
        package)
            [[ $* == 'package --locked --workspace --all-features' ]] || return 1
            echo package >> calls.log
            for name in unofficial-osmo-camera gst-plugin-unofficial-osmo; do
                [[ $name != "$TEST_OMIT_ARCHIVE" ]] || continue
                cp "dist/$name-$TEST_VERSION.crate" "target/package/$name-$TEST_VERSION.crate"
                if [[ $name == "$TEST_ALTER_ARCHIVE" ]]; then
                    echo 'different contents' >> "target/package/$name-$TEST_VERSION.crate"
                fi
            done
            return "$TEST_PACKAGE_EXIT"
            ;;
        publish)
            local name=${6}
            [[ $* == "publish --locked --registry crates-io -p $name --all-features" ]] || return 1
            echo "$name" >> calls.log
            return "$TEST_PUBLISH_EXIT"
            ;;
        *) echo "Unexpected Cargo invocation: $*" >&2; return 1 ;;
    esac
}
export -f curl cargo

assert_calls() {
    local actual=''
    if [[ -f calls.log ]]; then
        actual=$(cat calls.log)
    fi
    if [[ $actual != "$1" ]]; then
        echo "Unexpected release commands: $actual" >&2
        exit 1
    fi
    rm -f calls.log
}

bash "$scripts/publish-crates.sh" "$version"
assert_calls $'package\nunofficial-osmo-camera\ngst-plugin-unofficial-osmo'
export TEST_EXISTING_PLUGIN
TEST_EXISTING_PLUGIN=$(printf different-checksum | sha256sum)
TEST_EXISTING_PLUGIN=${TEST_EXISTING_PLUGIN%% *}
assert_fails 'already exists with different contents' bash "$scripts/publish-crates.sh" "$version"
assert_calls ''
export TEST_EXISTING_CAMERA
TEST_EXISTING_CAMERA=$(sha256sum "dist/unofficial-osmo-camera-$version.crate")
TEST_EXISTING_CAMERA=${TEST_EXISTING_CAMERA%% *}
TEST_EXISTING_PLUGIN=$(sha256sum "dist/gst-plugin-unofficial-osmo-$version.crate")
TEST_EXISTING_PLUGIN=${TEST_EXISTING_PLUGIN%% *}
bash "$scripts/publish-crates.sh" "$version"
assert_calls ''
TEST_EXISTING_PLUGIN=''
bash "$scripts/publish-crates.sh" "$version"
assert_calls $'package\ngst-plugin-unofficial-osmo'
TEST_EXISTING_CAMERA=''
for name in unofficial-osmo-camera gst-plugin-unofficial-osmo; do
    TEST_ALTER_ARCHIVE=$name
    assert_fails 'refusing to publish' bash "$scripts/publish-crates.sh" "$version"
    assert_calls package
done
TEST_ALTER_ARCHIVE=''
TEST_PACKAGE_EXIT=1
assert_fails 'Pre-publication packaging and verification failed' bash "$scripts/publish-crates.sh" "$version"
assert_calls package
TEST_PACKAGE_EXIT=0
TEST_OMIT_ARCHIVE=unofficial-osmo-camera
rm -f "target/package/unofficial-osmo-camera-$version.crate"
assert_fails 'Packaging did not produce unofficial-osmo-camera archive' bash "$scripts/publish-crates.sh" "$version"
assert_calls package
TEST_OMIT_ARCHIVE=''
TEST_PUBLISH_EXIT=1
assert_fails 'Publishing unofficial-osmo-camera failed' bash "$scripts/publish-crates.sh" "$version"
assert_calls $'package\nunofficial-osmo-camera'
TEST_PUBLISH_EXIT=0
TEST_HTTP_STATUS=403
assert_fails 'failed with HTTP 403' bash "$scripts/publish-crates.sh" "$version"
assert_calls ''

gst-inspect-1.0() {
    if [[ $1 == --version ]]; then
        echo 'Synthetic GStreamer runtime'
    else
        touch "$GST_REGISTRY"
        printf 'Filename %s/%s\n' "$GST_PLUGIN_PATH" "$TEST_LIBRARY"
    fi
}
rustc() {
    echo 'Synthetic Rust toolchain'
}
export -f gst-inspect-1.0 rustc
export TEST_LIBRARY
printf 'Synthetic MIT license\n' > LICENSE
printf 'Synthetic README\n' > README.md
printf 'Synthetic notices\n' > dist/THIRD-PARTY-LICENSES.txt
for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin; do
    case "$target" in
        *apple*) TEST_LIBRARY=libgstosmo.dylib ;;
        *) TEST_LIBRARY=libgstosmo.so ;;
    esac
    mkdir -p "target/$target/release"
    printf 'Synthetic plugin\n' > "target/$target/release/$TEST_LIBRARY"
    printf 'Synthetic utility\n' > "target/$target/release/osmo-pair"
    chmod +x "target/$target/release/osmo-pair"
    bash "$scripts/package-native-release.sh" "$version" "$target"
    name="osmo-gstreamer-$version-$target"
    tar -tzf "dist/$name.tar.gz" > "$temp/archive.log"
    for entry in "lib/$TEST_LIBRARY" bin/osmo-pair LICENSE README.md THIRD-PARTY-LICENSES.txt BUILD-INFO.txt; do
        grep -Fxq "$name/$entry" "$temp/archive.log"
    done
    case "$OSTYPE" in
        linux*|darwin*) [[ -x dist/$name/bin/osmo-pair ]] ;;
    esac
    assert_fails 'Release staging directory already exists' bash "$scripts/package-native-release.sh" "$version" "$target"
done
assert_fails 'Unsupported native target' bash "$scripts/package-native-release.sh" "$version" invalid-target
echo 'Release tests passed without registry uploads or hardware.'
