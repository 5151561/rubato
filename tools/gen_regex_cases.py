#!/usr/bin/env python3
"""从 fixtures/sources 抽取 ##pattern##replacement(###) 三元组,生成 regex-compat 差分用例。

三元组切分与裁判一致:s.split("##") → [1]=pattern, [2]=replacement, 段数>3 → replaceFirst。
每个三元组对若干典型输入文本生成 replaceRegex 用例;每个唯一 pattern 另生成
regexFind 用例(直接比对匹配与分组语义)。
输出 fixtures/cases/regex-compat/corpus.json(生成物,不入库)。
"""
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = ROOT / "fixtures" / "sources"
OUT = ROOT / "fixtures" / "cases" / "regex-compat" / "corpus.json"

# 典型输入文本:覆盖中文正文、广告行、\r\n、全角空格/NBSP、日期数字、
# 英文混排、HTML 片段、URL、emoji、行终止符边界
TEXTS = [
    # 0: 章节正文样本(始终参与)
    "第一章 风起\n　　夜色如墨,街上行人稀少。\n您可以在百度里搜索本书。\n"
    "李明说:“今天是2024年3月15日。”\n一秒记住,精彩小说无弹窗免费阅读!\n"
    "(本章完)\n",
    # 1: \r\n 与行尾
    "Chapter 1: Test\r\nHello world\r\n第2章 再见\r\n",
    # 2: 空白变体
    "a　b c d\te\x0bf\ng",
    # 3: 数字/字母混排
    "abc123def 456 xyz789 2024-03-15 12:34:56 https://example.com/book/123?page=2",
    # 4: HTML 片段
    '<div class="content"><p>正文第一段</p><br/><p>正文第二段</p></div>',
    # 5: 广告与特殊标点
    "最新章节!笔趣阁 www.biquge.com 更新最快…………??!!【广告】《书名》(完)",
    # 6: emoji 与生僻字
    "书🎉名😀第一章巘戅测试戅巘完",
    # 7: 短串
    "abc",
    # 8: 空串
    "",
    # 9: 多段落带尾终止符
    "段落一。\n\n段落二!\n\n  段落三?\n",
]


def collect_triples():
    triples = []
    seen = set()

    def walk(o):
        if isinstance(o, str):
            if "##" in o:
                parts = o.split("##")
                if len(parts) > 1 and parts[1]:
                    pat = parts[1]
                    rep = parts[2] if len(parts) > 2 else ""
                    first = len(parts) > 3
                    key = (pat, rep, first)
                    if key not in seen:
                        seen.add(key)
                        triples.append(key)
        elif isinstance(o, dict):
            for v in o.values():
                walk(v)
        elif isinstance(o, list):
            for v in o:
                walk(v)

    for p in sorted(SRC_DIR.glob("*.json")):
        walk(json.loads(p.read_text(encoding="utf-8")))
    return triples


def main():
    triples = collect_triples()
    cases = []
    n = 0

    def pick_texts(pat):
        h = int(hashlib.md5(pat.encode()).hexdigest(), 16)
        extra = [1 + h % (len(TEXTS) - 1), 1 + (h // 7) % (len(TEXTS) - 1)]
        return sorted({0, *extra})

    pats_seen = set()
    for pat, rep, first in triples:
        for ti in pick_texts(pat):
            n += 1
            cases.append({
                "id": f"r{n:05d}", "op": "replaceRegex", "data": TEXTS[ti],
                "pattern": pat, "replacement": rep, "first": first,
            })
        if pat not in pats_seen:
            pats_seen.add(pat)
            for ti in pick_texts(pat)[:2]:
                n += 1
                cases.append({
                    "id": f"r{n:05d}", "op": "regexFind", "data": TEXTS[ti],
                    "pattern": pat,
                })

    OUT.write_text(json.dumps(cases, ensure_ascii=False, indent=1) + "\n",
                   encoding="utf-8")
    print(f"{len(triples)} triples -> {len(cases)} cases -> {OUT}", file=sys.stderr)


if __name__ == "__main__":
    main()
