#!/bin/zsh
# rule-engine 差分:AnalyzeRule(Kotlin 裁判,JS/网络走确定性桩)
# vs rule-engine crate(Rust,同契约桩)。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_rule_cases.py"
exec "$ROOT/tools/case_diff.sh" rule-engine "$@"
