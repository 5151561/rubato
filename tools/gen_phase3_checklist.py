#!/usr/bin/env python3
"""**Phase 3 的分母**:把 1704 源里真正用到 C 层能力的那些落成一份可 review 的清单。

Phase 3 的判据是「**指定清单源逐个验收,不设总 pass 率**」(docs/plan.md §4)。
判据要有分母,分母就得先落库、能 review、能被下一轮重算 —— 这份脚本就是那个
分母的生成器,产物 `fixtures/phase3/checklist.json`,口径写在
`fixtures/phase3/README.md`。

**这份清单不是差分套**:webView / 验证码 / 登录页天然进不了自动差分
(裁判那侧是真 Android WebView,快照录不下来),plan §3 早写明「Phase 3 手工清单」。
所以它给的是**验收单**:每个源要走哪条线、踩到四步里的哪一步、触发写法长什么样。

能力探测在 `tools/phase3_caps.py`(与分层 `classify` 同一份口径,别再抄第二份)。

用法:

    tools/gen_phase3_checklist.py            # 重算清单 + 打表
    tools/gen_phase3_checklist.py --check    # 只核对:清单与语料不一致就非零退出(CI 用)
"""
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import phase3_caps as caps  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = ROOT / "fixtures" / "sources"
OUT = ROOT / "fixtures" / "phase3" / "checklist.json"
TOP100 = ROOT / "fixtures" / "cases" / "top100" / "selection.json"

# 验收线的排序(报表与清单都按它排):按「不做就有多少源跑不动」排,
# 不是按实现难度排 —— 难度是我们的事,分母是语料的事。
LINE_ORDER = ["webview", "browser", "verify", "login", "font", "fs"]


def load_sources():
    out = []
    for f in sorted(SRC_DIR.glob("*.json")):
        data = json.load(open(f, encoding="utf-8"))
        for s in (data if isinstance(data, list) else [data]):
            if isinstance(s, dict):
                out.append((f.name, s))
    return out


def build():
    top100 = set()
    if TOP100.exists():
        top100 = {e["url"] for e in json.load(open(TOP100, encoding="utf-8"))}

    entries = []
    for pack, s in load_sources():
        hits = caps.hits(s)
        if not hits:
            continue
        cs = caps.caps(s)
        entries.append({
            "url": s.get("bookSourceUrl", ""),
            "name": s.get("bookSourceName", ""),
            "pack": pack,
            "tier": caps.classify(s),
            # 验收线:一个源可能同时踩两条(webView 搜索 + 登录页)
            "lines": sorted({caps.LINES[c] for c in cs}, key=LINE_ORDER.index),
            "caps": cs,
            "steps": sorted({h["step"] for h in hits}),
            "hits": hits,
            "in_top100": s.get("bookSourceUrl", "") in top100,
        })

    # 排序:先按验收线(最要紧的在前),再按 url —— 稳定、可 diff
    entries.sort(key=lambda e: (LINE_ORDER.index(e["lines"][0]), e["url"]))

    totals = Counter()
    for pack, s in load_sources():
        totals[caps.classify(s)] += 1
    return {
        "generated_by": "tools/gen_phase3_checklist.py",
        "note": "Phase 3 验收的分母。口径见 fixtures/phase3/README.md;别手改,改探测器再重跑。",
        "corpus": {
            "sources": len(load_sources()),
            "tiers": {k: totals[k] for k in ("A", "B", "C")},
        },
        "lines": {
            line: sorted({e["url"] for e in entries if line in e["lines"]})
            for line in LINE_ORDER
        },
        "sources": entries,
    }


def report(doc):
    entries = doc["sources"]
    print(f"语料 {doc['corpus']['sources']} 源,分层 {doc['corpus']['tiers']}")
    print(f"用到 Phase 3 能力的源:{len(entries)}\n")

    print("验收线            源数   其中在 top100   踩到的步")
    for line in LINE_ORDER:
        es = [e for e in entries if line in e["lines"]]
        if not es:
            print(f"  {line:14} {0:5}   —              —   (语料里一例都没有)")
            continue
        steps = Counter(st for e in es for h in e["hits"]
                        if caps.LINES[h["cap"]] == line for st in [h["step"]])
        top = sum(1 for e in es if e["in_top100"])
        stxt = " ".join(f"{k}:{v}" for k, v in steps.most_common())
        print(f"  {line:14} {len(es):5}   {top:<14} {stxt}")

    print("\n能力                     源数   命中字段位")
    hitcnt = Counter(h["cap"] for e in entries for h in e["hits"])
    srccnt = Counter(c for e in entries for c in e["caps"])
    for c in caps.LINES:
        print(f"  {c:24} {srccnt[c]:5}   {hitcnt[c]}")

    print("\n踩到 webView 的字段位(前 12):")
    fields = Counter(h["field"] for e in entries for h in e["hits"]
                     if h["cap"].startswith("webview"))
    for f, n in fields.most_common(12):
        print(f"  {f:34} {n}")

    多线 = [e for e in entries if len(e["lines"]) > 1]
    print(f"\n同时踩两条线以上的源:{len(多线)}")
    for e in 多线[:10]:
        print(f"  [{e['name']}] {'+'.join(e['lines'])}  {','.join(e['steps'])}")


def main():
    doc = build()
    if "--check" in sys.argv:
        if not OUT.exists():
            print(f"清单不存在:{OUT}", file=sys.stderr)
            return 1
        old = json.load(open(OUT, encoding="utf-8"))
        if old != doc:
            print("清单与语料对不上 —— 跑 tools/gen_phase3_checklist.py 重算", file=sys.stderr)
            return 1
        print("清单与语料一致")
        return 0
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(doc, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    report(doc)
    print(f"\n→ {OUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
