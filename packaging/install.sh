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
if [[ -d "$script_dir/assets" && -d "$script_dir/scenarios" ]]; then
    bundle="$script_dir"
else
    bundle="$(CDPATH= cd -- "$script_dir/.." && pwd -P)"
fi
binary="$bundle/bin/bv-sim"
if [[ ! -f "$binary" ]]; then
    binary="$bundle/target/release/bv-sim"
fi
if [[ ! -f "$binary" || ! -d "$bundle/assets" || ! -d "$bundle/scenarios" ]]; then
    echo "Install the complete release bundle, or first run cargo build --release -p bv-sim in the source checkout." >&2
    exit 1
fi
version_output="$("$binary" --version)"
if [[ ! "$version_output" =~ ^bv-sim[[:space:]]([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
    echo "Cannot determine the release version from bv-sim --version." >&2
    exit 1
fi
version="${BASH_REMATCH[1]}"

install_root="$HOME/.local/share/bv-sim"
bin_dir="$HOME/.local/bin"
release_dir="$install_root/releases/$version"
mkdir -p "$release_dir/bin" "$bin_dir"
cp -R "$bundle/assets" "$bundle/scenarios" "$release_dir/"
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
    printf 'Open a new terminal, start Docker, then run: bv-sim run suas_2026\n'
else
    printf 'Add %s to your PATH, start Docker, then run: bv-sim run suas_2026\n' "$bin_dir"
fi
