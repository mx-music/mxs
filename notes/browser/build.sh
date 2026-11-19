#!/bin/sh
# Builds mxs as WebAssembly into public/, which any static server then serves: python3 -m http.server -d public 5198
set -e
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/mxs_web.wasm public/
