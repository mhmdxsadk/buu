#!/usr/bin/env bash
# Print the Homebrew formula for a release, with the download URLs and
# checksums taken from that release's checksums.txt.
#
#   scripts/formula.sh vX.Y.Z checksums.txt >Formula/buu.rb
set -euo pipefail

tag=${1:?usage: formula.sh vX.Y.Z checksums.txt}
sums=${2:?usage: formula.sh vX.Y.Z checksums.txt}
version=${tag#v}
repo=${GITHUB_REPOSITORY:-mhmdxsadk/buu}
base="https://github.com/$repo/releases/download/$tag"

sha() {
  local file="buu_${version}_$1.tar.gz" sum
  sum=$(awk -v file="$file" '$2 == file { print $1 }' "$sums")
  [ -n "$sum" ] || { echo "no checksum for $file" >&2; exit 1; }
  printf '%s' "$sum"
}

darwin_arm64=$(sha darwin_arm64)
darwin_amd64=$(sha darwin_amd64)
linux_arm64=$(sha linux_arm64)
linux_amd64=$(sha linux_amd64)

cat <<FORMULA
class Buu < Formula
  desc "Lightweight CLI tool for maintaining Homebrew, written in Rust"
  homepage "https://github.com/$repo"
  version "$version"
  license "MIT"

  on_macos do
    on_arm do
      url "$base/buu_${version}_darwin_arm64.tar.gz"
      sha256 "$darwin_arm64"
    end
    on_intel do
      url "$base/buu_${version}_darwin_amd64.tar.gz"
      sha256 "$darwin_amd64"
    end
  end

  on_linux do
    on_arm do
      url "$base/buu_${version}_linux_arm64.tar.gz"
      sha256 "$linux_arm64"
    end
    on_intel do
      url "$base/buu_${version}_linux_amd64.tar.gz"
      sha256 "$linux_amd64"
    end
  end

  def install
    bin.install "buu"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/buu --version")
  end
end
FORMULA
