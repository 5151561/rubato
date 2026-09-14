#!/usr/bin/env python3
"""生成 webview 差分用例(`BackstageWebView` 的**策略层**)。

契约与观察面见 `fixtures/cases/webview/README.md`;裁判 `judge/wvharness`
(挂真身那 394 行 + 剧本 WebView + 虚拟时钟),被测 `difftest::webview_case_runner`
(底下是 `net::webview`)。

**手写而不是从语料抽**:这一套钉的是策略,而策略的分支是从真身源码上读出来的
(补 900ms / 重试梯子 / 嗅探 / 重定向 / cookie 回抄 / unescapeJson …),
语料里的 98 个 webView 源只会反复踩同一条最平常的路。语料那一面归
`fixtures/phase3/checklist.json` 的手工清单管。
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "fixtures" / "cases" / "webview" / "generated.json"

CASES = []


def case(**kw):
    c = {"id": f"wv{len(CASES) + 1:05d}", "op": "webview"}
    c.update(kw)
    CASES.append(c)
    return c


def wv(evals=None, page=None, resources=None, overrides=None, cookie=None, later=None):
    o = {}
    if overrides is not None:
        o["overrideUrls"] = overrides
    if resources is not None:
        o["loadResources"] = resources
    if page is not None:
        o["pageFinishedUrl"] = page
    if evals is not None:
        o["evals"] = evals
    if cookie is not None:
        o["cookie"] = cookie
    if later is not None:
        # `[(第几毫秒, 那张页的地址)]` —— 后来的 onPageFinished
        o["laterPageFinished"] = [{"at": at, "url": u} for at, u in later]
    return o


U = "https://a.example/p"
OK = '"<html>ok</html>"'


def main():
    # ── ① 起跑时刻:`javaScript == null && delayTime == 0` 补 900,之后 100 + delayTime ──
    # 空串的 javaScript **不是 null**:不补 900,但 getJs() 仍回默认那句 —— 两条分支
    # 分别由 `javaScript == null` 与 `it.isNotEmpty()` 判,差一个字就是两种行为
    for js in (None, "", "document.title", "1+1"):
        for delay in (0, 1, 500):
            case(url=U, javaScript=js, delayTime=delay, webview=wv(evals=[OK]))

    # ── ② 重试梯子:200/400/600/800/1000,之后一直 1000 ──
    for n in range(0, 8):
        case(url=U, webview=wv(evals=["null"] * n + [OK]))
    # 空串的回复也算「没结果」(`result.isNotEmpty()`),与 "null" 同档
    case(url=U, webview=wv(evals=["", "", OK]))
    # 字面量 "null" 与 JSON 串 "\"null\"" 不是一回事:后者是**取到了**内容 "null"
    case(url=U, webview=wv(evals=['"null"']))
    # retry > 30 → 「js执行超时」(32 次求值)
    case(url=U, webview=wv(evals=["null"]))
    case(url=U, javaScript="x", delayTime=0, webview=wv(evals=["null"]))

    # ── ③ timeout:`withTimeout(timeout ?: 60000)` ──
    for t in (99, 100, 1000, 1001, 1200):
        case(url=U, timeout=t, webview=wv(evals=["null", OK]))
    # 页面永远加载不完(剧本说这一次没有 pageFinished)—— 只能等超时
    case(url=U, timeout=5000, webview={"evals": [OK], "pageFinished": False})

    # ── ④ 重定向:`priorResponse(302)` 那一支 ──
    case(url=U, webview=wv(evals=[OK], overrides=[{"url": "https://a.example/q", "isRedirect": True}],
                           page="https://a.example/q"))
    case(url=U, webview=wv(evals=[OK], overrides=[{"url": "https://a.example/q", "isRedirect": False}],
                           page="https://a.example/q"))
    # 多跳:只要有一跳是转向,整次就算转向(`isRedirect = isRedirect || …`)
    case(url=U, webview=wv(evals=[OK], overrides=[
        {"url": "https://a.example/q", "isRedirect": False},
        {"url": "https://a.example/r", "isRedirect": True},
    ], page="https://a.example/r"))
    # 转向到一个 okhttp 收不下的地址 → 那一支**手搭 Response**,会抛
    case(url=U, webview=wv(evals=[OK], overrides=[{"url": "x", "isRedirect": True}],
                           page="thunder://abc"))
    # 没转向时 `StrResponse(url, body)` 的构造器**吞**掉非法地址 → http://localhost/
    case(url=U, webview=wv(evals=[OK], page="thunder://abc"))
    # 地址规范化:okhttp 会补斜杠、压端口
    case(url="https://a.example", webview=wv(evals=[OK], page="https://a.example:443"))

    # ── ⑤ 嗅探客户端:sourceRegex / overrideUrlRegex ──
    case(url=U, sourceRegex=r".*\.mp3", webview=wv(evals=[OK], resources=["https://cdn.example/a.mp3"]))
    case(url=U, sourceRegex=r".*\.mp3", webview=wv(evals=[OK], resources=["https://cdn.example/a.mp4"]))
    # `matches` 是**整串**匹配:`\.mp3` 匹配不上整个地址
    case(url=U, sourceRegex=r"\.mp3", webview=wv(evals=[OK], resources=["https://cdn.example/a.mp3"]))
    case(url=U, sourceRegex=r".*\.mp3", timeout=3000,
         webview=wv(evals=[OK], resources=["https://cdn.example/a.mp4", "https://cdn.example/b.mp3"]))
    case(url=U, overrideUrlRegex=r".*/final", webview=wv(evals=[OK],
         overrides=[{"url": "https://a.example/final", "isRedirect": False}]))
    case(url=U, overrideUrlRegex=r".*/final", timeout=3000, webview=wv(evals=[OK],
         overrides=[{"url": "https://a.example/other", "isRedirect": False}]))
    # 嗅探 + javaScript:JS **跑**(loadUrl("javascript:…"))但结果不收 → 只能等超时
    case(url=U, sourceRegex=r".*\.mp3", javaScript="doSniff()", timeout=3000,
         webview=wv(evals=[OK], resources=["https://cdn.example/a.mp4"]))
    case(url=U, sourceRegex=r".*\.mp3", javaScript="doSniff()", delayTime=400, timeout=3000,
         webview=wv(evals=[OK], resources=["https://cdn.example/a.mp4"]))
    # 空白的正则不算「配了」——仍走取网页源码那条路
    case(url=U, sourceRegex="   ", webview=wv(evals=[OK]))
    case(url=U, overrideUrlRegex="", webview=wv(evals=[OK]))
    # 编译不了的正则:真身 `it.toRegex()` 当场抛
    case(url=U, sourceRegex="(", webview=wv(evals=[OK], resources=["https://cdn.example/a.mp3"]))

    # ── ⑥ cookie 回抄:`tag?.let { CookieStore.setCookie(it, CookieManager.getCookie(url)) }` ──
    case(url=U, tag="https://a.example", webview=wv(evals=[OK], cookie="sid=1"))
    case(url=U, tag="https://a.example", webview=wv(evals=[OK]))
    case(url=U, webview=wv(evals=[OK], cookie="sid=1"))
    # 抄的是**加载完成那一刻的地址**,不是请求地址
    case(url=U, tag="k", webview=wv(evals=[OK], page="https://a.example/q", cookie="sid=2"))
    # 嗅探那条路也抄(onPageFinished 里同一句)—— **没命中**才走得到那里
    case(url=U, tag="k", sourceRegex=r".*\.mp3", timeout=3000,
         webview=wv(evals=[OK], resources=["https://x/a.mp4"], cookie="sid=3"))
    # **命中就不抄了**:`onLoadResource` 里出结果之后 WebView 当场还回池子
    # (真身 `WebViewPool.release` 会 stopLoading + 换掉 webViewClient),
    # 后面的 `onPageFinished` 根本不会送到 BackstageWebView 手上 ——
    # 于是 cookies 是空的。剧本两侧都照这一条停送事件
    case(url=U, tag="k", sourceRegex=r".*\.mp3",
         webview=wv(evals=[OK], resources=["https://x/a.mp3"], cookie="sid=4"))
    # overrideUrl 命中同理
    case(url=U, tag="k", overrideUrlRegex=r".*/final",
         webview=wv(evals=[OK], overrides=[{"url": "https://a.example/final",
                                            "isRedirect": False}], cookie="sid=5"))

    # ── ⑦ headerMap → UA 与转发头(`toWebViewRequestConfig` 真身) ──
    case(url=U, headerMap={"User-Agent": "UA/1", "referer": "https://r"}, webview=wv(evals=[OK]))
    case(url=U, headerMap={"user-agent": "UA/2"}, webview=wv(evals=[OK]))
    case(url=U, headerMap={"User-Agent": "   "}, webview=wv(evals=[OK]))
    case(url=U, headerMap={"User-Agent": "   ", "referer": "r"}, webview=wv(evals=[OK]))
    case(url=U, headerMap={"CookieJar": "1", "proxy": "socks://x", "referer": "r"}, webview=wv(evals=[OK]))
    case(url=U, headerMap={"cookiejar": "1", "PROXY": "y"}, webview=wv(evals=[OK]))
    case(url=U, headerMap={}, webview=wv(evals=[OK]))

    # ── ⑧ html 分支:loadDataWithBaseURL + getEncoding() ──
    case(url=U, html="<b>x</b>", webview=wv(evals=[OK]))
    case(url=U, html="<b>x</b>", encode="gbk", webview=wv(evals=[OK]))
    case(url=U, html="", webview=wv(evals=[OK]))
    case(url=None, html="<b>x</b>", webview=wv(evals=[OK], page="about:blank"))
    case(url=None, webview=wv(evals=[OK]))

    # ── ⑨ cacheFirst → cacheMode ──
    case(url=U, cacheFirst=True, webview=wv(evals=[OK]))

    # ── ⑩ result / isRule:注入与 window.result ──
    case(url=U, result="RES", webview=wv(evals=["null", OK]))
    case(url=U, isRule=True, webview=wv(evals=[OK]))
    case(url=U, isRule=True, javaScript="f()", webview=wv(evals=[OK]))

    # ── ⑪ unescapeJson + 剥首尾引号 ──
    for reply in (
        r'"a\nb"', r'"a\tb"', r'"a\\b"', r'"a\"b"', r'"a\/b"', r'"中文"',
        r'"\101\102"', r'"aA"', '"x"', 'x', '"x', 'x"', '""', '"""',
        r'"line1\r\nline2"', r'"\b\f"', r'"\u0041\u00e9"', r'"\uu0041"', r'"\u00"',
    ):
        case(url=U, webview=wv(evals=[reply]))

    # ── ⑫ 后来的 onPageFinished:**求值排期被重排**(真身 removeCallbacks + postDelayed) ──
    #
    # 跳转站(`Redirecting…` 那种页面自己 navigate 走)一次加载有**两次**
    # onPageFinished。真身每来一次都把还没跑的 `EvalJsRunnable` 撤掉、重排
    # `100 + delayTime` —— 取到的因此是**最后一次**之后的页面。只认第一次就会把
    # 加载到一半的 DOM 交上去(真机验收:`m.bqg225.com` 只拿回 538 字节的 `<head>`)。
    Q = "https://a.example/q"
    # 第二次在第一次求值(0+100+900=1000)之前 → 排期重来,求值只发生一次、在 1700
    case(url=U, webview=wv(evals=[OK], later=[(700, Q)]))
    # 第二次在第一次求值之后 → 第一次已经出结果并 destroy 了,后面的送不到
    case(url=U, webview=wv(evals=[OK], later=[(1500, Q)]))
    # 一直取不到结果时,后来的加载完成把重试梯子**重排**(retry 不归零 ——
    # 它是 runnable 的字段,真身复用同一个对象)
    case(url=U, webview=wv(evals=["null", "null", OK], later=[(1300, Q)]))
    # 三次加载完成:每一次都重排,最后一次说了算
    case(url=U, webview=wv(evals=[OK], later=[(300, Q), (600, "https://a.example/r")]))
    # `buildStrResponse` 用的是**第一次**那张页的地址(runnable 只建一次,
    # 捕获的是它建的时候那个 url)—— 这一条把「用第几次的地址」钉死
    case(url=U, webview=wv(evals=[OK], page="https://a.example/one", later=[(700, Q)]))
    # cookie **每次**加载完成都抄一遍(setCookie 在 onPageFinished 的第一行)
    case(url=U, tag="k", webview=wv(evals=[OK], page="https://a.example/one",
                                    later=[(700, Q)], cookie="sid=6"))
    # `result` 那句 `window.result = …` 也是**每次** onPageFinished 都跑
    case(url=U, result="RES", webview=wv(evals=["null", "null", OK], later=[(700, Q)]))
    # 重排之后越过 deadline → 超时(1700 > 1500)
    case(url=U, timeout=1500, webview=wv(evals=[OK], later=[(700, Q)]))
    # 嗅探客户端那边**不撤旧的**:每次加载完成排一发 LoadJsRunnable,
    # 于是 JS 跑两次(真身 SnifferWebClient.onPageFinished 没有 removeCallbacks)
    case(url=U, sourceRegex=r".*\.mp3", javaScript="doSniff()", timeout=3000,
         webview=wv(evals=[OK], resources=["https://cdn.example/a.mp4"], later=[(700, Q)]))

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(CASES, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(CASES)} webview cases -> {OUT}", file=sys.stderr)


if __name__ == "__main__":
    main()
