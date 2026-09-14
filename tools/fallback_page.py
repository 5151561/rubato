#!/usr/bin/env python3
"""把语料里真实出现的选择器**反向合成**成一张回落页面。

pipeline-corpus 用一张页面回答所有请求(docs/http-snapshot.md §6)。页面与规则
不成对时,绝大多数源只会落到「空结果 / toc_empty」分支,差分就只在错误路径上
打转。这里按语料里 `bookList` / `chapterList` / `content` 等选择器的**实际分布**,
为每条(去重后的)选择器造一个能被它选中的最小 DOM 块,拼成一张页面。

要点:
- 只认 jsoup 侧的容器写法;jsonpath / XPath / 正则 / JS 一律跳过(它们由 JSON
  回落页或分层剔除处理)。认得的形态(M2o 之前只有前两行):
  - legado 前缀:`class.x` / `id.x` / `tag.x` / `text.x` / `children`,以及它们
    带索引的样子(`tag.tr!0`、`class.a.0`、`tag.dd!0:1:2`)。**索引要先剥再认前缀**
    —— 真身 `findIndexSet` 就是这个顺序,剩下的才是 `beforeRule`。
  - CSS:`#x` / `.x` / `tag` / `[attr=v]` / **组合子 `>` `+` `~`**(带不带空格都认)
    / **jsoup 伪类** `:eq(n)` `:has(X)` `:matches(rx)` `:contains(t)` `:not(X)` …
  - **`class.` / `id.` 的名字里可以有空格**(`class.txt-list txt-list-row5`,
    语料 146 处):jsoup 的 `hasClass` 有一条等长快路,整串相等就命中。
    这条是拿 jsoup 的 jar 探出来的,不是推出来的 —— 见
    `rubato_core::host::jsoup_has_class` 的注释。
- 索引与切片后缀(`!0`、`.0`、`[1:]`、`:1:2`)一律丢掉,再把最内层元素**重复 3 份**,
  这样带索引的规则也有东西可选。
- 最内层容器装不下 payload 时(`tr` / `ul` / `dl` …)先包一层合法子元素
  (`PAYLOAD_WRAPPER`)—— 见那里的注释:少这一层,表格类规则测的就不是规则,
  是两个解析器对畸形 HTML 的分歧。
- 造页面的理解**只可能造成"选不中"**(退回空结果分支),不可能造出假 PASS ——
  两侧看到的是同一张页面,差异仍然只来自实现。
"""
import re

import rx_sample

# 需要特定父元素的标签(HTML 解析器会把放错地方的丢掉)
NEEDS_PARENT = {
    "tr": ("table", "tbody", "thead", "tfoot"),
    "td": ("tr",), "th": ("tr",),
    "tbody": ("table",), "thead": ("table",), "tfoot": ("table",),
    "dd": ("dl",), "dt": ("dl",),
    "li": ("ul", "ol"),
    "option": ("select",),
}
WRAPPER = {"tr": "tbody", "td": "tr", "th": "tr", "tbody": "table", "thead": "table",
           "tfoot": "table", "dd": "dl", "dt": "dl", "li": "ul", "option": "select"}

# `@` 链尾部的取值动作:到这里就不是容器了
ACTIONS = {
    "text", "textnodes", "owntext", "wholetext", "html", "all", "value",
    "href", "src", "content", "data", "title", "alt", "id", "class",
}

# 最内层容器**自己装不下** payload 时,先包一层它的合法子元素 —— 真实站点就是
# 这么写的(`<tr><td><a>`),而合成页此前直接写 `<tr><a>`。区别不是好看:
# `<tr>` 里的非 td/th 内容会被 HTML 解析器 **foster parenting** 挪到表外,
# 于是 `bookList: tbody tr` 这类规则选中的行永远是空的 —— 规则一步都没被测到,
# 差分反而打在两个解析器对畸形 HTML 的分歧上(机理见 html-compat/src/lib.rs
# 的头注释:jsoup 那边是它自己的 bug)。补上这一层,那批 case 才是在测规则。
# 容器 → (它能直接装的子标签, 装不下时往里补哪一层)。
# 与 NEEDS_PARENT 是同一件事的两个方向:那边管「子要什么父」(链首是 `td` 就补
# `tr`/`table`),这边管「父装不下这个子」——`tbody@tr@a` 的 `<a>` 与 payload
# 本身都落在这一档。
#
# **只列真会被解析器搬走的那一族**(table 那一支的 foster parenting)。
# `ul` / `ol` / `dl` 曾经也在这张表里,补的是「好看」而不是「解析正确」——
# `<dl><a>x</a></dl>` 在 HTML5 解析里 `<a>` 老老实实留在 `<dl>` 底下,
# 只有 table 一族会把非法子节点挪到表外。而多补那一层是**有害**的:
# 规则写 `#list dl>a`(**直接子代**)时,中间凭空多出个 `<dd>`,
# 裁判当场选不中(语料里 `dl>a` / `ul>a` 这一批)。
CHILD_OK = {
    "table": ({"tbody", "thead", "tfoot", "tr", "caption", "colgroup"}, "tbody"),
    "tbody": ({"tr"}, "tr"), "thead": ({"tr"}, "tr"), "tfoot": ({"tr"}, "tr"),
    "tr": ({"td", "th"}, "td"),
}


def _bridge(parent, child):
    """父子之间要补的标签串(补到 parent 装得下 child 为止)"""
    out = []
    while parent in CHILD_OK:
        ok, fill = CHILD_OK[parent]
        if child in ok:
            break
        parent = fill
        out.append(parent)
    return out


def _wrap_payload(tag, payload):
    # payload 是流式内容(`<h3>` / `<a>` / 文本),按 `div` 走同一张表
    for w in reversed(_bridge(tag, "div")):
        payload = f"<{w}>{payload}</{w}>"
    return payload


VOID = {"img", "br", "hr", "meta", "input", "link"}
# **内容模型会把 payload 吞掉**的标签:里面的东西不是元素而是原始文本
# (或者干脆被解析器挪走)。造页面时遇到它们仍然退回 `div`。
OPAQUE_TAGS = {"script", "style", "textarea", "title", "template", "noscript",
               "plaintext", "xmp", "iframe", "html", "head", "body"}
# 「长得像标签名」的白名单。**不再用它拦未知标签** —— jsoup 的 `getElementsByTag`
# 与选择器对自定义标签一视同仁(`data.chapter_lists` 选的是 `<data class=…>`),
# 一律拍成 `div` 只会造出裁判永远选不中的页面。留着它是因为「末段常常是半截 JS」:
# 名字必须先过 `[A-Za-z][A-Za-z0-9]*` 那道 fullmatch,再排掉 OPAQUE_TAGS。
KNOWN_TAGS = {
    "div", "span", "p", "a", "ul", "ol", "li", "dl", "dt", "dd", "table", "tbody",
    "thead", "tfoot", "tr", "td", "th", "h1", "h2", "h3", "h4", "h5", "h6", "em",
    "strong", "b", "i", "section", "article", "main", "header", "footer", "nav",
    "aside", "form", "label", "select", "option", "img", "meta", "font", "center",
    "big", "small", "u", "s", "pre", "code", "blockquote", "figure", "figcaption",
    "textarea", "button", "iframe", "body", "html", "head", "title",
}

IDX = re.compile(r"(!.*|\[[^\]]*\]|:[0-9:!\-]+|\.[0-9]+)$")


def _strip_index(name):
    prev = None
    while prev != name:
        prev = name
        name = IDX.sub("", name)
    return name


# `[k=v]` / `[k^=v]` / `[k]`:属性选择器。**不能**让 `_strip_index` 先动手
# (它的 `\[[^\]]*\]$` 会把 `[name=x]` 当索引剥掉),故先摘出来再剥索引。
ATTR_SEL = re.compile(r"""\[\s*([A-Za-z_:][\w:.\-]*)\s*"""
                      r"""(?:[~^*$|]?=\s*['"]?([^\]'"]*)['"]?)?\s*\]""")
# 尾部索引:`a.1` / `li!0` / `div[2]` / `tag.p:1:2`。真身 `ElementsSingle`
# 把这一截当**索引**而不是选择器(`beforeRule` 是它前面那段),所以
# `a.1@text` 是「第二个 a 的文本」,不是「class 为 1 的 a」——
# 按 class 造出来的那份页面,`a.1` 永远选不中。
IDX_NUM = re.compile(r"[.\[](-?\d+)|!(-?\d+)")


def _min_len(tok):
    """token 尾部的索引 → 这一层至少要造几份。

    **要照 `findIndexSet` 数全**,不能只看第一个数字:索引可以是一串
    (`tag.dd!0:1:2:3:4:5:6:7:8` —— 前九个全排除,得有 10 份才剩得下东西),
    也可以是负的(`li!-1` 排除最后一个,得有 2 份)。此前这里
    `!n → n+2` 只认单个非负数,于是:
      - `!-1` 算出 1 份 —— 造一份、再把它排掉,选中的永远是空(语料里
        `.lb li!-1@a`、`class.content@tag.p!-1@html` 这一批全卡在这);
      - `!0:1:…:8` 算出 2 份 —— 三份 `<dd>` 全在排除名单里,同样是空。
    """
    n = 1
    for pname, parg in _take_pseudos(tok)[1]:
        if pname in PSEUDO_NTH and parg.strip().lstrip("-").isdigit():
            v = int(parg.strip())
            n = max(n, v + 1 if v >= 0 else -v)
    # 被 `_strip_index` 剥掉的那一截就是索引部分,照它数
    base = ATTR_SEL.sub("", tok)
    tail = base[len(_strip_index(base)):].strip()
    nums = [int(x) for x in re.findall(r"-?\d+", tail)]
    if nums:
        need = max(v + 1 if v >= 0 else -v for v in nums)
        if tail[:1] == "!" or tail[:2] == "[!":     # 排除:这些之外还得剩一份
            need = max(need, len(nums)) + 1
        n = max(n, need)
    return min(n, 16)


# legado 的前缀写法。**整段**交给 `getElementsByClass/Tag/Id`,所以这一支
# 不能按空格切开(`class.txt-list txt-list-row5` 是一个名字,不是两级后代)。
LEGADO_PRE = ("class.", "id.", "tag.", "text.", "children")


def _seg_tokens(seg):
    """一段(两个 `@` 之间)→ [(组合子, token)]。组合子:`' '` 后代 / `'>'` 子代 /
    `'+'`、`'~'` 兄弟。括号内的分隔符不切(`:has(> a)` / `[href~=a b]`)。"""
    seg = seg.strip()
    if not seg:
        return []
    if seg.lower().startswith(LEGADO_PRE):
        return [(" ", seg)]
    out, comb, buf, depth = [], " ", "", 0
    for ch in seg:
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        elif depth == 0 and (ch.isspace() or ch in ">+~"):
            if buf:
                out.append((comb, buf))
                buf, comb = "", " "
            if ch in ">+~":
                comb = ch
            continue
        buf += ch
    if buf:
        out.append((comb, buf))
    return out


# ---- jsoup 的伪类。语料里 59 处,集中在目录/正文的「按文字锚定」那一类写法
# (`div.intro:matches(正文)~ul.chapter a`、`a:matches(下一页)@href`)。
# 认得的四档:**按文字**(matches/contains/…)→ 给节点放一段能命中的文字;
# **按序号**(eq/gt/lt/nth-*)→ 把这一层撑到足够份数;**:has(X)** → 给它造个
# 这样的孩子;其余(`:not(X)`、`:first-child`、`:root` …)一律丢掉 ——
# 丢掉只会「选不中」,不会造出假 PASS(见本文件头注释)。
PSEUDO_TEXT = {"matches", "matchesown", "contains", "containsown", "containsdata"}
PSEUDO_NTH = {"eq", "gt", "lt", "nth-child", "nth-of-type", "nth-last-child",
              "nth-last-of-type"}


def _take_pseudos(tok):
    """`div.intro:matches(正文)` → ('div.intro', [('matches', '正文')])。
    括号可嵌套(`:has(>*:has(>a))`),所以手写扫描而不是正则。

    **方括号里的 `:` 不是伪类**:`[name=og:novel:book_name]` 的属性值里就带冒号
    (语料里 `og:` 那一族 meta 有 50+ 处)。此前这里不看方括号,`:novel` /
    `:book_name` 被当成两个伪类摘走,剩下 `[name=og]` —— 合成出来的是
    `<div name="og" content="书名一">`,而书源要的是 `[name=og:novel:book_name]`,
    **永远选不中**。症状是 info 步「name 空、别的字段有值」(pb00399)。
    """
    out, buf, i, n = [], "", 0, len(tok)
    while i < n:
        if tok[i] == "[":                  # 属性选择器整段原样留着
            j, depth = i, 0
            while j < n:
                if tok[j] == "[":
                    depth += 1
                elif tok[j] == "]":
                    depth -= 1
                    if depth == 0:
                        break
                j += 1
            if j >= n:                     # 没闭合:当普通字符处理
                buf += tok[i]
                i += 1
                continue
            buf += tok[i:j + 1]
            i = j + 1
            continue
        if tok[i] == ":" and i + 1 < n and (tok[i + 1].isalpha() or tok[i + 1] == "-"):
            j = i + 1
            while j < n and (tok[j].isalnum() or tok[j] in "-_"):
                j += 1
            name = tok[i + 1:j].lower()
            arg = ""
            if j < n and tok[j] == "(":
                depth, k = 0, j
                while k < n:
                    if tok[k] == "(":
                        depth += 1
                    elif tok[k] == ")":
                        depth -= 1
                        if depth == 0:
                            break
                    k += 1
                if k >= n:                 # 括号没闭合:不是伪类,原样留着
                    buf += tok[i]
                    i += 1
                    continue
                arg = tok[j + 1:k]
                j = k + 1
            out.append((name, arg))
            i = j
            continue
        buf += tok[i]
        i += 1
    return buf, out


_META = set(r"\^$.|?*+()[]{}")


def _literal(rx):
    r"""正则参数 → 一段一定能被它匹配的字面量;挑不出来返回 ''。
    `\d、|\d\.|第|分节|番外` → `第`(取第一个不含元字符的分支)。"""
    for alt in rx.split("|"):
        alt = alt.strip()
        if alt and not (_META & set(alt)):
            return alt
    return ""


def _token_to_tag(tok):
    """一个选择器片段 → (tag, attrs) 或 None(不认)"""
    tok = tok.strip()
    if not tok:
        return None
    # **属性选择器先摘走、索引再剥,最后才认前缀** —— 后两步的顺序是真身的:
    # `ElementsSingle.findIndexSet` 在整段上逆向扫出 `!0` / `.1` / `[1:3]`,
    # **剩下的**才是 `beforeRule`,然后才 `beforeRule.split(".")` 去分派
    # class / tag / id / text。此前这里只在 CSS 那一支剥索引,于是 `tag.tr!0`
    # 走进 `tag.` 分支拿到名字 `tr!0`,`[A-Za-z0-9_\-]+` 不认 → 整条链作废
    # (语料里最大的一块:`tag.tr!0` 84 处、`tag.li!0` 30 处、
    # `tag.dd!0:1:…` 16 处,全是「表格/列表的第一行是表头,排除掉」这一种写法)。
    # 属性选择器必须**先**摘:`[property=og:image]` 长得和 `[1:3]` 一样,
    # 让 `_strip_index` 先动手会把它整个吃掉(实测 info 命中面掉 14 个点)。
    attrs = {}

    def _take(m):
        attrs[m.group(1)] = m.group(2) or "x"
        return ""

    tok, pseudos = _take_pseudos(tok)
    tok = _strip_index(ATTR_SEL.sub(_take, tok)).strip()
    if not tok and not attrs and not pseudos:
        return None
    anchor, has = "", None
    for pname, parg in pseudos:
        if pname in PSEUDO_TEXT and not anchor:
            anchor = parg if pname.startswith("contains") else _literal(parg)
        elif pname == "has" and has is None:
            has = parg
    low = tok.lower()
    # legado 前缀写法。真身 `beforeRule.split(".")` **只取 rules[1]**:
    # `class.foo.bar` 就是 getElementsByClass("foo"),后面那节被忽略。
    for pre, kind in (("class.", "class"), ("id.", "id"), ("tag.", "tag"),
                      ("text.", "text")):
        if low.startswith(pre):
            name = tok[len(pre):].split(".")[0].strip()
            if kind == "text":
                # getElementsContainingOwnText:锚的是**文本**,中文照收
                return ("div", {}, name) if name else None
            # **名字里可以有空格**(`class.txt-list txt-list-row5`,语料 146 处)。
            # 此前这里按「一个 CSS token」验,空格一律不认 —— 那是想当然:
            # 真身走的是 `getElementsByClass(rules[1])`,而 jsoup 的 `hasClass`
            # 有一条**等长快路**(1.16.2:`if (len == wantLen) return
            # className.equalsIgnoreCase(classAttr)`),于是 `class="txt-list
            # txt-list-row5"` 整串相等就命中。拿 jsoup 的 jar 探过才敢写
            # (P.java:`getElementsByClass("txt-list txt-list-row5")` → 1)。
            # `id` 同理(`Evaluator.Id` 比的是 `element.id()` 整串)。
            # `tag` 不行:`getElementsByTag("a b")` 找不到这样的标签名。
            if not name:
                return None
            if kind == "tag":
                if not re.fullmatch(r"[A-Za-z][A-Za-z0-9_\-]*", name):
                    return None
                low_name = name.lower()
                return None if low_name in OPAQUE_TAGS else (low_name, {})
            if not re.fullmatch(r"[A-Za-z0-9_\- ]+", name):
                return None
            return ("div", {kind: name})
    # CSS 写法:tag#id.class[attr=val](只接受简单形态;属性与索引在上面已摘掉)
    body = "" if tok == "*" else tok         # `*` 选得中一切,造个普通容器就行
    m = re.fullmatch(r"([A-Za-z][A-Za-z0-9]*)?((?:[#.][A-Za-z0-9_\-]+)*)", body)
    if not m:
        return None
    tag = (m.group(1) or "div").lower()
    if tag in OPAQUE_TAGS:
        tag = "div"
    classes = []
    for part in re.findall(r"[#.][A-Za-z0-9_\-]+", m.group(2) or ""):
        if part[0] == "#":
            attrs["id"] = part[1:]
        else:
            classes.append(part[1:])
    if classes:
        attrs["class"] = " ".join(classes)
    if (tag == "div" and not attrs and not m.group(1) and not anchor
            and has is None and tok != "*"):
        return None
    return (tag, attrs, anchor, has)


# ---- XPath → 本文件的 `@` 链写法 ----
#
# 生成器从第一天起就整条跳过 XPath(`sel[0] == '/'` 直接 return None),于是
# **全库 213 处** XPath 规则的源一律退回底板 —— B 层 203 例「没报错、就是空」
# 里 35 例的规则里有它。裁判那边跑的是 JsoupXpath(**在 jsoup 树上**跑的方言,
# 不是标准 XPath),所以只要造出一棵「等价 CSS 选得中」的树,它就选得中。
#
# 只认语料里真出现的那一小撮形态(93 条去重规则,头部是
# `//meta[@property='og:image']/@content`、`//*[@id="list"]//dt[2]/following-sibling::dd/a`、
# `//div[5]/div[2]/div/ul/li`、`//td[3]/text()`)。**认不出的整条返回 None**
# 退回底板 —— 与本文件其它地方同一条底线:造不出只会「选不中」,不会造出假 PASS。
#
# 谓词里有一样认不出就整条 None,不做「丢掉这个谓词凑合造」:`//div[@id='list']`
# 丢了 id 就是一个普通 `<div>`,XPath 照样选不中,白造一张页还把底板挤掉了。
_XP_FUNC_ACTION = {                       # 末段的取值函数 → `@` 链的动作
    "text": "text", "alltext": "text", "owntext": "ownText",
    "html": "html", "outerhtml": "html", "tidytext": "text",
}
_XP_AXIS = {"descendant": " ", "descendant-or-self": " ", "child": ">",
            "following-sibling": "~", "preceding-sibling": "~"}
_XP_NAME = re.compile(r"([A-Za-z_][\w\-]*|\*)")
_XP_PRED_ATTR = re.compile(r"""^@([A-Za-z_:][\w:.\-]*)\s*=\s*['"]([^'"]*)['"]$""")
_XP_PRED_CONTAINS = re.compile(
    r"""^contains\s*\(\s*@([A-Za-z_:][\w:.\-]*)\s*,\s*['"]([^'"]*)['"]\s*\)$""")
_XP_PRED_TEXT = re.compile(
    r"""^(?:contains\s*\(\s*)?text\s*\(\s*\)\s*(?:,|=)\s*['"]([^'"]*)['"]\s*\)?$""")


def _xp_split(s):
    """XPath → [(组合子, 一段)];方括号/圆括号/引号内的 `/` 不切"""
    out, i, n = [], 0, len(s)
    while i < n:
        if s[i] != "/":
            return None                    # 只认 `/` 开头的路径写法
        comb = " " if s.startswith("//", i) else ">"
        i += 2 if comb == " " else 1
        j, depth, quote = i, 0, None
        while j < n:
            c = s[j]
            if quote:
                if c == quote:
                    quote = None
            elif c in "'\"":
                quote = c
            elif c in "([":
                depth += 1
            elif c in ")]":
                depth -= 1
            elif c == "/" and depth == 0:
                break
            j += 1
        out.append((comb, s[i:j].strip()))
        i = j
    return out or None


def _xp_preds(raw):
    """一段里的谓词 → CSS 片段列表;有一个认不出就返回 None"""
    css, i, n = [], 0, len(raw)
    while i < n:
        if raw[i] != "[":
            return None                    # 名字后面只可能跟谓词
        depth, j = 0, i
        while j < n:
            if raw[j] == "[":
                depth += 1
            elif raw[j] == "]":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        if j >= n:
            return None
        p = raw[i + 1:j].strip()
        i = j + 1
        if p.isdigit() or (p.startswith("-") and p[1:].isdigit()):
            css.append(f"[{p}]")           # 位置谓词:交给 _strip_index/_min_len 撑份数
            continue
        m = _XP_PRED_ATTR.match(p)
        if m:
            css.append(f"[{m.group(1)}={m.group(2)}]")
            continue
        m = _XP_PRED_CONTAINS.match(p)
        if m:
            css.append(f"[{m.group(1)}*={m.group(2)}]")
            continue
        m = _XP_PRED_TEXT.match(p)
        if m:
            css.append(f":contains({m.group(1)})")
            continue
        if re.fullmatch(r"@[A-Za-z_:][\w:.\-]*", p):
            css.append(f"[{p[1:]}]")
            continue
        return None                        # `or` / `last()` / 嵌套路径…:整条不认
    return css


def _xpath_to_css(xp):
    """XPath → `sel1@sel2@动作` 的链写法;认不出返回 None"""
    s = re.sub(r"^@xpath:", "", xp.strip(), flags=re.I).strip()
    if not s.startswith("/") or "|" in s:  # `|` 是并集,一条链摆不下
        return None
    steps = _xp_split(s)
    if not steps:
        return None
    parts, action, last = [], None, len(steps) - 1
    for idx, (comb, raw) in enumerate(steps):
        if not raw:
            return None
        if raw.startswith("@"):            # `/@href`:取属性,只能在末段
            if idx != last or not re.fullmatch(r"@[A-Za-z_:][\w:.\-]*", raw):
                return None
            action = raw[1:]
            continue
        if "::" in raw:
            axis, raw = raw.split("::", 1)
            axis = axis.strip().lower()
            if axis == "self":
                continue
            if axis not in _XP_AXIS:
                return None
            comb, raw = _XP_AXIS[axis], raw.strip()
        fn = re.fullmatch(r"([A-Za-z]+)\s*\(\s*\)", raw)
        if fn:                             # `text()` / `allText()`:取值,只能在末段
            name = fn.group(1).lower()
            if idx != last or name not in _XP_FUNC_ACTION:
                return None
            action = _XP_FUNC_ACTION[name]
            continue
        m = _XP_NAME.match(raw)
        if not m:
            return None
        preds = _xp_preds(raw[m.end():])
        if preds is None:
            return None
        parts.append((comb, m.group(1) + "".join(preds)))
    if not parts:
        return None
    # 头一段不带组合子;后面按 `_seg_tokens` 认得的写法拼(后代用空格)
    chain = parts[0][1] + "".join(
        (" " if comb == " " else comb) + tok for comb, tok in parts[1:]
    )
    return f"{chain}@{action}" if action else chain


# 尾巴上的 JS 后处理块。真身 `splitSourceRule` 把 `<js>…</js>` / `@js:…` 切成
# **后一步**,前面那截仍然是元素规则 —— `.grid tr!0\n<js>…` 的容器就是 `.grid tr!0`。
# 此前整条丢掉,于是「先用选择器选、再用 JS 加工」这一批(语料里很常见)一个都合不出。
JS_TAIL = re.compile(r"(<js>|@js:)", re.IGNORECASE)


# `@put:{…}` 是**副作用后缀**,不是选择器的一部分 —— 真身 `SourceRule.splitPutRule`
# 拿 `@put:(\{[^}]+?\})` 把它整段从规则里 **replace 掉**,剩下的才是 `Name`。
# 此前这里见到 `@put:` 就整条不认,于是 `name: "Name@put:{id:$.Id}"` 的源
# 页面上**没有 Name 这个属性** —— 容器选中了、name 取到空串,而 legado 的
# `getSearchItem` 把没有 name 的项整条丢掉,`books` 就是空的。
# 实测:203 例「没报错、就是空」里 **40 例**的规则里有它。
# 与 `@get:{…}` 不同,那一个是**取值占位**(把变量值替换进规则串),
# 替换出来的选择器随运行时状态变,造不出来 —— 仍然整条不认。
PUT_SUFFIX = re.compile(r"@put:\{[^}]*?\}", re.IGNORECASE)


def _normalize_sel(sel):
    """规则串 → 去掉前缀与尾巴之后的选择器;不认的形态返回 None"""
    if not isinstance(sel, str):
        return None
    # `<js>…</js>` 是**有界的**,`</js>` 之后那截是下一条规则;`@js:` 才吃到结尾。
    # 口径与真身 splitSourceRule 一致,只记一处(fallback_json.rule_segment)。
    sel = rule_segment(sel).split("##")[0].strip()
    # `+` 开头是「与上一条合并」,不影响选择器本身
    sel = sel.lstrip("+").strip()
    if not sel:
        return None
    # XPath 交给 `_xpath_to_css` 翻成本文件的链写法(翻不动它自己返回 None)
    if sel[0] == "/" or sel.lower().startswith("@xpath:"):
        sel = _xpath_to_css(sel)
        if sel is None:
            return None
    elif sel[0] in "$:{":
        return None                       # jsonpath / 正则 / {{}}
    elif sel.lower().startswith("@json:"):
        return None
    if sel.startswith("@css:") or sel.startswith("@CSS:"):
        sel = sel[5:]
    if sel.startswith("@@"):
        sel = sel[2:]
    sel = PUT_SUFFIX.sub("", sel).strip()
    if not sel:
        return None
    if "@put:" in sel or "@get:" in sel or "{{" in sel:
        return None
    return sel


def parse_chain(sel):
    """选择器 → 容器标签链;不认的形态返回 None"""
    sel = _normalize_sel(sel)
    if sel is None:
        return None
    chain = []
    for seg in sel.split("@"):
        seg = seg.strip()
        if not seg:
            continue
        if seg.lower() in ACTIONS:
            break
        for comb, tok in _seg_tokens(seg):
            if comb in "+~":
                return None      # 兄弟关系:线性嵌套的这张页摆不出来(按源那张认)
            t = _token_to_tag(tok)
            if t is None:
                return None
            chain.append(t)
    return chain or None


def _render(chain, payload, repeat=3):
    """按链造 DOM,最内层重复 repeat 份,里面塞 payload"""
    def open_close(node):
        tag = node[0]
        attrs = node[1]
        text = node[2] if len(node) > 2 else ""
        a = "".join(f' {k}="{v}"' for k, v in attrs.items())
        if tag in VOID:
            return f"<{tag}{a}>", "", text
        return f"<{tag}{a}>", f"</{tag}>", text

    # 补必需的父元素
    full = []
    for node in chain:
        tag = node[0]
        need = NEEDS_PARENT.get(tag)
        if need:
            parent = full[-1][0] if full else None
            if parent not in need:
                w = WRAPPER[tag]
                # 可能要补两层(td 需要 tr,tr 需要 tbody/table)
                stack = [w]
                while True:
                    n2 = NEEDS_PARENT.get(stack[0])
                    if not n2:
                        break
                    p2 = full[-1][0] if full else None
                    if p2 in n2:
                        break
                    stack.insert(0, WRAPPER[stack[0]])
                for w2 in stack:
                    full.append((w2, {}))
        # 反方向:父装不下这个子(`tbody@tr@a` 的 `<a>`)——补上中间那层,
        # 否则 HTML 解析器会把它 foster parenting 挪到表外,这条链等于没造。
        if full:
            for w2 in _bridge(full[-1][0], tag):
                full.append((w2, {}))
        full.append(node)

    if not full:
        return ""
    inner_open, inner_close, inner_text = open_close(full[-1])
    payload = _wrap_payload(full[-1][0], payload)
    inner = "".join(inner_open + inner_text + payload + inner_close for _ in range(repeat))
    out = inner
    for node in reversed(full[:-1]):
        o, c, t = open_close(node)
        out = o + t + out + c
    return out


BOOK_PAYLOAD = (
    '<h3 class="bookname"><a class="name" href="b1.html">书名一</a></h3>'
    '<p class="author"><a href="a1.html">作者甲</a></p>'
    '<p class="kind cat">玄幻</p><p class="intro desc">简介第一句。简介第二句。</p>'
    '<p class="update last"><a href="c1.html">第 10 章 最新</a></p>'
    '<p class="wordcount">12.3万字</p>'
    '<img class="cover" src="cover1.jpg" alt="书名一">'
    '<span class="time">2026-08-29</span>'
)
CHAPTER_PAYLOAD = '<a href="c1.html" title="第一章 起">第一章 起</a><span class="time">2026-08-29</span>'
CONTENT_PAYLOAD = ('正文第一段,写了一些字。<br><p>正文第二段,又写了一些字。</p>'
                   '<p>正文第三段。</p>')

# 每张页面固定带的骨架(常见 meta / 通用类名),不依赖语料
SKELETON = """<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>书名一_作者甲_合成页</title>
<meta name="keywords" content="书名一,作者甲,玄幻">
<meta name="description" content="简介第一句。简介第二句。">
<meta property="og:novel:book_name" content="书名一">
<meta property="og:novel:author" content="作者甲">
<meta property="og:novel:status" content="连载中">
<meta property="og:novel:category" content="玄幻">
<meta property="og:novel:update_time" content="2026-08-29 12:00:00">
<meta property="og:novel:latest_chapter_name" content="第 10 章 最新">
<meta property="og:image" content="cover1.jpg">
</head><body>
%(own)s<div id="main" class="container wrap">
<div id="search" class="result-list bookList">%(book)s</div>
<div id="info" class="bookinfo detail">
 <h1 class="bookname">书名一</h1><span class="author">作者甲</span>
 <span class="kind cat">玄幻</span><span class="status">连载中</span>
 <span class="update">2026-08-29</span><span class="wordcount">12.3万字</span>
 <div id="intro" class="intro desc">简介第一句。简介第二句。简介第三句。</div>
 <a id="toclink" class="more" href="t1.html">查看目录</a>
 <img id="cover" class="cover" src="cover1.jpg" alt="书名一">
 <p class="last"><a href="c1.html">第 10 章 最新</a></p>
</div>
<div id="toc" class="tocwrap">%(chapter)s</div>
<div id="body" class="contentwrap">%(content)s</div>
<div class="page pagelist"><a href="p.html">下一页</a><a href="p.html">下一章</a></div>
</div>
<script>var chapterId = 1;</script>
%(bait)s</body></html>
"""


def build(book_selectors, chapter_selectors, content_selectors, limit=None):
    """按语料里的选择器造页面。每类**去重后全收**(limit 只在试验时用来截断)。

    此前这里截在 80 条:一层的去重链有 130~280 条,截掉的那一半意味着
    「生成器认得这条选择器、页面里却没有它」—— 命中面的最大一块缺口就在
    这里(实测 B 层 search 的 html 页 40% → 68%)。CSS 选择器位置无关,
    一张 DOM 摆得下全部形态(JSON 那边不行,故按源一张,见 fallback_json.py)。
    代价是页面从 46 KB 涨到 ~240 KB,两侧都要解析它 —— 差分套慢一截,可接受。
    """
    def blocks(sels, payload, repeat):
        seen, out = set(), []
        for sel in sels:
            chain = parse_chain(sel)
            if not chain:
                continue
            key = repr(chain)
            if key in seen:
                continue
            seen.add(key)
            out.append(_render(chain, payload, repeat))
            if limit is not None and len(out) >= limit:
                break
        return "\n".join(out)

    return SKELETON % {
        "own": "",                       # 共用的那张没有「源自己那一块」
        "book": blocks(book_selectors, BOOK_PAYLOAD, 3),
        "chapter": blocks(chapter_selectors, CHAPTER_PAYLOAD, 4),
        "content": blocks(content_selectors, CONTENT_PAYLOAD, 1),
        "bait": "",                      # 诱饵是**按源**的(见 `_bait`),底板上没有
    }


# ---------------------------------------------------------------- 按「源 × 步」反向合成
# 上面那张是**全语料共用**的一张页,它只反推了**容器**规则(bookList /
# chapterList / content);字段规则(name / author / bookUrl …)拿的是一份写死的
# payload。命中面报表把这一档的代价摆了出来:容器选中了、字段规则对不上,
# `getSearchItem` 拿不到 name 就把整条丢掉 —— 于是「html 页」那一列长期停在
# 40% 上下(实测 B 层 search:183 例是「容器认得、仍然没命中」)。
#
# 出路与 JSON 那张一样(fallback_json.py 的头注释):**把「一张页」这个约束去掉**。
# 回落页的名字是 case 自己带的,所以一个「源 × 步」可以有自己的一张,按**这个源
# 自己的字段规则**反向合成。合不出结构的源仍然拿共用的那张(底板)——
# 那张逐字节没变,所以这一改只可能往上抬命中面。
#
# 真身的两条口径(`AnalyzeByJSoup`,照着它写的):
#   - **列表规则**(`getElements`):按 `@` 切开后**每一段都是元素步**,没有动作段;
#   - **字段规则**(`getStringList`):**末段永远是动作** ——
#     text/textNodes/ownText/html/all 取文本,**其余一律 `element.attr(末段)`**
#     (所以 `bookUrl: "href"` 是「取当前元素的 href」,而 `name: "h4"` 取的是
#     名叫 h4 的属性、不是选 h4 元素)。

from fallback_json import (  # noqa: E402  值表与字段表只留一份,免得两处漂
    BOOK_FIELDS, INFO_FIELDS, TOC_FIELDS, VALUES, rule_segment, split_top,
)

# `getResultLast` 的 when 分支:只有这五个取文本,别的都是取属性
TEXT_LAST = {"text", "textnodes", "owntext", "html", "all"}

# 步 → (容器规则字段, 相对容器的字段表, 相对根的字段表)
STEP_SYNTH = {
    "search": ("bookList", BOOK_FIELDS, {}),
    "explore": ("bookList", BOOK_FIELDS, {}),
    "toc": ("chapterList", TOC_FIELDS, {"nextTocUrl": "tocUrl"}),
    # info 的容器是 `init`(`setContent(getElement(init))`);没有 init 时
    # 字段直接相对根
    "info": ("init", INFO_FIELDS, {}),
    "content": (None, {}, {"content": "content", "nextContentUrl": "chapterUrl"}),
}
LIST_STEPS = ("search", "explore", "toc")


def _esc(text):
    return text.replace("&", "&amp;").replace("<", "&lt;")


class _Node:
    """合成树的一层。`attrs` 是选择器要求的属性(参与去重),`vals` 是字段规则
    要取的属性值(不参与去重 —— 同一个元素既被 `@text` 又被 `@href` 取很常见)"""

    __slots__ = ("tag", "attrs", "anchor", "vals", "text", "repeat", "kids")

    def __init__(self, tag, attrs=None, anchor=""):
        self.tag, self.attrs, self.anchor = tag, dict(attrs or {}), anchor
        self.vals, self.text, self.repeat, self.kids = {}, "", 1, []

    @property
    def key(self):
        return (self.tag, tuple(sorted(self.attrs.items())), self.anchor)


def _descend(parent, node_t, minlen=1):
    """把一层挂到 parent 底下(同形态的合并),返回 (那一层, **它真正的父**)。

    父要连着返回:补桥之后(`tbody@tr@a` 里补出来的 `<td>`)真正的父已经不是
    传进来的那个了,而兄弟组合子(`li~li`)要挂回**同一个父**。"""
    tag = node_t[0]
    # 父装不下这个子 / 子要求特定的父:补桥(与 _render 用同两张表)
    for w in _bridge(parent.tag, tag):
        parent, _ = _descend(parent, (w, {}))
    need = NEEDS_PARENT.get(tag)
    if need and parent.tag not in need:
        stack = [WRAPPER[tag]]
        while True:
            n2 = NEEDS_PARENT.get(stack[0])
            if not n2 or parent.tag in n2:
                break
            stack.insert(0, WRAPPER[stack[0]])
        for w2 in stack:
            parent, _ = _descend(parent, (w2, {}))
    node = _Node(tag, node_t[1], node_t[2] if len(node_t) > 2 else "")
    for k in parent.kids:
        if k.key == node.key:
            node = k
            break
    else:
        parent.kids.append(node)
    node.repeat = max(node.repeat, minlen)
    return node, parent


def _emit(node):
    if not node.tag:                       # 虚根:只出孩子
        return "".join(_emit(k) for k in node.kids)
    a = "".join(f' {k}="{v}"' for k, v in list(node.attrs.items()) + list(node.vals.items()))
    if node.tag in VOID:
        return f"<{node.tag}{a}>" * node.repeat
    inner = _esc(node.anchor) + node.text + "".join(_emit(k) for k in node.kids)
    return f"<{node.tag}{a}>{inner}</{node.tag}>" * node.repeat


# CSS 的**选择器组**(`#a .x, #a .y`):jsoup 的 `select` 收它,「或」的关系。
# 造页面只要满足**第一支**就够了 —— 造全了只会让页面更大,选中的还是第一支。
# 只在**段内**切(`.list@span,li` 是两段:`.list` 和 `span,li`),不跨 `@`:
# 跨段切会把 `A@B,C` 切成 `A@B` 与 `C`,后者落到根上就是造错地方。
# 方括号/圆括号/引号里的逗号不算(`[a=b,c]`、`:matches(a,b)`)。
def _first_alt(seg):
    depth, quote = 0, None
    for i, c in enumerate(seg):
        if quote:
            if c == quote:
                quote = None
        elif c in "'\"":
            quote = c
        elif c in "[(":
            depth += 1
        elif c in "])":
            depth = max(0, depth - 1)
        elif c == "," and depth == 0:
            head = seg[:i].strip()
            return head or seg
    return seg


def _steps(sel, drop_last):
    """选择器 → [(节点, 最少份数)];`drop_last` 时末段当动作丢掉。
    认不出任何一段就整条返回 None(那一支退回底板)"""
    sel = _normalize_sel(sel)
    if sel is None:
        return None
    segs = [x for x in (t.strip() for t in sel.split("@")) if x]
    if drop_last:
        segs = segs[:-1]
    out = []
    for seg in segs:
        for comb, tok in _seg_tokens(_first_alt(seg)):
            t = _token_to_tag(tok)
            if t is None:
                return None
            # `>`(子代)与 ` `(后代)都用「直接嵌套」满足;`+`/`~`(兄弟)
            # 要挂回父层,交给 _place 处理。
            out.append((t, _min_len(tok), comb))
    return out


def _steps_css(css):
    """一小段纯 CSS(`:has()` 的参数)→ 步列表;不认返回 None"""
    out = []
    for comb, tok in _seg_tokens(css):
        t = _token_to_tag(tok)
        if t is None:
            return None
        out.append((t, _min_len(tok), comb))
    return out or None


def _place(root, steps):
    """按 [(节点, 份数, 组合子)] 走一遍树,返回落点。兄弟组合子挂回父层,
    并把紧邻的前一个兄弟撑到两份 —— `li~li` / `li+li` 要求「前面还有一个」。"""
    cur = parent = root
    for node_t, minlen, comb in steps:
        if comb in "+~" and cur is not root:
            cur.repeat = max(cur.repeat, 2)
            base = parent
        else:
            base = cur
        cur, parent = _descend(base, node_t, minlen)
        # `:has(X)`:给它造一个这样的孩子(不然这一层永远选不中)。
        # 造不出来就算了 —— 少个孩子只会「选不中」。
        want = node_t[3] if len(node_t) > 3 else None
        if want:
            sub = _steps_css(want)
            if sub:
                _place(cur, sub)
    return cur


def _action(sel):
    """字段规则的末段动作(`@text` / `@href` / …)"""
    sel = _normalize_sel(sel)
    if sel is None:
        return None
    segs = [x for x in (t.strip() for t in sel.split("@")) if x]
    return segs[-1] if segs else None


# 合法的属性名。书源里的末段常常是半截 JS(`result.replace(/(.*)/,'…')`),
# 真身照样拿它去 `element.attr(...)`(取到空串),但**造页面时不能照抄** ——
# `<div result.replace(/(.*)/,'…')="简介">` 是畸形属性名,两个解析器在那里
# 分歧(实测 pb00353 / pb02125:jsoup 与 html5ever 对属性名里的换行/引号
# 断句不同),测的就不是规则了。
ATTR_NAME = re.compile(r"[A-Za-z_:][\w:.\-]*")


# 字段规则尾巴上的 `@js:` 对**这个字段的值**的期待。
#
# 语料里最常见的一种写法(B 层未命中 93 例里 34 例):
#
#     bookUrl: "tag.a.0@href@js:result.match(/_(\\d+)/)[1]"
#
# 这里的 `result` **不是页面**,是前一截选择器取出来的那个值 —— 页尾那个
# `__bait` 诱饵(`_bait`,冲着 `src.match(…)` 去的)对它一点用没有:`match`
# 照样返回 null、`[1]` 当场 TypeError,**这一步死在字段规则上**,两侧一起抛、
# 一起「一致」,而这个字段一个字节都没被测到。
#
# 补法:诱饵放到**它真正取值的那个位置**去 —— 让合成页上那个 `href` 的值
# 本身就能被这条正则匹配上。三条约束与 `_bait` 同源:
#
# 1. **先试「原值 + 样例」**:原值(`VALUES[role][0]`)是别的规则也在读的东西,
#    直接换掉会把本来命中的字段弄空;拼在后面既保住原值又满足正则。
#    拼完仍匹配不上才退到「样例 + 原值」、最后才是「只要样例」。
# 2. **每一步都拿 `re` 验一遍**(`rx_sample.satisfies`)—— 验不过就不动这个字段,
#    宁可不注入。
# 3. **属性位上不放需要转义的字符**(`"` / `<` / `&`):`_emit` 拼属性时不转义,
#    放进去就是畸形 HTML,两个解析器在那里分歧、测的就不是规则了。
#    文本位没有这个限制(`_esc` 会转义,解析器再还原回来,正则看到的仍是原样)。
_ATTR_UNSAFE = set('"<&')


def _fit_value(branch, value, attr):
    """(这一支规则, 本来要放的值, 是不是放在属性上) → 能满足尾巴上那些正则的值"""
    m = JS_TAIL.search(branch or "")
    if not m:
        return value
    for rx in rx_sample.result_regexes(branch[m.start():]):
        if rx_sample.satisfies(rx, value):
            continue                       # 本来就匹配得上,不动
        s = rx_sample.sample(rx)
        if not s or (attr and any(ch in _ATTR_UNSAFE for ch in s)):
            continue
        for cand in (value + s, s + value, s):
            if rx_sample.satisfies(rx, cand):
                value = cand
                break
    return value


def _apply(node, action, value, branch=""):
    """把值放到该放的地方:取文本的放文本,别的放同名属性"""
    if not node.tag:                       # 虚根上放不了东西(整份文档的 text)
        return
    if action.lower() in TEXT_LAST:
        # 装不下文本的容器(`id.x@tbody@html` 这种)要先补一层:裸文本落在
        # `<tbody>` 里会被 foster parenting 挪到表外 —— 与 PAYLOAD_WRAPPER 同一课
        for w in _bridge(node.tag, "div"):
            node, _ = _descend(node, (w, {}))
        if not node.text:
            node.text = _fit_value(branch, value, attr=False)
    elif ATTR_NAME.fullmatch(action):
        node.vals.setdefault(action, _fit_value(branch, value, attr=True))


# 一步的 JS 里 `java.getString("<规则>")` 读的**那条规则**。
#
# 上面 `_fit_value` 管的是「字段规则自己取出来的值」;这一份管的是另一条取值路 ——
# 整条是 `@js:` 的容器规则里,书源常常自己再写一条规则去页面上取东西:
#
#     chapterList: "@js:\n var path=\"@@.last_page@href\";
#                    var data=java.getString(path);
#                    var page=data.match(/page=(\\d+)/)[1]; …"
#
# 合成页上没有 `.last_page` 这个元素(它不在任何**规则字段**里,只在 JS 的字符串
# 里),`data` 是空串、`match` 返回 null —— 与 `_fit_value` 那一档同一个死法。
# 页尾的诱饵也帮不上:JS 读的是**这条规则**的结果,不是页面全文。
#
# 补法:把这些规则串也当规则反向合成,并且**值要能满足随后作用在它上面的正则**。
# 两条口径照真身分开(`AnalyzeByJSoup`,与 synth 里那两张表同源):
#   - `getElement(s)` 收的是**列表规则** —— 每一段都是元素步,没有动作段;
#   - `getString(List)` 收的是**字段规则** —— 末段永远是动作。
#
# **只在整条是 `@js:` / `<js>` 的规则上认**:那时 `java.getString` 面对的
# content 就是整张页,落点在根上算得准;字段规则尾巴上的 `@js:` 里 content 是
# 当前那个元素(`getSearchItem` 的循环里),落点不一样,认了会造错地方。
# 两个实参的 `java.getString(rule, x)` 也不认(content 是第二个实参给的)。
_JS_STR = r'"((?:[^"\\\n]|\\.)*)"|\'((?:[^\'\\\n]|\\.)*)\''
_STR_ASSIGN = re.compile(r"([A-Za-z_$][\w$]*)\s*=\s*(?:" + _JS_STR + r")")
_JAVA_GET = re.compile(
    r"(?:([A-Za-z_$][\w$]*)\s*=\s*)?java\s*\.\s*(getString|getStringList|getElement|getElements)"
    r"\s*\(\s*([^),]+?)\s*\)")
# 读回来的值放什么:与 VALUES 那张表同源,按「取文本还是取链接」和这一步分。
_READ_VALUE = {
    ("toc", True): "chapterName", ("toc", False): "chapterUrl",
    ("content", True): "content", ("content", False): "chapterUrl",
}


def _js_reads(step, rules):
    """(步, 该步的规则对象) → [(规则串, 是不是列表规则, 要放的值)]"""
    out = []
    for v in (rules or {}).values():
        if not isinstance(v, str):
            continue
        for branch in split_top(v):
            m = JS_TAIL.search(branch)
            if not m or branch[:m.start()].strip():
                continue                   # 只认整条就是 JS 的
            js = branch[m.start():]
            lits = {}
            for a in _STR_ASSIGN.finditer(js):
                lits[a.group(1)] = a.group(2) if a.group(2) is not None else a.group(3)
            for g in _JAVA_GET.finditer(js):
                var, fn, arg = g.group(1), g.group(2), g.group(3).strip()
                rule = arg[1:-1] if arg[:1] in "\"'" else lits.get(arg)
                if not rule or JS_TAIL.search(rule):
                    continue
                is_list = fn in ("getElement", "getElements")
                for r in split_top(rule):
                    if is_list:
                        out.append((r, True, ""))
                        continue
                    text = (_action(r) or "").lower() in TEXT_LAST
                    val = VALUES[_READ_VALUE.get((step, text), "name" if text else "bookUrl")][0]
                    if var:                # 取回来之后拿正则匹配的,值要能匹配上
                        for rx in rx_sample.regexes_on(js, var):
                            if rx_sample.satisfies(rx, val):
                                continue
                            sm = rx_sample.sample(rx)
                            if not sm:
                                continue
                            for cand in (val + sm, sm + val, sm):
                                if rx_sample.satisfies(rx, cand):
                                    val = cand
                                    break
                    out.append((r, False, val))
    return out


def synth(step, rules):
    """(步, 该步的规则对象) → 这个源自己那一块 HTML;合不出返回 ""。"""
    spec = STEP_SYNTH.get(step)
    if not spec or not isinstance(rules, dict):
        return ""
    list_field, fields, root_fields = spec

    def rule(name):
        v = rules.get(name)
        return v.strip() if isinstance(v, str) and v.strip() else None

    root = _Node("")
    items, hit = [], False
    r = rule(list_field) if list_field else None
    # 容器规则在、却一支都合不出(最常见的是整条就是 `@js:`)—— 那时**字段规则
    # 不能造**:它们是相对容器的,落到根上就是造错地方。但 `_js_reads` 那一档
    # 仍然要造:它读的规则本来就相对整张页(见那个函数的头注释)。
    no_container = False
    if r:
        # `||` / `&&` 的每一支都造:它们是「或」的关系,全都满足只会更早命中
        for branch in split_top(r):
            st = _steps(branch, drop_last=False)
            if not st:
                continue
            n = _place(root, st)
            n.repeat = max(n.repeat, 3 if step in LIST_STEPS else 1)
            items.append(n)
            hit = True
        no_container = not items
    elif step in LIST_STEPS:
        no_container = True      # 列表步没有容器规则:字段也无从相对

    # JS 里 `java.getString/getElements("<规则>")` 读的那些规则(落点在根上,
    # 理由见 _js_reads)。**先造它们** —— 整条是 `<js>` 的容器规则里
    # `chapterList = java.getElements(".section_list li a")` 这种写法,
    # 读到的那批元素**就是**这一步的容器,字段规则要相对它们造。
    js_items = []
    for r2, is_list, val in _js_reads(step, rules):
        st = _steps(r2, drop_last=not is_list)
        if st is None:
            continue
        node = _place(root, st)
        if is_list:
            node.repeat = max(node.repeat, 3 if step in LIST_STEPS else 1)
            js_items.append(node)
        else:
            act = _action(r2)
            if not act:
                continue
            _apply(node, act, val)
        hit = True
    if no_container and js_items:
        items, no_container = js_items, False

    if not items:
        items = [root]           # info 没写 init / content:字段相对根

    if not no_container:
        for tgt, table in ((items, fields), ([root], root_fields)):
            for fname, role in table.items():
                r = rule(fname)
                if not r:
                    continue
                for branch in split_top(r):
                    st = _steps(branch, drop_last=True)
                    act = _action(branch)
                    if st is None or not act:
                        continue
                    for item in tgt:
                        _apply(_place(item, st), act, VALUES[role][0], branch)
                    hit = True
    return _emit(root) if hit else ""


# 一步的规则里那些 `<js>` / `@js:` 片段对**页面本身**的期待。
#
# 语料里大量书源是这么写目录/正文的:`chapterList: "@js:\nvar n =
# src.match(/共(\\d+)页/)[1]; …"` —— 合成页里没有「共 7 页」这样的字样,
# `match` 返回 null、`[1]` 当场 TypeError,**整步死在选择器之前**;裁判与被测侧
# 一起抛、一起「一致」,而这一步的规则一个字节都没被测到。B 层 437 例
# 「配了却没命中」里 **117 例**是这一档(见该套 README 的「命中面」段)。
#
# 补法:把那些**用来 match 的**正则抽出来(`rx_sample.regexes` 只认 match/exec/
# test/search/split 那几个位置,`.replace(/…/,'')` 那种清洗用的不算),给每条造一段
# 能被它匹配的文本,**原样(不转义)**缀在 `</body>` 前 —— 这样 `src.match(...)`
# 才有东西可匹配。造不出样例的整条丢掉(`rx_sample.sample` 生成完自己验一遍)。
#
# 为什么放在最后、为什么不转义:正则多半是冲着**标记**去的
# (`/<b>(\d+)<\/b>/`),转义了就永远匹配不上;放最后是把「诱饵自己长出来的
# 那点 DOM」对别的规则的干扰压到最小。它**不可能造出假 PASS** —— 两侧读的是
# 同一张页,差异仍然只来自实现。
def _bait(rules):
    if not isinstance(rules, dict):
        return ""
    js = "\n".join(v for v in rules.values() if isinstance(v, str))
    if "<js>" not in js.lower() and "@js:" not in js.lower():
        return ""
    out = []
    for rx in rx_sample.regexes(js):
        s = rx_sample.sample(rx)
        if s and s not in out:
            out.append(s)
    if not out:
        return ""
    return '<div id="__bait">' + "".join(out) + "</div>\n"


def build_source(step, rules):
    """(步, 该步的规则对象) → 这个源自己那一张页;合不出返回 ""(退回共用的那张)"""
    own = synth(step, rules)
    bait = _bait(rules)
    if not own and not bait:
        return ""
    # 骨架照旧(常见 meta / 通用类名那一份),但**不带**全语料的那些块 ——
    # 那些块是别的源的形态,在这里只会让页面白白大几十倍。
    return SKELETON % {"own": (own + "\n") if own else "",
                       "book": "", "chapter": "", "content": "", "bait": bait}
