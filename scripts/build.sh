#!/usr/bin/env bash
# build.sh: compile the release binary into bin/rs-http-apilab.
# The rustc version is baked into /version at build time by build.rs.
set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v cargo >/dev/null 2>&1; then
	echo "FATAL: cargo not found; rust-toolchain.toml pins the version" >&2
	exit 1
fi

echo "building with: $(rustc --version)"
mkdir -p bin
cargo build --release --locked
cp -f target/release/rs-http-apilab bin/rs-http-apilab
echo "built: bin/rs-http-apilab"
