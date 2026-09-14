#!/bin/zsh
# analyze-url 差分:AnalyzeUrl 离线面(Kotlin 裁判,私有字段用反射读)
# vs net crate(Rust)。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_url_rule_cases.py"
exec "$ROOT/tools/case_diff.sh" analyze-url "$@"
