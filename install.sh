#!/usr/bin/env bash
# Download and install the latest published release without a source checkout.
set -euo pipefail

case "$(uname -s)/$(uname -m)" in
    Darwin/arm64|Darwin/aarch64) target=aarch64-apple-darwin ;;
    Linux/x86_64|Linux/amd64) target=x86_64-unknown-linux-gnu ;;
    *) echo "Supported releases: Apple Silicon macOS and x86_64 Linux (Ubuntu 24.04 or compatible)." >&2; exit 1 ;;
esac
for program in curl tar; do
    command -v "$program" >/dev/null || { echo "Install $program, then retry." >&2; exit 1; }
done

repository=https://github.com/BuckeyeVertical/bv_sim
echo "Finding the latest BV Sim release..."
release_url="$(curl --fail --silent --show-error --location --head \
    --output /dev/null --write-out '%{url_effective}' "$repository/releases/latest")"
tag="${release_url##*/}"
if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Could not resolve a published release from $repository/releases/latest" >&2
    exit 1
fi
version="${tag#v}"
download_dir="$(mktemp -d "${TMPDIR:-/tmp}/bv-sim-download.XXXXXX")"
trap 'rm -rf "$download_dir"' EXIT
archive="bv-sim-$version-$target.tar.gz"
echo "Downloading BV Sim $version for $target..."
curl --fail --silent --show-error --location \
    "$repository/releases/download/$tag/$archive" --output "$download_dir/release.tar.gz"
tar -xzf "$download_dir/release.tar.gz" -C "$download_dir"
bash "$download_dir/bv-sim-$version-$target/install.sh" "$@"
