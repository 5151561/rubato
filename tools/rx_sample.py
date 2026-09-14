#!/usr/bin/env python3
"""正则字面量 → **一段能被它匹配的样例文本**。

为什么要有这个(`fixtures/cases/pipeline-corpus-b/README.md`「命中面」段的
第二行):回落页至今只按**选择器**反推,没按**JS 对页面的期待**反推。语料里
大量书源的目录/正文是这么写的 ——

    chapterList: "@js:\\nvar page = src.match(/<b>(\\d+)<\\/b>/)[1]; …"

合成页里没有 `<b>7</b>` 这样的东西,`match` 返回 null,`[1]` 当场 TypeError,
**整步死在选择器之前** —— 裁判与被测侧一起抛、一起「一致」,而这一步的规则
一个字节都没被测到。B 层 437 例「配了却没命中」里有 **117 例**是这一档
(其中 72 例的正则直接作用在 `result` / `src` 上,也就是页面本身)。

补法:把 JS 里那些**用来 match 的**正则字面量抽出来,给每条造一段能匹配的文本,
原样(不转义)缀在页面末尾。这样 `src.match(...)` 就有东西可匹配。

三条约束,都是为了「造页面只可能造成选不中,不可能造出假 PASS」这条底线:

1. **生成完一定拿 `re` 自己验一遍**(`_verify`)—— 下面对 JS 方言的处理都是近似,
   验不过就整条丢掉,宁可不注入。
2. **只认能确定生成的结构**:环视、反向引用、`\\p{…}`、条件组一律返回 None。
   宁可少注入,不注入一段其实匹配不上的垃圾。
3. 生成的是**确定性**文本(同一条正则永远同一段字节)—— 回落页按内容哈希去重,
   随机化会让每次生成的页名都变,`check_generated.sh` 当场红。
"""
import re
import re._parser as sre

# 各字符类挑哪个字符。挑法不是随手定的:
# - 数字给 `3`:语料里这些正则抓的多半是**页数 / 章节数**(`共(\d+)页`),
#   而抓到之后书源会 `for(i=1;i<=n;i++)` 造列表。给 `0` 会造出空列表(还是没命中),
#   给 `9` 会让页面白白多九份。`3` 与回落页「最内层重复 3 份」是同一个约定。
# - 字母给 `a`、空白给半角空格:最不容易撞上别的规则。
DIGIT = "3"
WORD = "a"
SPACE = " "
ANY = "x"

# 生成时不往里走的结构:走了也保证不了「生成出来的真能匹配」。
_BAIL = {sre.GROUPREF, sre.GROUPREF_EXISTS, sre.ASSERT, sre.ASSERT_NOT, sre.ATOMIC_GROUP}

# 一条正则最多生成多长 —— 防住 `(\d{1,1000})` 这种把页面撑爆的写法
MAX_LEN = 200


def _pick_in(items, negate):
    """字符集 `[...]` 挑一个字符;挑不出返回 None"""
    if negate:
        # 反字符集:挑一个**不在**集合里的。按 ANY/WORD/DIGIT/空格的顺序试。
        for cand in (ANY, WORD, DIGIT, SPACE, "-", "|"):
            if not _in_set(items, cand):
                return cand
        return None
    for op, av in items:
        if op is sre.LITERAL:
            return chr(av)
        if op is sre.RANGE:
            return chr(av[0])
        if op is sre.CATEGORY:
            return _category(av)
    return None


def _in_set(items, ch):
    for op, av in items:
        if op is sre.LITERAL and chr(av) == ch:
            return True
        if op is sre.RANGE and av[0] <= ord(ch) <= av[1]:
            return True
        if op is sre.CATEGORY and _category(av) is not None:
            cat = str(av)
            if "DIGIT" in cat and ch.isdigit():
                return True
            if "WORD" in cat and (ch.isalnum() or ch == "_"):
                return True
            if "SPACE" in cat and ch.isspace():
                return True
    return False


def _category(av):
    cat = str(av)
    if "NOT" in cat:                       # \D \W \S:挑一个不在那一类里的
        if "DIGIT" in cat:
            return WORD
        if "WORD" in cat:
            return "-"
        if "SPACE" in cat:
            return WORD
        return None
    if "DIGIT" in cat:
        return DIGIT
    if "WORD" in cat:
        return WORD
    if "SPACE" in cat:
        return SPACE
    return None


def _gen(seq):
    """解析树 → 样例文本;遇到生成不了的结构抛 _Bail"""
    out = []
    for op, av in seq:
        if op is sre.LITERAL:
            out.append(chr(av))
        elif op is sre.NOT_LITERAL:
            out.append(ANY if chr(av) != ANY else WORD)
        elif op is sre.ANY:
            out.append(ANY)
        elif op is sre.IN:
            neg = bool(av) and av[0][0] is sre.NEGATE
            ch = _pick_in(av[1:] if neg else av, neg)
            if ch is None:
                raise _Bail()
            out.append(ch)
        elif op in (sre.MAX_REPEAT, sre.MIN_REPEAT):
            lo, hi, item = av
            # 至少一次 —— `(\d+)` 生成空串就白造了;`*`/`?` 也给一次,
            # 多出来的那一次照样匹配得上(验证那一步会兜住)。
            n = max(lo, 1) if hi >= 1 else lo
            body = _gen(item)
            if n * len(body) > MAX_LEN:
                n = max(lo, 1)
            out.append(body * n)
        elif op is sre.SUBPATTERN:
            out.append(_gen(av[3]))
        elif op is sre.BRANCH:
            for alt in av[1]:              # 挑第一支生成得出来的
                try:
                    out.append(_gen(alt))
                    break
                except _Bail:
                    continue
            else:
                raise _Bail()
        elif op is sre.AT:                 # ^ $ \b:不产生字符
            continue
        elif op in _BAIL:
            raise _Bail()
        else:
            raise _Bail()
        if sum(len(x) for x in out) > MAX_LEN:
            raise _Bail()
    return "".join(out)


class _Bail(Exception):
    pass


# JS 正则里 Python 不认或含义不同的几处
# 替换串一律用函数:`[\s\S]` 直接当模板会被 `re.sub` 当转义读掉。
_JS_FIXES = (
    (re.compile(r"\(\?<([A-Za-z_]\w*)>"), lambda m: f"(?P<{m.group(1)}>"),   # 具名组
    (re.compile(r"\[\^\]"), lambda m: r"[\s\S]"),                            # JS 的「任意字符」
)


def _to_python(rx):
    """JS 正则源 → Python 能解析的;认不了返回 None"""
    if "\\p{" in rx or "\\P{" in rx:       # Unicode 属性转义:Python re 不支持
        return None
    if "(?<=" in rx or "(?<!" in rx or "(?=" in rx or "(?!" in rx:
        return None                        # 环视:生成不出可靠样例(见文件头第 2 条)
    for pat, rep in _JS_FIXES:
        rx = pat.sub(rep, rx)
    return rx


def sample(rx):
    """JS 正则字面量的**源**(不含两侧的 `/` 与 flags)→ 样例文本;造不出返回 None。

    生成完一定验一遍 —— 上面对 JS 方言的处理都是近似,验不过宁可不注入。
    """
    py = _to_python(rx)
    if py is None:
        return None
    try:
        tree = sre.parse(py)
    except Exception:
        return None
    try:
        s = _gen(tree)
    except (_Bail, RecursionError):
        return None
    if not s or len(s) > MAX_LEN:
        return None
    return s if _verify(py, s) else None


def _verify(py, s):
    try:
        return re.search(py, s) is not None
    except Exception:
        return False


# ---- 从 JS 源码里把「用来 match 的」正则字面量抽出来 ----
#
# 只要 match/exec/test/search/split 那几个位置的:`.replace(/…/, '')` 这一档是
# **清洗**,它匹配不上完全没关系,给它造样例只会往页面里塞垃圾。
_RX_LIT = r"/((?:[^/\\\n\[]|\\.|\[(?:[^\]\\]|\\.)*\])+)/[gimsuyd]*"
_USED = (
    re.compile(r"\.\s*(?:match|matchAll|search|split)\s*\(\s*" + _RX_LIT),
    re.compile(_RX_LIT + r"\s*\.\s*(?:exec|test)\s*\("),
)


# 正则先存进变量再用的写法(语料里 4 处):`var re=/第(\d+)页/; src.match(re)`。
# 只认「同一段 JS 里赋过一次正则字面量」的名字 —— 赋两次就不认(取哪一次都可能错)。
_RX_ASSIGN = re.compile(r"(?:var|let|const)?\s*([A-Za-z_$][\w$]*)\s*=\s*" + _RX_LIT)
# 变量名出现在 match 家族的实参位上
_USED_VAR = re.compile(r"\.\s*(?:match|matchAll|search|split)\s*\(\s*([A-Za-z_$][\w$]*)\s*\)")


def _rx_vars(js):
    """名字 → 它被赋的那条正则源码;同名赋过两次的丢掉"""
    seen, dup = {}, set()
    for m in _RX_ASSIGN.finditer(js):
        name = m.group(1)
        if name in seen and seen[name] != m.group(2):
            dup.add(name)
        seen[name] = m.group(2)
    return {k: v for k, v in seen.items() if k not in dup}


def regexes(js):
    """JS 源码 → 里面**用来 match 的**正则字面量源码(去重、保序)"""
    out = []
    for pat in _USED:
        for m in pat.finditer(js):
            rx = m.group(1)
            if rx not in out:
                out.append(rx)
    names = _rx_vars(js)
    for m in _USED_VAR.finditer(js):
        rx = names.get(m.group(1))
        if rx and rx not in out:
            out.append(rx)
    return out


# ---- 「这一条**字段规则**取出来的值」上的正则 ----
#
# 上面那一份是给**页面本身**造诱饵的(`src.match(…)`,缀在 `</body>` 前)。
# 但语料里更常见的是另一种:字段规则的尾巴上挂 `@js:` ——
#
#     bookUrl: "tag.a.0@href@js:result.match(/_(\d+)/)[1]"
#
# 这里的 `result` **不是页面**,是前一截选择器(`tag.a.0@href`)取出来的那个值。
# 页尾的诱饵对它一点用没有:`match` 照样 null、`[1]` 当场 TypeError,
# **这一步死在字段规则上**。诱饵要放到**它真正取值的那个位置**去 ——
# 也就是合成页上那个 `href` 的值里(`fallback_page._fit_value`)。
def _recv(name):
    n = re.escape(name)
    return r"(?:String\s*\(\s*" + n + r"\s*\)|\b" + n + r"\b)"


def regexes_on(js, name):
    """JS 片段 → 其中**作用在这个名字上**的 match 家族正则字面量(去重、保序)。

    `String(x).match(…)` 与 `x.match(…)` 都算(书源里两种写法一样多)。"""
    out = []
    head = _recv(name) + r"\s*\.\s*(?:match|matchAll|search|split)\s*\(\s*"
    for m in re.finditer(head + _RX_LIT, js):
        if m.group(1) not in out:
            out.append(m.group(1))
    names = _rx_vars(js)
    for m in re.finditer(head + r"([A-Za-z_$][\w$]*)\s*\)", js):
        rx = names.get(m.group(1))
        if rx and rx not in out:
            out.append(rx)
    return out


def result_regexes(js):
    """JS 片段 → 其中**作用在 `result` 上**的正则字面量源码(去重、保序)"""
    return regexes_on(js, "result")


def satisfies(rx, text):
    """这段文本能不能被这条正则匹配上;认不了这条正则一律 False(宁可不注入)"""
    py = _to_python(rx)
    return False if py is None else _verify(py, text)


if __name__ == "__main__":                 # 手工探一条:rx_sample.py '<b>(\d+)</b>'
    import sys
    for a in sys.argv[1:]:
        print(repr(a), "->", repr(sample(a)))
