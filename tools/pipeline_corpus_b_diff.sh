#!/bin/zsh
# pipeline-corpus-b 差分:**B 层真实书源 × WebBook 四步,JS 是真引擎**。
#
# 与 pipeline_corpus_diff.sh(A 层)的唯一区别是裁判进程:那边是 :harness
# (com.script 是确定性桩),这边是 :jsharness(真 Rhino)—— 两者的 com.script
# 互斥,故分模块,也因此分两套。被测侧同一个 case_runner 走不了(它注的是
# StubHost),用 js_case_runner 的 `op: "pipeline"`(Js::Real)。
#
# 契约 fixtures/cases/pipeline-corpus-b/README.md;回落页 docs/http-snapshot.md §6。
# 用法: pipeline_corpus_b_diff.sh [--show=N]
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

NAME="pipeline-corpus-b"
CASES_DIR="$ROOT/fixtures/cases/$NAME"
WORK="${TMPDIR:-/tmp}/rubato-diff-$NAME"
mkdir -p "$WORK"

python3 "$ROOT/tools/gen_pipeline_corpus_cases.py" --tier=B

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

# **不复制 determinism.js**:确定性垫片进不去四步内部的求值(裁判侧没有
# 「先在作用域里跑一段」的入口),单边冻比不冻更糟 —— 两侧都用真时钟,
# 摸时钟/随机的书源由生成器整条剔除。见 README「为什么不冻时钟」。
SNAP="$ROOT/fixtures/http-$NAME"
(cd "$ROOT/judge" && ./gradlew -q :jsharness:run --args="$WORK/cases.json $WORK/judge.jsonl $SNAP")

cargo build -q --manifest-path "$ROOT/rust/Cargo.toml" -p difftest --bin js_case_runner "${CARGO_PROFILE_FLAGS[@]}"
"$ROOT/rust/target/$PROFILE_DIR/js_case_runner" "$WORK/cases.json" "$WORK/rust.jsonl" "$SNAP"

# --cases 一律传:它既是 pattern 豁免要的用例内容,也是**分母**。
EXEMPT_ARGS=(--cases="$WORK/cases.json")
if [[ -f "$CASES_DIR/exemptions.json" ]]; then
  EXEMPT_ARGS+=(--exempt="$CASES_DIR/exemptions.json")
fi
rc=0
python3 "$ROOT/tools/compare_jsonl.py" "$WORK/judge.jsonl" "$WORK/rust.jsonl" \
  "${EXEMPT_ARGS[@]}" "$@" || rc=$?
# 命中面:「一致」不看命中面就是骗人(见 tools/hit_report.py)。
# **打在最后**:all_diff.sh 每套只留 `tail -40`,打在前面会被 FAIL 明细挤掉。
echo
python3 "$ROOT/tools/hit_report.py" "$WORK/judge.jsonl" --cases="$WORK/cases.json"
exit $rc
