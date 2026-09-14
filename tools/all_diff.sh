#!/bin/zsh
# 全部差分套跑一遍,逐套报 PASS/FAIL,任何一套不绿就整体退 1。
#
# 为什么要有这个:plan §4 的 Phase 2 判据白纸黑字写着「差分 CI 常绿」,而这
# 十五套此前全靠手跑 —— 改 A 套时 B 套回归了没人知道。CI(.github/workflows/ci.yml)
# 调的就是本脚本。
#
# 用法: all_diff.sh [--only=a,b,c] [--show=N]
#   --only 只跑列出的套(名字见下面的 SUITES)
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# 顺序:先纯离线的小套(挂了先看它们),再流水线与语料大套
SUITES=(
  syntax          # rule-syntax:RuleAnalyzer
  regex           # regex-compat:java.util.regex 方言
  json            # json-compat:jayway JSONPath + AnalyzeByJSonPath
  html            # html-compat:jsoup 选择器/序列化 + AnalyzeByJSoup
  xpath           # xpath-compat:JsoupXpath 2.5.5 + AnalyzeByXPath
  url             # rubato-core:java.net.URL / getAbsoluteURL
  charset         # net:字符集嗅探
  analyze_url     # net:AnalyzeUrl 离线面(option JSON / 编码 / 页码)
  fetch           # net:HTTP 录放全链路 + CookieStore
  source          # rubato-core:BookSource 实体与规则位
  rule            # rule-engine:AnalyzeRule 全链路
  js              # js-host:真 Rhino vs rquickjs
  pipeline        # pipeline:WebBook 四步
  pipeline_corpus # pipeline-corpus:A 层真实书源 × 四步(JS 是确定性桩)
  # B 层那套放最后:它最慢(2783 例两侧都是真 JS 引擎)
  pipeline_corpus_b # pipeline-corpus-b:B 层真实书源 × 四步(真 Rhino vs 真 QuickJS)
  top100          # top100:自选 100 个常用源 × 四步(判据**按源**算,见该套 README)
  webview         # webview:BackstageWebView 的**策略层**(裁判是第三个 harness :wvharness)
  explore         # explore:发现页分类列表(exploreKinds();裁判 :jsharness,挂真身那份文件)
)

ONLY=""
ARGS=()
for a in "$@"; do
  case "$a" in
    --only=*) ONLY="${a#--only=}" ;;
    *) ARGS+=("$a") ;;
  esac
done

if [[ -n "$ONLY" ]]; then
  SUITES=("${(@s/,/)ONLY}")
fi

# 每套允许的 FAIL 上限。全 0 之前也要守住「不新增」—— 判据见文件里的说明。
BASELINE="$ROOT/fixtures/diff-baseline.json"

typeset -a FAILED
typeset -a LOOSE
typeset -a SUMMARY
# 合计。**必须由脚本打出来**:此前只有各套自己那一行,总数得手加 ——
# docs/plan.md 首页的「92119 例」就是这么飘掉的(实测 96123)。
typeset -i TOT_CASES=0 TOT_PASS=0 TOT_FAIL=0 TOT_EXEMPT=0 PARSED=0
for s in $SUITES; do
  script="$ROOT/tools/${s}_diff.sh"
  if [[ ! -x "$script" ]]; then
    echo "!! 没有 $script" >&2
    FAILED+=("$s(缺脚本)")
    continue
  fi
  echo "──────── $s ────────"
  out="$("$script" ${ARGS[@]:-} 2>&1)" || true
  line="$(printf '%s\n' "$out" | grep -m1 '== diff' || true)"
  printf '%s\n' "$out" | tail -40

  if [[ -z "$line" ]]; then
    SUMMARY+=("$(printf '%-16s %s' "$s" '(没有比较行 —— 执行器自己挂了)')")
    FAILED+=("$s(没跑出比较行)")
    continue
  fi
  # 「== diff: 1854 PASS / 30 FAIL (共 1884) ==」/「…(共 1732,豁免 6) ==」
  n_fail="$(printf '%s' "$line" | sed -n 's/.*PASS \/ \([0-9][0-9]*\) FAIL.*/\1/p')"
  n_pass="$(printf '%s' "$line" | sed -n 's/.*diff: \([0-9][0-9]*\) PASS.*/\1/p')"
  n_cases="$(printf '%s' "$line" | sed -n 's/.*(共 \([0-9][0-9]*\).*/\1/p')"
  n_exempt="$(printf '%s' "$line" | sed -n 's/.*豁免 \([0-9][0-9]*\).*/\1/p')"
  TOT_PASS+=${n_pass:-0}
  TOT_FAIL+=${n_fail:-0}
  TOT_CASES+=${n_cases:-0}
  TOT_EXEMPT+=${n_exempt:-0}
  PARSED+=1
  allow="$(python3 -c "import json,sys; print(json.load(open(sys.argv[1])).get(sys.argv[2], 0))" "$BASELINE" "$s")"
  mark=""
  if (( n_fail > allow )); then
    mark="  ← 超基线 $allow"
    FAILED+=("$s($n_fail > $allow)")
  elif (( n_fail < allow )); then
    mark="  ← 好于基线 $allow,把 fixtures/diff-baseline.json 调下来"
    LOOSE+=("$s($n_fail < $allow)")
  fi
  SUMMARY+=("$(printf '%-16s %s%s' "$s" "$line" "$mark")")
done

echo
echo "════════ 汇总 ════════"
for l in $SUMMARY; do echo "$l"; done

# 合计。引用到 docs/plan.md 的就是这一行 —— 别再手加。
if (( PARSED )); then
  note=""
  (( PARSED < ${#SUITES} )) && note="  ← 有 $(( ${#SUITES} - PARSED )) 套没跑出比较行,合计不全"
  [[ -n "$ONLY" ]] && note="$note  ← --only 子集,不是全量"
  printf '%-16s 合计 %d 套 %d 例:%d PASS / %d FAIL(豁免 %d)%s\n' \
    '' "$PARSED" "$TOT_CASES" "$TOT_PASS" "$TOT_FAIL" "$TOT_EXEMPT" "$note"
  # 自校验:各套的 PASS + FAIL + 豁免 应当正好是「共」。对不上说明这里的
  # 解析跟 compare_jsonl.py 的输出格式脱节了 —— 那正是合计会骗人的方式。
  if (( TOT_PASS + TOT_FAIL + TOT_EXEMPT != TOT_CASES )); then
    echo "!! 合计对不上:$TOT_PASS + $TOT_FAIL + $TOT_EXEMPT != $TOT_CASES(解析与 compare_jsonl.py 的格式脱节了)" >&2
    FAILED+=("合计自校验")
  fi
fi

if (( ${#FAILED} )); then
  echo
  echo "超基线的套:${FAILED[*]}"
  exit 1
fi
if (( ${#LOOSE} )); then
  echo
  echo "基线该往下调:${LOOSE[*]}"
fi
echo "没有新增失败"
