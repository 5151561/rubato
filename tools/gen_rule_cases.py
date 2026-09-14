#!/usr/bin/env python3
"""生成 rule-engine(AnalyzeRule)差分用例。

handmade.json:模式识别 × 取值方式 × 后处理(##正则 / {{}} / @get / @put /
$N / isUrl)的交叉矩阵;corpus.json:从 fixtures/sources 抽真实规则串,
对合成页面求值——重点校验切分与分发在长尾规则上的两侧一致性。

JS 一律走确定性桩,指令表见 fixtures/cases/rule-engine/README.md。
"""
import json
import random
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = ROOT / "fixtures" / "sources"
OUT_DIR = ROOT / "fixtures" / "cases" / "rule-engine"

HTML_LIST = """<html><head><title>书库</title></head><body>
<div id="main">
 <ul class="book-list">
  <li class="item" data-id="1"><a href="/book/1.html" class="name">凡人修仙传</a>
    <span class="author">忘语</span><em class="score">9.4</em></li>
  <li class="item" data-id="2"><a href="book/2.html" class="name">诡秘之主</a>
    <span class="author">爱潜水的乌贼</span><em class="score">9.8</em></li>
  <li class="item" data-id="3"><a href="http://other.com/3.html" class="name">大道朝天</a>
    <span class="author">猫腻</span></li>
 </ul>
 <div class="intro">简介 &amp; 说明&nbsp;结束&#65;</div>
 <script>var x = 1;</script>
 <p class="page">第 12 页 / 共 34 页</p>
</div></body></html>"""

HTML_TOC = """<html><body><div id="list"><dl>
<dt>正文</dt>
<dd><a href="/c/1.html">第一章 开始</a></dd>
<dd><a href="/c/2.html">第二章 继续</a></dd>
<dd><a href="/c/3.html">第三章 结束</a></dd>
</dl></div><div id="content">正文第一段<br>正文第二段</div></body></html>"""

JSON_LIST = json.dumps({
    "code": 0,
    "data": {
        "total": 3,
        "books": [
            {"name": "凡人修仙传", "author": "忘语", "url": "/book/1.html", "score": 9.4},
            {"name": "诡秘之主", "author": "爱潜水的乌贼", "url": "book/2.html", "score": 9.8},
            {"name": "大道朝天", "author": "猫腻", "url": "http://other.com/3.html"},
        ],
    },
}, ensure_ascii=False)

TEXT = "书名:凡人修仙传 作者:忘语 字数:748万 状态:完本"

DOCS = [("list", HTML_LIST), ("toc", HTML_TOC), ("json", JSON_LIST), ("text", TEXT)]

def real_pages():
    """fixtures/pages 里的真实页面(html-compat 差分同款语料)"""
    out = []
    for p in sorted((ROOT / "fixtures" / "pages").glob("*.html")):
        out.append((p.stem, p.read_text(encoding="utf-8", errors="replace")))
    return out

RULES = [
    # 基础 jsoup
    "", " ", "class.item@text", ".item@a@href", "#main@class.intro@text",
    "@@class.item!0@text", "@CSS:.item .name@text", "tag.li@a@text",
    "class.item.0@text", "class.item[1:]@text", "class.name@href",
    "text", "html", "all", "class.intro@ownText",
    # JSON
    "$.data.books[*].name", "$.data.total", "@Json:$.data.books[0].author",
    "$.data.books[*].url", "$.data.books[*].name&&$.data.books[*].author",
    # ## 正则后处理
    "class.intro@text##说明", "class.intro@text##说明##备注",
    "class.page@text##第(\\d+)页##$1", "class.page@text##(\\d+)##[$1]###",
    "class.item@text##[0-9]+##N", "class.name@text##(##)",
    # {{}} / @get / @put
    "{{#echo:注入}}", "前{{#echo:中}}后", "{{#null}}尾", "{{#num:12}}",
    "{{#num:12.5}}", "{{#list:a|b}}", "{{@@class.name@text}}", "{{$.data.total}}",
    "@put:{\"k\":\"class.name@text\"}class.author@text",
    "@put:{\"k\":\"class.name@text\"}@get:{k}",
    "@put:{k:class.author@text}@get:{k}",
    "@put:{'k':'class.name@text','j':'class.author@text'}@get:{j}-@get:{k}",
    "@get:{missing}", "@get:{bookName}", "@get:{title}",
    "{{#get:seed}}", "{{#put:seed=新值}}@get:{seed}",
    # JS 段
    "<js>#echo:JS结果</js>", "@js:#echo:尾部JS", "class.name@text<js>#result</js>",
    "<js>#null</js>", "<js>#err</js>", "<js>#baseUrl</js>", "<js>#src</js>",
    "<js>#list:x|y|z</js>", "@webjs:[\"a\",\"b\"]", "@webjs:纯文本回显",
    "class.name@text@js:#result", "<js>#echo:A</js><js>#echo:B</js>",
    # $N / 正则模式
    ":书名:(.*?) 作者", ":书名:(.*?) 作者:(.*)", "$1", "书名是$1",
    ":(\\d+)万", "class.name@text\n$0",
    # 畸形与边界
    "##", "###", "@@", "@CSS:", "$", "class.notexist@text",
    "class.item@a@nothing", "@Json:$.nope", "  class.name@text  ",
    # 组合分隔符
    "class.name@text&&class.author@text", "class.name@text||class.notexist@text",
    "class.notexist@text||class.author@text", "class.name@text%%class.author@text",
    "class.item@a@href&&class.item@span@text",
    # 嵌套 {{}} 与 @get 混排
    "{{@get:{k}}}", "@put:{\"k\":\"class.name@text\"}前{{@get:{k}}}后",
    "{{#echo:A}}{{#echo:B}}{{#echo:C}}", "{{}}", "{{ }}",
    "{{#echo:x}}##x##y", "class.name@text##(.)##<$1>",
    "@get:{k}##凡##FAN",
    # $N 与正则模式
    ":书名:(.*?) 作者:(.*?) 字数", ":(书名):(.*)", "$1-$2", ":无匹配(.*)",
    ":(.*)&&:(作者:.*)", "$99", "$0$1",
    # 省略根的 JSON 路径(jayway 会补 $.)
    "data.books[*].name", "data.total", "data.books[0].name",
    "$['data']['books'][0,1]", "$.data['books','total']", "$['a','b']",
    # webjs 各形态
    "@webjs:{\"k\":\"v\"}", "@webjs:[{\"k\":1},{\"k\":2}]", "@webjs:[1,2]",
    "@webjs:短",
    # js 与普通规则混排
    "class.name@text<js>#echo:JS</js>class.author@text",
    "@js:#put:a=1", "<js>#put:b=2</js>@get:{b}",
    "<js>#bool:true</js>", "<js>#fromBookInfo</js>", "<js>#nextChapterUrl</js>",
    "<js>#num:0</js>", "<js>#num:-3</js>", "<js>#num:1e21</js>", "<js>#num:abc</js>",
    "<js>#list:</js>", "<js>#get:v1</js>",
]

URL_RULES = [
    "class.name@href", "$.data.books[*].url", "class.item@a@href",
    "{{#echo:/x/y.html}}", "class.notexist@href", "<js>#echo:../up.html</js>",
]

MODES = ["string", "stringList", "element", "elements"]


# `@webjs:` 走的是 **BackstageWebView 的策略层**(两侧都是真身/移植,不再是
# 「回传 javaScript 原文」的 Phase 1 桩)。平台那一半是**剧本**:
# `webview.evals` 是每次求值的回值,契约见 fixtures/cases/webview/README.md。
#
# 剧本的回值这里钉成「**把那段 js 源码本身回给你**」——与从前那个桩的口径
# 逐字一致,于是 `fromJsonArray` / `##替换` / 四个 mode 的下游覆盖一条没丢,
# 而路上跑的已经是真策略(默认 JS 之外的排期、unescapeJson、剥引号、还 WebView)。
WEBJS_RE = re.compile(r"@webjs:([\w\W]{5,})", re.IGNORECASE)


def webview_script(rule):
    """规则里带 `@webjs:` 的,配一份让它取得到内容的剧本;不带就不配。"""
    m = WEBJS_RE.search(rule)
    if not m:
        return None
    # SourceRule 里 `##` 之后是 replaceRegex,不属于 js
    js = m.group(1).split("##")[0]
    # 从前的 `#nullbody` 指令钉的是「body 为 null → `.toString()` 得字符串
    # "null"」。真策略的 body 永不为 null,但**同一个观察面**取得到:
    # 让页面上那段 js 回一个字符串 "null" 就是了。
    body = "null" if js == "#nullbody" else js
    return {"evals": [json.dumps(body, ensure_ascii=False)]}


def base_case(n, doc_name, doc, rule, mode, **extra):
    c = {
        "id": f"re{n:05d}", "op": "analyzeRule", "content": doc,
        "baseUrl": "http://www.example.com/a/b.html",
        "redirectUrl": "http://www.example.com/a/b.html",
        "rule": rule, "mode": mode,
    }
    script = webview_script(rule)
    if script:
        c["webview"] = script
    c.update(extra)
    return c


def handmade():
    out = []
    for doc_name, doc in DOCS:
        for rule in RULES:
            for mode in MODES:
                out.append(base_case(len(out), doc_name, doc, rule, mode))
    # isUrl
    for doc_name, doc in [("list", HTML_LIST), ("json", JSON_LIST)]:
        for rule in URL_RULES:
            for mode in ["string", "stringList"]:
                out.append(base_case(len(out), doc_name, doc, rule, mode, isUrl=True))
    # 变量层组合:book / chapter / 初始变量
    var_rules = [
        "@get:{bookName}", "@get:{title}", "@get:{v1}", "@get:{v2}",
        "@put:{\"v1\":\"class.name@text\"}@get:{v1}",
        "{{#put:v3=写入}}@get:{v3}", "{{#get:v1}}",
    ]
    for kind in ["plain", "book", "none"]:
        for chapter in [False, True]:
            for rule in var_rules:
                out.append(base_case(
                    len(out), "list", HTML_LIST, rule, "string",
                    ruleData=kind, chapter=chapter, bookName="书名A", title="章节T",
                    vars={"v1": "初值1", "v2": "初值2"},
                    chapterVars={"v1": "章节值1"},
                ))
    # baseUrl/redirectUrl 缺失
    for rule in ["class.name@href", "class.notexist@href"]:
        out.append(base_case(len(out), "list", HTML_LIST, rule, "string", isUrl=True,
                             baseUrl=None, redirectUrl=None))
    for c in out:
        for k in ("baseUrl", "redirectUrl"):
            if c.get(k) is None:
                c.pop(k, None)
    return out


def corpus_rules():
    rules = set()
    for f in sorted(SRC_DIR.glob("*.json")):
        for src in json.load(open(f, encoding="utf-8")):
            for key in ("ruleSearch", "ruleBookInfo", "ruleToc", "ruleContent", "ruleExplore"):
                v = src.get(key)
                if isinstance(v, str):
                    try:
                        v = json.loads(v)
                    except Exception:
                        rules.add(v)
                        continue
                if isinstance(v, dict):
                    for x in v.values():
                        if isinstance(x, str) and x.strip():
                            rules.add(x)
    return sorted(r for r in rules if len(r) < 400)


def corpus():
    rng = random.Random(20260829)
    rules = corpus_rules()
    rng.shuffle(rules)
    rules = rules[:3500]
    out = []
    pages = real_pages()
    for i, rule in enumerate(rules):
        doc_name, doc = DOCS[i % 2]  # list / toc 交替
        for mode in ["string", "stringList"]:
            out.append(base_case(len(out), doc_name, doc, rule, mode))
        if i % 5 == 0:
            out.append(base_case(len(out), "json", JSON_LIST, rule, "string"))
        if i % 7 == 0:
            out.append(base_case(len(out), doc_name, doc, rule, "elements"))
        if i % 11 == 0:
            out.append(base_case(len(out), doc_name, doc, rule, "element"))
        if pages and i % 13 == 0:
            pn, pd = pages[(i // 13) % len(pages)]
            out.append(base_case(len(out), pn, pd, rule, "stringList"))
    for c in out:
        c["id"] = "rc" + c["id"][2:]
    return out, len(rules)



# ---------------------------------------------------------------- 漂移补覆盖
# plan §4 里挂着的「已探明但未被差分覆盖」几条,逐条铺成用例。

# gson LinkedTreeMap 内容(contentType=map)。数值字面量刻意铺满
# MapDeserializerDoubleAsIntFix 的每条分支。
MAP_CONTENT = json.dumps({
    "name": "凡人修仙传",
    "author": "忘语",
    "url": "/book/1.html",
    "three": 3.0,            # ceil==toLong → Long 3
    "half": 2.5,             # ceil(2.5)=3 != 2 → Double 2.5
    "neghalf": -2.5,         # ceil(-2.5)=-2 == -2 → Long -2(反直觉,要钉住)
    "exp": 1e3,              # Long 1000
    "int": 7,
    "big": 9007199254740993,  # 超 2^53 的整数字面量:Long 原值
    "bigf": 1.0e20,          # ceil 与 toLong 都对不上 → Double
    "flag": True,
    "nil": None,
    "list": ["a", "b"],
    "nums": [1.0, 2.5, -2.5],
    "obj": {"x": 1.0, "y": "z"},
    "multi": "第一行\n第二行",
    "amp": "A&amp;B",
    "$.name": "美元键",
}, ensure_ascii=False)

MAP_RULES = [
    # 直取键(LinkedTreeMap 短路分支:不跑 put / makeUpRule / ##)
    "name", "author", "three", "half", "neghalf", "exp", "int", "big", "bigf",
    "flag", "nil", "list", "nums", "obj", "multi", "amp", "missing",
    # 短路分支下这些「后处理」应当**不生效**(整串当键查 → 查不到)
    "name##凡人##FAN", "{{#echo:x}}", "@get:{k}",
    "@put:{\"k\":\"name\"}@get:{k}",
    # isJSON=true 让它们被切成 Mode.Json,但短路分支根本不看 mode
    "$.name", "$.list[*]", "$.obj.x",
    # 多段规则:短路只认 first()
    "name&&author", "name@text",
]

# webJs 的 body 是字符串 "null"(剧本让页面上那段 js 回 "null" ——
# 从前是桩指令 `#nullbody`,口径不变,见 webview_script)
WEBJS_RULES = [
    "@webjs:#nullbody",
    "class.name@text@webjs:#nullbody",
    "@webjs:#nullbody##null##空",
    # fromJsonArray 含 null 元素 → 抛 → 回退成原串
    "@webjs:[\"a\",null]",
    "@webjs:[\"a\",\"b\"]",
    "@webjs:[1,2.0,true]",
    "@webjs:{\"k\":1}",
]

# JS_PATTERN 的两个捕获组:`<js></js>` 命中 group(1)、group(2) 为 **null**;
# `@js:` 命中 group(2)(空体时是**空串**而非 null)。裁判取
# `group(2) ?: group(1)`,只判 null 不判空 —— 铺满空/非空 × 两支。
JS_GROUP_RULES = [
    "<js></js>", "<JS></JS>", "<js> </js>", "<js>\n</js>",
    "@js:", "@JS:", "@js: ", "@js:\n",
    "class.name@text<js></js>", "<js></js>class.author@text",
    "class.name@text@js:", "<js></js><js></js>", "<js></js>@js:",
    "<js>#echo:A</js>@js:", "@js:<js></js>",
    "<js></js>##x##y", "<js></js>&&class.name@text",
    # webjs 的 group(1):`{5,}` 不满足时整条不匹配,退回普通规则串
    "@webjs:", "@webjs:1234", "@webjs:12345",
]


def drift():
    """plan §4「⬜ 尚未被差分覆盖」几条的用例(id 前缀 rd)"""
    out = []

    def add(rule, mode, **extra):
        c = base_case(len(out), "drift", MAP_CONTENT, rule, mode)
        c.update(extra)
        out.append(c)

    # 1) LinkedTreeMap 内容
    for rule in MAP_RULES:
        for mode in MODES:
            add(rule, mode, contentType="map")
        add(rule, "string", contentType="map", isUrl=True)
        add(rule, "stringList", contentType="map", isUrl=True)
    # 内容是空 map / 只有 null 值
    for content in ["{}", '{"a":null}']:
        for rule in ["a", "missing"]:
            for mode in MODES:
                c = base_case(len(out), "drift", content, rule, mode)
                c["contentType"] = "map"
                out.append(c)

    # 2) webJs 空 body 与 fromJsonArray 的 null 元素
    for rule in WEBJS_RULES:
        for mode in MODES:
            c = base_case(len(out), "drift", HTML_LIST, rule, mode)
            out.append(c)

    # 3) JS_PATTERN / WebJS_PATTERN 的捕获组空值之别
    for rule in JS_GROUP_RULES:
        for mode in MODES:
            c = base_case(len(out), "drift", HTML_LIST, rule, mode)
            out.append(c)
        c = base_case(len(out), "drift", HTML_LIST, rule, "string")
        c["isUrl"] = True
        out.append(c)

    for c in out:
        c["id"] = "rd" + c["id"][2:]
    return out


def main():
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    hand = handmade()
    (OUT_DIR / "handmade.json").write_text(
        json.dumps(hand, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(hand)} handmade cases -> {OUT_DIR/'handmade.json'}", file=sys.stderr)
    dr = drift()
    (OUT_DIR / "drift.json").write_text(
        json.dumps(dr, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(dr)} drift cases -> {OUT_DIR/'drift.json'}", file=sys.stderr)
    corp, n = corpus()
    (OUT_DIR / "corpus.json").write_text(
        json.dumps(corp, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(corp)} corpus cases({n} 条真实规则)-> {OUT_DIR/'corpus.json'}", file=sys.stderr)


if __name__ == "__main__":
    main()
