#!/bin/zsh
# regex-compat 差分:java.util.regex(Kotlin 裁判)vs regex-compat(Rust)。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_regex_cases.py"
exec "$ROOT/tools/case_diff.sh" regex-compat "$@"
