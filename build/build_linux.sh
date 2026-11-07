#!/usr/bin/env bash

set -euo pipefail

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
echo "Linux package created in $script_dir/linux"
