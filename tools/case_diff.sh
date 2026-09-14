#!/bin/zsh
# 通用规则级差分:合并指定用例目录下的全部 *.json,
# 跑 judge/harness(Kotlin 裁判)与 difftest case_runner(Rust),结构化比较。
# 用法: case_diff.sh <fixtures/cases/下的目录名> [--show=N]
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

NAME="$1"; shift
CASES_DIR="$ROOT/fixtures/cases/$NAME"
WORK="${TMPDIR:-/tmp}/rubato-diff-$NAME"
mkdir -p "$WORK"

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

# HTTP 录放快照根:默认 fixtures/http;某套自带 fixtures/http-<套名> 就用它
# (各套生成器会清空自己的根,共用一个根会互相删)
SNAP="$ROOT/fixtures/http"
if [[ -d "$ROOT/fixtures/http-$NAME" ]]; then
  SNAP="$ROOT/fixtures/http-$NAME"
fi

(cd "$ROOT/judge" && ./gradlew -q :harness:run --args="$WORK/cases.json $WORK/judge.jsonl $SNAP")

cargo build -q --manifest-path "$ROOT/rust/Cargo.toml" -p difftest --bin case_runner "${CARGO_PROFILE_FLAGS[@]}"
"$ROOT/rust/target/$PROFILE_DIR/case_runner" "$WORK/cases.json" "$WORK/rust.jsonl" "$SNAP"

# --cases 一律传:它既是 pattern 豁免要的用例内容,也是**分母** ——
# 两侧同时漏掉某个 case 时按 FAIL 记,而不是从分母里静默消失。
EXEMPT_ARGS=(--cases="$WORK/cases.json")
if [[ -f "$CASES_DIR/exemptions.json" ]]; then
  EXEMPT_ARGS+=(--exempt="$CASES_DIR/exemptions.json")
fi
rc=0
python3 "$ROOT/tools/compare_jsonl.py" "$WORK/judge.jsonl" "$WORK/rust.jsonl" \
  "${EXEMPT_ARGS[@]}" "$@" || rc=$?
# 命中面(只有 pipeline 类的套打得出来):「一致」不看命中面就是骗人 ——
# 两侧一起空手而归也叫一致。见 tools/hit_report.py 的模块注释。
# **打在最后**:all_diff.sh 每套只留 `tail -40`,打在前面会被 FAIL 明细挤掉。
python3 "$ROOT/tools/hit_report.py" "$WORK/judge.jsonl" --cases="$WORK/cases.json"
exit $rc
