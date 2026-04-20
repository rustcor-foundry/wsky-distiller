#!/bin/bash
set -e

echo "Installing distill v1.0.0 ecosystem..."

# Build all tools
echo "Building distill..."
cargo build --release --manifest-path ../distill/Cargo.toml

echo "Building distill-render..."
cargo build --release --manifest-path ../distill-render/Cargo.toml

echo "Building distill-gui..."
cargo build --release --manifest-path ../distill-gui/Cargo.toml

# Install
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

cp ../distill/target/release/distill "$INSTALL_DIR/"
cp ../distill-render/target/release/distill-render "$INSTALL_DIR/"
cp ../distill-gui/target/release/distill-gui "$INSTALL_DIR/"

echo "✅ Installed to $INSTALL_DIR"
echo ""
echo "Make sure $INSTALL_DIR is in your PATH"
echo "Example: export PATH=\"\$HOME/.local/bin:\$PATH\""
echo ""
echo "Run 'distill-gui' to start the GUI"
