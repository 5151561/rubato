#!/bin/zsh
# html-compat 差分:jsoup 1.16.2(Kotlin 裁判)vs html-compat(Rust)。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_html_cases.py"
exec "$ROOT/tools/case_diff.sh" html-compat "$@"
