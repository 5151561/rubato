#!/usr/bin/env python3
"""**未命中**报表:命中面里那些没命中的 case,按**裁判打出来的真因**分堆。

`hit_report.py` 回答「有多少 case 裁判侧真的装配出了东西」;这一份回答下一个问题:
**剩下那些为什么没装配出来**。两份分开是因为口径不同 —— 那边只看结果空不空,
这边要把 `pipeline_error` 拆开。

为什么非要跑一遍裁判才算数(记过两次的一课):

    裁判把一切异常归一成 `pipeline_error`,**一个标签盖住几十种因**。
    按规则的**长相**猜真因会翻车 —— 2026-08-31 就翻过一次:数出「33 例页已按源
    造了却仍选不中」,断定是生成器造错了;真跑一遍才看清它们**根本没走到选择器
    那一步**,是后面的 `@js:` 当场抛了。

所以本脚本干的是:挑出未命中的 case → 单独喂给 `JSHARNESS_DEBUG=1` 的裁判
(它会把每条的真因打到 stderr)→ 按真因分堆打表。

用法(先跑过一次该套的 `*_diff.sh`,它会把 cases.json / judge.jsonl 留在工作目录):

    tools/miss_report.py pipeline-corpus-b
    tools/miss_report.py pipeline-corpus-b --step=toc
"""
import json
import os
import re
import subprocess
import sys
from collections import Counter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tools"))
from hit_report import unconfigured  # noqa: E402  (同一份「源没配」的口径,不另抄)

# 裁判 stderr 那一行 → 真因的堆名。**从粗到细排,先匹配到的算数**。
# 每一堆后面那句是「能不能修」——写在这里而不是写在报表外面,是因为分堆本身就是
# 为了回答这个问题;不写清楚下一轮又要重新判一遍。
BUCKETS = [
    (("无法读取 null", "无法调用 null", "无法读取 undefined", "无法调用 undefined"),
     "JS 在合成页上跑不通(match 返回 null 之类)",
     "**三条取值路已接**(2026-08-31 第六轮:页面 / 字段规则取出来的值 / 请求地址);"
     "剩下的是 JSON 页那张(诱饵与形态只造在 HTML 页上)、java.ajax 取回的另一张页、DOM 导航"),
    (("Unexpected token: <", "Unexpected token in object literal"),
     "源把页面内容当 JS/JSON 求值",
     "能修:这一步该发 JS/JSON 文本页(gen_pipeline_corpus_cases._js_parses_page)"),
    (("wrong 4-byte ending unit", "valid bits", "Illegal base64 character"),
     "源要 base64 解码,页面是明文",
     "**多半修不动**:实测这一档的 base64 后面接着 AES/3DES,密钥在书源 JS 里"),
    (("找不到函数",),
     "JSON 页的形状对不上源的期待",
     "**多半修不动**:实测 28/29 是 `JSON.parse(result).data.replace(…)` 之后拿去解密"),
    (("multiple of 16", "Wrong algorithm", "CryptoException"),
     "源要 AES 解密,页面是明文",
     "**修不动**:要拿书源自己的密钥把 payload 加密回去"),
    (("语法错误",),
     "书源的 JS 自己在 Rhino 上就语法错误",
     "**不能修**:真身也跑不动这个源"),
    (("正则表达式文字没有限制",),
     "Rhino 正则字面量方言",
     "不用修:已知豁免族(被测侧是超集)"),
    (("url不能为空", "链接为空"),
     "源没配 url",
     "不用修"),
    (("PathNotFound",),
     "JSONPath 没找到",
     "能修:JSON 回落页的形状"),
]

HIT_FIELD = {"search": "books", "explore": "books", "toc": "chapters", "content": "content"}


def hit(step, out):
    if step == "info":                      # 命中信号是 name,理由见 hit_report.py
        return bool((out.get("book") or {}).get("name"))
    return bool(out.get(HIT_FIELD.get(step)))


def bucket(msg):
    for keys, name, fix in BUCKETS:
        if any(k in msg for k in keys):
            return name, fix
    return "其它:" + msg[:60], ""


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    only_step = next((a[len("--step="):] for a in sys.argv[1:] if a.startswith("--step=")), None)
    if not args:
        print(__doc__)
        sys.exit(2)
    name = args[0]
    work = os.path.join(os.environ.get("TMPDIR", "/tmp"), f"rubato-diff-{name}")
    cases_p, judge_p = os.path.join(work, "cases.json"), os.path.join(work, "judge.jsonl")
    if not (os.path.exists(cases_p) and os.path.exists(judge_p)):
        sys.exit(f"没有 {cases_p} / {judge_p} —— 先跑一遍 tools/{name.replace('-', '_')}_diff.sh")

    cases = {c["id"]: c for c in json.load(open(cases_p, encoding="utf-8"))}
    judge = {}
    for line in open(judge_p, encoding="utf-8"):
        o = json.loads(line)
        judge[o["id"]] = o

    miss = []
    for cid, c in cases.items():
        step = c.get("step")
        if step is None or (only_step and step != only_step):
            continue
        o = judge.get(cid, {})
        if hit(step, o) or unconfigured(step, c):   # 命中的、以及「源没配这一步」的不算
            continue
        miss.append(c)
    if not miss:
        print("没有未命中的 case")
        return

    # 只有报了错的那些才需要问裁判「错在哪」;没报错就是「选择器/路径没选中」
    errs = [c for c in miss if judge[c["id"]].get("error") == "pipeline_error"]
    causes = {}
    if errs:
        sub = os.path.join(work, "miss_cases.json")
        json.dump(errs, open(sub, "w", encoding="utf-8"), ensure_ascii=False)
        snap = os.path.join(ROOT, f"fixtures/http-{name}")
        print(f"把 {len(errs)} 条报错的 case 喂给裁判问真因(JSHARNESS_DEBUG=1)…", file=sys.stderr)
        p = subprocess.run(
            ["./gradlew", "-q", ":jsharness:run",
             f"--args={sub} {os.path.join(work, 'miss_judge.jsonl')} {snap}"],
            cwd=os.path.join(ROOT, "judge"),
            env={**os.environ, "JSHARNESS_DEBUG": "1"},
            capture_output=True, text=True,
        )
        for line in p.stderr.splitlines():
            m = re.match(r"\[pipeline\] (\w+): (.*)", line)
            if m:
                causes[m.group(1)] = m.group(2)
        # 真因逐条落盘:下一步总要挑某一堆出来看它们长什么样,
        # 而问一次裁判要跑一遍 gradle。
        json.dump(causes, open(os.path.join(work, "miss_causes.json"), "w", encoding="utf-8"),
                  ensure_ascii=False, indent=1)

    tally, fixes = Counter(), {}
    for c in miss:
        if judge[c["id"]].get("error") == "pipeline_error":
            k, fix = bucket(causes.get(c["id"], "(裁判没打出真因)"))
        else:
            k, fix = "没报错、就是空(选择器/JSONPath 没选中)", "能修:生成器的形态缺口"
        tally[k] += 1
        fixes[k] = fix

    head = f"{name} 的未命中" + (f"(只看 {only_step} 步)" if only_step else "")
    print(f"\n== {head}:{len(miss)} 例,按裁判打出来的真因分 ==")
    for k, v in tally.most_common():
        print(f"  {v:5d}  {k}\n         → {fixes[k]}")
    print("\n(分母已扣掉「源没配这一步的入口规则」那一档,与 hit_report.py 同一份口径)")


if __name__ == "__main__":
    main()
