#!/usr/bin/env python3
"""生成 explore 差分用例(发现页分类列表 / `BookSourceExtensions.exploreKinds()`)。

契约:fixtures/cases/explore/README.md。入口:tools/explore_diff.sh。

**分母是语料**:1704 源里有 `exploreUrl` 的那些,一源一例 —— 走哪条路
(JSON 数组 / `title::url` 行 / `@js:`+`<js>`)由规则自己决定,这正是要比的。

**剔除的两类,写在明处**:

- **摸时钟 / 随机**:裁判侧 `exploreKinds()` 里的 `evalJS` 是真身自己调的,
  没有「先在作用域里跑一段」的入口 —— 确定性垫片进不去(与
  `tools/gen_js_cases.py` 的 `op="analyzeUrl"` 同一条理由)。扫的是
  **url 加整份裁剪后的 source**(书源常把 JS 藏在 `bookSourceComment` 里
  再 `eval` 它)。
- 没有 `exploreUrl` 的源(真身直接 `return emptyList()`,两侧都是空表,
  比它没有信息量;这一支由手写探针钉)。

输出 fixtures/cases/explore/{01-probe,02-corpus}.json(生成物,不入库)。
"""
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCES = ROOT / "fixtures" / "sources"
OUT_DIR = ROOT / "fixtures" / "cases" / "explore"

# 与 tools/gen_js_cases.py 的 SOURCE_FIELDS 同一份口径,外加 exploreUrl 本身
SOURCE_FIELDS = [
    "bookSourceUrl", "bookSourceName", "bookSourceType", "bookSourceGroup",
    "bookSourceComment", "enabled", "enabledCookieJar", "header", "loginUrl",
    "loginUi", "concurrentRate", "jsLib", "variableComment", "exploreUrl",
    "respondTime", "weight", "customOrder", "lastUpdateTime",
]

_NONDET = re.compile(r"\bDate\b|Math\.random|randomUUID")

BASE = {
    "bookSourceUrl": "https://difftest.example.com",
    "bookSourceName": "difftest",
    "bookSourceType": 0,
    "enabled": True,
}


def probe(name: str, explore_url, **extra) -> dict:
    """手写探针:一条 `exploreUrl` 一例(书源其余字段用缺省)"""
    src = dict(BASE)
    if explore_url is not None:
        src["exploreUrl"] = explore_url
    src.update(extra)
    return {
        "id": f"ex-{name}",
        "op": "exploreKinds",
        "source": json.dumps(src, ensure_ascii=False),
    }


# 一条规则一句话说明它钉住的是什么,FAIL 时照着定位。
PROBES = [
    # --- 空与空白:`isNullOrBlank()` 那一支,直接空表
    ("empty-absent", None),
    ("empty-blank", "   \n  "),
    ("empty-str", ""),
    # --- `title::url` 那一支
    ("plain-one", "分类::/list"),
    ("plain-title-only", "只有标题"),
    ("plain-two-lines", "甲::/a\n乙::/b"),
    ("plain-amp", "甲::/a&&乙::/b"),
    # Kotlin 的 Regex.split **保留尾部空片** —— 真身实实在在多出一格空标题
    ("plain-trailing-newline", "甲::/a\n"),
    ("plain-leading-newline", "\n甲::/a"),
    # `(&&|\n)+`:连着的分隔符算一个
    ("plain-run-of-seps", "甲::/a&&\n\n&&乙::/b"),
    # `kindCfg.getOrNull(1)` 只取第二段,第三段起丢掉
    ("plain-three-segments", "甲::/a::多余"),
    ("plain-empty-title", "::/a"),
    ("plain-only-seps", "&&\n&&"),
    # --- JSON 数组那一支。`isJsonArray()` **只看首尾字符**
    ("json-minimal", '[{"title":"甲","url":"/a"}]'),
    ("json-not-really-json", "[这不是 json]"),
    ("json-unclosed", '[{"title":"甲"}'),          # 首尾不成对 → 走 split 那一支
    ("json-empty-array", "[]"),
    ("json-null-element", '[{"title":"甲"},null]'),  # 「列表不能存在null元素」
    ("json-nested-array", "[[1,2]]"),
    # gson 宽松档:键不加引号
    ("json-unquoted-keys", "[{title:'甲',url:'/a'}]"),
    # 缺席字段保留 Kotlin 默认值(data class 全默认参数 → 有无参构造)
    ("json-defaults", '[{"title":"甲"}]'),
    ("json-type", '[{"title":"甲","type":"text"}]'),
    ("json-all-fields",
     '[{"title":"甲","url":"/a","type":"select","action":"go","chars":["x","y"],'
     '"default":"x","viewName":"v"}]'),
    ("json-null-fields",
     '[{"title":null,"url":null,"type":null,"action":null,"chars":null,'
     '"default":null,"viewName":null,"style":null}]'),
    # String 字段吃基元与结构(StringJsonDeserializer)
    ("json-number-title", '[{"title":123,"url":45.6}]'),
    ("json-bool-title", '[{"title":true}]'),
    ("json-object-title", '[{"title":{"a":1}}]'),
    ("json-array-url", '[{"url":[1,2]}]'),
    # --- style:给没给 与 取值 是两位
    ("style-empty-object", '[{"title":"甲","style":{}}]'),
    ("style-grow", '[{"title":"甲","style":{"layout_flexGrow":1}}]'),
    ("style-all",
     '[{"title":"甲","style":{"layout_flexGrow":2,"layout_flexShrink":3,'
     '"layout_alignSelf":"center","layout_flexBasisPercent":0.5,'
     '"layout_wrapBefore":true,"layout_justifySelf":"center"}}]'),
    ("style-align-unknown", '[{"title":"甲","style":{"layout_alignSelf":"nope"}}]'),
    ("style-align-each",
     '[{"title":"a","style":{"layout_alignSelf":"auto"}},'
     '{"title":"b","style":{"layout_alignSelf":"flex_start"}},'
     '{"title":"c","style":{"layout_alignSelf":"flex_end"}},'
     '{"title":"d","style":{"layout_alignSelf":"center"}},'
     '{"title":"e","style":{"layout_alignSelf":"baseline"}},'
     '{"title":"f","style":{"layout_alignSelf":"stretch"}}]'),
    # 数字位吃字符串(gson 内建 Float 适配器 lenient)
    ("style-string-number", '[{"title":"甲","style":{"layout_flexGrow":"1.5"}}]'),
    ("style-bad-number", '[{"title":"甲","style":{"layout_flexGrow":"x"}}]'),
    ("style-bool-string", '[{"title":"甲","style":{"layout_wrapBefore":"true"}}]'),
    ("style-bool-string-other", '[{"title":"甲","style":{"layout_wrapBefore":"yes"}}]'),
    ("style-not-object", '[{"title":"甲","style":"x"}]'),
    # --- `@js:` / `<js>` 两支
    ("js-string", "@js:'甲::/a'"),
    ("js-json", "@js:JSON.stringify([{title:'甲',url:'/a'}])"),
    ("js-trim", "@js:'  甲::/a  '"),
    ("js-null", "@js:null"),
    # 完成值是数字时 `.toString()` 出什么形态 —— Rhino 的整数装箱在这里露头
    ("js-number", "@js:1"),
    ("js-number-double", "@js:1.5"),
    ("js-number-integral-double", "@js:2.0"),
    ("js-number-div", "@js:4/2"),
    ("js-number-big", "@js:2147483648"),
    ("js-throws", "@js:null.x"),
    ("js-syntax-error", "@js:@@@"),
    ("js-uses-source", "@js:String(source.bookSourceUrl)+'::/a'"),
    # `baseUrl` 在第三个宿主上是 `getKey()`(bookSourceUrl)
    ("js-baseurl", "@js:String(baseUrl)+'::/a'"),
    # `<js>` 那支取的是 `substring(4, lastIndexOf("<"))` —— **最后一个 `<`**
    ("jstag-plain", "<js>'甲::/a'</js>"),
    ("jstag-inner-lt", "<js>var a = 1 < 2; '甲::/a'</js>"),
    ("jstag-no-close", "<js>'甲::/a'"),
    ("jstag-upper", "<JS>'甲::/a'</JS>"),
    ("js-upper-prefix", "@JS:'甲::/a'"),
    # `java.put/get` 在这个宿主上打的是 CacheManager
    ("js-cache", "@js:java.put('k','v');'甲::/'+java.get('k')"),
]


def probe_cases() -> list:
    return [probe(name, url) for name, url in PROBES]


def corpus_cases() -> list:
    seen: set[str] = set()
    cases = []
    for f in sorted(SOURCES.glob("*.json")):
        for src in json.load(open(f, encoding="utf-8")):
            if not isinstance(src, dict):
                continue
            url = src.get("exploreUrl")
            if not url or not str(url).strip():
                continue
            trimmed = {k: src[k] for k in SOURCE_FIELDS if k in src}
            blob = json.dumps(trimmed, ensure_ascii=False)
            if _NONDET.search(blob):
                continue
            if blob in seen:
                continue
            seen.add(blob)
            h = hashlib.sha1(blob.encode("utf-8")).hexdigest()[:10]
            cases.append({
                "id": f"ex-corpus-{h}",
                "op": "exploreKinds",
                "source": blob,
            })
    return cases


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    probes = probe_cases()
    corpus = corpus_cases()
    (OUT_DIR / "01-probe.json").write_text(
        json.dumps(probes, ensure_ascii=False), encoding="utf-8"
    )
    (OUT_DIR / "02-corpus.json").write_text(
        json.dumps(corpus, ensure_ascii=False), encoding="utf-8"
    )
    print(f"explore: 探针 {len(probes)} + 语料 {len(corpus)}")


if __name__ == "__main__":
    main()
