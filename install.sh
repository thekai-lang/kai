#!/usr/bin/env bash
set -e

KAI_HOME="$HOME/.kai"
KAI_BIN="$KAI_HOME/bin"

echo "=> Building Kai compiler in release mode..."
cargo build --release

echo "=> Preparing Kai installation directory ($KAI_BIN)..."
mkdir -p "$KAI_BIN"

echo "=> Installing kai binary..."
cp target/release/kai "$KAI_BIN/kai"
chmod +x "$KAI_BIN/kai"

echo "=> Installation complete!"
echo "=> Kai version:"
"$KAI_BIN/kai" --version

PROFILE_STRING="export PATH=\"\$HOME/.kai/bin:\$PATH\""

add_to_profile() {
    local profile_file="$1"
    if [ -f "$profile_file" ]; then
        if ! grep -q "$PROFILE_STRING" "$profile_file"; then
            echo "" >> "$profile_file"
            echo "# Kai Compiler" >> "$profile_file"
            echo "$PROFILE_STRING" >> "$profile_file"
            echo "=> Added ~/.kai/bin to $profile_file"
        else
            echo "=> ~/.kai/bin is already in $profile_file"
        fi
    fi
}

echo "=> Configuring PATH..."
add_to_profile "$HOME/.bashrc"
add_to_profile "$HOME/.zshrc"

echo ""
echo "=========================================================="
echo "INSTALLATION SUCCESSFUL!"
echo "To use Kai in this current terminal immediately, run:"
echo "  source ~/.bashrc   (or source ~/.zshrc)"
echo "Or simply restart your terminal."
echo "=========================================================="
