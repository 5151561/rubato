#!/usr/bin/env python3
"""生成 html-compat 差分用例(选择器方言 + text/html 序列化)。

- fixtures/pages 的真实页面注册为 defineDoc,跑选择器 × 动作矩阵;
- 合成片段覆盖序列化边角(pre/br/nbsp/实体/注释/script/布尔属性/深嵌套);
- 语料中的 @css: 选择器逐条对真实页面执行。
输出 fixtures/cases/html-compat/corpus.json(生成物,不入库)。
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PAGES = ROOT / "fixtures" / "pages"
SRC_DIR = ROOT / "fixtures" / "sources"
OUT = ROOT / "fixtures" / "cases" / "html-compat" / "corpus.json"

ACTIONS = ["text", "ownText", "html", "outerHtml", "wholeText"]

PAGE_SELECTORS = [
    "#toc a", "h1", "p", "td", "li", "a", "div", "span.title", ".book",
    "li.booklink a.link", "body", "html", "*", "head", "title",
    "p:first-child", "li:last-child", "li:eq(0)", "li:eq(2)", "li:lt(3)", "li:gt(5)",
    "a[href]", "a[href^=/]", "a[href$=.html]", "a[href*=book]", "a[^data-]",
    "div:has(p)", "p:contains(the)", "p:contains(章)", "a:matches(\\d+)",
    "li:not(.booklink)", "tr:nth-child(2n)", "td:nth-of-type(2)", "li:nth-child(odd)",
    "div > p", "table td", "tr + tr", "h1 ~ p", "div p, span",
    "p:matchesOwn(^\\s*T)", "a:containsOwn(link)", ":root", "div:empty",
    "ul li:first-of-type", "[id]", "[class~=book.*]",
]

SNIPPETS = [
    ("sn-pre", "<div><pre>  line1\n  line2\t x</pre><p> after </p></div>"),
    ("sn-br", "<p>one<br>two<br/>three</p>"),
    ("sn-nbsp", "<p>a b  c&nbsp;d</p>"),
    ("sn-entity", "<p title=\"a&amp;b<c>\">x &lt;tag&gt; &amp; &quot;q&quot; 'y'</p>"),
    ("sn-comment", "<div><!-- hi --><p>a</p><!-- tail --></div>"),
    ("sn-script", "<div>before<script>var a = '<p>' && 1 < 2;</script>after<style>p { color: red; }</style></div>"),
    ("sn-bool", "<div><input type=\"checkbox\" checked disabled=\"disabled\" data-x=\"\"><option selected=\"selected\">o</option></div>"),
    ("sn-deep", "<div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><div><p>deep</p></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div></div>"),
    ("sn-inline", "<div>text <b>bold</b> <i>it</i> tail</div>"),
    ("sn-blocks", "<div>a<div>b</div>c<span>d</span><div>e</div>f</div>"),
    ("sn-table", "<table><tr><td>1</td><td>2</td></tr><tr><td>3</td></tr></table>"),
    ("sn-textarea", "<div><textarea>  keep\n  me </textarea></div>"),
    ("sn-title", "<html><head><title>  T  i  </title></head><body>b</body></html>"),
    ("sn-empty-el", "<div><img src=\"x.png\"><hr><br><wbr>tail</div>"),
    ("sn-unknown", "<div><foo>inside</foo><bar attr=\"1\">after</bar></div>"),
    ("sn-attrs", "<a data-b=\"2\" href=\"/x\" title=\"t\" class=\"k\">l</a>"),
    ("sn-ws", "<div>\n  <p>\n    a\n  </p>\n  <p>b </p>\n</div>"),
    ("sn-cn", "<div>　全角空格　<p>第一章　风云</p>​零宽­软连字</div>"),
    ("sn-quote", "<p class=\"a b C\">x</p><p class=\" a \">y</p>"),
    ("sn-nested-inline", "<p><b>a<i>b<u>c</u></i></b>d</p>"),
    ("sn-li", "<ul>\n<li>一</li>\n<li>二<p>块</p></li>\n</ul>"),
    ("sn-h", "<h1>标题 <small>小</small></h1><p>para</p>"),
]

SNIPPET_SELECTORS = [
    "div", "p", "pre", "b", "span", "table", "td", "tr", "textarea", "title",
    "input", "option", "img", "foo", "bar", "a", "ul", "li", "h1", "body", "*",
]


def corpus_css_selectors():
    sels = set()

    def walk(o):
        if isinstance(o, str):
            for m in re.finditer(r'@css:([^@#]+?)(?:@|##|$)', o):
                s = m.group(1).strip()
                if 0 < len(s) < 80 and '<' not in s:
                    sels.add(s)
        elif isinstance(o, dict):
            for v in o.values():
                walk(v)
        elif isinstance(o, list):
            for v in o:
                walk(v)

    for p in sorted(SRC_DIR.glob("*.json")):
        walk(json.loads(p.read_text(encoding="utf-8")))
    return sorted(sels)


def main():
    cases = []
    n = 0

    def add(case):
        nonlocal n
        n += 1
        case["id"] = f"h{n:05d}"
        cases.append(case)

    # 1. 真实页面
    page_names = []
    for p in sorted(PAGES.glob("*.html")):
        name = p.stem
        page_names.append(name)
        add({"op": "defineDoc", "name": name, "html": p.read_text(encoding="utf-8")})
    for name in page_names:
        for sel in PAGE_SELECTORS:
            for action in ["text", "html", "outerHtml", "attr:href"]:
                add({"op": "cssSelect", "doc": name, "selector": sel, "action": action})

    # 2. 合成片段 × 全动作
    for sn_name, html in SNIPPETS:
        add({"op": "defineDoc", "name": sn_name, "html": html})
        for sel in SNIPPET_SELECTORS:
            for action in ACTIONS:
                add({"op": "cssSelect", "doc": sn_name, "selector": sel, "action": action})

    # 3. 语料 @css 选择器 → 对全部真实页面执行
    for sel in corpus_css_selectors():
        for name in page_names:
            add({"op": "cssSelect", "doc": name, "selector": sel, "action": "text"})

    # 4. DSL(AnalyzeByJSoup):手工索引/动作用例 + 语料默认 DSL 规则
    dsl_page = "sn-dsl"
    add({"op": "defineDoc", "name": dsl_page, "html":
         '<div id="main"><ul class="Book list"><li class="a">一</li><li class="b">二'
         '<a href="/2">L2</a></li><li class="a">三</li><li>四</li><li>五</li></ul>'
         '<div class="book"><h2>标题</h2><p>说明<script>x()</script></p></div>'
         '<span>s1</span><span>s2</span>文本节点<br>尾部</div>'})
    DSL_RULES = [
        "tag.li@text", "tag.li.0@text", "tag.li.-1@text", "tag.li.0:2@text",
        "tag.li!0@text", "tag.li!0:1@text", "tag.li.-1:10:2@text",
        "tag.li[0]@text", "tag.li[-1]@text", "tag.li[0,2]@text", "tag.li[1:3]@text",
        "tag.li[-1:0]@text", "tag.li[0:4:2]@text", "tag.li[!0,1]@text", "tag.li[3:0:-1]@text",
        "class.a@text", "class.a.1@text", "class.Book@tag.li@text", "id.main@tag.span@text",
        "children@text", "children.0@tag.li@text", "text.二@text",
        "tag.li@tag.a@href", "tag.a@href", "tag.li.1@html", "tag.li@ownText",
        "tag.ul@textNodes", "div.book@all", "tag.p@html",
        "class.list@text&&tag.span@text", "class.nope@text||tag.span@text",
        "class.a@text%%tag.span@text",
        "@CSS:ul > li.a@text", "@css:div.book h2@text", "@CSS:li:eq(1)@html",
        "tag.li.x@text", "tag.li.@text", "tag.@text", "@text",
        "ul.1@text", "li[1:]@text", "li[:2]@text", "tag.li[ 1 , 3 ]@text",
        "-tag.li@text", "tag.div@tag.span.0@text",
    ]
    for rule in DSL_RULES:
        for mode in ["stringList", "string"]:
            add({"op": "jsoupDsl", "doc": dsl_page, "rule": rule, "mode": mode})
    for rule in DSL_RULES[:12]:
        add({"op": "jsoupDsl", "doc": dsl_page, "rule": rule, "mode": "elements"})

    # 语料默认 DSL 规则(## 前段;排除 js/json/xpath/模板类)
    dsl_rules = set()
    def walk_rules(o):
        if isinstance(o, dict):
            for k, v in o.items():
                if isinstance(v, str) and v:
                    r = v.split("##")[0].strip()
                    if not r or len(r) > 100:
                        continue
                    if r[0] in "$/:@<{" and not r.lower().startswith("@css:"):
                        continue
                    if any(t in r for t in ("<js>", "@js:", "{{", "{$.", "@put:", "@get:", "@json:", "@Json:")):
                        continue
                    dsl_rules.add(r)
                elif isinstance(v, (dict, list)):
                    walk_rules(v)
        elif isinstance(o, list):
            for v in o:
                walk_rules(v)
    for p2 in sorted(SRC_DIR.glob("*.json")):
        walk_rules(json.loads(p2.read_text(encoding="utf-8")))
    for rule in sorted(dsl_rules):
        for page in ["messy", dsl_page]:
            add({"op": "jsoupDsl", "doc": page, "rule": rule, "mode": "stringList"})

    # 5. 非法选择器(两侧都应 selector_error)
    for sel in ["p:unknown(x)", "[", "p:", "..", ":contains()", "p:eq(x)"]:
        add({"op": "cssSelect", "doc": page_names[0], "selector": sel, "action": "text"})

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(cases, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{n} cases -> {OUT}", file=sys.stderr)


if __name__ == "__main__":
    main()
