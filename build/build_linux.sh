#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 1 || ! "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Usage: bash build/build_linux.sh X.Y.Z" >&2
    exit 1
fi
version="$1"

if [[ "$(uname -s)" != "Linux" ]]; then
    echo "This helper must be run on Linux." >&2
    exit 1
fi

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
project_dir="$(cd -- "$script_dir/.." && pwd)"
cd "$project_dir"

host="$(rustc -vV | sed -n 's/^host: //p')"
if [[ -z "$host" ]]; then
    echo "Could not determine the Rust host target." >&2
    exit 1
fi

if LC_ALL=C grep -q $'\r' collect_linux.sh; then
    echo "collect_linux.sh contains CRLF line endings; convert it to LF before packaging." >&2
    exit 1
fi

cargo build --release --target "$host" --target-dir "$project_dir/target"
mkdir -p -- "$script_dir/linux"
cp -- "$project_dir/target/$host/release/prime_sieve" "$script_dir/linux/prime_sieve"
cp -- "$project_dir/collect_linux.sh" "$script_dir/linux/collect_linux.sh"

cleanup() {
    rm -f -- "$script_dir/linux/.env"
    rm -rf -- "$script_dir/linux/results"
}
trap cleanup EXIT
cleanup
cd "$script_dir/linux"
bash collect_linux.sh
./prime_sieve count 1000000
shopt -s nullglob
reports=(results/result_test_count_1000000_*.txt)
if (( ${#reports[@]} != 1 )) || ! grep -q '^Limit: 1000000, Prime count: 78498,' "${reports[0]}"; then
    echo "Count smoke test failed: expected 78498 primes up to 1000000." >&2
    exit 1
fi
cleanup
trap - EXIT
cd "$script_dir"
archive="V.${version}-linux.zip"
tmp_archive="$(mktemp "$script_dir/.archive.XXXXXX.zip")"
trap 'rm -f -- "$tmp_archive"' EXIT
python3 -m zipfile -c "$tmp_archive" linux
mv -f -- "$tmp_archive" "$archive"
trap - EXIT
echo "Linux package created in $script_dir/linux"
echo "Archive created in $script_dir/$archive"
