#!/bin/bash
# Build HiWave browser for Linux

set -e

cd "$(dirname "$0")/.."

# Install dependencies if needed (Debian/Ubuntu)
install_deps() {
    echo "Installing build dependencies..."
    sudo apt-get update
    sudo apt-get install -y \
        build-essential \
        pkg-config \
        libgtk-3-dev \
        libwebkit2gtk-4.1-dev \
        libssl-dev \
        libsoup-3.0-dev \
        libjavascriptcoregtk-4.1-dev
}

# Check if we need to install deps
if ! pkg-config --exists gtk+-3.0 webkit2gtk-4.1 2>/dev/null; then
    echo "Missing dependencies. Install them? (y/n)"
    read -r response
    if [[ "$response" =~ ^[Yy]$ ]]; then
        install_deps
    else
        echo "Please install the required dependencies manually:"
        echo "  - gtk+-3.0"
        echo "  - webkit2gtk-4.1"
        echo "  - libssl"
        echo "  - libsoup-3.0"
        exit 1
    fi
fi

echo "Building HiWave for Linux..."

# Build in release mode
cargo build --release

echo ""
echo "Build complete!"
echo "Binary: $(pwd)/target/release/hiwave"
echo ""
echo "Run with: ./scripts/run.sh"
