#!/bin/bash
# Run HiWave browser on Linux (GTK WebKit mode)

set -e

cd "$(dirname "$0")/.."

echo "Building HiWave for Linux..."
cargo build --release

echo "Running HiWave..."
./target/release/hiwave "$@"
