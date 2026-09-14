#!/bin/zsh
# rule-syntax 差分:RuleAnalyzer(Kotlin 裁判)vs rule-syntax(Rust)。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_syntax_cases.py"
exec "$ROOT/tools/case_diff.sh" rule-syntax "$@"
