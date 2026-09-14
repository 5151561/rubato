#!/usr/bin/env python3
"""source 差分(BookSource 的 GSON 反序列化语义)的用例生成。

corpus:fixtures/sources 里的每个书源原样一条;
handmade:类型边角(int-as-string、rule-as-string、对象转字符串等)。
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "fixtures" / "cases" / "source"

HANDMADE = [
    # rule 是「字符串包 JSON」
    '{"bookSourceUrl":"http://a","ruleToc":"{\\"chapterList\\":\\"@css:li\\"}"}',
    # int 字段给字符串:不转,保默认
    '{"bookSourceUrl":"http://a","bookSourceType":"2","customOrder":"5","weight":"7"}',
    '{"bookSourceUrl":"http://a","bookSourceType":2.9}',
    # String 字段给数字/布尔/对象/数组
    '{"bookSourceUrl":"http://a","searchUrl":123,"header":{"UA":"x"},"loginUrl":true,"jsLib":[1,2]}',
    # Long 字段
    '{"bookSourceUrl":"http://a","lastUpdateTime":"123","respondTime":5000}',
    '{"bookSourceUrl":"http://a","respondTime":"abc"}',
    '{"bookSourceUrl":"http://a","lastUpdateTime":1.5e3}',
    # Boolean 字段
    '{"bookSourceUrl":"http://a","enabled":"false","enabledCookieJar":"TRUE"}',
    '{"bookSourceUrl":"http://a","enabled":1}',
    '{"bookSourceUrl":"http://a","enabledCookieJar":null}',
    # rule 各种类型
    '{"bookSourceUrl":"http://a","ruleSearch":null}',
    '{"bookSourceUrl":"http://a","ruleSearch":[1,2]}',
    '{"bookSourceUrl":"http://a","ruleSearch":123}',
    '{"bookSourceUrl":"http://a","ruleSearch":"not json"}',
    '{"bookSourceUrl":"http://a","ruleSearch":""}',
    '{"bookSourceUrl":"http://a","ruleSearch":{"name":123,"bookList":{"x":1},"author":null}}',
    # 宽松 JSON
    "{bookSourceUrl:'http://a',searchUrl:'/s'}",
    '{"bookSourceUrl":"http://a",}',
    '{"坏',
    '[]',
    '"just a string"',
    # 未知键忽略
    '{"bookSourceUrl":"http://a","ruleReview":{"reviewUrl":"x"},"unknown":[{}]}',
    # 完整常规源
    json.dumps({
        "bookSourceUrl": "https://www.example.com",
        "bookSourceName": "示例源",
        "bookSourceGroup": "测试",
        "bookSourceType": 0,
        "enabledCookieJar": True,
        "searchUrl": "/search?q={{key}}",
        "exploreUrl": "分类::/sort/1_{{page}}.html",
        "ruleSearch": {"bookList": "@css:.result", "name": "h3@text", "bookUrl": "a@href"},
        "ruleBookInfo": {"name": "h1@text", "tocUrl": "#toc@href"},
        "ruleToc": {"chapterList": "@css:#list a", "chapterName": "text", "chapterUrl": "href"},
        "ruleContent": {"content": "#content@html", "replaceRegex": "##广告"},
    }, ensure_ascii=False),
]


def main():
    cases = []

    def case(js):
        cases.append({"id": f"sc{len(cases):05d}", "op": "parseSource", "json": js})

    for js in HANDMADE:
        case(js)

    n_corpus = 0
    for f in sorted((ROOT / "fixtures" / "sources").glob("*.json")):
        for src in json.load(open(f, encoding="utf-8")):
            case(json.dumps(src, ensure_ascii=False))
            n_corpus += 1

    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "generated.json").write_text(
        json.dumps(cases, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(cases)} source cases({n_corpus} 条语料)-> {OUT/'generated.json'}",
          file=sys.stderr)


if __name__ == "__main__":
    main()
