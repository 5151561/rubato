#!/usr/bin/env python3
"""生成 analyze-url(AnalyzeUrl 离线面)差分用例。

handmade.json:option JSON 的每个字段 × 编码路径 × <js>/{{}}/<page> 交叉;
corpus.json:从 fixtures/sources 抽真实 searchUrl / exploreUrl 的 url 串。
"""
import json
import random
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = ROOT / "fixtures" / "sources"
OUT_DIR = ROOT / "fixtures" / "cases" / "analyze-url"

BASE = "http://www.example.com/a/b.html"

URLS = [
    # 纯 url
    "/search?q=abc", "search?q=abc", "http://other.com/s?q=x", "",
    "/搜索?关键字=中文&页=1", "/s?q=a b&x=%E4%B8%AD", "/s?q=%zz", "/s?q=a+b",
    "/s?a=1&b=&c", "/s?=v", "/s?a=1&&b=2", "/s?", "/s#frag?q=1",
    # option:method
    '/s,{"method":"POST"}', '/s,{"method":"post"}', '/s,{"method":"HEAD"}',
    '/s,{"method":"PUT"}', '/s,{"method":""}', '/s,{"method":123}',
    # option:body
    '/s,{"method":"POST","body":"a=1&b=中文"}',
    '/s,{"method":"POST","body":{"a":1,"b":"x"}}',
    '/s,{"method":"POST","body":[{"a":1},{"b":2}]}',
    '/s,{"method":"POST","body":"{\\"a\\":1}"}',
    '/s,{"method":"POST","body":"<x>1</x>"}',
    '/s,{"method":"POST","body":"a=1","headers":{"Content-Type":"text/plain"}}',
    '/s,{"method":"POST","body":""}',
    # option:charset
    '/s?q=中文,{"charset":"gbk"}', '/s?q=中文,{"charset":"escape"}',
    '/s?q=中文,{"charset":"utf-8"}', '/s?q=中文,{"charset":"big5"}',
    '/s?q=中文,{"charset":"nosuchcharset"}', '/s?q=中文,{"charset":""}',
    '/s,{"method":"POST","body":"q=中文","charset":"gbk"}',
    '/s,{"method":"POST","body":"q=中文","charset":"escape"}',
    # option:headers
    '/s,{"headers":{"User-Agent":"UA1","Referer":"http://r"}}',
    '/s,{"headers":"{\\"User-Agent\\":\\"UA2\\"}"}',
    '/s,{"headers":{"n":1,"b":true,"f":1.5,"nil":null}}',
    '/s,{"headers":{"proxy":"http://p"}}',
    '/s,{"headers":[1,2]}',
    # option:其它字段
    '/s,{"type":"json"}', '/s,{"retry":3}', '/s,{"retry":"3"}', '/s,{"retry":"x"}',
    '/s,{"webView":true}', '/s,{"webView":"false"}', '/s,{"webView":false}',
    '/s,{"webView":0}', '/s,{"webView":""}', '/s,{"webView":"1"}',
    '/s,{"webJs":"js1"}', '/s,{"bodyJs":"js2"}', '/s,{"dnsIp":"1.2.3.4"}',
    '/s,{"resolveIp":"1.2.3.4"}', '/s,{"dnsIp":"1.2.3.4","resolveIp":"5.6.7.8"}',
    # option:timeout / followRedirects(LegadoTeam 新增)
    '/s,{"timeout":5000}', '/s,{"timeout":"5000"}', '/s,{"timeout":0}',
    '/s,{"timeout":-1}', '/s,{"timeout":1.5}', '/s,{"timeout":"x"}',
    '/s,{"timeout":true}', '/s,{"timeout":2147483647}', '/s,{"timeout":2147483648}',
    '/s,{"followRedirects":true}', '/s,{"followRedirects":false}',
    '/s,{"followRedirects":"true"}', '/s,{"followRedirects":"FALSE"}',
    '/s,{"followRedirects":1}', '/s,{"followRedirects":0}',
    '/s,{"followRedirects":2}', '/s,{"followRedirects":"yes"}',
    '/s,{"followRedirects":""}', '/s,{"timeout":3000,"followRedirects":false}',
    '/s,{"serverID":12}', '/s,{"serverID":"12"}', '/s,{"serverID":"x"}',
    '/s,{"webViewDelayTime":500}', '/s,{"webViewDelayTime":-5}',
    '/s,{"origin":"http://o"}', '/s,{"js":"#echo:http://from-js/"}',
    '/s,{"js":"#null"}', '/s,{"js":"#num:12"}',
    # option 的宽松 JSON
    "/s,{method:POST}", "/s,{'method':'POST'}", '/s,{method:"POST",retry:1}',
    "/s,{ method : POST , charset : gbk }", '/s,{"method":"POST",}',
    '/s,{"a":1} 尾巴', '/s,{坏', '/s,{}',
    # 分隔符本身的怪相
    '/s , {"retry":1}', '/s,  {"retry":1}', '/s,{"a":"b,{c}"}',
    '/s,{"body":"x,{y}"},{"retry":2}',
    # <js> / @js: / {{}}
    "<js>#echo:http://js-built/</js>",
    "/pre<js>#echo:JS</js>", "@js:#echo:http://at-js/",
    "http://x/@result<js>#echo:Y</js>", "<js>#result</js>",
    "<js>#null</js>", "<js>#err</js>",
    "/s?q={{#key}}", "/s?p={{#page}}", "/s?q={{#num:12}}", "/s?q={{#num:1.5}}",
    "/s?q={{#null}}", "/s?q={{key}}", "/s?q={{#get:v1}}", "/s?q={{#put:w=1}}",
    "/s?q={{#echo:中文}}", "/s?q={{",
    # <page>
    "/list<1,2,3>.html", "/list<a,b>.html", "/list<x>.html", "/list<>.html",
    "/list<1,2,3>/<x,y>.html",
]

PAGES = [None, 1, 2, 5]


def handmade():
    out = []

    def add(url, **extra):
        c = {"id": f"ah{len(out):05d}", "op": "analyzeUrl", "mUrl": url, "baseUrl": BASE}
        c.update(extra)
        out.append(c)

    for url in URLS:
        add(url)
    for url in ["/list<1,2,3>.html", "/s?p={{#page}}", "/list<a,b>.html"]:
        for page in PAGES:
            if page is None:
                continue
            add(url, page=page)
    # key / 变量 / header 注入
    for url in ["/s?q={{#key}}", "/s?q={{#get:v1}}", "/s?q={{#put:w=写入}}"]:
        add(url, key="关键字", vars={"v1": "初值1"}, ruleData="book", bookName="书名A")
        add(url, key="关键字", vars={"v1": "初值1"}, chapter=True, title="章节T")
    for h in [{"User-Agent": "UA-F"}, {"proxy": "http://p"}, {"Cookie": "a=1"}]:
        add("/s?q=1", headers=h)
        add('/s?q=1,{"headers":{"User-Agent":"UA-O"}}', headers=h)
    # baseUrl 变体
    for base in ["", "http://b.com", "http://b.com/x/y.html", 'http://b.com/x,{"a":1}']:
        add("/s?q=1", baseUrl=base)
        add("rel.html", baseUrl=base)
    # 编码矩阵:query / form × charset × 各类字符
    payloads = [
        "q=中文", "q=a b", "q=a+b", "q=%E4%B8%AD", "q=%zz", "q=a&b=c", "q=", "q",
        "q=a=b", "q=符号!*'();:@&=+$,/?#[]", "q=emoji😀", "q=tab\tx", "q=引号\"'",
        "a=1&b=中文&c=%20", "=v", "&&", "q=%E4%B8", "q=" + "长" * 20,
    ]
    charsets = [None, "", "utf-8", "UTF-8", "gbk", "GBK", "gb2312", "gb18030",
                "big5", "escape", "iso-8859-1", "nosuch"]
    for p in payloads:
        for cs in charsets:
            opt = "" if cs is None else ',{"charset":%s}' % json.dumps(cs)
            add("/s?" + p + opt)
            post = '{"method":"POST","body":%s%s}' % (
                json.dumps(p), "" if cs is None else ',"charset":%s' % json.dumps(cs))
            add("/s," + post)
    return out


def corpus():
    urls = set()
    for f in sorted(SRC_DIR.glob("*.json")):
        for src in json.load(open(f, encoding="utf-8")):
            for key in ("searchUrl", "exploreUrl"):
                v = src.get(key)
                if not isinstance(v, str) or not v.strip():
                    continue
                urls.add(v)
                # exploreUrl 常是 JSON 数组,里面每项的 url 才是规则
                for m in re.finditer(r'"url"\s*:\s*"((?:[^"\\]|\\.){0,400})"', v):
                    try:
                        urls.add(json.loads('"' + m.group(1) + '"'))
                    except Exception:
                        pass
    urls = sorted(u for u in urls if 0 < len(u) < 600)
    rng = random.Random(20260829)
    rng.shuffle(urls)
    urls = urls[:6000]
    out = []
    for i, u in enumerate(urls):
        c = {"id": f"ac{len(out):05d}", "op": "analyzeUrl", "mUrl": u,
             "baseUrl": "https://www.example.com/x/y.html", "key": "关键字",
             "ruleData": "book", "bookName": "书名A"}
        if i % 3 == 0:
            c["page"] = 2
        out.append(c)
    return out, len(urls)



# ---------------------------------------------------------------- 漂移补覆盖
# INITIAL_GSON 的 MapDeserializerDoubleAsIntFix:注册给 `Map<String?, Any?>`,
# 而 UrlOption 的 headers 字段声明是 `Any?`。两支数字语义要分别钉住:
#   - headers 是 JSON **对象** → gson 按 Any? 走 ObjectTypeAdapter + LONG_OR_DOUBLE;
#   - headers 是 JSON **字符串** → getHeaderMap() 里显式
#     `GSON.fromJsonObject<Map<String, Any>>(value)`,声明类型命中(或没命中)
#     那个反序列化器 —— 差分来定这条路到底整数化不整数化。
NUMS = '{"a":3.0,"b":2.5,"c":-2.5,"d":1e3,"e":7,"f":9007199254740993,' \
       '"g":1.0e20,"h":true,"i":null,"j":[1.0,2.5],"k":{"x":1.0},"l":"3.0"}'


def drift():
    out = []

    def add(url, **extra):
        c = {"id": f"ad{len(out):05d}", "op": "analyzeUrl", "mUrl": url, "baseUrl": BASE}
        c.update(extra)
        out.append(c)

    # headers 作为**字符串**(声明类型 Map<String, Any>)
    add("/s?q=1," + json.dumps({"headers": NUMS}, ensure_ascii=False))
    # headers 作为**对象**(字段声明 Any? → ObjectTypeAdapter)
    add("/s?q=1," + json.dumps({"headers": json.loads(NUMS)}, ensure_ascii=False))
    # body 同样两形态(getBody 是 `it as? String ?: GSON.toJson(it)`)
    add("/s?q=1," + json.dumps({"method": "POST", "body": json.loads(NUMS)},
                               ensure_ascii=False))
    add("/s?q=1," + json.dumps({"method": "POST", "body": NUMS}, ensure_ascii=False))
    # headers 字符串但不是对象(数组 / 标量)→ getHeaderMap() 给 null
    for h in ["[1,2]", "\"x\"", "3.0", "null", "{bad", ""]:
        add("/s?q=1," + json.dumps({"headers": h}, ensure_ascii=False))
    # 单个数字键的最小对照组(方便定位)
    for v in ["3.0", "2.5", "-2.5", "-0.0", "1e3", "1E-3", "0.1", "7", "7.5",
              "9007199254740993", "1.0e20", "-1.5", "2.0000000000000004"]:
        add("/s?q=1," + json.dumps({"headers": '{"n":%s}' % v}, ensure_ascii=False))
        add("/s?q=1," + json.dumps({"headers": {"n": json.loads(v)}}, ensure_ascii=False))
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
    print(f"{len(corp)} corpus cases({n} 条真实 url 规则)-> {OUT_DIR/'corpus.json'}", file=sys.stderr)


if __name__ == "__main__":
    main()
