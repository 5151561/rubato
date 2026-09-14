#!/bin/zsh
# fetch 差分:HTTP 录放全链路(AnalyzeUrl.getStrResponse + CookieStore)。
# 两侧回放客户端契约:docs/http-snapshot.md。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_fetch_cases.py"
"$ROOT/tools/case_diff.sh" fetch "$@"
# snapshot_miss 是合法结果(两侧一致才 PASS),但要单列提醒补录
MISS=$(grep -c "snapshot_miss" "${TMPDIR:-/tmp}/rubato-diff-fetch/judge.jsonl" 2>/dev/null || true)
echo "snapshot_miss(两侧一致的未命中,key 等价类用例属正常): $MISS"
