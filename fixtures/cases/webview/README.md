# webview 差分用例契约(BackstageWebView 的**策略层**)

入口:`tools/webview_diff.sh`(先跑 `tools/gen_webview_cases.py` 重建用例)。
裁判 `:wvharness`,被测 `difftest::webview_case_runner`(底下是 `net::webview`)。

## 这一套钉的是什么:webView 的**两半**

plan §3 写的是「BackstageWebView 不进自动差分(Phase 3 手工清单)」。那句话对
**WebView 本身**成立,对**它外面那层策略**不成立 —— 两半要分开算账:

- **不能差分的那半**:真的去加载一个页面、执行页面自己的脚本、过盾。
  裁判那侧是 Android 的 `WebView`,快照录不下来,只能上真机逐个源看
  (那才是 `fixtures/phase3/checklist.json` 那份手工清单的分母)。
- **能差分的那半 —— 就是本套**:`BackstageWebView.kt` 那 394 行里,
  **WebView 之外全是策略**。策略两侧都得逐字节一致,而它完全可以在
  「WebView 换成同一份确定性剧本」的前提下比。

本套钉住的策略面(每一条都有专门的用例,见 `tools/gen_webview_cases.py`):

| 策略 | 真身位置 | 观察面 |
|---|---|---|
| 默认 JS 是 `document.documentElement.outerHTML` | `companion object JS` | `evals[].js` |
| `javaScript == null && delayTime == 0` → delay 补 **900** | `getStrResponse()` 头几行 | `evals[0].at` |
| `onPageFinished` 之后 `100L + delayTime` 起跑 | `HtmlWebViewClient.onPageFinished` | `evals[0].at` |
| **后来的 `onPageFinished` 把求值重排**(`removeCallbacks` + `postDelayed`),`retry` 不归零 | `HtmlWebViewClient.onPageFinished` | `evals[].at` |
| `setCookie(url)` 与 `window.result = …` 是**每次** `onPageFinished` 都做 | 同上 | `cookies` / `evals` |
| 嗅探客户端那边**不撤旧的**:每次加载完成排一发,JS 因此跑好几次 | `SnifferWebClient.onPageFinished` | `evals[].at` |
| 取不到结果按 `200/400/600/800/1000` 重试(之后一直 1000) | `EvalJsRunnable.intervals` | `evals[].at` 的差 |
| `retry > 30` → 报「js执行超时」 | `handleResult` | `error` |
| 结果过 `unescapeJson` 再剥掉**首尾**引号 | `handleResult` | `body` |
| 跟过重定向 → `priorResponse(302)` 的 StrResponse | `buildStrResponse` | `url` / `isRedirect` |
| `sourceRegex`/`overrideUrlRegex` 非空 → 换 `SnifferWebClient`,回**资源 URL 本身** | `createWebView` / `SnifferWebClient` | `body` |
| 嗅探客户端在 `onPageFinished` 之后才跑 JS,且**不收结果** | `SnifferWebClient.onPageFinished` | `evals` |
| 页面加载完把 `CookieManager` 的 cookie 抄进 `CookieStore` | `setCookie(url)` | `cookies` |
| UA 取 `headerMap` 里的 `User-Agent`(先精确后忽略大小写),其余头转发,`cookieJar`/`proxy` 不外发 | `toWebViewRequestConfig`(**真身挂载**,不是垫片) | `ua` / `headers` |
| `timeout ?: 60000` 到点 → `withTimeout` 抛 | `getStrResponse()` | `error` |

## 这份剧本谁在用(M3c 起不止本套)

`{"webView": true}` 的书源、`@webjs:` 规则、`java.webView*` 三处都走
`BackstageWebView` 的策略层,所以 **fetch / rule-engine / js-host / pipeline
那几套的 case 也可以带一个 `webview` 剧本** —— 字段与语义与本套逐字同一份
(解析:裁判侧 `harness.parseWvScript` / `jsharness.parseWvScript`,
被测侧 `difftest::wv_script::WvScript::parse`)。

**不带 `webview` 字段 = 缺省剧本**:页面加载得完、每次求值都回 `"null"` ——
于是重试梯子跑满、报「js执行超时」。语料里的 webView 源在自动差分里就是这一档:
**这一页录不下来,两侧都到不了内容**。那是诚实的结局,不是假装取到了东西
(真要看那些源能不能跑,是 `fixtures/phase3/checklist.json` 那份手工清单的事)。

裁判侧那三个 harness 的**时钟推法不同**,契约相同:

| harness | 挂载 | 时钟 |
|---|---|---|
| `:wvharness`(本套) | 真身 + 录痕迹的垫片 | 虚拟时钟,从**外面** drain(`evals[].at` 是观察面) |
| `:harness` / `:jsharness` | 真身 + 各自的 shims | **内联泵**(`WvStage`):入队即就地跑完 |

为什么那两个不能照抄本套:它们的入口(`AnalyzeUrl.getStrResponse()` /
`AnalyzeRule.getWebJsResult()` / `JsExtensions.webView()`)都是 `runBlocking { … }`
—— 协程一挂起就没人再喂那个事件循环,从外面 drain 直接死锁。内联泵让回调在
`suspendCancellableCoroutine` 那个块**返回之前**就 resume,协程根本不挂起。
`withTimeout` 的那个数(`AnalyzeUrl` 60000 / `getWebJsResult` 10000)由剧本
WebView 顺着 `HtmlWebViewClient` 的 `this$0` 反射读回来。

## 确定性 WebView:剧本契约(两侧逐字实现同一份)

真 WebView 换成一份**剧本**:case 里写清「加载这一页会依次发生什么」,
两侧照着放事件。剧本字段(`webview` 对象):

```json
{
  "id": "wv00001",
  "op": "webview",
  "url": "https://a.example/p",
  "html": null,
  "encode": null,
  "tag": "https://a.example",
  "headerMap": {"User-Agent": "UA/1", "cookieJar": "1"},
  "javaScript": null,
  "sourceRegex": null,
  "overrideUrlRegex": null,
  "delayTime": 0,
  "timeout": null,
  "webview": {
    "overrideUrls": [{"url": "https://a.example/q", "isRedirect": true}],
    "loadResources": ["https://cdn.example/a.mp3"],
    "pageFinished": true,
    "pageFinishedUrl": "https://a.example/q",
    "evals": ["null", "\"<html>x</html>\""],
    "cookie": "sid=1",
    "laterPageFinished": [{"at": 700, "url": "https://a.example/r"}]
  }
}
```

事件顺序**照真 WebView**:`loadUrl` / `loadDataWithBaseURL` 之后,
① `overrideUrls` 逐条喂 `shouldOverrideUrlLoading`(返回 true 就到此为止 ——
嗅探客户端已经出结果了);② `loadResources` 逐条喂 `onLoadResource`;
③ `onPageFinished(pageFinishedUrl ?: 请求地址)`。

`pageFinished: false` = 这一次**永远加载不完**(缺省 true)。真机上那就是干等到
`withTimeout` 到点 —— 没有这一位就钉不住超时那条路。

**`laterPageFinished` = 后来的加载完成**(`at` 是从 `loadUrl` 那一刻起算的毫秒)。
跳转站(`Redirecting…` 那种页面自己 navigate 走)一次加载有**两次甚至更多**
`onPageFinished`,而真身每来一次都 `removeCallbacks(runnable)` +
`postDelayed(runnable, 100 + delayTime)` —— **求值排期被重排**,取到的是
**最后一次**之后的页面。`retry` 不跟着归零(它是 runnable 的字段,真身复用同一个
对象),`buildStrResponse` 用的仍是**第一次**那张页的地址(runnable 只建一次、
捕获的是那时的 `url`)。

> 这一位是 **M3h** 补的,补它的理由是真机验收:`m.bqg225.com` 在 macOS 上出书、
> 在真机上只拿回 **538 字节的 `<head>`** —— 页面正跳转到一半就被求值了。
> 此前的剧本**只能有一次** `onPageFinished`,于是这条策略两侧都没被比过。
> **「差分全绿」不等于「面都比过了」,只等于「比过的面一致」。**

「请求地址」= `loadUrl` 的地址;`loadDataWithBaseURL(baseUrl = null, …)` 那一支是
`about:blank`(真 WebView 的行为,两侧同此)。

`evals` 是 **每一次 JS 求值**的回值(**JS 侧的原样串** —— Android 给的就是 JSON
编码后的串,所以 `"null"` 表示 JS 返回 null、`"\"abc\""` 表示 JS 返回字符串
`abc`)。**用完之后一直重复最后一个**,这样「一直取不到结果」的重试梯子只要写
一个 `"null"` 就能钉;剧本没给 `evals` 时一律当 `"null"`。

**「每一次」包括不收结果的那两次**:`isRule` 那条路的
`evaluateJavascript("window.result = …", null)`,以及嗅探客户端的
`loadUrl("javascript:…")` —— 它们照样占一条回值、照样进 `evals` 观察面。
真身那边它们走的就是同一个 WebView 入口,分开记就等于假装 JS 没跑过。

## 时间:两侧用**两种办法**得到同一条时间轴

- **裁判侧**是虚拟时钟(`wvharness/VirtualClock.kt`):`Handler.postDelayed`
  与「加载完成」全排进队列,执行器按 `(到点时间, 入队序)` 推进 —— 不真的睡。
  真身那 394 行**一个字节没改**,时间是从它底下抽走的。
- **被测侧不需要时钟**:移植版把**排期算在策略层**(`net::webview` 自己推
  `now`:补 900、`100 + delayTime`、重试梯子),平台只负责「等到那一刻」
  (`WebViewHost::wait`)。差分侧的 `wait` 只把时刻记下来;产品侧才真的等。

两条路得到的 `evals[].at` 直接对拍。于是 `900 + 100`、重试梯子
`200/400/600/800/1000`、`retry > 30` 与 60 s 超时**逐毫秒可比**,
而整套 89 例跑完不到一秒。

**这也是接口的分界线**:凡是能算的都在策略层(可差分),平台只剩
「加载」「求值」「等」「还」四件原语(`rubato-core::host::WebViewHost`)。

## 输出(规范化 JSONL,两侧逐字节比)

```json
{"id": "wv00001",
 "url": "https://a.example/q", "body": "<html>x</html>", "isRedirect": true,
 "evals": [{"at": 1000, "js": "document.documentElement.outerHTML"}],
 "ua": "UA/1", "cacheMode": -1, "blockNetworkImage": true,
 "loadedUrl": "https://a.example/p", "loadedHtml": null, "loadedEncoding": null,
 "headers": {"referer": "…"},
 "cookies": {"https://a.example": "sid=1"},
 "released": 1}
```

出错时 `url`/`body`/`isRedirect` 换成一个 `error`,**别的字段照出**
(`evals`、`headers`、`released` 在出错的路上一样要比 —— 出错前跑了几次 JS、
WebView 还没还,都是判据):`timeout`(`withTimeout` 到点,含「页面一直加载
不完」)/ `js_timeout`(「js执行超时」)/ `NullPointerException`(url 为 null
且没有 html)/ `PatternSyntaxException` / `IllegalArgumentException` / …

`released` 是 `destroy()` 被调用的次数 —— 真身在拿到结果、报错、超时三条路上
都要还 WebView,漏还在真机上是**池子里泄漏一个 WebView**,差分里只有这一位
看得见。

`loadedUrl` / `loadedHtml` / `loadedEncoding` 钉的是「发给平台的是哪一次加载」:
`loadUrl(url)` / `loadUrl(url, headers)` / `loadDataWithBaseURL(url, html,
"text/html", encode ?: "utf-8", url)` 三条分支怎么选、参数怎么填。
`headers` 为空对象时真身走的是**不带头的 `loadUrl` 重载** —— 这一位有语义。

## 不在本套里的(说清楚,免得把这套的 pass 率读大了)

- **真的加载页面**:剧本不模拟网络、不跑页面自己的脚本、不过盾。
  「这个源能不能过盾」是手工清单的事(`fixtures/phase3/README.md`)。
- **`isRule = true` 的 JS 注入那半**:`isRule` 会让真身给 WebView 挂三个
  JavascriptInterface(`WebCacheManager` / `BaseSource` / `WebJsExtensions`)。
  本套钉住的是它**在 JS 上的投影**(拼在前面的 `getInjectionString`、
  `window.result = …` 那一次求值),挂接口本身不钉。
  真身那三个名字是**每进程随机**的,两侧各钉一个固定串(`__wvCache` 等)。

  > 走这条路的只有 `AnalyzeRule.getWebJsResult`(`@webjs:`,M3c 起两侧都是
  > 真策略,判据在 rule-engine 那套)与 RssSource 的正文页 —— 后者整块砍了
  > (plan §6)。`AnalyzeUrl` 与 `java.webView*` 都不给 `isRule`。
- **WebView 池**(`WebViewPool`)的复用与超时回收:垫片一律新建一个,
  `release` 只标记。池化对本套的观察面没有投影(`destroy()` 那一位除外)。

  > 但**「还回去之后还送不送事件」有投影**(M3c 补):真身 `release` 会
  > `stopLoading()` 并把 `webViewClient` 换成池子自己的,所以嗅探命中
  > (`onLoadResource` 出结果)之后,后面的 `onPageFinished` 根本送不到
  > BackstageWebView 手上 —— **cookie 也就不抄**。三份剧本都按这一条走
  > (wv00053 / wv00054 专钉它);此前剧本照送,而被测侧命中即返回,
  > 那是一处没人踩到的单边差异(生成器恰好没写「命中 + tag」那一组)。
