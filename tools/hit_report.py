#!/usr/bin/env python3
"""**命中面**报表:三套 pipeline 差分里,裁判侧真的装配出东西的比例。

为什么要有这份东西(memory/plan 里记了两遍的那一课):

    「一致」不看命中面就是骗人 —— 两侧一起空手而归也叫一致。

回落页与真实站点的规则不成对,大量 case 落在「空结果 / toc_empty /
content_empty」分支上。那一档的一致性也在比,但它**不覆盖字段装配**。
所以每套的 README 都要写死命中面,报通过率时连着一起报。此前这张表是手算的,
现在由这份脚本打 —— 手算的数字会和代码悄悄漂。

命中的定义(只看**裁判侧**:它是行为规范,被测侧命中与否是被比的结果):

    search / explore → books 非空
    toc             → chapters 非空
    content         → content 非空
    info            → **book.name 非空**

`info` 为什么只看 name:`BookInfo.kt L158` 有 `if (book.tocUrl.isEmpty())
book.tocUrl = baseUrl` —— tocUrl **永远非空**,拿它当命中信号会把一整套读成
96%(实测 tocUrl 96/100 而 name 79/100)。name 是每个源都会配的字段,
它非空才说明那一步真的从页面上取到了东西。

用法: hit_report.py <judge.jsonl> --cases=cases.json
"""
import json
import sys
from collections import Counter

# 这一步的「入口规则」:源没配它,这个 case **不可能命中** —— 裁判也拿不到东西。
# 把这一档单列出来,不然命中面的分母里混着一堆注定为 0 的 case,
# 数字读起来像缺口,其实是「这个源没有探索页 / 详情页不配 name」。
# 实测(B 层 2783):725 个未命中里 **188 个**是这一档,
# 扣掉之后 explore 67%→83%、info 68%→83%,真正的缺口集中在 toc 与 content。
ENTRY_RULE = {
    "search": ("ruleSearch", "bookList"),
    "explore": ("ruleExplore", "bookList"),
    "toc": ("ruleToc", "chapterList"),
    "info": ("ruleBookInfo", "name"),       # 命中信号就是 name,故看它自己
    "content": ("ruleContent", "content"),
}


def unconfigured(step, case):
    """这个 case 的源**没配**这一步的入口规则 → 注定命中不了"""
    spec = ENTRY_RULE.get(step)
    src = case.get("source")
    if not spec or src is None:
        return False
    if isinstance(src, str):
        try:
            src = json.loads(src)
        except ValueError:
            return False
    if not isinstance(src, dict):
        return False
    rules = src.get(spec[0])
    if isinstance(rules, str):
        try:
            rules = json.loads(rules)
        except ValueError:
            rules = None
    if not isinstance(rules, dict):
        return True
    v = rules.get(spec[1])
    return not (isinstance(v, str) and v.strip())


def hit(step, out):
    if not isinstance(out, dict):
        return False
    if step in ("search", "explore"):
        return bool(out.get("books"))
    if step == "toc":
        return bool(out.get("chapters"))
    if step == "content":
        return bool(out.get("content"))
    if step == "info":
        # tocUrl 不算:它取空时被 BookInfo 兜底成 baseUrl,永远非空
        return bool((out.get("book") or {}).get("name"))
    return False


def main():
    cases_path = None
    args = []
    for a in sys.argv[1:]:
        if a.startswith("--cases="):
            cases_path = a.split("=", 1)[1]
        elif not a.startswith("--"):
            args.append(a)
    assert cases_path and args, "用法: hit_report.py <judge.jsonl> --cases=cases.json"

    cases = {c["id"]: c for c in json.load(open(cases_path, encoding="utf-8"))}
    judge = {}
    with open(args[0], encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                o = json.loads(line)
                judge[o["id"]] = o

    order = ["search", "explore", "info", "toc", "content"]
    tot, ok, nocfg = Counter(), Counter(), Counter()
    ftot, fok = Counter(), Counter()
    for cid, c in cases.items():
        step = c.get("step")
        if step not in order:
            continue
        # 回落页口味:html 一张,json 按源一张(`json-<hash>`)。
        # 手工套(fixtures/cases/pipeline)不走回落页 —— 那一档不分列。
        fb = c.get("fallback")
        fl = None if not fb else ("json" if str(fb).startswith("json") else "html")
        out = judge.get(cid) or {}
        h = hit(step, out)
        tot[step] += 1
        if fl:
            ftot[(step, fl)] += 1
        if h:
            ok[step] += 1
            if fl:
                fok[(step, fl)] += 1
        elif unconfigured(step, c):
            nocfg[step] += 1

    if not sum(tot.values()):
        return          # 这一套没有 pipeline 用例:什么都不用报

    def pct(a, b):
        return f"{a}/{b} ({a * 100 // b if b else 0}%)"

    split = bool(ftot)          # 有回落页的套才分「html 页 / json 页」两列
    print("== 命中面(裁判侧真的装配出东西的比例)")
    head = f"   {'步':<9} {'共':>5} {'命中':>14} {'源没配':>10} {'配了没命中':>16}"
    print(head + (f" {'html 页':>16} {'json 页':>16}" if split else ""))
    for st in order:
        if not tot[st]:
            continue
        line = (f"   {st:<10} {tot[st]:>4} {pct(ok[st], tot[st] - nocfg[st]):>16}"
                f" {nocfg[st]:>10} {tot[st] - ok[st] - nocfg[st]:>14}")
        if split:
            line += (f" {pct(fok[(st, 'html')], ftot[(st, 'html')]):>16}"
                     f" {pct(fok[(st, 'json')], ftot[(st, 'json')]):>16}")
        print(line)
    n, k, u = sum(tot.values()), sum(ok.values()), sum(nocfg.values())
    line = (f"   {'合计':<9} {n:>4} {pct(k, n - u):>16}"
            f" {u:>10} {n - k - u:>14}")
    if split:
        def s_(d, f):
            return sum(v for kk, v in d.items() if kk[1] == f)
        line += (f" {pct(s_(fok, 'html'), s_(ftot, 'html')):>16}"
                 f" {pct(s_(fok, 'json'), s_(ftot, 'json')):>16}")
    print(line)
    print("   「命中」的分母已扣掉「源没配」——那一档裁判也拿不到东西,"
          "留在分母里会把「这个源没有探索页」读成缺口。")
    print("   html/json 两列是**未扣**的原始数(按回落页口味分),口径见本脚本头注释。")


if __name__ == "__main__":
    main()
