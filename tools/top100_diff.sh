#!/bin/zsh
# top100 差分:**自选的 100 个常用源 × WebBook 四步,JS 是真引擎**。
#
# plan §4 Phase 2 的最后一条判据:「自选 top-100 常用源 ≥90%」。
# 与 pipeline-corpus / pipeline-corpus-b 的关系:
#   - 同一个 `op: "pipeline"`、同一份回落页机制、同一张投影(生成器都是
#     tools/gen_pipeline_corpus_cases.py,只是 --tier 不同)——**不另起一套执行器**;
#   - 不同的是**选源**与**判据口径**:那两套按「层」全量入册、按 case 算通过率;
#     这一套按「常用」自选 100 源(跨 A/B/C 三层)、**按源**算 ——
#     一个源的四五个 step 全一致才算它过,≥90/100。
#
# 裁判是 :jsharness(真 Rhino):名单里有 40 个 B 层源与 8 个 C 层源,
# 确定性桩跑不了它们。
#
# 契约 fixtures/cases/top100/README.md;回落页 docs/http-snapshot.md §6。
# 用法: top100_diff.sh [--show=N]
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

NAME="top100"
CASES_DIR="$ROOT/fixtures/cases/$NAME"
WORK="${TMPDIR:-/tmp}/rubato-diff-$NAME"
mkdir -p "$WORK"

python3 "$ROOT/tools/gen_pipeline_corpus_cases.py" --tier=top100

python3 - "$WORK/cases.json" "$CASES_DIR"/*.json <<'PY'
import json, sys
merged = []
for p in sys.argv[2:]:
    # exemptions.json 是豁免清单、selection.json 是自选名单 —— 都不是用例
    if p.endswith("exemptions.json") or p.endswith("selection.json"):
        continue
    merged += json.load(open(p, encoding="utf-8"))
ids = [c["id"] for c in merged]
assert len(ids) == len(set(ids)), "case id 冲突"
json.dump(merged, open(sys.argv[1], "w", encoding="utf-8"), ensure_ascii=False)
print(f"{len(merged)} cases", file=sys.stderr)
PY

# **不复制 determinism.js**:同 pipeline-corpus-b —— 确定性垫片进不去四步内部的
# 求值,单边冻比不冻更糟;摸时钟/随机的源由生成器整条剔除(选名单那一步就剔了)。
SNAP="$ROOT/fixtures/http-$NAME"
(cd "$ROOT/judge" && ./gradlew -q :jsharness:run --args="$WORK/cases.json $WORK/judge.jsonl $SNAP")

cargo build -q --manifest-path "$ROOT/rust/Cargo.toml" -p difftest --bin js_case_runner "${CARGO_PROFILE_FLAGS[@]}"
"$ROOT/rust/target/$PROFILE_DIR/js_case_runner" "$WORK/cases.json" "$WORK/rust.jsonl" "$SNAP"

# --cases 一律传:它既是 pattern 豁免要的用例内容,也是**分母**。
EXEMPT_ARGS=(--cases="$WORK/cases.json")
if [[ -f "$CASES_DIR/exemptions.json" ]]; then
  EXEMPT_ARGS+=(--exempt="$CASES_DIR/exemptions.json")
fi
# 三行报表,缺一不可:
#   ① **按 case**(compare_jsonl.py)—— all_diff.sh 与 diff-baseline.json 认的是
#      它,棘轮守「不新增」;
#   ② **按源**(top100_report.py)—— plan 的判据认的是它:一个源的四五个 step
#      全一致才算它过,≥90/100;
#   ③ **命中面**(hit_report.py)—— 通过率不看它就是骗人:两侧一起空手而归
#      也叫一致。
rc=0
python3 "$ROOT/tools/compare_jsonl.py" "$WORK/judge.jsonl" "$WORK/rust.jsonl" \
  "${EXEMPT_ARGS[@]}" "$@" || rc=$?
echo
python3 "$ROOT/tools/top100_report.py" "$WORK/judge.jsonl" "$WORK/rust.jsonl" \
  --selection="$CASES_DIR/selection.json" "${EXEMPT_ARGS[@]}" || rc=$?
# 三行报表的第三行:**命中面** —— 「一致」不看命中面就是骗人(两侧一起空手而归
# 也叫一致)。打在最后:all_diff.sh 每套只留 `tail -40`。
echo
python3 "$ROOT/tools/hit_report.py" "$WORK/judge.jsonl" --cases="$WORK/cases.json"
exit $rc
