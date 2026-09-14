#!/bin/zsh
# xpath 差分:AnalyzeByXPath(Kotlin 裁判,真 JsoupXpath 2.5.5)
# vs xpath-compat crate(Rust 移植)。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_xpath_cases.py"
exec "$ROOT/tools/case_diff.sh" xpath "$@"
