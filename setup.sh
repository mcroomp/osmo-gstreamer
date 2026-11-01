#!/usr/bin/env bash
set -euo pipefail

if ! command -v apt-get >/dev/null 2>&1 || ! command -v dpkg-query >/dev/null 2>&1; then
    echo "ERROR: setup.sh supports Debian/Ubuntu environments with apt-get." >&2
    exit 1
fi

packages=(build-essential ca-certificates curl gstreamer1.0-tools libdbus-1-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev pkg-config)
missing_packages=()
for package in "${packages[@]}"; do
    if ! dpkg-query -W -f='${db:Status-Status}' "$package" 2>/dev/null | grep -qx installed; then
        missing_packages+=("$package")
    fi
done

if ((${#missing_packages[@]})); then
    if ! command -v sudo >/dev/null 2>&1; then
        echo "ERROR: sudo is required to install missing Linux development packages: ${missing_packages[*]}" >&2
        exit 1
    fi
    sudo -v
    sudo apt-get update
    sudo apt-get install -y "${missing_packages[@]}"
else
    echo "Linux development packages are already installed."
fi

if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
        sh -s -- -y --profile minimal --default-toolchain none
fi

export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy

echo
echo "Linux development prerequisites are ready."
echo "Run the workspace tests with: cargo +1.99.1 test --locked --workspace --all-features"
