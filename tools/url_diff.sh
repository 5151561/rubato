#!/bin/zsh
# java-url 差分:java.net.URL + NetworkUtils.getAbsoluteURL(Kotlin 裁判)
# vs rubato-core::java_url(Rust)。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
python3 "$ROOT/tools/gen_url_cases.py"
exec "$ROOT/tools/case_diff.sh" java-url "$@"
