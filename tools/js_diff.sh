#!/bin/zsh
# js-host 差分:同一段书源 JS,裁判侧跑真 Rhino(judge/jsharness:AnalyzeRule +
# JsExtensions 真身),被测侧跑 rquickjs(difftest 的 js_case_runner),结构化比较。
#
# 与 case_diff.sh 的区别:裁判进程是 :jsharness(真 Rhino)而不是 :harness
# (确定性 JS 桩)—— 两者的 com.script 互斥,故分模块。
# 用法: js_diff.sh [--show=N]
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

NAME="js-host"
CASES_DIR="$ROOT/fixtures/cases/$NAME"
WORK="${TMPDIR:-/tmp}/rubato-diff-$NAME"
mkdir -p "$WORK"

# 用例三份(hand/corpus/url)全是生成物 —— 每次跑之前重建。
# 本脚本此前是十五个差分脚本里**唯一不调生成器**的那个,于是 hand.json 与
# 生成器悄悄漂了 9 条装箱探针(库里多、生成器少),谁顺手跑一次生成器就会
# 静默丢掉它们、判据从 1884 掉到 1875 还不报错。M2l 把探针同步回生成器、
# 补上这一句,漂移这一类就此关掉。
python3 "$ROOT/tools/gen_js_cases.py" >/dev/null

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

# 两侧共享的确定性垫片(冻时钟/Math.random)。复制到工作目录,两个执行器都从
# cases.json 的同级目录读它 —— 保证逐字同一份,契约见 fixtures/cases/js-host/README.md
cp "$CASES_DIR/determinism.js" "$WORK/determinism.js"

# 第三个参数是 HTTP 录放的快照根(契约 docs/http-snapshot.md)。js-host 套的
# 网络面靠回落页顶(_fallback/{json,html}),用例用 `fallback` 字段挑。
SNAP="$ROOT/fixtures/http-js-host"
(cd "$ROOT/judge" && ./gradlew -q :jsharness:run --args="$WORK/cases.json $WORK/judge.jsonl $SNAP")

cargo build -q --manifest-path "$ROOT/rust/Cargo.toml" -p difftest --bin js_case_runner "${CARGO_PROFILE_FLAGS[@]}"
"$ROOT/rust/target/$PROFILE_DIR/js_case_runner" "$WORK/cases.json" "$WORK/rust.jsonl" "$SNAP"

# --cases 一律传:它既是 pattern 豁免要的用例内容,也是**分母** ——
# 两侧同时漏掉某个 case 时按 FAIL 记,而不是从分母里静默消失。
EXEMPT_ARGS=(--cases="$WORK/cases.json")
if [[ -f "$CASES_DIR/exemptions.json" ]]; then
  EXEMPT_ARGS+=(--exempt="$CASES_DIR/exemptions.json")
fi
python3 "$ROOT/tools/compare_jsonl.py" "$WORK/judge.jsonl" "$WORK/rust.jsonl" "${EXEMPT_ARGS[@]}" "$@"
