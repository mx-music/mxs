#!/bin/sh
# Builds the mxs package into package/: mxs as WebAssembly, and the tables of the synths that play them.
set -e
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/mxs.wasm package/
mkdir -p package/tables
cp ../src/synths/figures/*.mx[fh] package/tables/
