#!/bin/zsh
# **生成物不入库**的判据。CI 与本地都跑这一条:
#
#     把 tools/gen_*.py 全跑一遍,工作区必须**没有任何变化** ——
#     既不能改到入库文件,也不能冒出未跟踪文件。
#
# 它一次守住两件事:
#  1. **入库的生成物**(判据会因此悄悄漂):M2l 那次 `fixtures/cases/js-host/hand.json`
#     比生成器多 9 条探针,谁顺手跑一次生成器,判据 1884 → 1875 一声不吭;
#  2. **新加的生成物被顺手 git add 了**:此前「先入库好 review」和「这是生成物删掉」
#     来回拉锯过四次(d086af6 / dc2f4b3 / e6ee878 / ebd8f17),同一批文件加进来又
#     删掉。规则只活在人的脑子里就一定会重演,所以让 CI 判。
#
# 留在库里的只有**输入**:fixtures/sources、fixtures/corpus-js、fixtures/pages、
# fixtures/spike-js、fixtures/http-js-host、fixtures/diff-baseline.json、
# 各套的 README.md,以及手写的用例/豁免清单。判别方法就是这条判据本身。
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# 跑之前先记一份工作区状态,跑完对比**增量** —— 这样本地在有改动的时候
# 也能跑(CI 上那份本来就是干净的,增量即全量)。
before="$(git status --porcelain)"

# 生成器的汇总行都打在 stderr,平时不用看:出错了再连日志一起吐出来
LOG="$(mktemp)"
trap 'rm -f "$LOG"' EXIT
run() { if ! "$@" >>"$LOG" 2>&1; then echo "!! 生成器失败:$*" >&2; cat "$LOG" >&2; exit 1; fi }

for g in tools/gen_*.py; do
  run python3 "$g"
done
# 分层的两套 + 自选 top-100:同一个生成器,--tier 不同
run python3 tools/gen_pipeline_corpus_cases.py --tier=B
run python3 tools/gen_pipeline_corpus_cases.py --tier=top100

after="$(git status --porcelain)"
dirty="$(comm -13 <(printf '%s\n' "$before" | sort) <(printf '%s\n' "$after" | sort))"
if [[ -n "${dirty//[[:space:]]/}" ]]; then
  echo "!! 跑完生成器之后工作区变脏了 —— 要么入库的生成物漂了,要么新生成物没进 .gitignore:" >&2
  echo "$dirty" >&2
  echo >&2
  echo "   改到的入库文件:把它从 git 里拿掉(git rm --cached)并加进 .gitignore;" >&2
  echo "   冒出来的未跟踪文件:同上。真·输入才留在库里。" >&2
  exit 1
fi
echo "生成物不入库:✅(跑完全部生成器,工作区仍然干净)"
