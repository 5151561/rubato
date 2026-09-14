#!/bin/zsh
# json-compat 差分:jayway JsonPath 3.0.0(Kotlin 裁判)vs json-compat(Rust)。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_json_cases.py"
exec "$ROOT/tools/case_diff.sh" json-compat "$@"
