#!/bin/zsh
# webview 差分:**BackstageWebView 的策略层**(真身 394 行里 WebView 之外那一半)。
#
# 与别的套不同的两处,都写在 fixtures/cases/webview/README.md 里:
#  1. 裁判是 **:wvharness**——第三个 harness。:harness 与 :jsharness 里的
#     `BackstageWebView` 都是确定性桩(fetch / analyze-url / rule-engine / js-host
#     四套的判据建在那个契约上),同一个 classpath 上只能有一份,故分模块;
#  2. 两侧的 WebView 都换成**同一份剧本 + 虚拟时钟** —— 于是补 900ms、
#     `100 + delayTime`、重试梯子 `200/400/600/800/1000`、`retry > 30`、60s 超时
#     逐毫秒可比,整套跑完不到一秒。**真的去加载一个页面**那一半进不了差分,
#     归 fixtures/phase3/checklist.json 的手工清单。
#
# 用法: webview_diff.sh [--show=N]
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Rust 侧的构建 profile。缺省 debug(差分判据面用它就够了);逐例计时
# (tools/time_diff.sh)一律 RUBATO_DIFF_PROFILE=release —— debug 的 Rust 在
# 正则/解析这类活上能慢一个量级,拿它去和 JIT 热了的 JVM 比是自己给自己抹黑。
typeset -a CARGO_PROFILE_FLAGS
CARGO_PROFILE_FLAGS=()
PROFILE_DIR=debug
if [[ "${RUBATO_DIFF_PROFILE:-debug}" == release ]]; then
  CARGO_PROFILE_FLAGS=(--release)
  PROFILE_DIR=release
fi

NAME="webview"
CASES_DIR="$ROOT/fixtures/cases/$NAME"
WORK="${TMPDIR:-/tmp}/rubato-diff-$NAME"
mkdir -p "$WORK"

python3 "$ROOT/tools/gen_webview_cases.py"

python3 - "$WORK/cases.json" "$CASES_DIR"/*.json <<'PY'
import json, sys
merged = []
for p in sys.argv[2:]:
    if p.endswith("exemptions.json"):
        continue
    merged += json.load(open(p, encoding="utf-8"))
ids = [c["id"] for c in merged]
assert len(ids) == len(set(ids)), "case id 冲突"
json.dump(merged, open(sys.argv[1], "w", encoding="utf-8"), ensure_ascii=False)
print(f"{len(merged)} cases", file=sys.stderr)
PY

(cd "$ROOT/judge" && ./gradlew -q :wvharness:run --args="$WORK/cases.json $WORK/judge.jsonl")

cargo build -q --manifest-path "$ROOT/rust/Cargo.toml" -p difftest --bin webview_case_runner "${CARGO_PROFILE_FLAGS[@]}"
"$ROOT/rust/target/$PROFILE_DIR/webview_case_runner" "$WORK/cases.json" "$WORK/rust.jsonl"

EXEMPT_ARGS=(--cases="$WORK/cases.json")
if [[ -f "$CASES_DIR/exemptions.json" ]]; then
  EXEMPT_ARGS+=(--exempt="$CASES_DIR/exemptions.json")
fi
exec python3 "$ROOT/tools/compare_jsonl.py" "$WORK/judge.jsonl" "$WORK/rust.jsonl" \
  "${EXEMPT_ARGS[@]}" "$@"
