#!/bin/zsh
# 发现页分类列表差分:`BookSourceExtensions.exploreKinds()`(裁判是 :jsharness,
# **挂的是那个函数的真身**)vs `pipeline::explore_kinds`(被测)。
#
# 与 js 套同一个裁判进程(真 Rhino)——`@js:` / `<js>` 那两支的发现规则要真
# 引擎才跑得起来;而与 js 套分开成一套,是因为分母不同:那套的分母是「书源
# 里的 JS 片段」,这套的分母是「有 exploreUrl 的书源」(语料 977 源)。
#
# 用法: explore_diff.sh [--show=N]
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

NAME="explore"
CASES_DIR="$ROOT/fixtures/cases/$NAME"
WORK="${TMPDIR:-/tmp}/rubato-diff-$NAME"
mkdir -p "$WORK"

python3 "$ROOT/tools/gen_explore_cases.py" >/dev/null

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

# **确定性垫片不复制**:这一套两侧都不装(裁判的 evalJS 在 exploreKinds() 内部,
# 没有前置拼接的入口)—— 摸时钟/随机的书源由生成器整条剔除。
# 网络面(`java.ajax` 的发现规则)与 js 套共用同一份回落页快照。
SNAP="$ROOT/fixtures/http-js-host"
(cd "$ROOT/judge" && ./gradlew -q :jsharness:run --args="$WORK/cases.json $WORK/judge.jsonl $SNAP")

cargo build -q --manifest-path "$ROOT/rust/Cargo.toml" -p difftest --bin js_case_runner "${CARGO_PROFILE_FLAGS[@]}"
"$ROOT/rust/target/$PROFILE_DIR/js_case_runner" "$WORK/cases.json" "$WORK/rust.jsonl" "$SNAP"

EXEMPT_ARGS=(--cases="$WORK/cases.json")
if [[ -f "$CASES_DIR/exemptions.json" ]]; then
  EXEMPT_ARGS+=(--exempt="$CASES_DIR/exemptions.json")
fi
python3 "$ROOT/tools/compare_jsonl.py" "$WORK/judge.jsonl" "$WORK/rust.jsonl" "${EXEMPT_ARGS[@]}" "$@"
