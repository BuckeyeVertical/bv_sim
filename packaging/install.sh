#!/usr/bin/env bash
# User-local install from an extracted release or a built source checkout.
set -euo pipefail

no_path=false
if [[ "${1:-}" == "--no-path" ]]; then
    no_path=true
    shift
fi
if [[ "$#" -ne 0 ]]; then
    echo "Usage: $0 [--no-path]" >&2
    exit 2
fi
: "${HOME:?HOME is required for a user-local installation}"

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)"
if [[ -f "$script_dir/VERSION" ]]; then
    bundle="$script_dir"
else
    bundle="$(CDPATH= cd -- "$script_dir/.." && pwd -P)"
fi
version="$(cat "$bundle/VERSION")"
case "$version" in
    ''|*[!0-9A-Za-z._-]*) echo "Invalid release VERSION" >&2; exit 1 ;;
esac
binary="$bundle/bin/bv-sim"
if [[ ! -f "$binary" ]]; then
    binary="$bundle/target/release/bv-sim"
fi
if [[ ! -f "$binary" || ! -d "$bundle/assets" || ! -d "$bundle/scenarios" ]]; then
    echo "Install the complete release bundle, or first run cargo build --release -p bv-sim in the source checkout." >&2
    exit 1
fi

install_root="$HOME/.local/share/bv-sim"
bin_dir="$HOME/.local/bin"
release_dir="$install_root/releases/$version"
mkdir -p "$release_dir/bin" "$bin_dir"
cp -R "$bundle/assets" "$bundle/scenarios" "$release_dir/"
cp "$bundle/VERSION" "$release_dir/VERSION"
install -m 755 "$binary" "$release_dir/bin/bv-sim"
ln -sfn "releases/$version" "$install_root/current"

# Emit a literal quoted path rather than evaluating any part of the home path.
shell_quote() {
    printf "'"
    printf '%s' "$1" | sed "s/'/'\\\\''/g"
    printf "'"
}
{
    printf '#!/bin/sh\nexec '
    shell_quote "$install_root/current/bin/bv-sim"
    printf ' "$@"\n'
} > "$bin_dir/bv-sim"
chmod 755 "$bin_dir/bv-sim"

config=""
case "${SHELL:-}" in
    */zsh) config="$HOME/.zshrc" ;;
    */bash)
        if [[ "$(uname -s)" == Darwin ]]; then config="$HOME/.bash_profile";
        else config="$HOME/.bashrc"; fi ;;
esac
path_line='export PATH="$HOME/.local/bin:$PATH"'
if [[ "$no_path" == false && -n "$config" ]]; then
    if [[ ! -f "$config" ]] || ! grep -Fqx "$path_line" "$config"; then
        printf '\n# BV simulator command\n%s\n' "$path_line" >> "$config"
    fi
fi

printf 'Installed bv-sim %s to %s\n' "$version" "$release_dir"
if [[ "$no_path" == false && -n "$config" ]]; then
    printf 'Open a new terminal, then run: bv-sim doctor\n'
else
    printf 'Add %s to your PATH, then run: bv-sim doctor\n' "$bin_dir"
fi
