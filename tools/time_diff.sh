#!/bin/zsh
# 逐例计时:把差分那十八套再跑一遍,量「同一条 case 两侧各花多久」。
#
# 为什么复用差分而不另建 benchmark:两侧已经在同一份 fixture、同一批 HTTP 快照
# 上跑同样的用例,分母是现成的、而且是**被判过一致**的 —— 自己另写一份 benchmark
# 只会量到两段不同的代码。
#
# 三件必须做对的事(做错了数就是假的):
#   ① Rust 走 **release**(RUBATO_DIFF_PROFILE=release):debug 在正则/解析上
#      能慢一个量级;
#   ② JVM 要**预热**(RUBATO_DIFF_ROUNDS,缺省 3,逐例取最小):第一轮跑在
#      解释器里;
#   ③ FAIL 数**不能变**:计时模式多跑几轮,若某套的 case 之间有状态残留
#      (cookie / 已定义的 doc),最后一轮的输出就会和单轮不同 —— 本脚本逐套
#      对 fixtures/diff-baseline.json 核一遍,变了就当场报出来。
#
# 用法: time_diff.sh [--only=a,b,c] [--rounds=N] [--json=out.json]
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# 套名 → 工作目录名(差分脚本传给 case_diff.sh 的那个 fixtures/cases 子目录名,
# 两者不同名的有六个 —— 别凭套名去猜目录)
typeset -A WORKDIR
WORKDIR=(
  syntax            rule-syntax
  regex             regex-compat
  json              json-compat
  html              html-compat
  xpath             xpath
  url               java-url
  charset           charset
  analyze_url       analyze-url
  fetch             fetch
  source            source
  rule              rule-engine
  js                js-host
  pipeline          pipeline
  pipeline_corpus   pipeline-corpus
  pipeline_corpus_b pipeline-corpus-b
  top100            top100
  webview           webview
  explore           explore
)
# 顺序与 all_diff.sh 一致:先离线小套,再流水线与语料大套
SUITES=(syntax regex json html xpath url charset analyze_url fetch source rule js
        pipeline pipeline_corpus pipeline_corpus_b top100 webview explore)

ROUNDS=3
JSON_ARG=()
ONLY=""
for a in "$@"; do
  case "$a" in
    --only=*)   ONLY="${a#--only=}" ;;
    --rounds=*) ROUNDS="${a#--rounds=}" ;;
    --json=*)   JSON_ARG=("$a") ;;
    *) echo "不认识的参数: $a" >&2; exit 2 ;;
  esac
done
[[ -n "$ONLY" ]] && SUITES=("${(@s/,/)ONLY}")

BASELINE="$ROOT/fixtures/diff-baseline.json"
export RUBATO_DIFF_TIME=1
export RUBATO_DIFF_ROUNDS="$ROUNDS"
export RUBATO_DIFF_PROFILE=release

typeset -a TRIPLES DRIFTED
for s in $SUITES; do
  dir="${WORKDIR[$s]:-}"
  script="$ROOT/tools/${s}_diff.sh"
  if [[ -z "$dir" || ! -x "$script" ]]; then
    echo "!! 跳过 $s(没有映射或没有脚本)" >&2
    continue
  fi
  echo "──────── $s(${ROUNDS} 轮,release)────────"
  out="$("$script" 2>&1)" || true
  line="$(printf '%s\n' "$out" | grep -m1 '== diff' || true)"
  if [[ -z "$line" ]]; then
    echo "!! $s 没跑出比较行,计时数据不可信,跳过" >&2
    printf '%s\n' "$out" | tail -15
    DRIFTED+=("$s(没跑出比较行)")
    continue
  fi
  echo "$line"
  # 判据面核对:计时模式不许改 FAIL 数
  n_fail="$(printf '%s' "$line" | sed -n 's/.*PASS \/ \([0-9][0-9]*\) FAIL.*/\1/p')"
  allow="$(python3 -c "import json,sys; print(json.load(open(sys.argv[1])).get(sys.argv[2], 0))" "$BASELINE" "$s")"
  if [[ "${n_fail:-0}" != "$allow" ]]; then
    echo "!! $s 的 FAIL ${n_fail} ≠ 基线 ${allow} —— 计时模式动了判据面,这一套的数不能用" >&2
    DRIFTED+=("$s($n_fail≠$allow)")
    continue
  fi
  W="${TMPDIR:-/tmp}/rubato-diff-$dir"
  TRIPLES+=("$s" "$W/judge.jsonl" "$W/rust.jsonl")
done

echo
echo "════════ 逐例计时 ════════"
if (( ${#TRIPLES} )); then
  python3 "$ROOT/tools/time_report.py" "${TRIPLES[@]}" ${JSON_ARG[@]:-}
else
  echo "没有可报的套"
fi

if (( ${#DRIFTED} )); then
  echo
  echo "!! 这些套没进报表:${DRIFTED[*]}" >&2
  exit 1
fi
