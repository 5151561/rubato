#!/usr/bin/env python3
"""从 fixtures/sources 的书源 JSON 抽取真实规则串,生成 rule-syntax 差分用例。

对每条去重后的规则串生成:
- split(sep=&&/||/%%, code=False)          —— AnalyzeByJSoup/XPath 外层切分
- split(sep=@, trim=True)                   —— AnalyzeByJSoup 内层切分
- 含 "$." 的再生成 split(sep=&&/||, code=True) —— AnalyzeByJSonPath
- url 类字段(searchUrl/exploreUrl/loginUrl/bookUrlPattern 及 *Url 规则)
  生成 innerRuleStr({{ }})                  —— AnalyzeUrl
输出 fixtures/cases/rule-syntax/corpus.json
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = ROOT / "fixtures" / "sources"
OUT = ROOT / "fixtures" / "cases" / "rule-syntax" / "corpus.json"

RULE_BLOCKS = ["ruleSearch", "ruleExplore", "ruleBookInfo", "ruleToc", "ruleContent"]
URL_FIELDS = ["searchUrl", "exploreUrl", "loginUrl", "bookUrlPattern"]


def collect(sources):
    rules = []  # (value, is_url)
    for s in sources:
        for f in URL_FIELDS:
            v = s.get(f)
            if isinstance(v, str) and v:
                rules.append((v, True))
        for block in RULE_BLOCKS:
            b = s.get(block)
            if not isinstance(b, dict):
                continue
            for k, v in b.items():
                if isinstance(v, str) and v:
                    rules.append((v, k.lower().endswith("url")))
    return rules


def main():
    sources = []
    for p in sorted(SRC_DIR.glob("*.json")):
        d = json.loads(p.read_text(encoding="utf-8"))
        sources.extend(d if isinstance(d, list) else [d])

    cases, seen = [], set()
    n = 0

    def add(case):
        nonlocal n
        key = json.dumps(
            {k: v for k, v in case.items() if k != "id"},
            ensure_ascii=False, sort_keys=True,
        )
        if key in seen:
            return
        seen.add(key)
        n += 1
        case["id"] = f"c{n:04d}"
        cases.append(case)

    for value, is_url in collect(sources):
        add({"op": "split", "data": value, "code": False, "trim": False,
             "sep": ["&&", "||", "%%"]})
        add({"op": "split", "data": value, "code": False, "trim": True,
             "sep": ["@"]})
        if "$." in value:
            add({"op": "split", "data": value, "code": True, "trim": False,
                 "sep": ["&&", "||"]})
        if is_url:
            add({"op": "innerRuleStr", "data": value,
                 "startStr": "{{", "endStr": "}}"})

    OUT.write_text(
        json.dumps(cases, ensure_ascii=False, indent=1) + "\n", encoding="utf-8"
    )
    print(f"{len(cases)} cases -> {OUT}", file=sys.stderr)


if __name__ == "__main__":
    main()
