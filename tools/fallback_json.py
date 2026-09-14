#!/usr/bin/env python3
"""把书源**自己的 JSONPath 规则**反向合成成一份能被它选中的 JSON 回落页。

与 `tools/fallback_page.py`(HTML 那张)同一个思路、同一条安全性论证:
页面由**规则**反推,两侧看到的是同一份字节,造得不好只可能「选不中」
(退回空结果分支),不可能造出假 PASS。

## 为什么 JSON 这张必须**按源一张**,而 HTML 那张可以全语料共用一张

CSS 选择器是**位置无关**的(`.bookname` 在树里任何地方都选得中),所以一张 DOM
可以把几十种形态的块并排摆进去,谁都选得中自己那块。JSONPath 不是:除了 `$..`
以外它**从根锚定**。语料里最常见的容器名恰恰互相打架 ——

    $.data[*]      (搜索列表:18 源)   →  data 必须是数组
    $.data.entry   (目录列表:4 源)    →  data 必须是对象
    $.data.content (正文:7 源)        →  data 必须是对象

一份文档满足不了这三条中的两条以上。手写的那份通用形状因此命中率只有 4~13%
(HTML 那张按语料反向合成,命中 78~87%)。

出路是**把「一张页」这个约束本身去掉**:回落页的名字是 case 自己带的
(`"fallback": "<name>"` → `<snapshot_root>/_fallback/<name>.json`,两侧
逐字同一套解析),所以一个 (源, 步) 可以有自己的一张。相同形态的源合成出
逐字节相同的文档 → 按内容哈希去重,文件数远小于源数。

## 合成规则

1. 规则串先按 `||` / `&&` / `%%` 顶层切分(括号/引号内不切),每支再砍掉
   `<js>` / `@js:` / `##` / `@put:` / `{{` 之后的尾巴 —— **每一支都合成**,
   它们是「或」的关系,全都满足只会更早命中。
2. 路径切成 token:`.key` / `..key` / `['key']` / `[*]` / `.*` / `[n]` /
   `[a:b]` / `[?(@.k)]`。认不出的形态**整条丢掉**(那一支就退回下面的底板)。
3. **列表规则**(`bookList` / `chapterList`)的末端一定是数组:路径没以容器
   token 结尾时补一个。`$.data` 与 `$.data[*]` 因此合成出同一份结构。
4. **字段规则**(`name` / `chapterUrl` / …)相对**列表项**求值(裁判把选中的
   元素当根),所以它们合成进 item;`ruleBookInfo` 没写 `init` 时字段是相对
   根的,直接合成进根。
5. `[?(@.k)]` / `[?(@.k==1)]` / `[?(@.k!=1)]` 的谓词键并进 item(**不新起一层**
   —— 过滤器筛的是当前集合,不是往下走一层),值取**让谓词成立**的那一个。
6. **内嵌规则也收**:`{{$.x}}` / `{$.x}` 在真身里不是 JS 而是规则
   (`SourceRule.makeUpRule` 的 `isRule()` 认 `$.`/`$[`/`@`/`//` 开头的内嵌体,
   AnalyzeRule.kt L832),`java.getString('$.x')` 同理。语料里
   `coverUrl: {{$.cover}}` 这类写法极多,不认它就丢掉一大片字段
   (实测:字段规则的可解析数 1172 → 1600)。
7. **页面要像真页**:键名像时间戳/id 的、以及被塞进 `timeFormat(...)` / JS 算术
   的路径,值给数字而不是展示串 —— 否则测的不是规则,是 `"玄幻" * 1000` 的
   NaN 行为(M2k 那一课:「合成的回落页会自己造出分歧」)。
8. 最后与**底板**(`BASE`,即此前手写的那份通用形状)合并,**源自己的结构优先**。
   这保证「合成不出东西的源」拿到的页面与改造前逐字节一致,不会比原来更差。
"""
import json
import re

# ---------------------------------------------------------------- 值表
# 同一个数组里的第 i 项取第 i 个值(书名一/书名二),与 HTML 回落页的用词一致,
# 这样两张页面上装配出来的东西长得一样,读 diff 时不用换脑子。
VALUES = {
    "name": ["书名一", "书名二"],
    "author": ["作者甲", "作者乙"],
    "kind": ["玄幻", "都市"],
    "intro": ["简介第一句。简介第二句。", "另一本的简介。"],
    "cover": ["cover1.jpg", "cover2.jpg"],
    "last": ["第 10 章 最新", "第 8 章 更新"],
    "wordCount": ["12.3万字", "8.0万字"],
    "bookUrl": ["b1.html", "b2.html"],
    "tocUrl": ["t1.html", "t2.html"],
    "chapterName": ["第一章 起", "第二章 承"],
    "chapterUrl": ["c1.html", "c2.html"],
    "updateTime": ["2026-08-29", "2026-08-28"],
    "content": ["正文第一段,写了一些字。\n正文第二段,又写了一些字。\n正文第三段。"],
    "flag": ["0"],
    "num": ["1", "2"],
    # 键名启发式(见 _key_role):真实 API 的 `*_time` / `version` 是**数字**,
    # `*id` 是短标识。给它们塞 "玄幻" 这种展示串,测的就不是规则而是
    # `"玄幻"*1000` 的 NaN 行为了 ——「合成页要尽量像真页」(M2k 那一课)。
    "epoch": ["1700000000", "1700000001"],
    "id": ["1", "2"],
}

# 键名 → 角色。字段规则给的角色只说「这一格是干什么用的」,键名才说
# 「这一格里装的是什么形状」。两者冲突时以键名为准:
#   `kind: {{java.timeFormat(java.getString('$.update_time')*1000)}}`
# 里 `update_time` 是时间戳而不是分类名。
KEY_ROLE = (
    (re.compile(r"time|date|version|updated|(^|_)at$", re.I), "epoch"),
    (re.compile(r"^([A-Za-z]*id|\w*_ids?)$", re.I), "id"),
)


def _key_role(key):
    for pat, role in KEY_ROLE:
        if pat.search(key):
            return role
    return None

# 数组一律 2 项(`[n]` 要求更多时按需加,封顶 MAX_ITEMS ——
# 否则 `[12]` 这类下标会把文档撑爆)
MAX_ITEMS = 6
MAX_DEPTH = 12
# `$..[?(...)]` / `$..[*]`:递归下降到「随便哪个容器」。`..` 在任何深度都找得到,
# 所以给它一个固定的包装键就够了。
DESC_ANY = "_any"


class Arr:
    __slots__ = ("item", "minlen")

    def __init__(self, item, minlen=2):
        self.item, self.minlen = item, minlen


class Leaf:
    __slots__ = ("role", "const")

    def __init__(self, role=None, const=None):
        self.role, self.const = role, const

    def value(self, i):
        if self.role is None:
            return self.const
        vs = VALUES[self.role]
        return vs[i % len(vs)]


# ---------------------------------------------------------------- 规则串 → token

NAME = re.compile(r"[A-Za-z0-9_\-\u4e00-\u9fff]+")
CUT = ("<js>", "@js:", "@JS:", "##", "@put:", "@get:", "{{", "@@", "@css:", "@CSS:")
JSON_PREFIX = re.compile(r"^@json:", re.I)


def split_top(rule):
    """按顶层的 `||` / `&&` / `%%` 切分(方括号/圆括号/引号内不切)"""
    out, depth, quote, start = [], 0, None, 0
    i = 0
    while i < len(rule):
        c = rule[i]
        if quote:
            if c == quote:
                quote = None
        elif c in "'\"":
            quote = c
        elif c in "[(":
            depth += 1
        elif c in "])":
            depth = max(0, depth - 1)
        elif depth == 0 and rule[i:i + 2] in ("||", "&&", "%%"):
            out.append(rule[start:i])
            i += 2
            start = i
            continue
        i += 1
    out.append(rule[start:])
    return [x for x in (s.strip() for s in out) if x]


# 真身 `AnalyzeRule.splitSourceRule` 把一条规则切成**一串**子规则:
# `JS_PATTERN = <js>([\w\W]*?)</js>|@js:([\w\W]*)`,匹配到的是 JS 段,
# **段与段之间的文本各自是一条规则**,按顺序作用在上一条的结果上。
#
# 两处不一样,而生成器此前把它们混为一谈(见到 `<js>` 就把后面全丢掉):
#   - `@js:` **吃到结尾**(`[\w\W]*`),后面确实没有规则了;
#   - `<js>…</js>` 是**有界的**,`</js>` 之后那截是**下一条规则** ——
#     语料里 44 个「源×步」的入口规则长这样(`<js>eval(source.bookSourceComment)</js>`
#     这种前奏块打头,真正的选择器/JSONPath 在后面),此前一律整条丢掉。
JS_SEG = re.compile(r"<js>[\w\W]*?</js>|@js:[\w\W]*", re.I)


def rule_segment(branch):
    """规则串 → 真身切出来的**第一条非 JS 子规则**(没有就是空串)"""
    pos = 0
    for m in JS_SEG.finditer(branch):
        seg = branch[pos:m.start()].strip()
        if seg:
            return seg
        pos = m.end()
    return branch[pos:].strip()


def _cut(branch):
    s = JSON_PREFIX.sub("", rule_segment(branch).strip()).strip()
    for m in CUT:
        i = s.find(m)
        if i >= 0:
            s = s[:i]
    return s.strip()


def _match_bracket(s, i):
    depth, quote = 0, None
    while i < len(s):
        c = s[i]
        if quote:
            if c == quote:
                quote = None
        elif c in "'\"":
            quote = c
        elif c == "[":
            depth += 1
        elif c == "]":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


PRED = re.compile(r"""@\.([A-Za-z0-9_\-]+)\s*(==|!=|>=|<=|>|<)?\s*"""
                  r"""('[^']*'|"[^"]*"|-?\d+)?""")


def _pred(inner):
    """`?(@.k)` / `?(@.k==1)` / `?(@.k!=1)` / `?(@.k>3)` → 让谓词**成立**的键值。

    比较运算符要认全:`[?(@.type != 1)]` 只写个 `type` 是不够的 ——
    键缺失时 jayway 认为 `!=` **成立**(实测 `$.arr[?(@.type != 1)]` 在没有
    `type` 的项上返回全部),给它塞个 `1` 反而把这一支变成不成立。
    """
    out = {}
    for m in PRED.finditer(inner):
        key, op, lit = m.group(1), m.group(2), m.group(3)
        if op is None or lit is None:
            out[key] = Leaf(const="1")
        elif lit[0] in "'\"":
            sv = lit[1:-1]
            out[key] = Leaf(const=(sv + "_") if op == "!=" else sv)
        else:
            n = int(lit)
            out[key] = Leaf(const={"==": n, "!=": n + 1, ">": n + 1, ">=": n,
                                   "<": n - 1, "<=": n}[op])
    return out


def _bracket(inner):
    inner = inner.strip()
    if inner == "*":
        return ("wild", None)
    if inner.startswith("?"):
        return ("filter", inner)
    if re.fullmatch(r"-?\d+", inner):
        return ("idx", int(inner))
    if re.fullmatch(r"-?\d+(\s*,\s*-?\d+)+", inner):     # `[0,1]` 下标并集
        return ("idx", max(int(x) for x in inner.split(",")))
    if ":" in inner and re.fullmatch(r"[-\d:\s]*", inner):
        return ("slice", None)
    m = re.match(r"""^['"]([^'"]*)['"]""", inner)
    if m:
        return ("key", m.group(1))     # `['a','b']` 只取第一个
    return None


def tokens(path):
    """路径 → token 序列;认不出返回 None"""
    s, i, toks = path, 0, []
    if not s:
        return None
    # jayway 对不以 `$` / `@` 开头的路径补 `$.`:裸键名 `title` → `$.title`,
    # 而 `.author` → `$..author`(递归下降)。语料里两种写法都有。
    if s[0] != "$":
        s = "$." + s
    i = 1
    while i < len(s):
        if s.startswith("..", i):
            i += 2
            if i < len(s) and s[i] == "[":
                toks.append(("desc", DESC_ANY))
                continue
            m = NAME.match(s, i)
            if not m:
                return None
            toks.append(("desc", m.group()))
            i = m.end()
        elif s[i] == ".":
            i += 1
            if i >= len(s):
                return None
            if s[i] == "*":
                toks.append(("wild", None))
                i += 1
            elif s[i] == "[":
                continue
            else:
                m = NAME.match(s, i)
                if not m:
                    return None
                toks.append(("key", m.group()))
                i = m.end()
        elif s[i] == "[":
            j = _match_bracket(s, i)
            if j < 0:
                return None
            tok = _bracket(s[i + 1:j])
            if tok is None:
                return None
            toks.append(tok)
            i = j + 1
        else:
            return None
    return toks if len(toks) <= MAX_DEPTH else None


CONTAINER = ("wild", "idx", "slice", "filter")


def normalize(toks):
    """容器 token 归一成 `("cont", {"min":…, "pred":…})`。
    紧跟在容器后面的过滤器**不新起一层** —— 它筛的是当前集合。"""
    out = []
    for kind, val in toks:
        if kind not in CONTAINER:
            out.append((kind, val))
            continue
        pred = _pred(val) if kind == "filter" else {}
        minlen = min(val + 1, MAX_ITEMS) if kind == "idx" and val >= 0 else 2
        if kind == "filter" and out and out[-1][0] == "cont":
            out[-1][1]["pred"].update(pred)
            continue
        out.append(("cont", {"min": max(2, minlen), "pred": pred}))
    return out


def build_path(toks, payload, as_list):
    """token 序列 + 末端载荷 → 骨架。as_list=True 时末端一定是数组。"""
    toks = normalize(toks)
    if as_list and (not toks or toks[-1][0] != "cont"):
        toks = toks + [("cont", {"min": 2, "pred": {}})]

    def go(ts):
        if not ts:
            return payload
        kind, val = ts[0]
        if kind in ("key", "desc"):
            child = go(ts[1:])
            if isinstance(child, Leaf) and child.role is not None:
                role = _key_role(val)
                if role:
                    child = Leaf(role)
            return {val: child}
        child = go(ts[1:])
        if val["pred"] and isinstance(child, dict):
            child = dict(val["pred"], **child)     # child 优先
        return Arr(child, val["min"])

    return go(toks)


def merge(a, b):
    """合并两份骨架,**a 优先**(形态打架时保留 a,只可能少命中一支)"""
    if isinstance(a, dict) and isinstance(b, dict):
        out = dict(a)
        for k, v in b.items():
            out[k] = merge(a[k], v) if k in a else v
        return out
    if isinstance(a, Arr) and isinstance(b, Arr):
        return Arr(merge(a.item, b.item), max(a.minlen, b.minlen))
    return a


def materialize(node, i=0):
    if isinstance(node, dict):
        return {k: materialize(v, i) for k, v in node.items()}
    if isinstance(node, Arr):
        return [materialize(node.item, k) for k in range(node.minlen)]
    if isinstance(node, Leaf):
        return node.value(i)
    return node


# ---------------------------------------------------------------- 步 → 规则字段
# 字段名 → 值表里的角色。没列出来的字段不合成(合成了也只是多几个键)。
BOOK_FIELDS = {
    "name": "name", "author": "author", "kind": "kind", "intro": "intro",
    "coverUrl": "cover", "lastChapter": "last", "wordCount": "wordCount",
    "bookUrl": "bookUrl",
}
INFO_FIELDS = dict(BOOK_FIELDS, tocUrl="tocUrl")
INFO_FIELDS.pop("bookUrl", None)
TOC_FIELDS = {
    "chapterName": "chapterName", "chapterUrl": "chapterUrl",
    "updateTime": "updateTime", "isVolume": "flag", "isVip": "flag",
    "isPay": "flag",
}
# 每步:(列表规则字段, 项目底板, 字段表, 根上的字段表)
#   `info` 没有列表:有 `init` 就是「一个对象」,没有就直接相对根。
BOOK_ITEM_BASE = {
    "name": Leaf("name"), "author": Leaf("author"), "kind": Leaf("kind"),
    "intro": Leaf("intro"), "url": Leaf("bookUrl"), "cover": Leaf("cover"),
    "last": Leaf("last"), "wordCount": Leaf("wordCount"),
}
CHAPTER_ITEM_BASE = {
    "title": Leaf("chapterName"), "url": Leaf("chapterUrl"), "id": Leaf("num"),
}

STEP_SPEC = {
    "search": ("bookList", BOOK_ITEM_BASE, BOOK_FIELDS, {}),
    "explore": ("bookList", BOOK_ITEM_BASE, BOOK_FIELDS, {}),
    "toc": ("chapterList", CHAPTER_ITEM_BASE, TOC_FIELDS,
            {"nextTocUrl": "tocUrl"}),
    "info": (None, BOOK_ITEM_BASE, INFO_FIELDS, {}),
    "content": (None, None, {}, {"content": "content",
                                 "nextContentUrl": "chapterUrl"}),
}


# `{{$.x}}` / `{$.x}` 是**内嵌规则**而不是 JS:`SourceRule.makeUpRule` 里
# `isRule()` 认 `$.` / `$[` / `@` / `//` 开头的内嵌体,交回规则引擎求值
# (AnalyzeRule.kt L832)。语料里 `coverUrl: {{$.cover}}` 这类写法极多,
# 不认它就丢掉一大片字段。`java.getString('$.x')` 同理 —— 那条路径是真的会读的。
EMBED = re.compile(r"\{\{(.*?)\}\}|\{(\$[^{}]*)\}", re.S)
GETSTR = re.compile(r"""getString(?:List)?\(\s*['"](\$[^'"]*)['"]""")


# 被塞进 JS 算术 / timeFormat 的路径,真站点上一定是**数字**。给它 "2026-08-29"
# 这种展示串,测的就是 Rhino「无法将 2026-08-29 转换为 java.lang.Long」而不是规则
# ——「合成的回落页会自己造出分歧」(M2k 那一课),这类分歧要在造页时消掉。
ARITH = re.compile(r"timeFormat|Date|parseInt|parseFloat|Number\s*\(|[*/+-]\s*\d")


def _embedded(branch):
    """(内嵌路径, 角色提示) 列表"""
    out = []
    for m in EMBED.finditer(branch):
        expr = (m.group(1) or m.group(2) or "").strip()
        if expr.startswith("$") and "getString" not in expr:
            out.append((expr, "epoch" if ARITH.search(expr) else None))
    for m in GETSTR.finditer(branch):     # `{{ }}` 外的 `<js>` 块里也有
        around = branch[max(0, m.start() - 60):m.end() + 60]
        out.append((m.group(1), "epoch" if ARITH.search(around) else None))
    return out


def _paths(rule, embedded=True):
    """规则串 → 若干 (token 序列, 角色提示);`embedded` 时连内嵌规则一起收"""
    out = []
    for branch in split_top(rule):
        cands = [(_cut(branch), None)]
        if embedded:
            cands += [(_cut(e), hint) for e, hint in _embedded(branch)]
        for cand, hint in cands:
            if not cand:
                continue
            t = tokens(cand)
            if t:
                out.append((t, hint))
    return out


# ---------------------------------------------------------------- JS 对 JSON 页的期待
#
# HTML 那张页有三条「按 JS 反推」的路(`fallback_page` 的 `_bait` / `_fit_value` /
# `_js_reads`),JSON 这张一条都没有 —— 于是书源写成
#
#     content: "@js:\n JSON.parse(result).data.page.map(i => '<img src=\"'+i.image+'\"/>')"
#
# 的那一档,合成页上没有 `data.page`,`.map` 当场 TypeError,**整步死在规则之前**。
# B 层 JSON 口味的未命中里 **39 例**的 JS 里有这样一条 `JSON.parse(result).<链>`。
#
# 反推三件:
#   1. **链本身**就是路径(`$.data.page`);
#   2. **链后面那一下**说它是什么:`map/forEach/reduce/filter/join/length/[i]`
#      ⇒ 数组,`replace/split/match/trim/toString` ⇒ 字符串,再跟一个 `.key`
#      ⇒ 对象(交给下一条链去补);
#   3. **数组的项长什么样**由回调的形参说:`.map(i => … i.image …)` ⇒ 项里有 `image`。
#      认不出形参就退回这一步自己的项目底板(至少有 name/url 那几格)。
# 别名也认一层:`$ = JSON.parse(result).data;` 之后的 `$.book_id` 接着往下走 ——
# 语料里这是最常见的写法。
#
# 与别处同一条底线:造不出就不造。多造一个键只可能让别的规则**多**选中一格,
# 不可能造出假 PASS(两侧读的是同一份字节)。
_JP = r"JSON\s*\.\s*parse\s*\(\s*(?:String\s*\(\s*)?(?:result|src)\s*\)?\s*\)"
_JS_CHAIN = re.compile(_JP + r"((?:\s*\.\s*[A-Za-z_$][\w$]*)*)")
_JS_ALIAS = re.compile(r"([A-Za-z_$][\w$]*)\s*=\s*" + _JP + r"((?:\s*\.\s*[A-Za-z_$][\w$]*)*)")
_ARRAY_NEXT = ("map", "forEach", "reduce", "filter", "join", "length", "slice",
               "concat", "sort", "reverse", "some", "every", "find", "push")
_STR_NEXT = ("replace", "split", "match", "trim", "substring", "substr", "indexOf")
# `.map(i => …)` / `.map(function(i){…})` / `.forEach((i,k) => …)` 的形参
_CB_PARAM = re.compile(r"\(\s*(?:function\s*)?\(?\s*([A-Za-z_$][\w$]*)")


def _chain_keys(raw):
    return [x for x in re.split(r"\s*\.\s*", raw.strip()) if x]


def _item_from_callback(js, at, param_hint=None):
    """`.map(i => … i.image …)` → 项目骨架 `{image: Leaf(...)}`;认不出返回 None"""
    m = _CB_PARAM.match(js, at)
    if not m:
        return None
    name = m.group(1)
    body = js[m.end():m.end() + 400]
    keys = set(re.findall(r"(?<![\w$])" + re.escape(name) + r"\s*\.\s*([A-Za-z_$][\w$]*)", body))
    keys.discard("length")
    if not keys:
        return None
    return {k: Leaf(_key_role(k) or "name") for k in keys}


def _js_shapes(step, rules, item):
    """(步, 规则对象, 项目骨架) → 这一步的 JS 期待 JSON 页长成的样子(骨架)"""
    root = {}
    js = "\n".join(v for v in (rules or {}).values() if isinstance(v, str))
    if "JSON" not in js:
        return root
    def _tail_of(keys, at):
        """链末尾那个名字如果是方法名,它是**终结符**不是键(`.data.page.map`);
        紧跟 `[` 的也当数组。返回 (键, 终结符, 回调的起点)"""
        tail = None
        if keys and (keys[-1] in _ARRAY_NEXT or keys[-1] in _STR_NEXT):
            tail = keys.pop()
        elif re.match(r"\s*\[", js[at:]):
            tail = "["
        return keys, tail, at

    chains = []
    # 别名一层:`$ = JSON.parse(result).data;` 之后的 `$.k…` 接着往下走。
    # 名字可能就叫 `$`,所以边界用 `(?<![\w$])` 不用 `\b`(`\b` 在 `$` 前不成立)。
    for m in _JS_ALIAS.finditer(js):
        base = _chain_keys(m.group(2))
        use = re.compile(r"(?<![\w$])" + re.escape(m.group(1))
                         + r"((?:\s*\.\s*[A-Za-z_$][\w$]*)+)")
        for u in use.finditer(js):
            chains.append(_tail_of(base + _chain_keys(u.group(1)), u.end()))
    for m in _JS_CHAIN.finditer(js):
        chains.append(_tail_of(_chain_keys(m.group(1)), m.end()))
    # 长的先进:merge 的 a 优先,先放深的,浅的那条(叶子)就盖不掉它
    for keys, tail, at in sorted(chains, key=lambda c: -len(c[0])):
        if not keys:
            continue
        toks = tokens("$." + ".".join(keys))
        if not toks:
            continue
        if tail in _ARRAY_NEXT or tail == "[":
            payload = (_item_from_callback(js, at) if at is not None else None) \
                or (dict(item) if item else None)
            if not payload:
                continue
            root = merge(root, build_path(toks, payload, as_list=True))
        else:
            role = _key_role(keys[-1]) or ("content" if step == "content" else "name")
            root = merge(root, build_path(toks, Leaf(role), as_list=False))
    return root


def synth(step, rules):
    """(步, 该步的规则对象) → 骨架(未与底板合并)。合不出东西时返回 {}。"""
    spec = STEP_SPEC.get(step)
    if not spec or not isinstance(rules, dict):
        return {}
    list_field, item_base, fields, root_fields = spec

    def rule(name):
        v = rules.get(name)
        return v.strip() if isinstance(v, str) and v.strip() else None

    # 项目骨架:字段规则相对**列表项**求值(裁判把选中的元素当根)
    item = dict(item_base) if item_base else {}
    for fname, role in fields.items():
        r = rule(fname)
        if not r:
            continue
        for toks, hint in _paths(r):
            item = merge(build_path(toks, Leaf(hint or role), as_list=False), item)

    root = {}
    for fname, role in root_fields.items():
        r = rule(fname)
        if not r:
            continue
        for toks, hint in _paths(r):
            root = merge(build_path(toks, Leaf(hint or role), as_list=False), root)

    # JS 对这张页的期待(见 _js_shapes)。**放在规则合成之后**:规则是明写的,
    # 形态打架时它优先(merge 的 a 优先)。
    root = merge(root, _js_shapes(step, rules, item))

    if list_field:
        r = rule(list_field)
        for toks, _ in (_paths(r, embedded=False) if r else []):
            root = merge(build_path(toks, item, as_list=True), root)
    elif step == "info":
        init = rule("init")
        if init:
            for toks, _ in _paths(init, embedded=False):
                root = merge(build_path(toks, item, as_list=False), root)
        else:
            root = merge(item, root)      # 没有 init:字段直接相对根
    return root


# ---------------------------------------------------------------- 底板
# 改造前手写的那份通用形状,现在退居**底板**:源自己合成出来的结构优先,
# 合不出的源拿到的页面与改造前逐字节一致 —— 这一改只会往上抬命中面。
def _base():
    book = dict(BOOK_ITEM_BASE)
    chapter = dict(CHAPTER_ITEM_BASE)
    info = {"name": Leaf("name"), "author": Leaf("author"), "kind": Leaf("kind"),
            "intro": Leaf("intro"), "cover": Leaf("cover"),
            "tocUrl": Leaf("tocUrl"), "last": Leaf("last")}
    base = {
        "code": Leaf(const=0), "msg": Leaf(const="ok"),
        "success": Leaf(const=True), "total": Leaf(const=2),
        "content": Leaf("content"),
        "list": Arr(book), "books": Arr(book), "chapters": Arr(chapter),
        "info": info,
    }
    base["data"] = {"list": Arr(book), "books": Arr(book), "chapters": Arr(chapter),
                    "info": info, "content": Leaf("content"),
                    "total": Leaf(const=2)}
    return base


BASE = _base()


# **JSON 页上不放正则诱饵**(试过,记在这里免得下一轮又试一遍):
# HTML 那张页把样例文本缀在 `</body>` 前买到了 36 例(`fallback_page._bait`),
# JSON 这张挂不了裸文本、只能加一个根键(`__bait`)。实测**只 +1 例**,
# 而代价是**每一张有 JS 的 JSON 页都多一个根键** —— 书源要是 `for (k in obj)`
# 数根键,那就是判据自己往页面里塞进去的一个面。**不值**,故不做。
# JSON 页要抬,靠的是上面的 `_js_shapes`(按 JS 的成员链反推形状)。
def build(step, rules):
    """(步, 该步的规则对象) → JSON 文本(合成 + 底板)。
    `step=None` / `rules=None` 时只出底板。"""
    return json.dumps(materialize(merge(synth(step, rules), BASE)),
                      ensure_ascii=False)
