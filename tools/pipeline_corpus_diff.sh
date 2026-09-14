#!/bin/zsh
# pipeline-corpus 差分:A 层真实书源 × WebBook 四步(回落页供页,
# 契约 fixtures/cases/pipeline-corpus/README.md)。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_pipeline_corpus_cases.py"
exec "$ROOT/tools/case_diff.sh" pipeline-corpus "$@"
