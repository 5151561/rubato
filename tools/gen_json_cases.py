#!/usr/bin/env python3
"""生成 json-compat 差分用例。

两部分:
1. 手工语义用例:典型文档 × 精心挑选的路径(definite/indefinite、切片、
   过滤器、渲染、编译错误);
2. 语料扫描:从 fixtures/sources 抽出的真实 $ 路径,对合成文档求值
   (多数 read_error,校验"编译失败/未命中"的两侧一致性)。
输出 fixtures/cases/json-compat/corpus.json(生成物,不入库);
手工用例在 handmade.json(入库)。
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = ROOT / "fixtures" / "sources"
OUT_DIR = ROOT / "fixtures" / "cases" / "json-compat"

DOC_BOOK = json.dumps({
    "code": 0, "msg": "ok",
    "data": {
        "total": 3,
        "books": [
            {"name": "凡人修仙传", "author": "忘语", "score": 9.4, "words": 7480000,
             "tags": ["仙侠", "热血"], "status": 1, "intro": None, "vip": True},
            {"name": "诡秘之主", "author": "爱潜水的乌贼", "score": 9.8, "words": 4460000,
             "tags": ["西幻"], "status": 0, "vip": False},
            {"name": "大道朝天", "author": "猫腻", "score": 8.9, "words": 2210000,
             "tags": [], "status": 1},
        ],
    },
}, ensure_ascii=False)

DOC_TYPES = json.dumps({
    "int": 42, "neg": -7, "float": 3.5, "exp": 1.2e9, "small": 0.0005, "zero": 0,
    "big": 12345678901234, "one": 1.0,
    "s": "文本", "b1": True, "b0": False, "nil": None,
    "arr": [1, "two", 3.0, None, True, {"k": "v"}, [7, 8]],
    "obj": {"中文键": "值", "a": {"b": {"c": "deep"}}},
    "empty_arr": [], "empty_obj": {},
    "esc": ["a\"b", "back\\slash", "sl/ash", "line\nbreak", "tab\tx", "ctl"],
}, ensure_ascii=False)

DOC_CHAPTERS = json.dumps({
    "chapters": [{"title": f"第{i}章", "cid": i, "free": i < 3} for i in range(1, 8)],
    "meta": {"name": "书", "chapters": [{"title": "内层"}]},
}, ensure_ascii=False)


def corpus_paths():
    paths = set()

    def walk(o):
        if isinstance(o, str):
            for m in re.finditer(r'\$[.\[][^\s"\'<>{}##]*', o):
                p = m.group(0)
                # 引擎会先按 &&/||/%% 切分、再剥 @js:/@put:/@get: 等后缀,
                # jayway 实际收到的是干净路径——语料抽取按同样口径截断
                p = re.split(r'&&|\|\||%%|@js:|@put:|@get:|@css:|\n', p)[0]
                p = p.rstrip('@&|%,)。:：.')
                if 2 < len(p) < 120:
                    paths.add(p)
        elif isinstance(o, dict):
            for v in o.values():
                walk(v)
        elif isinstance(o, list):
            for v in o:
                walk(v)

    for p in sorted(SRC_DIR.glob("*.json")):
        walk(json.loads(p.read_text(encoding="utf-8")))
    return sorted(paths)


def main():
    cases = []
    n = 0
    for path in corpus_paths():
        for doc_name, doc in [("book", DOC_BOOK), ("types", DOC_TYPES)]:
            n += 1
            cases.append({"id": f"j{n:05d}", "op": "jsonRead", "json": doc, "path": path})
    (OUT_DIR / "corpus.json").write_text(
        json.dumps(cases, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{n} corpus cases -> {OUT_DIR/'corpus.json'}", file=sys.stderr)

    # 手工用例:路径 × 文档
    hand = []
    def add(doc, path):
        hand.append({"id": f"jh{len(hand)+1:03d}", "op": "jsonRead", "json": doc, "path": path})

    for p in [
        "$.data.total", "$.msg", "$.data.books", "$.data.books[0].name",
        "$.data.books[-1].name", "$.data.books[0].intro", "$.data.books[0].tags",
        "$.data.books[9].name", "$.data.nope", "$.data.books[*].name",
        "$.data.books[*].tags", "$..name", "$..tags", "$.data.books[0,2].name",
        "$.data.books[1:].name", "$.data.books[:2].name", "$.data.books[-2:].name",
        "$.data.books[-1:0]", "$['data']['books'][0]['author']",
        "$.data.books[?(@.vip)].name", "$.data.books[?(@.intro)].name",
        "$.data.books[?(@.score > 9)].name", "$.data.books[?(@.score>=8.9)].name",
        "$.data.books[?(@.status==1)].name", "$.data.books[?(@.status == 0)].name",
        "$.data.books[?(@.name=='猫腻')].name", "$.data.books[?(@.author=='猫腻')].name",
        "$.data.books[?(@.author!='猫腻')].name", "$.data.books[?(@.vip==true)].name",
        "$.data.books[?(@.vip==false)].name", "$.data.books[?(@.intro==null)].name",
        "$.data.books[?(@.status==1 && @.score>9)].name",
        "$.data.books[?(@.status==1 || @.vip)].name",
        "$.data.books[?(!@.vip)].name",
        "$.data.books[?(@.words > $.data.total)].name",
        "$.data[?(@.total)]", "$..books[0].name", "$..[0]",
        "$.*", "$..*", "$", "$.data.books.name",
        "$.", "$[", "$.data.books[?(@.x=]", "a.b", "$..", "$.data..books",
        # **路径两头的空白**:jayway 的 JsonPath 构造**不 trim**,不以 `$`/`@`
        # 开头的路径直接补 `$.`。书源里 `$.a&&\n$.b` 这种写法(`&&` 由
        # AnalyzeByJSonPath 自己拆,拆出来的支带着前导 `\n`)全靠这一档定生死:
        # 带 `\n` 的那一支在真身里是读不到的,我们要一起读不到。
        # 只有**尾部的空格**被属性名读取器吃掉,`\t` / `\n` 不是。
        "\n$.data.total", " $.data.total", "\t$.data.total",
        "$.data.total ", "$.data.total\t", "$.data.total\n",
        "$.data. total", "$.data .total", "\ndata.total", "data.total ",
    ]:
        add(DOC_BOOK, p)

    for p in [
        "$.int", "$.neg", "$.float", "$.exp", "$.small", "$.zero", "$.big", "$.one",
        "$.s", "$.b1", "$.b0", "$.nil", "$.arr", "$.obj", "$.empty_arr", "$.empty_obj",
        "$.arr[3]", "$.arr[5]", "$.arr[6]", "$.arr[-2]", "$.obj.中文键", "$.obj.a.b",
        "$.obj.a.b.c", "$['obj']['中文键']", "$.arr[*]", "$..k", "$..b",
        "$.arr[1:100]", "$.arr[2:2]", "$.empty_arr[*]", "$.empty_arr[0]",
        "$.arr[?(@ > 1)]", "$.obj[?(@.a)]", "$.esc", "$..esc", "$.esc[*]",
        "$.s[*]", "$.int[*]", "$.s[0]", "$.nil[*]", "$.missing[*]",
        "$.missing.deeper", "$.arr[*].k", "$.arr[*].missing", "$.obj.*.b",
    ]:
        add(DOC_TYPES, p)

    for p in [
        "$.chapters[*].title", "$..chapters[*].title", "$.chapters[?(@.free)].cid",
        "$.chapters[?(@.cid<4)].title", "$.chapters[-1].title", "$.chapters[2:5].cid",
        "$..title", "$.meta.chapters[0].title",
    ]:
        add(DOC_CHAPTERS, p)

    # 非法 JSON 文档
    hand.append({"id": "jh900", "op": "jsonRead", "json": "not json", "path": "$.a"})
    hand.append({"id": "jh901", "op": "jsonRead", "json": "[1,2,", "path": "$[0]"})

    (OUT_DIR / "handmade.json").write_text(
        json.dumps(hand, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(hand)} handmade cases -> {OUT_DIR/'handmade.json'}", file=sys.stderr)

    gen_dsl()


DSL_RULES = [
    # 单条(走 read 分支)
    "$.data.books[0].name", "$.data.books[*].name", "$.nope", "$.data.total",
    "$.data.books[0].intro", "$.data.books", "$.data.books[*]",
    # && / || / %% 组合
    "$.data.books[0].name&&$.data.books[1].name",
    "$.data.books[0].name||$.data.books[1].name",
    "$.nope||$.data.books[1].name",
    "$.nope&&$.data.books[1].name",
    "$.data.books[*].name%%$.data.books[*].author",
    "$.data.books[*].name%%$.data.books[0].author",
    "$.data.books[0].name&&$.nope&&$.data.books[2].name",
    "$.data.books[*].tags||$.data.books[*].name",
    # {$.} 内嵌规则
    "第{$.data.total}本", "{$.data.books[0].name}-{$.data.books[0].author}",
    "前缀{$.nope}后缀", "{$.data.books[*].name}", "{$.msg}",
    "{$.data.books[0].name}&&{$.data.books[1].name}",
    "不平衡{$.data.total", "{$.data.books[0].tags}",
    # 空与畸形
    "", "$", "$.", "$..", "$[", "not a path", "$.data.books[?(@.bad",
]

DSL_MODES = ["string", "stringList", "list", "object"]

# **构造那一步**的边界:jayway 的 `ParseContextImpl.parse(String)` 头一行是
# `notEmpty(json, "json string can not be null or empty")` —— 空串在解析器之前
# 就抛 IllegalArgumentException,而**全是空白**的串它照样往下走(按裸字符串收)。
# 差一个字都是两种行为,所以这几条按**实跑**钉住(拿 jayway 的 jar 探的,不是
# 照源码推的);上面那批 DSL 用例的 doc 永远是合法 JSON,照不到这一档。
#
# 代价是实测出来的:书源 `<js>` 把正文解成空串再交给 `$.content_html`,
# 裁判整步抛而被测侧给 content_empty —— pipeline-corpus-b 的 pb02295 / pb02296。
DSL_EDGE_DOCS = ["", " ", "   ", "\n\t ", "not json", "null", "[]"]


def gen_dsl():
    """AnalyzeByJSonPath 的 DSL 面(&&/||/%% 切分 + {$.} 内嵌规则)"""
    out = []
    for doc_name, doc in [("book", DOC_BOOK), ("types", DOC_TYPES), ("chap", DOC_CHAPTERS)]:
        for rule in DSL_RULES:
            for mode in DSL_MODES:
                out.append({
                    "id": f"jd{len(out):05d}", "op": "jsonDsl",
                    "json": doc, "rule": rule, "mode": mode,
                })
    for doc in DSL_EDGE_DOCS:
        for rule in ("$.a", "$.data.books[*].name", "{$.a}", ""):
            for mode in DSL_MODES:
                out.append({
                    "id": f"jd{len(out):05d}", "op": "jsonDsl",
                    "json": doc, "rule": rule, "mode": mode,
                    "note": "构造那一步的边界:空串抛、全空白不抛",
                })
    (OUT_DIR / "dsl.json").write_text(
        json.dumps(out, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(out)} dsl cases -> {OUT_DIR/'dsl.json'}", file=sys.stderr)


if __name__ == "__main__":
    main()
