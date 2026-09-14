#!/usr/bin/env python3
"""fetch 差分(HTTP 录放全链路)的用例 + 合成快照生成器。

- 用例落 fixtures/cases/fetch/generated.json;
- 快照落 fixtures/http/(本脚本**独占**该目录:每次先清空重建);
- key 算法在 tools/http_snapshot.py(契约 docs/http-snapshot.md)。

快照条目里的 url / headers 是「预期两侧实际发出的请求」——okhttp 规范化后的
形态,由人工写死。写错的后果是两侧一起 snapshot_miss(diff 仍 PASS 但
入口脚本会报 miss 数),不会静默错配。
"""

import json
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from http_snapshot import write_snapshot

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
HTTP = os.path.join(ROOT, "fixtures", "http")
OUT = os.path.join(ROOT, "fixtures", "cases", "fetch", "generated.json")

UA = "rubato-judge"
HTML_UTF8 = {"content-type": "text/html; charset=utf-8"}

cases = []
_seq = 0


def case(mUrl, **kw):
    global _seq
    _seq += 1
    c = {"id": f"ft{_seq:04d}", "op": "fetch", "mUrl": mUrl}
    c.update(kw)
    cases.append(c)
    return c


def snap(**kw):
    write_snapshot(HTTP, **kw)


def main():
    shutil.rmtree(HTTP, ignore_errors=True)
    os.makedirs(HTTP, exist_ok=True)

    # ---- 基础 GET / 编码 ----

    # ft0001 简单 GET,content-type 带 charset
    case("http://example.com/index.html")
    snap(method="GET", url="http://example.com/index.html",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body="<html><body>你好,世界</body></html>".encode("utf-8"))

    # ft0002 unicode + 空格 query(okhttp 规范化 + key 的 query 排序)
    case("http://example.com/s?q=中 文&b=2&a=1")
    snap(method="GET", url="http://example.com/s?q=%E4%B8%AD%20%E6%96%87&b=2&a=1",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body="<p>搜索结果</p>".encode("utf-8"))

    # ft0003 GBK 页面,无 content-type charset,靠 meta 判定
    case("http://example.com/gbk.html")
    snap(method="GET", url="http://example.com/gbk.html",
         headers={"user-agent": UA},
         response_headers={"content-type": "text/html"},
         response_body=(
             '<html><head><meta charset="gbk"><title>书</title></head>'
             "<body>第一章 中文内容</body></html>"
         ).encode("gbk"))

    # ft0004 http-equiv 形式的 meta charset
    case("http://example.com/gbk2.html")
    snap(method="GET", url="http://example.com/gbk2.html",
         headers={"user-agent": UA},
         response_headers={"content-type": "text/html"},
         response_body=(
             '<html><head><meta http-equiv="Content-Type" '
             'content="text/html; charset=gbk"></head>'
             "<body>目录页</body></html>"
         ).encode("gbk"))

    # ft0005 charset(gbk) option:query 按 GBK 百分号编码
    case('http://example.com/so?wd=中文,{"charset":"gbk"}')
    snap(method="GET", url="http://example.com/so?wd=%D6%D0%CE%C4",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body="gbk query ok".encode("utf-8"))

    # ft0006 无 query GET + 自定义 UA
    case("http://example.com/plain", headers={"User-Agent": "MyUA/1.0"})
    snap(method="GET", url="http://example.com/plain",
         headers={"user-agent": "MyUA/1.0"},
         response_headers=HTML_UTF8,
         response_body=b"plain")

    # ft0007 UA=null:User-Agent 头被删除
    case("http://example.com/noua", headers={"User-Agent": "null"})
    snap(method="GET", url="http://example.com/noua",
         headers={},
         response_headers=HTML_UTF8,
         response_body=b"no ua")

    # ft0008 Referer 参与 key
    case("http://example.com/ref", headers={"Referer": "http://example.com/from"})
    snap(method="GET", url="http://example.com/ref",
         headers={"user-agent": UA, "referer": "http://example.com/from"},
         response_headers=HTML_UTF8,
         response_body=b"ref ok")

    # ft0009 相对 mUrl + baseUrl
    case("/rel?x=1", baseUrl="http://example.com/a/b.html")
    snap(method="GET", url="http://example.com/rel?x=1",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body=b"rel ok")

    # ft0010 非默认端口
    case("http://example.com:8080/alt")
    snap(method="GET", url="http://example.com:8080/alt",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body=b"alt port")

    # ft-extra 检测器兜底:无 content-type charset、无 meta → icu4j 检测
    case("http://example.com/detect-gbk")
    snap(method="GET", url="http://example.com/detect-gbk",
         headers={"user-agent": UA},
         response_headers={"content-type": "text/html"},
         response_body="<html><body>第一章 风雪山神庙 林冲在山神庙里避雪,忽然听到外面有人低声说话。"
                       "那雪下得正紧,北风卷着碎琼乱玉扑进破庙来。</body></html>".encode("gbk"))

    case("http://example.com/detect-big5")
    snap(method="GET", url="http://example.com/detect-big5",
         headers={"user-agent": UA},
         response_headers={"content-type": "text/plain"},
         response_body="夜色漸深,城外的燈火一盞一盞熄滅,只剩下更夫的梆子聲在長街上迴盪,"
                       "他推開木門,案上的書卷還攤著。".encode("big5"))

    # ---- POST 家族 ----

    # ft0011 表单 POST(encodedForm;生效 content-type 带补全的 charset)
    form_bytes = b"q=%E4%B8%AD%E6%96%87&x=1"
    case('http://example.com/api,{"method":"POST","body":"q=中文&x=1"}')
    snap(method="POST", url="http://example.com/api",
         headers={"user-agent": UA,
                  "content-type": "application/x-www-form-urlencoded; charset=utf-8"},
         body=form_bytes,
         response_headers={"content-type": "application/json"},
         response_body=b'{"ok":1}')

    # ft0012 JSON body → postJson
    case('http://example.com/api,{"method":"POST","body":{"a":1}}')
    snap(method="POST", url="http://example.com/api",
         headers={"user-agent": UA, "content-type": "application/json; charset=UTF-8"},
         body='{\n  "a": 1\n}'.encode("utf-8"),
         response_headers={"content-type": "application/json"},
         response_body=b'{"ok":2}')

    # ft0013 显式 Content-Type 头 + 非表单 body(字节按头的 charset 编码)
    case('http://example.com/api,{"method":"POST","body":"abc中文"}',
         headers={"Content-Type": "text/plain; charset=gbk"})
    snap(method="POST", url="http://example.com/api",
         headers={"user-agent": UA, "content-type": "text/plain; charset=gbk"},
         body=b"abc" + "中文".encode("gbk"),
         response_headers=HTML_UTF8,
         response_body=b"posted")

    # ft0014 gbk charset 的表单 POST
    case('http://example.com/api,{"method":"POST","body":"a=中","charset":"gbk"}')
    snap(method="POST", url="http://example.com/api",
         headers={"user-agent": UA,
                  "content-type": "application/x-www-form-urlencoded; charset=utf-8"},
         body=b"a=%D6%D0",
         response_headers=HTML_UTF8,
         response_body=b"gbk form")

    # ft0015 HEAD
    case('http://example.com/head,{"method":"HEAD"}')
    snap(method="HEAD", url="http://example.com/head",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body=b"")

    # ---- 重定向 ----

    # ft0016 GET 302 相对 Location
    case("http://example.com/old")
    snap(method="GET", url="http://example.com/old",
         headers={"user-agent": UA},
         status=302,
         response_headers={"location": "/new"},
         response_body=b"")
    snap(method="GET", url="http://example.com/new",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body=b"landed")

    # ft0017 POST 302 → 降级 GET(丢 body 与 Content-Type)
    case('http://example.com/submit,{"method":"POST","body":"a=1"}')
    snap(method="POST", url="http://example.com/submit",
         headers={"user-agent": UA,
                  "content-type": "application/x-www-form-urlencoded; charset=utf-8"},
         body=b"a=1",
         status=302,
         response_headers={"location": "http://example.com/done"},
         response_body=b"")
    snap(method="GET", url="http://example.com/done",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body=b"form done")

    # ft0018 307 + POST:不跟进,终态即 307
    case('http://example.com/t307,{"method":"POST","body":"b=2"}')
    snap(method="POST", url="http://example.com/t307",
         headers={"user-agent": UA,
                  "content-type": "application/x-www-form-urlencoded; charset=utf-8"},
         body=b"b=2",
         status=307,
         response_headers={"location": "/should-not-follow",
                           "content-type": "text/plain"},
         response_body="留在原地".encode("utf-8"))

    # ft0019 跨域重定向
    case("http://example.com/away")
    snap(method="GET", url="http://example.com/away",
         headers={"user-agent": UA},
         status=301,
         response_headers={"location": "http://other.example.org/here"},
         response_body=b"")
    snap(method="GET", url="http://other.example.org/here",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body=b"other host")

    # followRedirects=false:302 不跟进,终态即 302(LegadoTeam 新增的
    # urlOption;真身经 buildRequestClient 落到 okhttp client 配置上)
    case('http://example.com/noflw,{"followRedirects":false}')
    snap(method="GET", url="http://example.com/noflw",
         headers={"user-agent": UA},
         status=302,
         response_headers={"location": "http://example.com/noflw-target",
                           "content-type": "text/plain"},
         response_body="不跟进".encode("utf-8"))
    snap(method="GET", url="http://example.com/noflw-target",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body=b"should not be fetched")

    # followRedirects=true 显式打开:与默认一致,跟进
    case('http://example.com/flw,{"followRedirects":true}')
    snap(method="GET", url="http://example.com/flw",
         headers={"user-agent": UA},
         status=302,
         response_headers={"location": "http://example.com/flw-target"},
         response_body=b"")
    snap(method="GET", url="http://example.com/flw-target",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body=b"followed")

    # followRedirects 非法值(parseBooleanOption 给 null)→ 按默认跟进
    case('http://example.com/flwbad,{"followRedirects":"yes"}')
    snap(method="GET", url="http://example.com/flwbad",
         headers={"user-agent": UA},
         status=302,
         response_headers={"location": "http://example.com/flw-target"},
         response_body=b"")

    # timeout 只影响 okhttp 超时配置,不改回放输出(两侧都应原样跟进)
    case('http://example.com/tmo,{"timeout":5000}')
    snap(method="GET", url="http://example.com/tmo",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body="超时配置无副作用".encode("utf-8"))

    # ft0020 重定向环 → too_many_redirects
    case("http://example.com/loop")
    snap(method="GET", url="http://example.com/loop",
         headers={"user-agent": UA},
         status=302,
         response_headers={"location": "/loop"},
         response_body=b"")

    # ---- 重试 / 非 2xx ----

    # ft0021 retry=2,恒 500:三次请求,终态 500
    case('http://example.com/flaky,{"retry":2}')
    snap(method="GET", url="http://example.com/flaky",
         headers={"user-agent": UA},
         status=500,
         response_headers={"content-type": "text/plain"},
         response_body=b"boom")

    # ft0022 404 不重试
    case("http://example.com/missing")
    snap(method="GET", url="http://example.com/missing",
         headers={"user-agent": UA},
         status=404,
         response_headers=HTML_UTF8,
         response_body="页面不存在".encode("utf-8"))

    # ---- xml 补头 / bodyJs / type ----

    # ft0023 xml content-type 且 body 无 <?xml → 补声明
    case("http://example.com/feed")
    snap(method="GET", url="http://example.com/feed",
         headers={"user-agent": UA},
         response_headers={"content-type": "application/xml"},
         response_body=b"<root><item/></root>")

    # ft0024 已带 <?xml → 不补
    case("http://example.com/feed2")
    snap(method="GET", url="http://example.com/feed2",
         headers={"user-agent": UA},
         response_headers={"content-type": "text/atom+xml; charset=utf-8"},
         response_body=b'<?xml version="1.0"?><root/>')

    # ft0025 bodyJs 改写响应体(JS 桩指令表见 rule-engine README)
    case('http://example.com/bj,{"bodyJs":"#echo:REWRITTEN"}')
    snap(method="GET", url="http://example.com/bj",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body=b"original")

    # ft0026 type + data URI → hex(不发请求)
    case('data:application/octet-stream;base64,AQIDBA==,{"type":"bytes"}')

    # ft0027 type + 网络字节 → hex
    case('http://example.com/img,{"type":"png"}')
    snap(method="GET", url="http://example.com/img",
         headers={"user-agent": UA},
         response_headers={"content-type": "image/png"},
         response_body=bytes([0x89, 0x50, 0x4E, 0x47, 0x00, 0xFF]))

    # ---- cookie ----

    # ft0028 CookieJar:session/persistent 分流入 store
    case("http://example.com/login",
         source={"url": "http://example.com", "enabledCookieJar": True})
    snap(method="GET", url="http://example.com/login",
         headers={"user-agent": UA},
         response_headers={"content-type": "text/html; charset=utf-8",
                           "set-cookie": ["sid=abc; Path=/",
                                          "uid=u1; expires=Wed, 21 Oct 2065 07:28:00 GMT"]},
         response_body=b"login page")

    # ft0029 CookieJar 但响应无 Set-Cookie:仍写空 session 条目
    case("http://example.com/nocookie",
         source={"url": "http://example.com", "enabledCookieJar": True})
    snap(method="GET", url="http://example.com/nocookie",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body=b"none")

    # ft0030 预置库 cookie → setCookie 合并进请求头(headers 观察面)
    case("http://example.com/withck",
         cookies={"example.com": "k=v; t=2"},
         source={"url": "http://example.com", "enabledCookieJar": True})
    snap(method="GET", url="http://example.com/withck",
         headers={"user-agent": UA},
         response_headers=HTML_UTF8,
         response_body=b"got cookie")

    # ft0031 预置 session + 新 session 合并(updateSessionCookie 的 merge 路径)
    case("http://example.com/sess",
         sessionCookies={"example.com": "s0=old"},
         source={"url": "http://example.com", "enabledCookieJar": True})
    snap(method="GET", url="http://example.com/sess",
         headers={"user-agent": UA},
         response_headers={"content-type": "text/html; charset=utf-8",
                           "set-cookie": ["s1=new"]},
         response_body=b"sess")

    # ft0032 CookieJar + 重定向:两跳都存取 cookie
    case("http://example.com/step1",
         source={"url": "http://example.com", "enabledCookieJar": True})
    snap(method="GET", url="http://example.com/step1",
         headers={"user-agent": UA},
         status=302,
         response_headers={"location": "/step2",
                           "set-cookie": ["hop1=a"]},
         response_body=b"")
    snap(method="GET", url="http://example.com/step2",
         headers={"user-agent": UA},
         response_headers={"content-type": "text/html; charset=utf-8",
                           "set-cookie": ["hop2=b; Max-Age=3600"]},
         response_body=b"stepped")

    # ft0033 域不匹配的 Set-Cookie 被丢弃;公共后缀 domain 被丢弃
    case("http://foo.example.com/drop",
         source={"url": "http://foo.example.com", "enabledCookieJar": True})
    snap(method="GET", url="http://foo.example.com/drop",
         headers={"user-agent": UA},
         response_headers={"content-type": "text/html; charset=utf-8",
                           "set-cookie": ["a=1; domain=other.com",
                                          "b=2; domain=com",
                                          "c=3; domain=example.com",
                                          "d=4; expires=31 Feb 2065 07:28:00"]},
         response_body=b"drop some")

    # ft0034 enabledCookieJar=false:不发 CookieJar 头,响应 Set-Cookie 被忽略
    case("http://example.com/nojar",
         source={"url": "http://example.com", "enabledCookieJar": False})
    snap(method="GET", url="http://example.com/nojar",
         headers={"user-agent": UA},
         response_headers={"content-type": "text/html; charset=utf-8",
                           "set-cookie": ["x=1"]},
         response_body=b"nojar")

    # ---- cookie 键归一:sourceKey 是否 URL、跨域请求 ----
    # 真身 AnalyzeUrl L149-151:
    #   domain = sourceKey?.takeIf { getBaseUrl(it) == null } ?: getSubDomain(url)
    # 即 sourceKey **不是** http(s) 链接时直接当键;是链接时取的是**本次请求 url**
    # 的子域名(而不是 sourceKey 的)。跨域请求才照得出这条。

    # ft0035 sourceKey 非 URL(书源名)→ 直接当 cookie 键
    case("http://example.com/k1",
         cookies={"我的书源": "k=1", "example.com": "e=1"},
         source={"url": "我的书源", "enabledCookieJar": True})
    snap(method="GET", url="http://example.com/k1",
         headers={"user-agent": UA}, response_headers=HTML_UTF8, response_body=b"k1")

    # ft0036 sourceKey 是 URL 但**跨域**请求 → 取请求 url 的子域名,
    # sourceKey 那份 cookie 不该被带上
    case("http://b.com/k2",
         cookies={"a.com": "fromA=1", "b.com": "fromB=1"},
         source={"url": "http://www.a.com/", "enabledCookieJar": True})
    snap(method="GET", url="http://b.com/k2",
         headers={"user-agent": UA}, response_headers=HTML_UTF8, response_body=b"k2")

    # ft0037 同上,但库里只有 sourceKey 那份 → 请求头应当**没有** Cookie
    case("http://b.com/k3",
         cookies={"a.com": "fromA=1"},
         source={"url": "http://www.a.com/", "enabledCookieJar": True})
    snap(method="GET", url="http://b.com/k3",
         headers={"user-agent": UA}, response_headers=HTML_UTF8, response_body=b"k3")

    # ft0038 sourceKey 是 URL 且同域 → 与旧口径一致(回归位)
    case("http://www.a.com/k4",
         cookies={"a.com": "fromA=1"},
         source={"url": "http://www.a.com/", "enabledCookieJar": True})
    snap(method="GET", url="http://www.a.com/k4",
         headers={"user-agent": UA}, response_headers=HTML_UTF8, response_body=b"k4")

    # ft0039 sourceKey 空串 → getBaseUrl("") == null → 键就是空串
    case("http://example.com/k5",
         cookies={"": "empty=1", "example.com": "e=1"},
         source={"url": "", "enabledCookieJar": True})
    snap(method="GET", url="http://example.com/k5",
         headers={"user-agent": UA}, response_headers=HTML_UTF8, response_body=b"k5")

    # ft0040 sourceKey 是非 http 协议(不算 URL)→ 原串当键
    case("http://example.com/k6",
         cookies={"legado://src/1": "s=1"},
         source={"url": "legado://src/1", "enabledCookieJar": True})
    snap(method="GET", url="http://example.com/k6",
         headers={"user-agent": UA}, response_headers=HTML_UTF8, response_body=b"k6")

    # ft0041 sourceKey 大小写 scheme(getBaseUrl 前缀判定忽略大小写 → 算 URL)
    case("http://b.com/k7",
         cookies={"a.com": "fromA=1", "b.com": "fromB=1"},
         source={"url": "HTTP://www.a.com/", "enabledCookieJar": True})
    snap(method="GET", url="http://b.com/k7",
         headers={"user-agent": UA}, response_headers=HTML_UTF8, response_body=b"k7")

    # ft0042 sourceKey 是坏 URL(host 为空)→ getBaseUrl 给 null → 原串当键
    case("http://example.com/k8",
         cookies={"http:///nohost": "n=1"},
         source={"url": "http:///nohost", "enabledCookieJar": True})
    snap(method="GET", url="http://example.com/k8",
         headers={"user-agent": UA}, response_headers=HTML_UTF8, response_body=b"k8")

    # ft0043 跨域 + CookieJar 保存:存的键按**响应 url** 的子域名走
    case("http://b.com/k9",
         source={"url": "http://www.a.com/", "enabledCookieJar": True})
    snap(method="GET", url="http://b.com/k9",
         headers={"user-agent": UA},
         response_headers={"content-type": "text/html; charset=utf-8",
                           "set-cookie": ["nb=1; expires=Wed, 21 Oct 2065 07:28:00 GMT"]},
         response_body=b"k9")

    # ---- webView:BackstageWebView 的**策略层**(两侧都是真身/移植)----
    #
    # 平台那一半是**剧本**(case 的 `webview` 字段,契约
    # fixtures/cases/webview/README.md);策略那一半 —— 补 900 ms、
    # `100 + delayTime` 起跑、重试梯子、unescapeJson + 剥引号、跟过转向就换 url、
    # 加载完把 cookie 抄进 CookieStore —— 两侧逐字节比。
    # webview 那一套单独钉策略的每一条;这里钉的是 **AnalyzeUrl 怎么进出它**。

    # webView GET:不发请求,body 是页面上那段 JS 的回值
    case('http://example.com/wv,{"webView":true}',
         webview={"evals": ["\"<html>取到了</html>\""]})

    # webJs 指定 JS;第一次求值回 null(还没就绪)→ 走重试梯子,第二次才有
    case('http://example.com/wv,{"webView":true,"webJs":"getContent()"}',
         webview={"evals": ["null", "\"<b>第二拍</b>\""]})

    # webViewDelayTime:起跑时刻挪后(观察面在 webview 那套;这里只保证结果一致)
    case('http://example.com/wv,{"webView":true,"webViewDelayTime":1500}',
         webview={"evals": ["\"delayed\""]})

    # 页面加载完把 CookieManager 的 cookie 抄进 CookieStore ——
    # 键是 `source.getKey()`,没有 source 就**不抄**(下一条对照)
    case('http://example.com/wv,{"webView":true}',
         source={"url": "http://example.com", "enabledCookieJar": True},
         webview={"evals": ["\"有 cookie\""], "cookie": "wv=1"})
    case('http://example.com/wv,{"webView":true}',
         webview={"evals": ["\"没 source 就没 tag\""], "cookie": "wv=1"})

    # 跟过转向(shouldOverrideUrlLoading 的 isRedirect):StrResponse 包
    # priorResponse(302),url 换成**落地那一页**
    case('http://example.com/wv,{"webView":true}',
         webview={"overrideUrls": [{"url": "http://example.com/wv-final",
                                    "isRedirect": True}],
                  "pageFinishedUrl": "http://example.com/wv-final",
                  "evals": ["\"转向之后\""]})

    # 一直取不到结果:重试梯子跑满 → 「js执行超时」。**缺省剧本就是这一档**
    # ——「这一页录不下来」的诚实结局,不是假装取到了内容
    case('http://example.com/wv2,{"webView":true}')

    # 页面永远加载不完 → withTimeout 到点
    case('http://example.com/wv3,{"webView":true}',
         webview={"pageFinished": False})

    # webView POST:先走一次真实 POST(form),把 res.url / res.body 交给
    # webView 当 baseUrl / html
    case('http://example.com/wvp,{"webView":true,"method":"POST","body":"w=1"}',
         webview={"evals": ["\"POST 之后的页面\""]})
    snap(method="POST", url="http://example.com/wvp",
         headers={"user-agent": UA,
                  "content-type": "application/x-www-form-urlencoded; charset=utf-8"},
         body=b"w=1",
         response_headers=HTML_UTF8,
         response_body=b"inner")

    # POST + followRedirects=false 且这一跳是 3xx:**不进 webView**,
    # 直接把那个响应回给上层(shouldReturnRedirectBeforeWebView)
    case('http://example.com/wvr,{"webView":true,"method":"POST","body":"w=2",'
         '"followRedirects":false}',
         webview={"evals": ["\"不该跑到这里\""]})
    snap(method="POST", url="http://example.com/wvr",
         headers={"user-agent": UA,
                  "content-type": "application/x-www-form-urlencoded; charset=utf-8"},
         body=b"w=2",
         status=302,
         response_headers={"location": "http://example.com/wvr-target",
                           "content-type": "text/html; charset=utf-8"},
         response_body="转向体".encode("utf-8"))

    # ---- key 等价类:无快照,两侧 snapshot_miss 串必须逐字一致 ----
    # (key 里含 okhttp 规范化 + §3 归一化的完整链路)

    case("http://example.com/路径/文件?z=中&a=b c")
    case("http://EXAMPLE.com.:80/UPPER/../x/./y?dup=1&dup=1&B=%2f")
    case("http://example.com/a%2Fb/c?q=%E4%B8%AD")
    case("http://example.com/s?e=&f&g=+h")
    case("http://user:pass@example.com/auth?x=1")
    case('http://example.com/kp,{"method":"POST","body":"a=中 b","charset":"gbk"}')
    case("http://example.com:8443/tls-ish")
    case("https://example.com:443/https-default")

    # ---- 语料:真实书源 url 规则跑全链(不配快照)----
    # 两侧要么产出逐字一致的 snapshot_miss(整条「构造+okhttp 规范化+key」
    # 都对上),要么产出一致的 url_error / fetch_error。
    # (IDN host 此前按字面 authority 过滤掉 —— M2n 在 `net::http_url` 接上
    # okhttp 的 idnToAscii 之后不再过滤,那批 url 照常入册。)
    import random
    import re
    src_dir = os.path.join(ROOT, "fixtures", "sources")
    urls = set()
    for fn in sorted(os.listdir(src_dir)):
        if not fn.endswith(".json"):
            continue
        for src in json.load(open(os.path.join(src_dir, fn), encoding="utf-8")):
            for k in ("searchUrl", "exploreUrl"):
                v = src.get(k)
                if not isinstance(v, str) or not v.strip():
                    continue
                urls.add(v)
                for m in re.finditer(r'"url"\s*:\s*"((?:[^"\\]|\\.){0,400})"', v):
                    try:
                        urls.add(json.loads('"' + m.group(1) + '"'))
                    except Exception:
                        pass
    urls = sorted(u for u in urls if 0 < len(u) < 600)
    rng = random.Random(20260830)
    rng.shuffle(urls)
    for i, u in enumerate(urls[:3000]):
        c = case(u, baseUrl="https://www.example.com/x/y.html", key="关键字")
        if i % 3 == 0:
            c["page"] = 2
        if i % 7 == 0:
            c["source"] = {"url": "https://www.example.com", "enabledCookieJar": True}

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8") as f:
        json.dump(cases, f, ensure_ascii=False, indent=1)
        f.write("\n")
    print(f"{len(cases)} fetch cases -> {OUT}", file=sys.stderr)


if __name__ == "__main__":
    main()
