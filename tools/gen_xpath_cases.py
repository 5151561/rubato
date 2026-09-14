#!/usr/bin/env python3
"""生成 xpath 差分用例(AnalyzeByXPath / JsoupXpath 2.5.5 方言)。

契约:fixtures/cases/xpath/README.md。入口:tools/xpath_diff.sh。

两部分:
1. **手写探针**——把 JsoupXpath 的语法面铺满(轴 / 节点测试 / 函数 / 运算符 /
   谓词形态 / 三个 contentType 分支),一条规则一条规则地钉方言;
2. **语料规则**——1611 源里 107 个源用了 XPath,去重 137 条规则串。
   喂给 AnalyzeByXPath 的**不是**整条规则:`##正则` / `<js>` / `@js:` 由
   AnalyzeRule 在上游切走,`{{}}` 由 makeUpRule 先求值再拼回。故这里按同样口径
   裁剪,`{{…}}` 替换成占位串 —— 见 README「语料规则怎么裁」。

输出 fixtures/cases/xpath/{01-docs,02-probe,03-corpus}.json(生成物,不入库)。
文件名带序号:case_diff.sh 按 glob 序合并,defineDoc 必须排在用它的 case 之前。
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PAGES = ROOT / "fixtures" / "pages"
SRC_DIR = ROOT / "fixtures" / "sources"
OUT_DIR = ROOT / "fixtures" / "cases" / "xpath"

RULE_KEYS = ("ruleSearch", "ruleExplore", "ruleBookInfo", "ruleToc", "ruleContent")
MODES = ("string", "stringList", "elements")

# ---------------------------------------------------------------- 探针页面
# 铺的是中文书站的真实结构:dl/dt/dd 目录、table 列表、meta 头、
# 混排文本(text() 索引)、script(allText/text 走 data())、注释与实体。
PAGE_BOOK = """<!doctype html><html><head>
<meta charset="utf-8">
<meta property="og:novel:book_name" content="测试书名">
<meta property="og:novel:author" content="测试作者">
<meta name="description" content="这是一本测试用的书。">
<title>书页</title>
</head><body>
<div id="main" class="wrap">
  <div class="bookinfo">
    <p class="title">书名一</p>
    <p>作者:<a href="/author/1">甲</a></p>
    <p><i>玄幻</i><span>连载中</span></p>
  </div>
  <div class="bookcover"><a href="/b/1"><img src="cover1.jpg" alt="封面一"></a></div>
  <div id="list">
    <dl>
      <dt>最新章节</dt>
      <dd><a href="/c/99">第 99 章 最新</a></dd>
      <dt>正文卷</dt>
      <dd><a href="/c/1">第一章 起</a></dd>
      <dd><a href="/c/2">第二章 承</a></dd>
      <dd><a href="/c/3">第三章 转</a></dd>
    </dl>
  </div>
  <table id="resultDiv">
    <tr><th>序</th><th>书名</th><th>作者</th><th>字数</th><th>状态</th><th>更新</th></tr>
    <tr><td>1</td><td><a href="/b/1">书名一</a></td><td>甲</td><td>12.3万</td><td>连载</td><td>2026-01-02</td></tr>
    <tr><td>2</td><td><a href="/b/2">书名二</a></td><td>乙</td><td>8.0万</td><td>完本</td><td>2026-01-03</td></tr>
  </table>
  <ul class="lb fk">
    <li><a class="title" href="/book/12">列一</a></li>
    <li><span>无链接</span></li>
    <li><a class="title" href="/book/34">列三</a></li>
  </ul>
  <div class="pages"><a href="?p=1">上页</a><a href="?p=3">下页</a></div>
  <p id="mix">前面<b>粗</b>中间<i>斜</i>后面</p>
  <p id="imgp"><img src="a.jpg">图注一<a href="/x">链</a>图注二</p>
  <div id="content">正文第一段。<br>正文第二段。<!-- 广告 --></div>
  <script>var chapterId = 42; var s = '<p>' + 1;</script>
  <div id="empty"></div>
  <p id="ent">a&amp;b &lt;c&gt; &nbsp;d</p>
</div>
</body></html>"""

PAGE_FRAG_TD = "<td>甲</td><td>乙</td>"
PAGE_FRAG_TR = "<tr><td>甲</td><td>乙</td></tr>"
# 属性值两头带空白的页面 —— 单独一页,免得动 PAGE_BOOK 把上面几十条探针的
# 期望一起搅了。语料里 `href="  /book/…/  "` 这种写法真实存在(塔读文学的目录页)。
PAGE_PAD = (
    '<div id="pad">'
    '<a id=" p " class=" c d " href="  /x/y  ">  \u6587  </a>'
    '<a id="q" href="\t/z\n">\u4e59</a>'
    "</div>"
)

PAGE_XML = """<?xml version="1.0" encoding="utf-8"?>
<rss><channel><item><title>标题一</title><link>/a</link></item>
<item><title>标题二</title><link>/b</link></item></channel></rss>"""

# ---------------------------------------------------------------- 探针规则
# 每组一句话说明它钉住的是什么,FAIL 时照着定位。
PROBES = [
    # --- 基本路径与名字测试
    ("//a", "descendant 全表"),
    ("//*", "通配"),
    ("/html", "绝对路径:根是 doc.children() 即 <html>"),
    ("/html/body/div", "逐级 child"),
    ("/html//a", "绝对 + 递归"),
    ("//div/p", "递归后接 child"),
    ("//div//p", "两段递归"),
    ("//DIV", "标签名大小写(jsoup 小写化 vs nodeName 比较)"),
    ("//nosuch", "选不中"),
    ("//dl/dt", "child 只看直接子元素"),
    ("//dl/*", "child 通配"),
    (".", "当前节点"),
    ("//dd/..", "父节点(HashSet 归并)"),
    ("//dl/dt/..", "单元素父节点"),
    # --- 属性
    ("//a/@href", "属性取值"),
    ("//@href", "递归属性:走 select([href]) 那条"),
    ("//img/@src", "属性"),
    ("//img/@ src", "@ 与名字之间有空格(语料里真有这写法)"),
    ("//a/@nosuch", "属性缺失给空串"),
    ("//meta[@property='og:novel:book_name']/@content", "语料最高频形状"),
    ("//attribute::href", "attribute 轴"),
    ("//div[@id]/@id", "存在性谓词 + 取属性"),
    # --- 节点测试
    ("//p/text()", "text():非递归,逐 TextNode 建 JX_TEXT"),
    ("//div//text()", "text():递归走 NodeTraversor"),
    ("//p[@id='mix']/text()", "混排文本切成多个 JX_TEXT"),
    ("//p[@id='mix']/text()[1]", "JX_TEXT 的兄弟序号谓词"),
    ("//p[@id='mix']/text()[-1]", "JX_TEXT 的负序号"),
    ("//p[img]/text()[1]", "语料写法:有 img 的 p 的第一段文本"),
    ("//script/text()", "script 的 text() 走 data()"),
    ("//div[@id='content']/allText()", "allText():整棵子树文本"),
    ("//script/allText()", "allText() 对 script 也走 data()"),
    ("//div[@class='bookinfo']/html()", "html():innerHtml"),
    ("//div[@class='bookinfo']/outerHtml()", "outerHtml()"),
    ("//td/num()", "num():抽第一个数字"),
    ("//div[@id='empty']/num()", "num() 抽不到 → null"),
    ("//div[@class='bookinfo']/node()", "node():子元素 + ownText 合成元素"),
    ("//p[@id='imgp']/node()", "node() 遇到非空 ownText:真身 new Element(\"\") 必抛"),
    ("//comment()", "comment 节点测试未注册 → 报错"),
    # --- 谓词:序号
    ("//dd[1]", "同标签序号(在当前上下文集合内计数)"),
    ("//dd[2]", "序号 2"),
    ("//dd[last()]", "last():同标签总数"),
    ("//dd[-1]", "负序号 = 倒数第一"),
    ("//dd[-2]", "负序号 = 倒数第二"),
    ("//dd[-100]", "负序号越界 → 钳到 1"),
    ("//dd[0]", "序号 0 选不中(计数从 1 起)"),
    ("//tr/td[3]", "child 后的同标签序号"),
    ("//tr[position()>1]", "position()"),
    ("//tr[position()>1 and position() < 3]", "position() 区间(语料写法)"),
    ("//li[first()]", "first() 恒为 1"),
    ("//dl/dt[last()]/following-sibling::dd", "语料最爱:最后一个 dt 之后的 dd"),
    # --- 谓词:表达式
    ("//div[@class='bookinfo']", "属性等值"),
    ('//div[@class="bookinfo"]', "双引号"),
    ("//div[@class]", "属性存在"),
    ("//div[@nosuch]", "属性不存在"),
    ("//li[a]", "子元素存在性"),
    ("//li[span]", "子元素存在性(语料写法)"),
    ("//a[text()='下页']", "text() 等值"),
    ('//a[text()="第一章 起"]', "text() 等值(中文)"),
    ("//meta[@property='og:novel:book_name' or @property='og:novel:author']", "or"),
    ("//meta[@property='og:novel:book_name' and @content='测试书名']", "and"),
    ("//div[not(@id)]", "not()"),
    ("//a[contains(@href,'/c/')]", "contains()"),
    ("//a[starts-with(@href,'/c/')]", "starts-with()"),
    ("//a[@href^='/c/']", "^= 运算符"),
    ("//a[@href$='2']", "$= 运算符"),
    ("//a[@href*='/c/']", "*= 运算符"),
    ("//a[@href~='/c/\\\\d+']", "~= 正则整串匹配"),
    ("//a[@href!~='/c/\\\\d+']", "!~ 正则不匹配"),
    ("//td[string-length(text())>3]", "string-length()"),
    ("//td[.='甲']", ". 在谓词里"),
    ("//div[count(p)>1]", "count()"),
    ("//dd[a][1]", "两个谓词串联"),
    # --- 函数(在路径外用)
    ("concat('a','b','c')", "concat"),
    ("substring('abcdef',2,3)", "substring(1-based)"),
    ("substring-ex('abcdef',2,3)", "substring-ex(0-based)"),
    ("substring-after('a/b/c','/')", "substring-after"),
    ("substring-after-last('a/b/c','/')", "substring-after-last"),
    ("substring-before('a/b/c','/')", "substring-before"),
    ("substring-before-last('a/b/c','/')", "substring-before-last"),
    ("string-length('abc')", "string-length"),
    ("count(//dd)", "count(节点集)"),
    ("not(1=1)", "not"),
    ("contains('abc','b')", "contains"),
    ("starts-with('abc','a')", "starts-with"),
    ("sum(//td)", "sum"),
    ("nosuchfunc('a')", "未注册函数 → 报错"),
    # --- 运算符与字面量
    ("1+2", "加"),
    ("3-1", "减"),
    ("2*3", "乘"),
    ("7 `div` 2", "div"),
    ("7 `mod` 2", "mod"),
    ("-3", "一元负号"),
    ("'abc'", "字面量"),
    ("1=1", "等值"),
    ("1!=1", "不等"),
    ("1<2", "小于"),
    ("2>=2", "大于等于"),
    ("'a'|'b'", "union:两个串"),
    ("//dd|//dt", "union:两个节点集"),
    ("//dd|'x'", "union:节点集 + 串(合成 <V>)"),
    ("(1+2)*3", "括号"),
    # --- 轴
    ("//dt/child::dd", "child 轴(dt 无 dd 子)"),
    ("//dl/child::dt", "child 轴"),
    ("//dd/parent::dl", "parent 轴"),
    ("//dd/self::dd", "self 轴"),
    # descendant / descendant-or-self 用 HashSet<Element> 归并,迭代序由
    # identity hashCode 决定 —— 命中 **多于一个**元素时裁判自己都不可复现
    # (实测同一份 case 两次跑给不同顺序)。故这里只探**单命中**形态。
    ("//div[@class='bookcover']/descendant::img", "descendant 轴(单命中)"),
    ("//dl/descendant-or-self::dl", "descendant-or-self 轴(单命中)"),
    ("//dd/ancestor::div", "ancestor 轴"),
    ("//dd/ancestor-or-self::dd", "ancestor-or-self 轴"),
    ("//dt/following-sibling::dd", "following-sibling"),
    ("//dt/following-sibling::*", "following-sibling:文本节点包成 <text> 合成元素"),
    ("//dt/following-sibling::text()", "轴 + text() 节点测试"),
    ("//dd/preceding-sibling::dt", "preceding-sibling"),
    ("//dt/following-sibling-one::dd", "following-sibling-one"),
    ("//dd/preceding-sibling-one::dt", "preceding-sibling-one"),
    ("//dt/following::a", "following 轴"),
    ("//dd/preceding::dt", "preceding 轴"),
    ("//dd/sibling::dt", "未注册轴 → 报错"),
    ("//div[@id='list']//dt[2]/following-sibling::dd/a", "语料第一名(14 源)"),
    # --- 切分(AnalyzeByXPath 自己的 &&/||/%% 面)
    ("//td[3]/text()&&//td[4]/text()", "&& 合并"),
    ("//nosuch||//td[3]/text()", "|| 短路"),
    ("//td[3]/text()||//td[4]/text()", "|| 首个非空即停"),
    ("//td[3]/text()%%//td[4]/text()", "%% 交叉"),
    ("//nosuch&&//td[4]/text()", "&& 有一段空"),
    # --- 语法错误面
    ("//", "只有 //"),
    ("//[", "括号未闭合"),
    ("//a[", "谓词未闭合"),
    ("//a[]", "空谓词"),
    ("/api/book/X1", "语料里的相对 URL 被当成 XPath 的样子"),
    ("/getContent?bookid=X1&chapterid=X2", "带 ? & 的 URL 形"),
    ("", "空规则"),
    ("   ", "空白规则"),
]


def load_sources():
    seen = {}
    for p in sorted(SRC_DIR.glob("*.json")):
        data = json.loads(p.read_text(encoding="utf-8"))
        if isinstance(data, dict):
            data = [data]
        for s in data:
            if isinstance(s, dict) and s.get("bookSourceUrl"):
                seen[s["bookSourceUrl"]] = s
    return list(seen.values())


def rule_strings(src):
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


EVAL = re.compile(r"\{\{.*?\}\}|\{\$\..*?\}", re.S)
JS_CUT = re.compile(r"<js>|@js:", re.I)


def to_xpath_expr(rule):
    """按 AnalyzeRule 的上游口径裁剪出真正交给 AnalyzeByXPath 的那截。

    - `##正则` 由 replaceRegex 处理,不进 XPath;
    - `<js>` / `@js:` 由 splitSourceRule 切成独立 SourceRule,不进 XPath;
    - `{{…}}` 由 makeUpRule 先求值再拼回 —— 这里换成占位串,形状保真、取值不保真;
    - `@XPath:` 前缀由 SourceRule.init 剥掉。
    """
    r = JS_CUT.split(rule)[0]
    r = r.split("##")[0]
    n = [0]

    def sub(_m):
        n[0] += 1
        return f"X{n[0]}"

    r = EVAL.sub(sub, r).strip()
    if not r:
        return None
    low = r.lower()
    if low.startswith("@xpath:"):
        r = r[7:]
    elif not r.startswith("/"):
        return None
    return r or None


def main():
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    docs = {"book": PAGE_BOOK}
    for p in sorted(PAGES.glob("*.html")):
        docs[p.stem] = p.read_text(encoding="utf-8")

    # ---- 探针
    probe = []
    n = 0

    def add(lst, prefix, **kw):
        nonlocal n
        n += 1
        lst.append({"id": f"{prefix}{n:05d}", **kw})

    for rule, note in PROBES:
        for mode in MODES:
            add(probe, "xp", op="xpathDsl", doc="book", rule=rule, mode=mode, note=note)
    # 三个 contentType 分支 × 一小撮规则
    for rule in ("//a", "//td", "//dd[1]", "//a/@href", "//text()"):
        for ct, extra in (
            ("document", {}),
            ("element", {"at": "#main"}),
            ("element", {"at": "table"}),
        ):
            for mode in MODES:
                add(probe, "xp", op="xpathDsl", doc="book", rule=rule, mode=mode,
                    contentType=ct, note=f"contentType={ct}", **extra)
    # strToJXDocument 的三个包裹分支(只有 contentType=string 才走)
    for html, note in (
        (PAGE_FRAG_TD, "以 </td> 结尾 → 包 <tr> 再包 <table>"),
        (PAGE_FRAG_TR, "以 </tr> 结尾 → 包 <table>"),
        (PAGE_XML, "以 <?xml 开头 → xmlParser"),
    ):
        for rule in ("//td", "//td[1]", "//item/title", "//title/text()", "//*"):
            for mode in MODES:
                add(probe, "xp", op="xpathDsl", html=html, rule=rule, mode=mode, note=note)
    # 属性值的 trim:`JXDocument.selN` 只有 **isString() 那一支**走
    # `XValue.asString()`(兜底是 `String.valueOf(value).trim()`),isList() 那支
    # 是逐项 `JXNode.create(item)`,**不 trim**。于是同一条 `@href`,
    # 命中一个元素时两头被削,命中两个时原样 —— 这个岔口得钉住。
    # 谓词里的比较用的是**原值**(`[@id="p"]` 选不中 `id=" p "`)。
    for rule, note in (
        ('//a[@id="q"]/@href', "单命中 → isString() → trim(Java trim:c <= ' ')"),
        ("//a/@href", "多命中 → isList() → 逐项不 trim"),
        ("//@href", "递归属性恒走 isList() → 不 trim"),
        ('//a[@id=" p "]/@href', "谓词比原值:带空白才选得中"),
        ('//a[@id="p"]/@href', "谓词比原值:trim 过的写法选不中"),
        ('//a[@class=" c d "]/@class', "class 同理"),
        ('//a[@id^="q"]/@id', "运算符谓词 + 单命中取属性"),
        ("//a/text()", "文本节点不受这条影响(jsoup 侧已 trim)"),
    ):
        for mode in MODES:
            add(probe, "xp", op="xpathDsl", html=PAGE_PAD, rule=rule, mode=mode,
                contentType="document", note=note)

    # 其余真实页面上跑一小撮通用规则(结构与合成页不同,能照出别的分支)
    for name in sorted(docs):
        if name == "book":
            continue
        for rule in ("//a/@href", "//p/text()", "//li[1]", "//div//text()",
                     "//table//td[2]", "//h1/allText()", "//*[@id]/@id"):
            for mode in MODES:
                add(probe, "xp", op="xpathDsl", doc=name, rule=rule, mode=mode,
                    note=f"真实页 {name}")

    # ---- 语料
    corpus = []
    exprs = {}
    for src in load_sources():
        for r in rule_strings(src):
            e = to_xpath_expr(r)
            if e:
                exprs.setdefault(e, r)
    n = 0
    for i, e in enumerate(sorted(exprs)):
        for doc in ("book", "gutenberg", "messy"):
            for mode in MODES:
                add(corpus, "xc", op="xpathDsl", doc=doc, rule=e, mode=mode)

    defs = [{"id": f"xpdef-{name}", "op": "defineDoc", "name": name, "html": html}
            for name, html in sorted(docs.items())]
    for stale in ("probe.json", "corpus.json"):
        (OUT_DIR / stale).unlink(missing_ok=True)
    (OUT_DIR / "01-docs.json").write_text(
        json.dumps(defs, ensure_ascii=False, indent=1), encoding="utf-8")
    (OUT_DIR / "02-probe.json").write_text(
        json.dumps(probe, ensure_ascii=False, indent=1), encoding="utf-8")
    (OUT_DIR / "03-corpus.json").write_text(
        json.dumps(corpus, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"探针 {len(probe)} 例;语料 {len(exprs)} 条规则 → {len(corpus)} 例",
          file=sys.stderr)


if __name__ == "__main__":
    main()
