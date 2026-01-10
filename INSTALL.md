# HiWave Linux Installation Guide

This guide will walk you through installing HiWave on Linux from source. HiWave uses GTK WebKit2 for rendering web content.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
   - [Ubuntu/Debian](#ubuntudebian)
   - [Fedora/RHEL](#fedorarhel)
   - [Arch Linux](#arch-linux)
2. [Installing Rust](#installing-rust)
3. [Building HiWave](#building-hiwave)
4. [Running HiWave](#running-hiwave)
5. [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Ubuntu/Debian

```bash
# Update package lists
sudo apt update

# Install build essentials
sudo apt install -y build-essential

# Install required libraries (Ubuntu 22.04+ / Debian 12+)
sudo apt install -y \
    pkg-config \
    libssl-dev \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libsoup-3.0-dev \
    libjavascriptcoregtk-4.1-dev

# Install Git
sudo apt install -y git
```

**For older Ubuntu (20.04) / Debian 11:**
```bash
sudo apt install -y \
    pkg-config \
    libssl-dev \
    libgtk-3-dev \
    libwebkit2gtk-4.0-dev \
    libsoup2.4-dev
```

### Fedora/RHEL

```bash
# Install development tools
sudo dnf groupinstall -y "Development Tools"

# Install required libraries
sudo dnf install -y \
    pkg-config \
    openssl-devel \
    gtk3-devel \
    webkit2gtk4.1-devel \
    libsoup3-devel

# Install Git
sudo dnf install -y git
```

### Arch Linux

```bash
# Install base development tools
sudo pacman -S --needed base-devel

# Install required libraries
sudo pacman -S \
    pkg-config \
    openssl \
    gtk3 \
    webkit2gtk-4.1 \
    libsoup3

# Install Git
sudo pacman -S git
```

---

## Installing Rust

HiWave requires Rust 1.75 or newer.

### Using rustup (Recommended)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

When prompted:
1. Press `1` for default installation
2. Follow the on-screen instructions

After installation, load Rust into your current shell:

```bash
source "$HOME/.cargo/env"
```

**Verify installation:**
```bash
rustc --version
# Should output: rustc 1.75.0 or newer

cargo --version
# Should output: cargo 1.75.0 or newer
```

### Update Existing Rust

If you already have Rust installed:

```bash
rustup update stable
```

---

## Building HiWave

### Clone the Repository

```bash
git clone https://github.com/hiwavebrowser/hiwave-linux.git
cd hiwave-linux
```

### Build

**Using the build script:**
```bash
./scripts/build.sh
```

**Or manually with cargo:**
```bash
cargo build --release -p hiwave-app
```

The first build will take several minutes as it downloads and compiles dependencies.

### Build Output

After a successful build, you'll find the binary at:
```
target/release/hiwave
```

---

## Running HiWave

**Using the run script:**
```bash
./scripts/run.sh
```

**Or directly:**
```bash
./target/release/hiwave
```

**Or with cargo:**
```bash
cargo run --release -p hiwave-app
```

### Debug Mode

For development with verbose logging:

```bash
./scripts/run-debug.sh

# Or
RUST_LOG=debug cargo run -p hiwave-app
```

---

## Troubleshooting

### Missing WebKit2GTK

**Error:** `pkg-config: webkit2gtk-4.1 not found`

**Solution:** Install the WebKit2GTK development package:
```bash
# Ubuntu/Debian
sudo apt install libwebkit2gtk-4.1-dev

# Fedora
sudo dnf install webkit2gtk4.1-devel

# Arch
sudo pacman -S webkit2gtk-4.1
```

### Missing GTK3

**Error:** `pkg-config: gtk+-3.0 not found`

**Solution:**
```bash
# Ubuntu/Debian
sudo apt install libgtk-3-dev

# Fedora
sudo dnf install gtk3-devel

# Arch
sudo pacman -S gtk3
```

### OpenSSL Not Found

**Error:** `Could not find directory of OpenSSL`

**Solution:**
```bash
# Ubuntu/Debian
sudo apt install libssl-dev pkg-config

# Fedora
sudo dnf install openssl-devel

# Arch
sudo pacman -S openssl
```

### Linker Errors

**Error:** `/usr/bin/ld: cannot find -lXXX`

**Solution:** You're likely missing a development library. Check which library is missing and install the corresponding `-dev` or `-devel` package.

### Cargo Not Found

**Error:** `cargo: command not found`

**Solution:** Ensure Rust is installed and in your PATH:
```bash
source "$HOME/.cargo/env"
```

Or add this to your `~/.bashrc` or `~/.zshrc`:
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

---

## Updating HiWave

To update to the latest version:

```bash
cd hiwave-linux
git pull origin master
cargo build --release -p hiwave-app
```

---

## Uninstalling

To remove HiWave:

```bash
# Remove the HiWave directory
rm -rf ~/hiwave-linux  # Adjust path as needed

# Remove user data (optional)
rm -rf ~/.local/share/hiwave
rm -rf ~/.config/hiwave
```

To uninstall Rust (if no longer needed):
```bash
rustup self uninstall
```

---

## Getting Help

- **Issues:** [GitHub Issues](https://github.com/hiwavebrowser/hiwave-linux/issues)
- **Discussions:** [GitHub Discussions](https://github.com/hiwavebrowser/hiwave-linux/discussions)
