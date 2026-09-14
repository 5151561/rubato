#!/bin/zsh
# source 差分:BookSource 的 GSON 反序列化语义(裁判 GSON vs rubato-core::entities)。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_source_cases.py"
exec "$ROOT/tools/case_diff.sh" source "$@"
