#!/bin/bash
# Run HiWave browser in debug mode with verbose logging

set -e

cd "$(dirname "$0")/.."

echo "Building HiWave (debug)..."
cargo build

echo "Running HiWave with debug logging..."
RUST_LOG=debug ./target/debug/hiwave "$@"
