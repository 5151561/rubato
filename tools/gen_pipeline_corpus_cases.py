#!/usr/bin/env python3
"""生成 pipeline-corpus(真实书源 × 四步流水线)差分用例。

**两层两套**,由 `--tier=A|B` 挑(缺省 A):

- `--tier=A` → `fixtures/cases/pipeline-corpus/`,裁判 :harness(JS 是确定性桩);
- `--tier=B` → `fixtures/cases/pipeline-corpus-b/`,裁判 :jsharness(**真 Rhino**)。
  B 层 = 带 `<js>` / `@js:` / `jsLib` 的源,不接真引擎就根本跑不起来。

同一份生成逻辑(回落页反向合成、豁免、四五个 step)——两套的差别只有
「挑哪一层的源」与「摸时钟/随机的要不要剔除」,见下面的 TIER 分支。

契约:fixtures/cases/pipeline-corpus/README.md;回落页机制 docs/http-snapshot.md §6。

真实站点快照不可得(也不该进仓库),但「四步的调度、字段装配、翻页、错误分类」
在**真实规则**上的一致性仍要差分。做法:每个 case 带 `fallback`,回放层对
**未命中**的请求返回 `_fallback/<页名>.json` 那张合成页面,两侧看到的输入完全
相同,差异只可能来自实现。页名:`html-<内容哈希>`(`fallback_page.py`)与
`json-<内容哈希>`(`fallback_json.py`),**都是一个「源 × 步」一张**、按内容哈希
去重;合不出结构的源退回两张底板 `html` / `json`。

分层沿用 tools/phase3_caps.py 的 classify():C 层 = 四步路径上真的用到
webView/startBrowser/verificationCode 之类的 Phase 3 能力(**按值判定**,
空字段不算);其余带 <js>/@js:/jsLib 的是 B,剩下的是 A。
XPath 源(`@XPath:` 前缀或以 `/` 开头的规则段)**Phase 2 起入册** ——
两侧都实现了(xpath-compat / JsoupXpath),不再整源剔除。
"""
import base64
import hashlib
import json
import os
import re
import shutil
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fallback_json  # noqa: E402
import fallback_page  # noqa: E402
import phase3_caps  # noqa: E402
import rx_sample  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = ROOT / "fixtures" / "sources"
# 层由命令行挑(见模块注释)。两套各自一份用例目录与快照根 ——
# 生成器会清空自己的快照根,共用一个根会互相删。
# `--tier=A|B|top100`。前两个是「整层」,top100 是**自选的 100 个常用源**
# (跨 A/B/C 三层,选法见 select_top100 与 fixtures/cases/top100/README.md)。
TIER = next(
    (a.split("=", 1)[1] for a in sys.argv if a.startswith("--tier=")), "A"
).upper()
assert TIER in ("A", "B", "TOP100"), f"未知的 --tier={TIER}"
if TIER == "TOP100":
    SUFFIX = "-top100"
    OUT_DIR = ROOT / "fixtures" / "cases" / "top100"
    HTTP = ROOT / "fixtures" / "http-top100"
    CASE_PREFIX = "pt"
else:
    SUFFIX = "-b" if TIER == "B" else ""
    OUT_DIR = ROOT / "fixtures" / "cases" / f"pipeline-corpus{SUFFIX}"
    HTTP = ROOT / "fixtures" / f"http-pipeline-corpus{SUFFIX}"
    CASE_PREFIX = "pb" if TIER == "B" else "pc"

# **摸时钟 / 随机的书源整条不进 B 套**。确定性垫片进不去四步内部的求值
# (`<js>` / `@js:` 是真身自己 eval 的,裁判侧没有「先在作用域里跑一段」的入口)
# —— 单边冻时钟比不冻更糟。与 tools/gen_js_cases.py 的 `op: "analyzeUrl"` 同一条口径。
# `randomUUID` 也在内:两侧虽都接在定死的字节流上,但被测侧一条规则换一个宿主、
# 各自从 block 0 起,而裁判是每 case 一个全局计数器 —— 消耗序对不上。
NONDET = re.compile(r"\bDate\b|Math\.random|randomUUID")

RULE_KEYS = ("ruleSearch", "ruleExplore", "ruleBookInfo", "ruleToc", "ruleContent")

# ---------------------------------------------------------------- 回落页
# 两张页都是**一个 (源, 步) 一张**(相同形态按内容哈希去重),按**这个源自己的
# 规则**反向合成;合不出结构的源退回共用的那张底板。
# HTML 由 tools/fallback_page.py:容器规则定形状,字段规则(name/bookUrl/…)
#   按真身 `AnalyzeByJSoup` 的口径落到文本或同名属性上;底板是「按全语料的选择器
#   分布合成的那一张」。
# JSON 由 tools/fallback_json.py:按每个源自己的 JSONPath 合成;底板是改造前
#   手写的那份通用形状。
# 为什么 JSON 从一开始就只能按源一张、而 HTML 曾经共用一张:JSONPath 从根锚定,
# `$.data[*]`(搜索列表)与 `$.data.content`(正文)在一份文档里互斥;CSS 位置
# 无关,一张 DOM 摆得下几十种**容器**形态 —— 但**字段**形态摆不下
# (一个源的 `name` 是 `h4@text`、另一个是 `.s2@a@text`,payload 只能有一份),
# 那正是 html 页命中面长期停在 40% 的原因。
# 设计约束(两张页都守):
# 1. 小(元素数可控)——否则 bookList/chapterList 命中泛选择器时输出爆炸;
# 2. 所有 href 都是**同目录相对路径**——翻页规则命中时,第二跳解析回同一个
#    绝对 URL,撞上 BookChapterList/BookContent 的 nextUrlList 去重表即停,
#    页数天然有界;
# 3. 铺常见的中文书站结构与类名,让真实规则有一定命中率。

# 每步的规则对象:合成 JSON 页时按这一步自己的规则反推
STEP_RULE_KEY = {
    "search": "ruleSearch", "explore": "ruleExplore", "info": "ruleBookInfo",
    "toc": "ruleToc", "content": "ruleContent",
}


def write_fallback(name, body, content_type):
    d = HTTP / "_fallback"
    d.mkdir(parents=True, exist_ok=True)
    (d / f"{name}.json").write_text(json.dumps({
        "key": f"_fallback/{name}",
        "response": {
            "status": 200,
            "headers": {"content-type": content_type},
            "bodyBase64": base64.b64encode(body.encode("utf-8")).decode(),
        },
        "recordedAt": "2026-08-29T00:00:00Z",
        "note": "回落页(docs/http-snapshot.md §6):未命中的请求一律返回它",
    }, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")


# ---------------------------------------------------------------- 分层与筛选

# 分层口径只有一份:tools/phase3_caps.py。**按值判定**(键在不算数)——
# 老口径拿整份 JSON dump 找裸子串,`"webJs": ""` / `"sourceRegex": ""` 这两个
# ContentRule 默认就带的空键把 92 个源(29 A + 63 B)误判成 C、整源缺席 A/B 两套。
classify = phase3_caps.classify


def rule_strings(src):
    """四步规则对象里的全部规则串(rule 字段可能是「字符串包 JSON」)"""
    out = []
    for k in RULE_KEYS:
        v = src.get(k)
        if isinstance(v, str):
            try:
                v = json.loads(v)
            except Exception:
                out.append(v)
                continue
        if isinstance(v, dict):
            for x in v.values():
                if isinstance(x, str) and x.strip():
                    out.append(x)
    return out


XPATH_SEG = re.compile(r"(^|\|\||&&|%%)\s*(@xpath:|/)", re.I)


def uses_xpath(src):
    """带 XPath 规则段的源(只用来统计,Phase 2 起不再剔除)"""
    return any(XPATH_SEG.search(r) for r in rule_strings(src))


# 每一步该发哪张回落页,由**那一步自己的入口规则**定。
#   search → ruleSearch.bookList   explore → ruleExplore.bookList
#   info   → ruleBookInfo.init     toc     → ruleToc.chapterList
#   content→ ruleContent.content
# `init` 多数书源不写,故 info 那步再看 name / intro —— 它们读的是同一张详情页。
STEP_PROBE = {
    "search": (("ruleSearch", "bookList"),),
    "explore": (("ruleExplore", "bookList"),),
    "info": (("ruleBookInfo", "init"), ("ruleBookInfo", "name"),
             ("ruleBookInfo", "intro")),
    "toc": (("ruleToc", "chapterList"),),
    "content": (("ruleContent", "content"),),
}


# `[*]`(通配整个数组)与 `[?(` (过滤器)是 **JSONPath 独有**的写法 —— CSS 里
# `[*]` 不是合法的属性选择器,jsoup 选不中任何东西。语料里 **82 个「源×步」**
# 的入口规则长这样却**不带 `$` 前缀**(`data[*]`、`DATA.BOOKCATA[*].CHAPTERS[*]`)：
# 真身判 mode 看的是**内容**(`isJSON || startsWith("$.")`),页面是 JSON 时
# 这么写完全合法,所以书源作者根本不写 `$`。
# 只看 JS 块**之前**那一截:`<js>` 里出现的 `[*]` 是 JS 代码,不是规则。
_JSON_ONLY_SYNTAX = ("[*]", "[?(")


def _looks_json(rule):
    """规则**明写**了自己是 JSONPath(`$` 前缀,或 JSONPath 独有的语法)"""
    low = rule.strip().lower()
    if low.startswith("$.") or low.startswith("$[") or low.startswith("@json:"):
        return True
    # `<js>…</js>` 前奏块之后那截才是规则(口径见 fallback_json.rule_segment)
    head = fallback_json.rule_segment(rule)
    if head.lower().startswith(("$.", "$[", "@json:")):
        return True
    return any(m in head for m in _JSON_ONLY_SYNTAX)


# 这一步的 JS **把页面本身当 JSON/JS 文本吃**:`JSON.parse(result)` / `eval(src)`。
# 参数必须**整个**就是 `result` / `src` —— `JSON.parse(result.match(/…/)[1])` 是
# 「页面里**夹着**一段 JSON」,那种页面本身仍是 HTML,认错了就是往下砸命中面。
_JS_EATS_PAGE = re.compile(r"(?:JSON\s*\.\s*parse|eval)\s*\(\s*(?:result|src)\s*\)")


# 这一步的 JS **把取回来的什么东西当 JSON 吃**,但不一定是页面自己:
# `JSON.parse(java.ajax(...))`(回放层对未命中的请求发的还是这张回落页)、
# `JSON.parse(String(result))`、`JSON.parse([result].join(''))`。
# 这一档**要加条件**:只有 HTML 那张也造不出来时才翻 —— 页面只有一张,
# 既要给选择器选、又要给 JSON.parse 吃的时候,优先给**造得出来的**那一种。
_JS_EATS_FETCHED = re.compile(
    r"(?:JSON\s*\.\s*parse|eval)\s*\(\s*(?:"
    r"java\s*\.\s*(?:ajax|get)\b"          # JSON.parse(java.ajax(url))
    r"|String\s*\(\s*(?:result|src)\s*\)"  # JSON.parse(String(result))
    r"|\[\s*(?:result|src)\s*\]"            # JSON.parse([result].join(''))
    r")"
)


def _js_parses_fetched(rules):
    """JS 把**取回来的文本**当 JSON 吃(未必是页面自己)—— 见 `_JS_EATS_FETCHED`"""
    if not isinstance(rules, dict):
        return False
    return any(isinstance(v, str) and _JS_EATS_FETCHED.search(v) for v in rules.values())


def _js_parses_page(rules):
    """这一步的规则里,JS 直接把页面文本 `JSON.parse` / `eval` 了 → 这一步是 JSON 口味。

    为什么单列一条,不能指望 `_json_only`:这一档的规则**整条都是 JS**,两个生成器
    谁都反推不出形状(实测 52 例里 50 例 JSON 那张也只能给底板),于是
    `_json_only` 判 False、发一张 HTML 页 —— 而书源头一件事就是
    `JSON.parse(result)`,当场 `SyntaxError: Unexpected token: <`,**整步死在这里**。
    两侧一起抛、一起「一致」,判据看着绿,规则一个字节都没被测到。

    底板那张 JSON 至少是**合法 JSON**,`JSON.parse` 过得去,后面的字段规则才有机会
    被真的跑一遍。实测:B 层 46 个 case 命中这条,**一个都不是当前已命中的**
    (全在未命中里),所以翻口味在这一档上只可能往上抬。
    """
    if not isinstance(rules, dict):
        return False
    return any(
        isinstance(v, str) and _JS_EATS_PAGE.search(v) for v in rules.values()
    )


# `info` 步的一档「HTML 页怎么造都取不到值」——**探出来的,不是推的**
# (探针 t1/t2/t4 见下面的注释)。
#
# 真身这条路是:`BookInfo` 先 `setContent(analyzeRule.getElement(init))`,
# 而 `getElement` 走 `getAnalyzeByJSoup(content).getElements(rule)` —— 交回来的是
# **`Elements`(一个列表),不是 `Element`**。`AnalyzeByJSoup(doc: Any)` 对非
# `Element` 的入参一律 `Jsoup.parse(doc.toString())` ——**又变回一个 Document**。
# 于是字段规则 `name: "Name"` 只有一段,`getResultList` 不走选择器、直接
# `element.attr("Name")`,而 Document(`#root`)的属性在 HTML 里根本写不出来。
#
# 实测(拿 pb00515 的页面与规则做的四条探针):
#   t1  init="data" + name="Name"      → name 为 **空**
#   t2  无 init     + name="data@Name" → name = "书名一"
#   t4  init="data" + name="text.书名一" → 空(init 之后整个上下文都不对了)
# 也就是说这一档书源**只能**跑在 JSON 页上(那边 `init` 走 jayway getObject,
# 交回来的是 Map,`Name` 再对着 Map 求值,一路通)。
#
# **`init` 是 `<js>` 的除外**:那时 `setContent` 收到的是 JS 造出来的对象,
# 字段规则对着对象求值,HTML 页照样成立(实测 3 例正是这样命中的)。
_BARE_FIELD = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*")


def _html_unsatisfiable(step, rules):
    """这一步的入口规则在**任何** HTML 页上都取不到值 → 只能发 JSON 页"""
    if step != "info" or not isinstance(rules, dict):
        return False
    init = (rules.get("init") or "").strip()
    if re.search(r"<js>|@js:", init, re.IGNORECASE):
        return False                      # init 是 JS:上下文是对象,与 HTML 无关
    name = fallback_page._normalize_sel(rules.get("name") or "")
    if not name or "@" in name:           # 有动作段(`h1@text`)就不是这一档
        return False
    return bool(_BARE_FIELD.fullmatch(name.strip()))


def _json_only(step, rules):
    """两个生成器都试一遍:**只有 JSON 那张造得出** → 这一步是 JSON 口味。

    为什么要有这条:真身判 mode 看的是**内容**,不是规则前缀 ——
    `isJSON || ruleStr.startsWith("$.")`(AnalyzeRule.kt L682)。页面是 JSON 时
    **任何**规则都走 jayway,所以书源写 JSON 路径时根本不用写 `$`:
    `chapterList: data.list[*].list[*]`、`content: data.content` 满语料都是。
    只认 `$` 前缀,这些步就拿到一张 jsoup 永远选不中的 HTML 页 ——
    两侧一起空手而归、一起「一致」,判据看着绿,其实一个字段都没测
    (实测 B 层:toc 36 例、content 35 例卡在这里)。

    判据只能落在「哪张页面这条规则才**可能**选中」上,而这正是两个生成器
    各自回答的问题。**顺序要紧**:先修 fallback_page 认得的形态(砍 `<js>` 尾巴、
    自定义标签名…),再跑这条 —— 不然「html 造不出」里混的是生成器的缺口,
    会把本该是 HTML 的步误判成 JSON,那是**往下砸**命中面。
    """
    return bool(fallback_json.synth(step, rules)) and not fallback_page.synth(step, rules)


def flavor(src, step=None):
    """回落页口味:规则像 jsonpath 就发 JSON 页,否则发 HTML 页。

    **口味是按步算的,不是按源**。一整批书源(名单里 32 个同族源)搜索走 JSON API
    (`bookList: $..search[*]`)而目录/正文是普通 HTML 页(`chapterList:
    id.list@tag.dd@a`)。按源取一个口味,就必然有一半的步骤拿到**它永远匹配不上
    的那张页** —— 那些步骤两侧一起空手而归、一起「一致」,判据看着绿,
    其实什么都没测(top100 首轮实测:json 口味的 info 命中 2%、toc 4%,
    而 html 口味的同两步是 70% / 76%)。

    step=None 时退回旧的按源口径(四个入口规则里有一个像 jsonpath 就算 json)——
    只在拿不到步名时用。
    """
    if step is not None and step in STEP_PROBE:
        for key, field in STEP_PROBE[step]:
            rule = rule_field(src, key, field)
            if rule:
                if _looks_json(rule):
                    return "json"
                # 没明写的:先看 JS 有没有**直接把页面 JSON.parse 掉**(见
                # _js_parses_page),再交给「哪张页造得出」来判(见 _json_only)。
                # 顺序不能反:那一档两个生成器都反推不出形状,`_json_only` 会判 False。
                rules = rule_obj(src, STEP_RULE_KEY[step]) if step in STEP_RULE_KEY else None
                if rules and _js_parses_page(rules):
                    return "json"
                if rules and _html_unsatisfiable(step, rules) and fallback_json.synth(step, rules):
                    return "json"
                # 「JS 吃的是取回来的文本」那一档要让着 HTML:页面只有一张
                if rules and _js_parses_fetched(rules) and not fallback_page.synth(step, rules):
                    return "json"
                return "json" if rules and _json_only(step, rules) else "html"
        # 这一步一条入口规则都没有:退回按源
    for probes in STEP_PROBE.values():
        for key, field in probes:
            rule = rule_field(src, key, field)
            if rule and _looks_json(rule):
                return "json"
    return "html"


# ---------------------------------------------------------------- 自选 top-100
# plan §4 Phase 2 的最后一条判据是「**自选** top-100 常用源 ≥90%」。「自选」意味着
# 名单得由我们定 —— 于是它必须是**一条写死的、可复算的规则**,而不是一份手挑的
# 清单:手挑就能挑软柿子,判据当场作废。规则只用**包方自己的元数据**排序,
# 一个字节都不看被测侧跑得怎么样。
#
# 入池:`fixtures/sources/` 三份包里**人工策展的那两份**——
#   - `aoaostar_4dc410d1.json`(128 源,其中 90 条标着「精品」)
#   - `xiu2.json`(22 源,XIU2 那份广为人知的手维护清单)
# 另一份 `aoaostar_2a1f129b.json` 是 1554 源的**大杂烩**,没有「常用」的含义,
# 只拿它当**背书表**(见下面的 endorsed)。两份策展包合起来 150 源、零重复、
# 全都有 searchUrl。
CURATED_PACKS = ("xiu2.json", "aoaostar_4dc410d1.json")
BULK_PACK = "aoaostar_2a1f129b.json"

# 剔除(逐条带理由,统计打在生成器的汇总行上):
#   - **包方自己标了不可用**:`暂不可用` / `人机验证` 是打包人写的「它自己就跑
#     不了」。拿这种源当判据,量的是包的缺口不是我们的。
#   - **摸时钟 / 随机**:与 pipeline-corpus-b 同一条口径(NONDET),两侧都用真
#     时钟,单边冻比不冻更糟。
#   - 无 `bookSourceUrl`。
UNUSABLE_TAG = re.compile(r"暂不可用|人机验证")


def select_top100(by_pack):
    """按上面那条规则挑 100 源。返回 (选中列表, 剔除统计, 落选数)。

    排序键**全部来自包方元数据**,同档内一路打平到 bookSourceUrl 字典序,
    保证同一份 fixtures/sources 每次算出同一份名单(名单本身落在
    fixtures/cases/top100/selection.json,入库、可 review)。
      1. XIU2 那份优先(最知名的人工维护清单);
      2. 再是同时出现在大包「校验可用」组里的(**双重背书**);
      3. 再是带「精品」标签的;
      4. 同档按包方自己的 customOrder,再按 url 字典序。
    """
    endorsed = {
        s.get("bookSourceUrl")
        for s in by_pack.get(BULK_PACK, [])
        if "校验可用" in (s.get("bookSourceGroup") or "")
    }
    pool, dropped = [], Counter()
    for pack in CURATED_PACKS:
        for src in by_pack.get(pack, []):
            url = src.get("bookSourceUrl")
            if not isinstance(url, str) or not url.strip():
                dropped["无 bookSourceUrl"] += 1
                continue
            if UNUSABLE_TAG.search(src.get("bookSourceGroup") or ""):
                dropped["包方自己标了暂不可用/人机验证"] += 1
                continue
            if NONDET.search(json.dumps(src, ensure_ascii=False)):
                dropped["摸时钟/随机"] += 1
                continue
            pool.append((pack, src))
    pool.sort(key=lambda it: (
        0 if it[0] == "xiu2.json" else 1,
        0 if it[1].get("bookSourceUrl") in endorsed else 1,
        0 if "精品" in (it[1].get("bookSourceGroup") or "") else 1,
        it[1].get("customOrder") if isinstance(it[1].get("customOrder"), int) else 10 ** 9,
        it[1].get("bookSourceUrl"),
    ))
    return pool[:TOP_N], dropped, max(0, len(pool) - TOP_N)


TOP_N = 100


AUTHORITY = re.compile(r"://([^/?#]*)")


def ascii_authority(url):
    """authority 是不是纯 ASCII。**只是一个统计口径了** ——
    IDN(okhttp 的 UTS-46 映射 + punycode)M2n 已在 `net::http_url` 接上,
    这批源不再豁免,照常比。"""
    m = AUTHORITY.search(url)
    return m is None or all(ord(c) < 128 for c in m.group(1))


def explore_entry(src):
    """exploreUrl 的第一条 url(`分类::url` 行式 / JSON 数组式)"""
    ex = src.get("exploreUrl")
    if not isinstance(ex, str) or not ex.strip():
        return None
    ex = ex.strip()
    if ex.startswith("["):
        try:
            arr = json.loads(ex)
        except Exception:
            return None
        for item in arr:
            if isinstance(item, dict) and isinstance(item.get("url"), str) and item["url"].strip():
                return item["url"].strip()
        return None
    first = next((ln for ln in ex.splitlines() if ln.strip()), None)
    if first is None:
        return None
    first = first.strip()
    return first.split("::", 1)[1].strip() if "::" in first else first


# ---------------------------------------------------------------- 主流程

def rule_field(src, key, field):
    v = src.get(key)
    if isinstance(v, str):
        try:
            v = json.loads(v)
        except Exception:
            return None
    if isinstance(v, dict):
        x = v.get(field)
        if isinstance(x, str) and x.strip():
            return x.strip()
    return None


# ---------------------------------------------------------------- 已知回避面
# `regex-compat` 套已经把「Java 支持而 fancy-regex 0.14 不支持的有界变长
# lookbehind」逐条记成豁免(净化正则家族)。同一条 `##` 正则出现在 ruleContent
# 里时,四步这边会以「裁判把正文清成 ContentEmptyException 而被测侧原样留着」
# 的形态复发 —— **同一件事不该在这里被记成第二笔欠账**,所以从那份清单读,
# 不另立名目:一份真相,改一处两处一起动。
RX_EXEMPT_FILE = ROOT / "fixtures" / "cases" / "regex-compat" / "exemptions.json"


def regex_avoidance_patterns():
    if not RX_EXEMPT_FILE.exists():
        return set()
    return {
        e["pattern"] for e in json.load(open(RX_EXEMPT_FILE, encoding="utf-8"))
        if isinstance(e.get("pattern"), str)
    }


RX_AVOID = regex_avoidance_patterns()
RX_REASON = (
    "本步的 `##` 正则在 regex-compat 套里已是已知回避面:有界变长 lookbehind"
    "(`\\h{0,4}`、`[”）】]?` 之类)Java 支持而 fancy-regex 0.14 只支持定长,"
    "被测侧按编译失败降级为字面量替换 —— 于是裁判把正文清成 ContentEmpty "
    "而被测侧原样留着。清单在 fixtures/cases/regex-compat/exemptions.json,"
    "两处共用一份;待 fancy fork 实现有界变长后一起收敛(docs/plan.md §5)"
)


def uses_avoided_regex(rules):
    """这一步的规则里有没有用到那份清单上的正则(`##pattern##replacement`)"""
    for v in (rules or {}).values():
        if not isinstance(v, str) or "##" not in v:
            continue
        parts = v.split("##")
        if len(parts) > 1 and parts[1] in RX_AVOID:
            return True
    return False


# ---------------------------------------------------------------- 方言豁免(被测侧是超集)
# 与上面那份 `##` 正则的回避面**不是一回事**:那边是「我们还差一块能力」,
# 这边是「**我们比裁判更能跑**」—— 复刻裁判等于主动把书源做死。
# 目前只有一条:正则字面量里的 `\p`(不带 `u` 标志的恒等转义)。Annex B 允许,
# quickjs-ng 放行,而 Rhino 把 `\p`/`\P` 留给 Unicode 属性转义、直接 SyntaxError。
# 判据与语料数字记在 fixtures/cases/js-host/exemptions.json 的同名条目
# (js-dialect-regex-identity-escape-p),那边是一行的最小复现,这边是四步上的投影
# —— **同一件事只判一次**,那边改了这边跟着改。
#
# 探测是**保守**的:只认「一条 `/…/` 字面量里出现 `\p` 或 `\P` 且后面不跟 `{`」。
# 跟 `{` 的是 Unicode 属性转义(`\p{L}`),两侧都要 `u` 标志,不在这一档。
DIALECT_ESCAPE_P = re.compile(r"/[^/\n]*\\[pP](?!\{)[^/\n]*/[gimsuy]*")
DIALECT_REASON = (
    "**被测侧是超集**(不是欠账):这一步的书源 JS 里有 `/…\\p…/` 这样的正则字面量。"
    "不带 `u` 标志时 Annex B 允许「转义了但没有含义」的恒等转义,quickjs-ng 放行,"
    "而 Rhino 把 `\\p`/`\\P` 留给 Unicode 属性转义、直接 SyntaxError —— 于是裁判整步"
    "报 pipeline_error 而被测侧正常出结果。复刻裁判等于**主动让这个源在 Rubato 上也"
    "失效**。判据与语料数字见 fixtures/cases/js-host/exemptions.json 的 "
    "js-dialect-regex-identity-escape-p(那边是一行的最小复现,这边是它在四步上的投影)。"
)


def uses_dialect_superset(rules):
    """这一步的规则里有没有「被测侧是超集」的方言写法"""
    for v in (rules or {}).values():
        if isinstance(v, str) and DIALECT_ESCAPE_P.search(v):
            return True
    return False


# ------------------------------------------------- 裁判环境差(hutool + AES 全式 transformation)
# **第三档,既不是欠账也不是超集**:这一位在**这台裁判上问不出真身的答案**。
# hutool 5.8.22 的 `KeyUtil.generateKey(algorithm, key)` 在非 PBE / 非 DES 分支上
# 把**整条 transformation 当算法名**塞进 `SecretKeySpec`(字节码可查:
# `new SecretKeySpec(key, algorithm)`,没有 `getMainAlgorithm`)。于是密钥的
# `getAlgorithm()` 是 `AES/CBC/PKCS5Padding` —— 裁判跑在 JVM 上,**SunJCE 的
# AESCipher 查这个名字**、拒收;而真身跑在 Android 上,**Conscrypt 只查密钥长度**、
# 照跑到底。DES / DESede 那条路不受影响(hutool 的 DES 分支走 SecretKeyFactory)。
#
# 判据与语料数字见 fixtures/cases/js-host/exemptions.json 的
# js-hut-aes-full-transformation(那边是一行的最小复现,这边是它在四步上的投影)。
# **两档都给**:这不是「我们还没做」,B 层和 top100 一样问不出答案。
#
# 这六条从前是**碰巧一致**:被测侧那时对空输入解密抛 IllegalBlockSizeException、
# 与裁判一起以 CryptoException 收场 —— 一致,但两个理由。M3q 把空输入那一位按
# 真身修对之后,藏在下面的环境差才露出来(plan.md M3q)。
HUTOOL_SYM = re.compile(r"java\.(?:aes|des)\w*\s*\(|createSymmetricCrypto\s*\(")
AES_FULL = re.compile(r"\bAES/", re.IGNORECASE)
HUTOOL_AES_REASON = (
    "**裁判环境差**(既不是欠账也不是被测侧多做):这一步走 hutool 那条对称加解密路"
    "(`java.aes*` / `createSymmetricCrypto`)且 transformation 写的是 `AES/…` 全式。"
    "hutool 5.8.22 把整条 transformation 当算法名塞进 `SecretKeySpec`,裁判所在的 JVM"
    "(SunJCE)会查 `key.getAlgorithm()` 并拒收(CryptoException),而真身所在的 Android"
    "(Conscrypt)只查密钥长度、照跑 —— 于是裁判整步 pipeline_error、被测侧出结果。"
    "语料 1704 源里 17 个源这么写;复刻裁判等于让这 17 个源在 Rubato 上也失效,"
    "而失效的理由在真身上根本不存在。最小复现与字节码依据见 "
    "fixtures/cases/js-host/exemptions.json 的 js-hut-aes-full-transformation。"
)


def uses_hutool_aes_full(rules):
    """这一步的规则里有没有「hutool 对称加解密 + AES 全式 transformation」"""
    return any(isinstance(v, str) and HUTOOL_SYM.search(v) and AES_FULL.search(v)
               for v in (rules or {}).values())


# ---------------------------------------------------------------- LiveConnect 加解密
# **这一档已经收了(M3q),下面这段留着是给未来的自己看的**:曾有过第三档豁免,
# 理由是「整个 LiveConnect 加解密面还没接」—— 书源用 Rhino 的
# `new JavaImporter(Packages.javax.crypto, …)` 直接拿 JDK 的 `Cipher` /
# `SecretKeySpec` / `IvParameterSpec`(外加 `Packages.android.util.Base64`)
# 自己做 DESede/AES,而当时只移植了 `java.aesBase64DecodeToString` 那组宿主方法。
#
# 面本身 M2e 就接上了(`js-host/src/live_connect.rs`),但这条豁免一直挂到 M3q ——
# **豁免比它的理由活得久**。真正拆它的时候,72 个挂着豁免的 case 里只有 1 个还在分岔
# (`pb02492`:空输入解密该给空串,被测侧抛 IllegalBlockSizeException),
# 修完就是 0。教训写在 plan.md M3q:**豁免要按轮回头问一遍「它今天还成立吗」**,
# 不然它会把一块早就绿了的面继续盖着,也会把后来才长出来的分岔一起盖掉。

ABS_URL = re.compile(r'''https?://[^\s"'<>,\\]+''', re.IGNORECASE)


def request_base(src, url):
    """造 `book.bookUrl` / `tocUrl` / `chapter.url` 时用的**请求基址**。

    通常就是 `bookSourceUrl`。但语料里有 **13 个源**的 `bookSourceUrl` 根本不是
    URL(`小说合集`、`API-1`、`🐧` —— 真身只把它当**主键**用,能不能发请求靠
    searchUrl 自己写绝对地址)。照抄过来就会造出 `小说合集/rb/b1.html` 这种
    没有 scheme 的地址,okhttp 与被测侧**双双**在发请求之前就抛 ——
    60 个 case 里 43 个死在这儿,两侧一起报错、一起「一致」,**一个字段都没测**。

    **`bookSourceUrl` 本身一个字节不改**(它是源的主键:cookie 表、变量表、
    `sourceVariable_<key>` 全挂在它上面,改了就是改被测的东西)。这里只换那几个
    **本来就是生成器编出来的** URL 的基址,取法写死可复算:
    searchUrl → exploreUrl → 整份书源里第一个绝对地址 → 最后兜底一个按主键
    哈希出来的占位域名。
    """
    u = (url or "").strip().rstrip("/")
    if ABS_URL.fullmatch(u):
        return u
    def origin(found):
        # 只留 `scheme://host[:port]` —— 借来的那条 URL 后面的路径是别的步的,
        # 拼在 `/rb/b1.html` 前面只会让地址长得莫名其妙
        rest = found.split("://", 1)[1]
        return found.split("://", 1)[0] + "://" + rest.split("/")[0]

    for key in ("searchUrl", "exploreUrl"):
        v = src.get(key)
        if isinstance(v, str):
            m = ABS_URL.search(v)
            if m:
                return origin(m.group(0))
    m = ABS_URL.search(json.dumps(src, ensure_ascii=False))
    if m:
        return origin(m.group(0))
    return "http://%s.difftest.invalid" % hashlib.sha1(
        u.encode("utf-8")).hexdigest()[:10]


# 这一步的 JS 拿 **`baseUrl`** 去 match 的那些正则 —— 让**请求地址本身**满足它们。
#
# 与 `fallback_page._fit_value` / `._js_reads` 是同一件事的第三条取值路:书源的 JS
# 除了读页面、读自己写的规则,还常常从**当前地址**里抠 id ——
#
#     chapterUrl: "@js:\n n=baseUrl.match(/book_id=(\\d+)/)[1]; …"
#
# 合成的地址是 `…/rb/t1.html`,`match` 返回 null、`[1]` 当场 TypeError,
# **整步死在规则之前**(B 层未命中 59 例里 19 例是这一档)。
#
# `baseUrl` 是哪一个:info 是 `book.bookUrl`、toc 是 `book.tocUrl`、
# content 是这一章的 url(真身 `setBaseUrl` 的那三处);search / explore 的地址
# 由书源自己的 `searchUrl` 算出来,不在这里造,故不认。
#
# **补在查询串上**(`?…`),不是直接拼在路径后面:拼路径会把地址弄成
# `t1.htmlbook_id=3` 这种不像地址的东西,而查询串既满足正则又仍是个正常 URL。
# 每一步都拿 `re` 验一遍(`rx_sample.satisfies`),验不过就不动 —— 与诱饵同一条底线。
def base_url_fit(src, key, url):
    """(源, 这一步的规则字段, 本来的地址) → 能满足 JS 对 baseUrl 的正则期待的地址"""
    js = "\n".join(v for v in rule_obj(src, key).values() if isinstance(v, str))
    for rx in rx_sample.regexes_on(js, "baseUrl"):
        if rx_sample.satisfies(rx, url):
            continue
        s = rx_sample.sample(rx)
        if not s or any(ch in s for ch in ' "<>#'):
            continue
        cand = url + ("&" if "?" in url else "?") + s
        if rx_sample.satisfies(rx, cand):
            url = cand
    return url


def rule_obj(src, key):
    """一步的规则对象(`rule` 字段可能是「字符串包 JSON」)"""
    v = src.get(key)
    if isinstance(v, str):
        try:
            v = json.loads(v)
        except Exception:
            return {}
    return v if isinstance(v, dict) else {}


def build_html_fallback(a_sources):
    """按 A 层语料自身的选择器分布合成 HTML 回落页(tools/fallback_page.py)"""
    def top(pairs):
        c = Counter()
        for key, field in pairs:
            for src in a_sources:
                v = rule_field(src, key, field)
                if v:
                    c[v] += 1
        return [v for v, _ in c.most_common()]

    return fallback_page.build(
        book_selectors=top([("ruleSearch", "bookList"), ("ruleExplore", "bookList")]),
        chapter_selectors=top([("ruleToc", "chapterList")]),
        content_selectors=top([("ruleContent", "content"), ("ruleBookInfo", "intro")]),
    )


def main():
    by_pack, sources = {}, []
    for f in sorted(SRC_DIR.glob("*.json")):
        data = json.load(open(f, encoding="utf-8"))
        if isinstance(data, list):
            by_pack[f.name] = [s for s in data if isinstance(s, dict)]
            sources += by_pack[f.name]

    # 入册的源:整层(A/B)或自选 top-100。**这一步只挑源,不看被测侧**。
    top_pack, top_dropped, top_left = {}, Counter(), 0
    if TIER == "TOP100":
        picked, top_dropped, top_left = select_top100(by_pack)
        top_pack = {s.get("bookSourceUrl"): pack for pack, s in picked}
        selected = [s for _, s in picked]
    else:
        selected = [
            s for s in sources
            if classify(s) == TIER
            # 摸时钟/随机的整条不进 B 套(见 NONDET 顶部)。扫的是**整份书源**:
            # 书源常把 JS 藏在 bookSourceComment 里再 eval 它。
            and not (TIER == "B" and NONDET.search(json.dumps(s, ensure_ascii=False)))
        ]
    # 回落页按**入册的这批源自己的**选择器分布反向合成
    # (B 层的规则形态与 A 层不同;top100 跨三层,又是另一份分布)
    tier_sources = selected

    shutil.rmtree(HTTP, ignore_errors=True)
    html = build_html_fallback(tier_sources)
    write_fallback("html", html, "text/html; charset=utf-8")
    # `json` 是**底板**(改造前那份手写的通用形状):留着当默认页,也当
    # 「一条 JSONPath 都合成不出来的源」的回落 —— 那些源拿到的页面与改造前
    # 逐字节一致,这一改只会往上抬命中面。
    write_fallback("json", fallback_json.build(None, None),
                   "application/json; charset=utf-8")

    json_pages = {}      # 内容哈希 → 页名(相同形态的源共用一张)
    json_synth = [0, 0]  # [合成出结构的 (源,步) 数, JSON 口味的 (源,步) 总数]
    html_pages = {}
    html_synth = [0, 0, 0]   # [合成出结构的, 总数, 只有 JS 诱饵没有结构的]

    def html_page(src, step):
        """(源, 步) → 它自己那张 HTML 回落页的名字;合不出就用共用的那张。

        与 json_page 同一个机制(docs/http-snapshot.md §6):回落页名是 case
        自己带的,所以「一张页回答所有请求」只是最初的用法、不是限制。
        共用的那张只反推了**容器**规则,字段规则拿的是写死的 payload ——
        容器选中了、name 对不上,那一步照样什么都装配不出来。
        """
        rules = rule_obj(src, STEP_RULE_KEY[step])
        html_synth[1] += 1
        body = fallback_page.build_source(step, rules)
        if not body:
            return "html"                 # 底板:共用的那张,逐字节没变
        # 「合成出了结构」只算 synth() 那一半 —— 光有 JS 诱饵(`_bait`)的页
        # 单列一档,不然这个数字会把「给正则造了段样例」读成「把容器反推出来了」。
        if fallback_page.synth(step, rules):
            html_synth[0] += 1
        else:
            html_synth[2] += 1
        h = hashlib.sha1(body.encode("utf-8")).hexdigest()[:10]
        name = html_pages.get(h)
        if name is None:
            name = html_pages[h] = f"html-{h}"
            write_fallback(name, body, "text/html; charset=utf-8")
        return name

    def json_page(src, step):
        """(源, 步) → 它自己那张 JSON 回落页的名字。相同形态去重。"""
        rules = rule_obj(src, STEP_RULE_KEY[step])
        json_synth[1] += 1
        if fallback_json.synth(step, rules):
            json_synth[0] += 1
        body = fallback_json.build(step, rules)
        h = hashlib.sha1(body.encode("utf-8")).hexdigest()[:10]
        name = json_pages.get(h)
        if name is None:
            name = json_pages[h] = f"json-{h}"
            write_fallback(name, body, "application/json; charset=utf-8")
        return name

    tier_total = len(selected) if TIER == "TOP100" else sum(
        1 for s2 in sources if classify(s2) == TIER)
    stats = {"total": len(sources), "tier": tier_total, "xpath_sources": 0,
             "no_url": 0, "used": 0, "idn_sources": 0, "nondet": 0,
             "rx_cases": 0, "dialect_cases": 0, "jvm_cases": 0}
    cases = []
    exemptions = []

    def case(**kw):
        reason = kw.pop("_exempt", None)
        c = {"id": f"{CASE_PREFIX}{len(cases) + 1:05d}", "op": "pipeline"}
        c.update(kw)
        cases.append(c)
        if reason:
            # 两档分开数:`##` 正则是**欠账**(我们差一块能力),
            # 方言是**超集**(我们比裁判更能跑)—— 混在一起报会让人以为都是账
            stats["rx_cases" if reason is RX_REASON
                  else "jvm_cases" if reason is HUTOOL_AES_REASON
                  else "dialect_cases"] += 1
            exemptions.append({"id": c["id"], "reason": reason})

    stats["nondet"] = (
        top_dropped["摸时钟/随机"] if TIER == "TOP100"
        else sum(1 for s2 in sources
                 if classify(s2) == TIER
                 and TIER == "B" and NONDET.search(json.dumps(s2, ensure_ascii=False)))
    )
    selection = []
    for src in selected:
        raw = json.dumps(src, ensure_ascii=False)
        url = src.get("bookSourceUrl")
        if not isinstance(url, str) or not url.strip():
            stats["no_url"] += 1
            continue
        if uses_xpath(src):
            stats["xpath_sources"] += 1
        stats["used"] += 1
        if not ascii_authority(url):
            stats["idn_sources"] += 1

        base = request_base(src, url)
        # 口味按**步**算(见 flavor 的注释):一个源的搜索是 JSON API、
        # 目录/正文是 HTML 页,是名单里最常见的形态。
        fb_of = {st: (json_page(src, st) if flavor(src, st) == "json"
                      else html_page(src, st))
                 for st in STEP_PROBE}
        # 这一步用到的 `##` 正则若在 regex 套的已知回避面上,本例进豁免
        # (只在两侧真的不一致时才生效 —— compare 先判等再判豁免)
        rx_of = {
            st: (RX_REASON if uses_avoided_regex(rule_obj(src, STEP_RULE_KEY[st]))
                 # 方言豁免走另一条口径(被测侧是超集,不是欠账),见 DIALECT_REASON
                 else DIALECT_REASON
                 if uses_dialect_superset(rule_obj(src, STEP_RULE_KEY[st]))
                 # 裁判环境差(hutool + AES 全式 transformation),见 HUTOOL_AES_REASON
                 else HUTOOL_AES_REASON
                 if uses_hutool_aes_full(rule_obj(src, STEP_RULE_KEY[st]))
                 else None)
            for st in STEP_PROBE
        }
        s = json.dumps(src, ensure_ascii=False)
        # 地址要能满足这一步的 JS 对 `baseUrl` 的正则期待(见 base_url_fit)
        book_url = base_url_fit(src, "ruleBookInfo", f"{base}/rb/b1.html")
        toc_url = base_url_fit(src, "ruleToc", f"{base}/rb/t1.html")
        chapter_url = base_url_fit(src, "ruleContent", f"{base}/rb/c1.html")
        first_case = len(cases)

        case(source=s, step="search", key="小说", page=1, fallback=fb_of["search"], _exempt=rx_of["search"])
        ex = explore_entry(src)
        if ex:
            case(source=s, step="explore", url=ex, page=1,
                 fallback=fb_of["explore"], _exempt=rx_of["explore"])
        case(source=s, step="info", fallback=fb_of["info"], _exempt=rx_of["info"],
             book={"bookUrl": book_url})
        case(source=s, step="toc", fallback=fb_of["toc"], _exempt=rx_of["toc"],
             book={"bookUrl": book_url, "tocUrl": toc_url, "name": "书名一",
                   "author": "作者甲"})
        case(source=s, step="content", fallback=fb_of["content"], _exempt=rx_of["content"],
             book={"bookUrl": book_url, "tocUrl": toc_url, "name": "书名一",
                   "author": "作者甲"},
             chapter={"url": chapter_url, "title": "第一章 起",
                      "baseUrl": toc_url, "index": 0})
        selection.append({
            "url": url,
            "name": src.get("bookSourceName") or "",
            "pack": top_pack.get(url, ""),
            "tier": classify(src),
            "group": src.get("bookSourceGroup") or "",
            "cases": [c["id"] for c in cases[first_case:]],
        })

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    if TIER == "TOP100":
        # **名单本身入库**:判据是「自选 top-100」,那份「自选」得能被 review。
        # 生成器每次由 select_top100 重算,名单变了 git 上就看得见。
        (OUT_DIR / "selection.json").write_text(
            json.dumps(selection, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    (OUT_DIR / "corpus.json").write_text(
        json.dumps(cases, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    (OUT_DIR / "exemptions.json").write_text(
        json.dumps(exemptions, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(
        f"{len(cases)} {'top100' if TIER == 'TOP100' else 'pipeline-corpus' + SUFFIX}"
        f" cases -> {OUT_DIR/'corpus.json'}\n"
        f"  语料 {stats['total']} 源;{TIER} 层 {stats['tier']};"
        f"剔除摸时钟/随机 {stats['nondet']};"
        f"剔除无 bookSourceUrl {stats['no_url']};"
        f"含 XPath 规则的 {stats['xpath_sources']} 源自 Phase 2 起一并入册;"
        f"实际入册 {stats['used']} 源"
        f"(其中 {stats['idn_sources']} 源 authority 含非 ASCII —— M2n 起照常比,不豁免);"
        f"豁免清单 {len(exemptions)} 例 = regex 已知回避面 {stats['rx_cases']}(欠账)"
        f"+ 引擎方言超集 {stats['dialect_cases']}(被测侧更能跑)"
        f"+ 裁判环境差 {stats['jvm_cases']}(JVM 的 JCE 与 Android 的不是一套)\n"
        f"  HTML 回落页 {len(html_pages)} 张 + 共用底板 {len(html)} 字符"
        f"(按每个源自己的选择器反向合成、内容哈希去重;"
        f"{html_synth[1]} 个 HTML 口味的步里 {html_synth[0]} 个合成出了结构、"
        f"另有 {html_synth[2]} 个只造了 JS 正则诱饵);"
        f"JSON 回落页 {len(json_pages)} 张(按每个源自己的 JSONPath 反向合成、"
        f"内容哈希去重;{json_synth[1]} 个 JSON 口味的步里 {json_synth[0]} 个合成出了结构)",
        file=sys.stderr)
    if TIER == "TOP100":
        print(
            f"  自选名单:策展包 {sum(len(by_pack.get(p2, [])) for p2 in CURATED_PACKS)} 源 → "
            f"剔除 {dict(top_dropped)} → 候选 {len(selected) + top_left} → 取前 {TOP_N}"
            f"(落选 {top_left});"
            f"层次 {dict(Counter(e['tier'] for e in selection))};"
            f"来源 {dict(Counter(e['pack'] for e in selection))}",
            file=sys.stderr)


if __name__ == "__main__":
    main()
