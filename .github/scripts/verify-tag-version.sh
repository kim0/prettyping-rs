#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <tag>"
  exit 2
fi

raw_tag="$1"
if [[ "$raw_tag" != v* ]]; then
  echo "tag must start with 'v' (got: $raw_tag)"
  exit 2
fi

tag_version="${raw_tag#v}"

cargo_version="$(python3 - <<'PY'
import pathlib
import tomllib

cargo_toml = pathlib.Path("Cargo.toml").read_text(encoding="utf-8")
data = tomllib.loads(cargo_toml)
print(data["package"]["version"])
PY
)"

if [[ "$tag_version" != "$cargo_version" ]]; then
  echo "Tag/version mismatch: tag=$tag_version Cargo.toml=$cargo_version"
  exit 1
fi

echo "Tag/version match: $raw_tag == Cargo.toml version $cargo_version"
