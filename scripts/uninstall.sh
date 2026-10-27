#!/bin/sh
set -e

# Pleno Uninstaller
# Reverses scripts/install.sh: removes $PLENO_HOME (~/.pleno by default),
# with the installed tools and plm's package cache, and the PATH line the
# installer added to your shell profile.

PLENO_DIR="${PLENO_HOME:-$HOME/.pleno}"
PATH_LINE="export PATH=\"\$PATH:$PLENO_DIR/bin\""

if [ -d "$PLENO_DIR" ]; then
    rm -rf "$PLENO_DIR"
    echo "Removed $PLENO_DIR"
else
    echo "$PLENO_DIR does not exist; nothing to remove"
fi

for profile in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
    if [ -f "$profile" ] && grep -qxF "$PATH_LINE" "$profile"; then
        # Keep a backup, then drop only the exact line the installer wrote.
        cp "$profile" "$profile.pleno-backup"
        grep -vxF "$PATH_LINE" "$profile.pleno-backup" > "$profile" || true
        echo "Removed the Pleno PATH entry from $profile (backup: $profile.pleno-backup)"
    fi
done

echo "Pleno uninstalled. Restart your terminal to update PATH."
