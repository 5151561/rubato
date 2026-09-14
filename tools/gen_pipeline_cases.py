#!/usr/bin/env python3
"""生成 pipeline(WebBook 四步)差分用例 + 配套 HTTP 快照。

契约:fixtures/cases/pipeline/README.md;快照 key 算法 docs/http-snapshot.md。

用例是**手工合成**的:书源规则与页面成对设计,一条轴一条轴地铺
(列表/详情/目录/正文 × jsoup / jsonpath / 正则 × 翻页 / 变量 / 净化)。
快照落在 fixtures/http-pipeline/(与 fetch 套的 fixtures/http/ 分开,
两个生成器各自清空自己的根)。
"""
import copy
import json
import os
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from http_snapshot import write_snapshot  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "fixtures" / "cases" / "pipeline"
HTTP = ROOT / "fixtures" / "http-pipeline"

UA = "rubato-judge"
HTML_UTF8 = {"content-type": "text/html; charset=utf-8"}
JSON_UTF8 = {"content-type": "application/json; charset=utf-8"}

cases = []
_seq = 0


def case(**kw):
    global _seq
    _seq += 1
    c = {"id": f"pl{_seq:04d}", "op": "pipeline"}
    c.update(kw)
    cases.append(c)
    return c


def deep_merge(dst, over):
    for k, v in over.items():
        if isinstance(v, dict) and isinstance(dst.get(k), dict):
            deep_merge(dst[k], v)
        else:
            dst[k] = v
    return dst


def variant(base, **over):
    """在基准书源上改一条轴,返回书源 JSON 串。"""
    return json.dumps(deep_merge(copy.deepcopy(base), over), ensure_ascii=False)


def page(url, body, headers=None, method="GET", status=200):
    """落一份 GET 快照(请求头就是两侧都会发的那份:默认 UA)。"""
    write_snapshot(
        HTTP,
        method=method,
        url=url,
        headers={"user-agent": UA},
        status=status,
        response_headers=headers or HTML_UTF8,
        response_body=body.encode("utf-8") if isinstance(body, str) else body,
    )


# ---------------------------------------------------------------- HTML 书源

H = "http://h.example.com"

HTML_SOURCE = {
    "bookSourceUrl": H,
    "bookSourceName": "HTML 源",
    "bookSourceType": 0,
    "enabled": True,
    "searchUrl": "/s?q={{#key}}&p={{#page}}",
    "exploreUrl": "分类::/e/{{#page}}.html",
    "ruleSearch": {
        "bookList": ".book",
        "name": "h3@text",
        "author": ".author@text",
        "kind": ".kind@text",
        "intro": ".intro@text",
        "lastChapter": ".last@text",
        "wordCount": ".wc@text",
        "coverUrl": "img@src",
        "bookUrl": "a@href",
    },
    "ruleExplore": {
        "bookList": ".ebook",
        "name": "h3@text",
        "author": ".author@text",
        "bookUrl": "a@href",
    },
    "ruleBookInfo": {
        "init": ".info",
        "name": "h1@text",
        "author": ".author@text",
        "intro": ".intro@text",
        "kind": ".kind@text",
        "lastChapter": ".last@text",
        "updateTime": ".up@text",
        "coverUrl": "img@src",
        "tocUrl": "#toc@href",
        "wordCount": ".wc@text",
    },
    "ruleToc": {
        "chapterList": "#list li",
        "chapterName": "a@text",
        "chapterUrl": "a@href",
        "nextTocUrl": "#nexttoc@href",
    },
    "ruleContent": {
        "content": "#content@html",
        "nextContentUrl": "#nextpage@href",
    },
}

SEARCH_HTML = """<html><body>
<div class="book">
  <a href="/book/1.html"><h3>书名一</h3></a>
  <span class="author">作者甲</span><span class="kind">玄幻</span>
  <span class="intro">简介一   带  空白</span><span class="last">第 10 章</span>
  <span class="wc">120000</span><img src="/cover/1.jpg">
</div>
<div class="book">
  <a href="book/2.html"><h3>  书名二  </h3></a>
  <span class="author">作者乙</span><span class="kind">都市</span>
  <span class="intro"><b>简介二</b></span><span class="last">第 2 章</span>
  <span class="wc">8000</span><img src="http://cdn.example.org/c2.jpg">
</div>
</body></html>"""

EXPLORE_HTML = """<html><body>
<div class="ebook"><a href="/book/3.html"><h3>发现书三</h3></a>
  <span class="author">作者丙</span></div>
</body></html>"""

INFO_HTML = """<html><body><div class="info">
<h1>书名一(详情)</h1>
<span class="author">作者:作者甲</span>
<span class="kind">玄幻,连载</span>
<span class="intro">详情简介<br>第二行</span>
<span class="last">第 10 章 最新</span>
<span class="up">2026-08-01</span>
<span class="wc">123456</span>
<img src="/cover/1.jpg">
<a id="toc" href="/book/1/toc.html">目录</a>
</div></body></html>"""

TOC_HTML_1 = """<html><body><ul id="list">
<li><a href="/book/1/1.html">第一章</a></li>
<li><a href="/book/1/2.html">第二章</a></li>
</ul><a id="nexttoc" href="/book/1/toc2.html">下一页</a></body></html>"""

TOC_HTML_2 = """<html><body><ul id="list">
<li><a href="/book/1/3.html">第三章</a></li>
</ul></body></html>"""

CONTENT_HTML_1 = """<html><body><div id="content">
正文第一页<br>第二行&nbsp;带实体
</div><a id="nextpage" href="/book/1/1_2.html">下一页</a></body></html>"""

CONTENT_HTML_2 = """<html><body><div id="content">
正文第二页
</div></body></html>"""


# ---- 列表步各轴用的页面 ----

FORMAT_HTML = """<html><body>
<div class="book">
  <a href="/book/f1.html"><h3>《书名带书名号》(TXT下载)</h3></a>
  <span class="author">作者:  作者甲  著</span><span class="kind">玄幻</span>
  <span class="intro">简介</span><span class="last">第 1 章</span>
  <span class="wc">1000</span><img src="/c.jpg">
</div>
</body></html>"""

WORDCOUNT_HTML = """<html><body>
<div class="book"><a href="/book/w1.html"><h3>字数一</h3></a>
  <span class="wc">12345</span></div>
<div class="book"><a href="/book/w2.html"><h3>字数二</h3></a>
  <span class="wc">123456</span></div>
<div class="book"><a href="/book/w3.html"><h3>字数三</h3></a>
  <span class="wc">约 12 万字</span></div>
<div class="book"><a href="/book/w4.html"><h3>字数四</h3></a>
  <span class="wc"></span></div>
<div class="book"><a href="/book/w5.html"><h3>字数五</h3></a>
  <span class="wc">-8</span></div>
</body></html>"""

INTRO_HTML = """<html><body>
<div class="book"><a href="/book/i1.html"><h3>简介书</h3></a>
  <div class="intro"><p>第一段</p><p>第二段</p><br>尾巴&nbsp;&amp;&lt;
  <img src="/x.jpg">多   空白</div></div>
</body></html>"""

# ---- 详情步各轴用的页面 ----

USEHTML_INFO = """<html><body><div class="info">
<h1>usehtml 书</h1><span class="author">作者甲</span>
<div class="intro">&lt;usehtml&gt;<b>原样保留</b></div>
<a id="toc" href="/book/1/toc.html">目录</a>
</div></body></html>"""

KINDS_INFO = """<html><body><div class="info">
<h1>多分类书</h1><span class="author">作者甲</span>
<span class="kind">玄幻</span><span class="kind">连载</span><span class="kind">爽文</span>
<a id="toc" href="/book/1/toc.html">目录</a>
</div></body></html>"""

# ---- 目录步各轴用的页面 ----

TOC_MULTI = """<html><body><ul id="list">
<li><a href="/book/1/1.html">多页第一章</a></li>
</ul>
<a id="nexts" href="/tv/p2.html">2</a><a id="nexts" href="/tv/p3.html">3</a>
</body></html>"""

TOC_P2 = """<html><body><ul id="list">
<li><a href="/book/1/2.html">多页第二章</a></li></ul></body></html>"""

TOC_P3 = """<html><body><ul id="list">
<li><a href="/book/1/3.html">多页第三章</a></li></ul></body></html>"""

TOC_FLAGS = """<html><body><ul id="list">
<li><a href="/book/1/v1.html">第一卷</a>
    <span class="vol">true</span><span class="info">卷一说明</span></li>
<li><a href="/book/1/1.html">章一</a>
    <span class="vip">1</span><span class="pay">0</span>
    <span class="info">2026-01-01 3.2万字</span></li>
<li><a href="/book/1/2.html">章二</a>
    <span class="vip">false</span><span class="pay">yes</span>
    <span class="info">字数:1200字</span></li>
<li><a href="/book/1/3.html">章三</a>
    <span class="vip"></span><span class="info">没有字数信息</span></li>
</ul></body></html>"""

TOC_NOURL = """<html><body><ul id="list">
<li><a>无链接卷</a><span class="vol">true</span></li>
<li><a>无链接章</a></li>
</ul></body></html>"""

TOC_SELF = """<html><body><ul id="list">
<li><a href="/book/1/1.html">自指章</a></li>
</ul><a id="nexttoc" href="/tv/self.html">下一页</a></body></html>"""

# ---- 正文步各轴用的页面 ----

CONTENT_MULTI = """<html><body><div id="content">多页正文一</div>
<a id="nexts" href="/cv/c2.html">2</a><a id="nexts" href="/cv/c3.html">3</a>
</body></html>"""

CONTENT_P2 = """<html><body><div id="content">多页正文二</div></body></html>"""

CONTENT_P3 = """<html><body><div id="content">多页正文三</div></body></html>"""

CONTENT_IMG = """<html><body><div id="content">
<img src="/img/1.jpg"><img src="http://cdn.example.org/2.jpg">
</div></body></html>"""


# ---------------------------------------------------------------- JSON 书源

J = "http://j.example.com"

JSON_SOURCE = {
    "bookSourceUrl": J,
    "bookSourceName": "JSON 源",
    "bookSourceType": 0,
    "enabled": True,
    "searchUrl": "/api/s?w={{#key}}",
    "ruleSearch": {
        "bookList": "$.data.list[*]",
        "name": "$.title",
        "author": "$.au",
        "intro": "$.desc",
        "bookUrl": "$.id##^##/api/book/",
    },
    "ruleBookInfo": {
        "name": "$.book.title",
        "author": "$.book.au",
        "intro": "$.book.desc",
        "tocUrl": "$.book.toc",
    },
    "ruleToc": {
        "chapterList": "$.chapters[*]",
        "chapterName": "$.n",
        "chapterUrl": "$.u",
    },
    "ruleContent": {"content": "$.content"},
}

SEARCH_JSON = json.dumps(
    {"data": {"list": [
        {"title": "JSON 书一", "au": "JA", "desc": "JD1", "id": "101"},
        {"title": "JSON 书二", "au": "JB", "desc": "JD2", "id": "102"},
    ]}},
    ensure_ascii=False,
)

INFO_JSON = json.dumps(
    {"book": {"title": "JSON 书一(详情)", "au": "JA", "desc": "JSON 详情简介",
              "toc": "/api/toc/101"}},
    ensure_ascii=False,
)

TOC_JSON = json.dumps(
    {"chapters": [{"n": "J 第一章", "u": "/api/c/101/1"},
                  {"n": "J 第二章", "u": "/api/c/101/2"}]},
    ensure_ascii=False,
)

CONTENT_JSON = json.dumps({"content": "JSON 正文一"}, ensure_ascii=False)


def main():
    shutil.rmtree(HTTP, ignore_errors=True)
    os.makedirs(HTTP, exist_ok=True)
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    html_src = json.dumps(HTML_SOURCE, ensure_ascii=False)
    json_src = json.dumps(JSON_SOURCE, ensure_ascii=False)

    # ---- HTML 源:四步 ----
    page(f"{H}/s?p=1&q=abc", SEARCH_HTML)
    case(source=html_src, step="search", key="abc", page=1)

    page(f"{H}/e/1.html", EXPLORE_HTML)
    case(source=html_src, step="explore", url="/e/1.html", page=1)

    page(f"{H}/book/1.html", INFO_HTML)
    case(source=html_src, step="info",
         book={"bookUrl": f"{H}/book/1.html", "name": "书名一", "author": "作者甲"})

    page(f"{H}/book/1/toc.html", TOC_HTML_1)
    page(f"{H}/book/1/toc2.html", TOC_HTML_2)
    case(source=html_src, step="toc",
         book={"bookUrl": f"{H}/book/1.html", "tocUrl": f"{H}/book/1/toc.html",
               "name": "书名一"})

    page(f"{H}/book/1/1.html", CONTENT_HTML_1)
    page(f"{H}/book/1/1_2.html", CONTENT_HTML_2)
    case(source=html_src, step="content",
         book={"bookUrl": f"{H}/book/1.html", "name": "书名一"},
         chapter={"url": f"{H}/book/1/1.html", "title": "第一章",
                  "baseUrl": f"{H}/book/1/toc.html", "index": 0})

    # ---- JSON 源:四步 ----
    page(f"{J}/api/s?w=abc", SEARCH_JSON, JSON_UTF8)
    case(source=json_src, step="search", key="abc", page=1)

    page(f"{J}/api/book/101", INFO_JSON, JSON_UTF8)
    case(source=json_src, step="info",
         book={"bookUrl": f"{J}/api/book/101", "name": "JSON 书一"})

    page(f"{J}/api/toc/101", TOC_JSON, JSON_UTF8)
    case(source=json_src, step="toc",
         book={"bookUrl": f"{J}/api/book/101", "tocUrl": f"{J}/api/toc/101",
               "name": "JSON 书一"})

    page(f"{J}/api/c/101/1", CONTENT_JSON, JSON_UTF8)
    case(source=json_src, step="content",
         book={"bookUrl": f"{J}/api/book/101", "name": "JSON 书一"},
         chapter={"url": f"{J}/api/c/101/1", "title": "J 第一章",
                  "baseUrl": f"{J}/api/toc/101", "index": 0})

    # ================= 列表步:一条轴一条轴 =================

    # bookList 前缀 `-`(不反转)/ `+`(反转,与默认同)
    for tag, rule in (("rev", "-.book"), ("plus", "+.book")):
        page(f"{H}/sv/{tag}?q=abc", SEARCH_HTML)
        case(source=variant(HTML_SOURCE, searchUrl=f"/sv/{tag}?q={{{{#key}}}}",
                            ruleSearch={"bookList": rule}),
             step="search", key="abc", page=1)

    # 列表选不中且无 bookUrlPattern → 整页按详情页解析
    page(f"{H}/sv/empty?q=abc", INFO_HTML)
    case(source=variant(HTML_SOURCE, searchUrl="/sv/empty?q={{#key}}",
                        ruleSearch={"bookList": ".nothing-here"}),
         step="search", key="abc", page=1)

    # bookUrlPattern 命中 → 直接按详情页解析(不走列表)
    page(f"{H}/sv/pat?q=abc", INFO_HTML)
    case(source=variant(HTML_SOURCE, searchUrl="/sv/pat?q={{#key}}",
                        bookUrlPattern=".*/sv/pat.*"),
         step="search", key="abc", page=1)

    # checkKeyWord:命中 / 不命中
    for tag, kw in (("hit", "书名一"), ("miss", "不存在的词")):
        page(f"{H}/sv/ck{tag}?q=abc", SEARCH_HTML)
        case(source=variant(HTML_SOURCE, searchUrl=f"/sv/ck{tag}?q={{{{#key}}}}",
                            ruleSearch={"checkKeyWord": kw}),
             step="search", key="abc", page=1)

    # name 取空的条目会被跳过(整表为空 → 落回详情页解析)
    page(f"{H}/sv/noname?q=abc", SEARCH_HTML)
    case(source=variant(HTML_SOURCE, searchUrl="/sv/noname?q={{#key}}",
                        ruleSearch={"name": ".no-such@text"}),
         step="search", key="abc", page=1)

    # 书名/作者格式化(BookHelp.formatBookName/Author 的净化正则)
    page(f"{H}/sv/fmt?q=abc", FORMAT_HTML)
    case(source=variant(HTML_SOURCE, searchUrl="/sv/fmt?q={{#key}}"),
         step="search", key="abc", page=1)

    # 字数格式化 wordCountFormat:纯数字 / 超万 / 非数字 / 空
    page(f"{H}/sv/wc?q=abc", WORDCOUNT_HTML)
    case(source=variant(HTML_SOURCE, searchUrl="/sv/wc?q={{#key}}"),
         step="search", key="abc", page=1)

    # 简介的 HtmlFormatter.formatIntro(<p>/<br>/实体/多空白)
    page(f"{H}/sv/intro?q=abc", INTRO_HTML)
    case(source=variant(HTML_SOURCE, searchUrl="/sv/intro?q={{#key}}",
                        ruleSearch={"intro": ".intro@html"}),
         step="search", key="abc", page=1)

    # 变量:@put 写入 + 列表项变量写回 SearchBook.variable
    page(f"{H}/sv/var?q=abc", SEARCH_HTML)
    case(source=variant(
        HTML_SOURCE, searchUrl="/sv/var?q={{#key}}",
        ruleSearch={"bookList": ".book@put:{'v1':'h3@text'}",
                    "name": "@get:{v1}"}),
         step="search", key="abc", page=1)

    # 正则模式的列表规则(`:` 开头)
    page(f"{H}/sv/re?q=abc", SEARCH_HTML)
    case(source=variant(
        HTML_SOURCE, searchUrl="/sv/re?q={{#key}}",
        ruleSearch={"bookList": ':<h3>(.*?)</h3>', "name": "$1",
                    "author": "", "kind": "", "intro": "", "lastChapter": "",
                    "wordCount": "", "coverUrl": "", "bookUrl": "$1"}),
         step="search", key="abc", page=1)

    # 搜索页 302 到详情页(isRedirect;两侧都要按详情页解析)
    write_snapshot(HTTP, method="GET", url=f"{H}/sv/redir?q=abc",
                   headers={"user-agent": UA}, status=302,
                   response_headers={"location": f"{H}/sv/redir-target"},
                   response_body=b"")
    page(f"{H}/sv/redir-target", INFO_HTML)
    case(source=variant(HTML_SOURCE, searchUrl="/sv/redir?q={{#key}}",
                        ruleSearch={"bookList": ".nothing-here"}),
         step="search", key="abc", page=1)

    # searchUrl 为空 → 两侧一致报错
    case(source=variant(HTML_SOURCE, searchUrl=""), step="search", key="abc", page=1)

    # explore:exploreUrl 的 JSON 形态(checkExploreJson)
    page(f"{H}/ev/json/1.html", EXPLORE_HTML)
    case(source=variant(
        HTML_SOURCE,
        exploreUrl=json.dumps([{"title": "分类", "url": "/ev/json/{{page}}.html"}],
                              ensure_ascii=False)),
         step="explore", url="/ev/json/1.html", page=1)

    # explore 的 ruleExplore.bookList 为空 → 回落 ruleSearch
    page(f"{H}/ev/fallback.html", SEARCH_HTML)
    case(source=variant(HTML_SOURCE, ruleExplore={"bookList": ""}),
         step="explore", url="/ev/fallback.html", page=1)

    # ================= 详情步 =================

    # canReName 规则非空 + canReName=true → 覆写已有书名/作者
    page(f"{H}/iv/rename.html", INFO_HTML)
    for can in (True, False):
        case(source=variant(HTML_SOURCE, ruleBookInfo={"canReName": "true"}),
             step="info", canReName=can,
             book={"bookUrl": f"{H}/iv/rename.html", "name": "旧书名", "author": "旧作者"})
    # canReName 规则为空 → 有名字就不覆写
    case(source=html_src, step="info", canReName=True,
         book={"bookUrl": f"{H}/iv/rename.html", "name": "旧书名", "author": "旧作者"})

    # tocUrl 取不到 → 落回 baseUrl,并把 body 存进 tocHtml
    case(source=variant(HTML_SOURCE, ruleBookInfo={"tocUrl": "#no-toc@href"}),
         step="info", book={"bookUrl": f"{H}/iv/rename.html"})

    # intro 的 <usehtml> 前缀直通(不过 formatIntro)
    page(f"{H}/iv/usehtml.html", USEHTML_INFO)
    case(source=variant(HTML_SOURCE, ruleBookInfo={"intro": ".intro@html"}),
         step="info", book={"bookUrl": f"{H}/iv/usehtml.html"})

    # kind 多值 → join(",")
    page(f"{H}/iv/kinds.html", KINDS_INFO)
    case(source=variant(HTML_SOURCE, ruleBookInfo={"kind": ".kind@text"}),
         step="info", book={"bookUrl": f"{H}/iv/kinds.html"})

    # init 规则选不中 → setContent(null) 后各字段取空
    case(source=variant(HTML_SOURCE, ruleBookInfo={"init": ".no-such-init"}),
         step="info", book={"bookUrl": f"{H}/iv/rename.html", "name": "保留书名"})

    # ================= 目录步 =================

    # nextTocUrl 一次给出多页(并发分支,threadCount=1 → 串行)
    page(f"{H}/tv/multi.html", TOC_MULTI)
    page(f"{H}/tv/p2.html", TOC_P2)
    page(f"{H}/tv/p3.html", TOC_P3)
    case(source=variant(HTML_SOURCE, ruleToc={"nextTocUrl": "#nexts@href"}),
         step="toc", book={"bookUrl": f"{H}/book/1.html",
                           "tocUrl": f"{H}/tv/multi.html", "name": "书名一"})

    # 章节列表为空 → TocEmptyException
    page(f"{H}/tv/empty.html", "<html><body>无目录</body></html>")
    case(source=html_src, step="toc",
         book={"bookUrl": f"{H}/book/1.html", "tocUrl": f"{H}/tv/empty.html"})

    # isVolume / isVip / isPay / updateTime(含 tocCountWords 的字数抽取)
    page(f"{H}/tv/flags.html", TOC_FLAGS)
    case(source=variant(HTML_SOURCE, ruleToc={
        "chapterList": "#list li", "chapterName": "a@text", "chapterUrl": "a@href",
        "isVolume": "class.vol@text", "isVip": "class.vip@text",
        "isPay": "class.pay@text", "updateTime": "class.info@text",
        "nextTocUrl": ""}),
         step="toc", book={"bookUrl": f"{H}/book/1.html",
                           "tocUrl": f"{H}/tv/flags.html"})

    # chapterUrl 取空:普通章节落回 baseUrl,卷落回「标题+序号」
    page(f"{H}/tv/nourl.html", TOC_NOURL)
    case(source=variant(HTML_SOURCE, ruleToc={
        "chapterList": "#list li", "chapterName": "a@text",
        "chapterUrl": "no-such@href", "isVolume": "class.vol@text",
        "nextTocUrl": ""}),
         step="toc", book={"bookUrl": f"{H}/book/1.html",
                           "tocUrl": f"{H}/tv/nourl.html"})

    # formatJs 改写标题
    case(source=variant(HTML_SOURCE, ruleToc={"formatJs": "#echo:格式化标题"}),
         step="toc", book={"bookUrl": f"{H}/book/1.html",
                           "tocUrl": f"{H}/book/1/toc.html"})

    # chapterList 前缀 `-`(不反转)
    case(source=variant(HTML_SOURCE, ruleToc={"chapterList": "-#list li"}),
         step="toc", book={"bookUrl": f"{H}/book/1.html",
                           "tocUrl": f"{H}/book/1/toc.html"})

    # nextTocUrl 指回本页 → 不跟进(防环)
    page(f"{H}/tv/self.html", TOC_SELF)
    case(source=html_src, step="toc",
         book={"bookUrl": f"{H}/book/1.html", "tocUrl": f"{H}/tv/self.html"})

    # ================= 正文步 =================

    # nextContentUrl 一次给出多页(并发分支)
    page(f"{H}/cv/multi.html", CONTENT_MULTI)
    page(f"{H}/cv/c2.html", CONTENT_P2)
    page(f"{H}/cv/c3.html", CONTENT_P3)
    case(source=variant(HTML_SOURCE, ruleContent={"nextContentUrl": "#nexts@href"}),
         step="content", book={"bookUrl": f"{H}/book/1.html", "name": "书名一"},
         chapter={"url": f"{H}/cv/multi.html", "title": "多页章",
                  "baseUrl": f"{H}/book/1/toc.html", "index": 0})

    # nextContentUrl == nextChapterUrl → 立刻停(不跟进下一章)
    case(source=html_src, step="content",
         nextChapterUrl=f"{H}/book/1/1_2.html",
         book={"bookUrl": f"{H}/book/1.html", "name": "书名一"},
         chapter={"url": f"{H}/book/1/1.html", "title": "第一章",
                  "baseUrl": f"{H}/book/1/toc.html", "index": 0})

    # replaceRegex 净化
    case(source=variant(HTML_SOURCE,
                        ruleContent={"replaceRegex": "##正文##章节"}),
         step="content", book={"bookUrl": f"{H}/book/1.html", "name": "书名一"},
         chapter={"url": f"{H}/book/1/1.html", "title": "第一章",
                  "baseUrl": f"{H}/book/1/toc.html", "index": 0})

    # 正文取空 → ContentEmptyException
    page(f"{H}/cv/empty.html", "<html><body><div id='content'></div></body></html>")
    case(source=html_src, step="content",
         book={"bookUrl": f"{H}/book/1.html", "name": "书名一"},
         chapter={"url": f"{H}/cv/empty.html", "title": "空章",
                  "baseUrl": f"{H}/book/1/toc.html", "index": 0})

    # 卷章节(url 以 title 开头)→ 一级目录正文不解析
    case(source=html_src, step="content",
         book={"bookUrl": f"{H}/book/1.html", "name": "书名一"},
         chapter={"url": "第一卷0", "title": "第一卷", "isVolume": True,
                  "baseUrl": f"{H}/book/1/toc.html", "index": 0})

    # sourceRegex(图片正文)与 imageStyle
    page(f"{H}/cv/img.html", CONTENT_IMG)
    case(source=variant(HTML_SOURCE, ruleContent={
        "content": "#content@html", "imageStyle": "FULL", "nextContentUrl": ""}),
         step="content", book={"bookUrl": f"{H}/book/1.html", "name": "书名一"},
         chapter={"url": f"{H}/cv/img.html", "title": "图章",
                  "baseUrl": f"{H}/book/1/toc.html", "index": 0})

    # title 规则(正文页自带标题)
    case(source=variant(HTML_SOURCE, ruleContent={"title": "#content@text"}),
         step="content", book={"bookUrl": f"{H}/book/1.html", "name": "书名一"},
         chapter={"url": f"{H}/book/1/1.html", "title": "第一章",
                  "baseUrl": f"{H}/book/1/toc.html", "index": 0})

    # ================= bookSourceType → BookType 位 =================
    # `BookSource.getBookType()`(help/source/BookSourceExtensions L130):
    # file=3 → text|webFile、image=2、audio=1、**video=4 → BookType.video(0b100)**、
    # 其余 → text。video 那支是 LegadoTeam 基准新加的,`allBookType` 也含 video
    # (决定 resetType 清位时清不清 0b100)——四步的 type 投影全部铺一遍。
    for st in (0, 1, 2, 3, 4, 5, -1):
        src = variant(HTML_SOURCE, bookSourceType=st)
        case(source=src, step="search", key="abc", page=1)
        case(source=src, step="info", book={"bookUrl": f"{H}/book/1.html"})
        # 已有 type 位的书:reset 前先清 allBookType(含 video)再或上新位
        for pre in (0, 4, 8, 0b1111111111):
            case(source=src, step="info",
                 book={"bookUrl": f"{H}/book/1.html", "name": "书名一", "type": pre})

    (OUT_DIR / "handmade.json").write_text(
        json.dumps(cases, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(cases)} pipeline cases -> {OUT_DIR/'handmade.json'}", file=sys.stderr)


if __name__ == "__main__":
    main()
