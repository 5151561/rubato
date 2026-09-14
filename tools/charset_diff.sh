#!/bin/zsh
# charset 差分:EncodingDetect.getHtmlEncode(meta 判定 + icu4j CharsetDetector)。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_charset_cases.py"
exec "$ROOT/tools/case_diff.sh" charset "$@"
