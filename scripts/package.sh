#!/usr/bin/env bash
# Assemble the release assets for a tag from the per-platform binaries built
# by the release workflow.
#
#   scripts/package.sh vX.Y.Z [BIN_DIR] [OUT_DIR]
#
# BIN_DIR holds buu-<os>-<arch>/buu for each platform. OUT_DIR receives a
# reproducible tar.gz and a syft SBOM per platform, plus checksums.txt over
# both. Needs GNU tar (set TAR to point at it) and syft.
set -euo pipefail

tag=${1:?usage: package.sh vX.Y.Z [bin-dir] [out-dir]}
bin_dir=${2:-bin}
out_dir=${3:-dist}
tar_cmd=${TAR:-tar}
version=${tag#v}
epoch=$(git log -1 --format=%ct)

mkdir -p "$out_dir"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

for platform in linux-amd64 linux-arm64 darwin-amd64 darwin-arm64; do
  archive="buu_${version}_${platform//-/_}.tar.gz"
  install -m 0755 "$bin_dir/buu-$platform/buu" "$stage/buu"
  install -m 0644 LICENSE README.md "$stage/"
  "$tar_cmd" --sort=name --owner=0 --group=0 --numeric-owner \
    --mtime="@$epoch" -cf - -C "$stage" buu LICENSE README.md |
    gzip -n >"$out_dir/$archive"
  syft "$out_dir/$archive" --output "spdx-json=$out_dir/$archive.sbom.json"
done

cd "$out_dir"
if command -v sha256sum >/dev/null; then
  sha256sum -- *.tar.gz *.sbom.json >checksums.txt
else
  shasum -a 256 -- *.tar.gz *.sbom.json >checksums.txt
fi
