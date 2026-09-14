#!/bin/zsh
# pipeline 差分:WebBook 四步流水线(搜索/发现/详情/目录/正文)。
# 契约 fixtures/cases/pipeline/README.md;快照 docs/http-snapshot.md。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_pipeline_cases.py"
"$ROOT/tools/case_diff.sh" pipeline "$@"
MISS=$(grep -c "snapshot_miss" "${TMPDIR:-/tmp}/rubato-diff-pipeline/judge.jsonl" 2>/dev/null || true)
echo "snapshot_miss(两侧一致的未命中,应为 0): $MISS"
