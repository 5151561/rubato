# js-host 差分套(Phase 2 地基)

**钉什么**:两件。
1. `op: "js"` —— 一段书源 JS 在某个宿主上求值出来是什么:引擎方言 + `java` 宿主
   API + 绑定面(result/baseUrl/src/key/page/…)+ 变量层(`java.put/get`)。
2. `op: "analyzeUrl"` —— **一条 url 规则整条走 `AnalyzeUrl`**(`@js:` / `<js>` /
   `{{js}}` / `<a,b,c>` 页码 + option JSON + 绝对化 + query/form 编码 +
   `getHeaderMap()`),而 JS 是**真引擎**。语料是 1704 源的真实 `searchUrl`。

**不钉什么**:规则的切分、调度与拼接(那是 `rule-engine` 套,那边的 JS 是确定性桩);
四步流水线(`pipeline` / `pipeline-corpus` 两套);**HTTP 客户端本身**(那是 `fetch`
与 `analyze_url` 两套)。**书源 JS 里的网络调用**(`java.ajax` 等)从 M2c 起在本套 ——
钉的是「这段 JS 会发出什么请求、拿到响应后算出什么」,见下「网络面」。

| | 裁判 | 被测 |
|---|---|---|
| 进程 | `judge/jsharness`(`./gradlew :jsharness:run`) | `rust/crates/difftest/src/bin/js_case_runner.rs` |
| JS 引擎 | **真 Rhino**(htmlunit-core-js 5.3.0-legado.4,`judge/rhino` 真身) | rquickjs(quickjs-ng) |
| `java` 绑定 | `AnalyzeRule` 真身(实现 `JsExtensions` 真身) | `js-host::host_env::QuickJsHost` |
| 网络 | `AnalyzeUrl` 真身 + `jsharness/ReplayHttp`(录放) | `net::AnalyzeUrl` + `difftest::js_net::ReplayNet` |
| op | `js`(一段 JS)/ `analyzeUrl`(一条 url 规则) | 同左 |
| 入口 | `tools/js_diff.sh` | 同左 |

> `judge/jsharness` 与 `judge/harness` 是**两个模块**:同一个 classpath 上
> `com.script` 只能有一份,而 harness 那边是有意为之的确定性桩(Phase 1 的
> 92362 例绿全建立在它上面)。jsharness 挂真身 com.script,两边互不影响。
> jsharness 的 shims 从 harness 那套**复制**了一部分(实体/Room/Flexbox 等),
> 复制不是共享:改 jsharness 的垫片不会动 harness 的成绩,反之亦然。

## 三个宿主(`host` 维度)

真身有**三个** JS 宿主,`java` 指向的对象与绑定面都不同。挑错宿主会把语义钉歪,
所以用例带一个 `host` 字段,两侧照它分派。

| | `host: "rule"`(缺省) | `host: "url"` |
|---|---|---|
| 入口 | `AnalyzeRule.evalJS`(AnalyzeRule.kt L893) | `AnalyzeUrl.evalJS`(AnalyzeUrl.kt L377) |
| `java` 是谁 | **AnalyzeRule 本身** —— JS 能反过来调规则求值(`getString`/`getElements`/`setContent`) | AnalyzeUrl —— **只有 `JsExtensions` 那一面**,`java.getString` 在这里是 TypeError |
| 独有绑定 | `src` / `title` / `nextChapterUrl` / `fromBookInfo` / `chapter` | `key` / `speakText` / `speakSpeed` / `infoMap` |
| 共有绑定 | `result` / `baseUrl` / `page` / `book` / `source` / `cookie` / `cache` | 同左 |
| 变量层 `java.get` | 先查 `localBindings`,再 chapter → book → ruleData → source | 先查 `extraParams`,再 chapter → book(**没有 localBindings 那层**) |
| 哪些规则位走它 | 除 searchUrl/exploreUrl 外的全部 | `searchUrl` / `exploreUrl` |

**这条是踩出来的**:首轮 803 例里有 64 例的裁判报 `ReferenceError: "key" 未定义` ——
因为整套都跑在 `AnalyzeRule` 上,而 `AnalyzeRule.evalJS` **根本不绑 `key`**。
但那些片段来自 `searchUrl`,产品里它们跑的是 `AnalyzeUrl.evalJS`,`key` 必须有值。
照着 FAIL「实现」成 ReferenceError 会直接把搜索做死。故这不是实现缺口,是**契约缺口**。

被测侧对应 `rubato_core::host::JsHost`;裁判侧对应 jsharness 里
`AnalyzeUrl` 垫片上那三个**逐字复制自真身**的方法(`evalJS`/`put`/`get`)——
该垫片的网络面仍是桩,只有 JS 宿主面做真。

### 第三个:`host: "source"`(`BaseSource.evalJS`,BaseSource.kt L395)

source 的 `header` / `loginUrl` / `loginCheckJs` 走它(B 层 274 源有 `header`)。
绑定面**最窄**,而且和另两个宿主的差别不只是「少几个名字」:

| | `host: "source"` |
|---|---|
| 入口 | `BaseSource.evalJS` |
| `java` 是谁 | **书源实体本身** —— 探针 `js-src-bind-same-object` 实测 `java === source === sourceApi` 为 true |
| 绑定 | 只有 `java` / `source` / `sourceApi` / `baseUrl` / `cookie` / `cache` |
| `baseUrl` | **`getKey()`(bookSourceUrl)**,不是页面地址 |
| 未声明的名字 | `result` / `book` / `chapter` / `page` / `key` —— 是 **ReferenceError**,不是 null(探针 `js-src-bind-undeclared`) |
| `java.put/get` | 打 **CacheManager**(`v_<sourceKey>_<key>`),**不是**那套四层变量 |
| 规则反调面 | 没有(`java.getString` 是 TypeError) |

登录那一面也在这个宿主上:`getVariable`/`putVariable`/`setVariable`
(`sourceVariable_<key>`)、`getLoginHeader`/`putLoginHeader`
(`loginHeader_<key>`)、`getLoginInfo`/`putLoginInfo`
(`userInfo_<key>`,**AES(androidId 前 16 字节) + base64** 往返)。
探针 `js-src-*` 共 23 条。

**`source` 绑定在三个宿主上都带这一面** —— 它就是书源实体。差分抓到:
searchUrl 的 `@js:` 里 `source.getVariable()` / `source.setVariable(v)` 是常见写法
(书源用它记「用户选了哪个源站」),被测侧当初只给了 `getKey`/`getTag`,直接 TypeError。

## 用例形状

```json
{
  "id": "js-corpus-009985b177",
  "op": "js",
  "host": "rule",
  "code": "var id = result.match(/_(\\d+)/)[1];",
  "result": "第 12 章 混沌雷池",
  "baseUrl": "https://difftest.example.com/book/1",
  "content": "<html>…</html>",
  "title": "第 12 章",
  "nextChapterUrl": "https://difftest.example.com/book/1/3.html",
  "key": "斗破苍穹",
  "page": "2",
  "fromBookInfo": false,
  "vars": { "k": "v" },
  "source": "<BookSource 的 JSON 串,可省>"
}
```

- `host` 缺省 `"rule"`,见上「两个宿主」;`"url"` 的用例不吃 `content`/`title`/
  `nextChapterUrl`(那个宿主不绑);
- `resultRule`:**`result` 绑的不是串时**用它 —— 给一条规则,两侧都走**真身
  `getElement`** 拿到那个对象本身再绑(与产品路径逐字同一条:`getElement` →
  `bind_value` → `evalJS`)。见下「`result` 的形态」。带了它就不看 `result`。
  约束:里面的 JS 只许是**纯表达式** —— 被测侧那一步跑在另一份 QuickJsHost 上
  (本套的 `AnalyzeRule` 平时挂空壳,只在这一步临时换上真引擎),日志与 cache
  不汇进本 case 的观察面;裁判侧是同一个 AnalyzeRule,会汇。
- `content` 走 `AnalyzeRule.setContent(content, baseUrl)` → JS 里的 `src`;
- `page` 走 `setLocal("page", …)`(真身 `AnalyzeUrl` 就是这么塞的),值是**串**,
  JS 侧按 `toIntOrNull() ?: 原串`;
- `vars` 是 `Book.putVariable` 的初值(`java.put/get` 打到的那层);
- 缺省 `source` = `{"bookSourceUrl":"https://difftest.example.com","bookSourceName":"difftest",…}`;
- `fallback`:HTTP 录放的**回落页**名(`json` / `html`,缺省 `json`),见下「网络面」;
- `cache`:**预置 CacheManager**(键值原样落表)。`loginHeader_<key>` /
  `userInfo_<key>` / `sourceVariable_<key>` 这几条路要先有值才测得到读取那一半。

## 输出形状(两侧逐字段比较)

```json
{"id": "...", "type": "...", "str": "...", "norm": "...", "vars": {...}, "logs": [...]}
{"id": "...", "error": "..."}                     // 求值抛异常
{"id": "...", "type": "...", "normError": "..."}  // 值本身没问题,但字符串化那步抛
```

- **`type`**:`null` / `bool` / `number` / `string` / `list` / `object` / `wrapped` / `other`。
- **`str`**:`AnalyzeRule` 的消费口径(`AnalyzeRule.kt` L798-806)——`<js>` 的结果
  真正拼进规则时长什么样。`null`→缺省;`String`→原样;`Double` 且整数值→`%.0f`;
  其余→`toString()`。
- **`norm`**:`JsSourceEngine.normalizeJsResult` 的口径 —— **header JS / `@js:`**
  那条路径的字符串化(对象走引擎自己的 `NativeJSON.stringify`,不是 gson 反射)。
- **`vars`**:变量层终态(按 key 排序)。
- **`logs`**:`java.log` / `java.toast` 的落点。
- **`cache`**:`CacheManager` 终态(裁判侧 `CacheManager.snapshot()`,按 key 排序)。
  `source` 宿主的 `java.put/get` 打的就是这张表,登录头 / 登录信息 / 源变量也落它。
  空则不出这个字段。
- **`cacheFile`**:**ACache 终态**(`cache.putFile/getFile` 那张,裁判侧
  `CacheManager.fileSnapshot()`)。与 `cache` 是**两处存储** —— 真身
  `CacheManager.put/get` 打 cacheDao + 内存,`putFile/getFile` 打 ACache(落盘),
  写一张读另一张读不到。合成一张会把「写内存、读磁盘」这类分歧照成一致。空则不出。
- **`hops`**:本 case **发出的请求序列**,每项 `"<METHOD> <规范化 URL>"`。
  这是 `ajax` 类用例最要紧的观察面 —— URL 拼装、方法、跳数两侧最容易走岔;
  一个请求都没发就不出这个字段。**出错路径上也记**(异常往往正是多发/少发一跳
  造成的),故裁判侧是在 catch 清完字段**之后**才挂它。
  响应体不比 —— 两侧回放的是同一份字节。
- **`error`**:归一后的错误**类别**,不是消息文本 —— 两个引擎的消息必然不同。
  - `js:SyntaxError` / `js:ReferenceError` / `js:TypeError` / `js:RangeError` / `js:throw`
  - `host:<异常简名>`(JS 里调宿主 API,宿主抛了)
  - `unsupported:net` / `unsupported:fs`(见下)
  - `timeout`(单 case 5 秒)

## `op: "analyzeUrl"` —— **`@js:` url 面**(M2f)

一条真实的 `searchUrl` 整条走 `AnalyzeUrl`:`analyzeJs`(`@js:`/`<js>` + `@result`
占位)→ `replaceKeyPageJs`(`{{js}}` 内嵌 + `<a,b,c>` 页码)→ `analyzeUrl`
(切 option JSON、绝对化、套用 option、按 method 编码 query/form),**JS 是真引擎**。

| | 裁判 | 被测 |
|---|---|---|
| 主体 | `AnalyzeUrl` 真身(反射读私有字段) | `net::AnalyzeUrl`(Phase 1 已 0 FAIL 的那份移植) |
| JS | 真 Rhino | rquickjs |
| header | **不传 `headerMapF`** → `source.getHeaderMap()` 真的跑 | `net::source_header::source_header_map` |

与 `analyze-url` 那套(`judge/harness`,两侧 JS 都是**确定性桩**,6613 例 0 FAIL)
的分工:那边钉拼装与编码的全部分支,这边把 JS 打开 —— 钉「书源真的写的那些
url JS 跑出什么」。加权:1704 源里 `searchUrl` 带 `{{}}`/`@js:` 的有 1456 个。

用例形状:
```json
{"id":"js-url-…","op":"analyzeUrl","mUrl":"<searchUrl 原文>",
 "baseUrl":"<bookSourceUrl>","key":"斗破苍穹","page":2,
 "source":"<裁剪过的 BookSource JSON 串>"}
```
投影出 `ruleUrl` / `url` / `urlNoQuery` / `type` / `method` / `body` /
`encodedForm` / `encodedQuery` / `charset` / `proxy` / `retry` / `useWebView` /
`webJs` / `bodyJs` / `dnsIp` / `readTimeoutMs` / `urlTimeoutConfigured` /
`followRedirects` / `webViewDelayTime` / `serverID` / `userAgent` / `isPost` /
`headers`(LinkedHashMap 插入序)/ `vars` / `logs` / `cache` / `hops`。

### 这个 op **不装确定性垫片**

垫片进不去 `AnalyzeUrl` 的内部求值:`@js:`/`{{}}` 是真身自己 eval 的,裁判侧没有
「先在作用域里跑一段再跑用例」的入口(`jsLib` 那条共享作用域在 jsharness 是桩)。
只在被测侧冻时钟会造成**单边**差异,反而更糟 —— 故**两侧都用真时钟**,
摸时钟/随机的书源整条不进本 op(`tools/gen_js_cases.py` 的 `_NONDET`,
扫的是 url **加整份裁剪后的 source**:书源常把 JS 藏在 `bookSourceComment` 里
再 `eval(String(source.bookSourceComment))`)。语料里被排掉的只有 2 条,
而它们的 JS 片段本来就已经由 `op: "js"` 的用例覆盖(那边垫片是前置拼接的)。

判据仍是「两侧各连跑两次逐字节相同」,已通过。

### 书源只带裁剪过的字段

`source` 串只保留 url 与 header 这条路真正读得到的字段(`bookSourceUrl` /
`bookSourceName` / `bookSourceType` / `bookSourceGroup` / `bookSourceComment` /
`enabled` / `enabledCookieJar` / `header` / `loginUrl` / `loginUi` /
`concurrentRate` / `jsLib` / `variableComment` / `searchUrl` / `respondTime` /
`weight` / `customOrder` / `lastUpdateTime`)。两侧同一份,不影响判据;
书源 JS 读 `source.ruleSearch` 那一面由 `op: "js"` 的用例钉(那里带整份 raw)。

### 这个 op 抓出来的实现缺口(M2f 落地)

1. **`org.jsoup.Jsoup.parse` 在 `url` 宿主上也要有**。它是 Rhino 的 LiveConnect
   **全局面**,与 `java` 是谁无关 —— searchUrl 的 `@js:` 里 `java.ajax` 回来再
   `Jsoup.parse(...).select(...).attr(...)` 是常见写法。被测侧的元素面挂在
   `RuleHost` 上,而 `AnalyzeUrl` 那条路的求值环境只有 `RuleData` → 直接 TypeError。
   修法:`net::SplitEnv` —— 变量层仍是 `AnalyzeUrl` 自己那份 `RuleData`
   (真身 `AnalyzeUrl.put/get` 打的就是 ruleData),**元素面**另外注入
   (`UrlArgs.rule_host`;net 建不出 AnalyzeRule,依赖方向不允许)。
   规则求值那几个(`getString`/`getElements`/`setContent`)**不转发** ——
   真身在这个宿主上就没有它们。
2. **option JSON 只有宽松档解得开时会记一条 Debug 日志**
   (`链接参数 JSON 格式不规范,请改为规范格式`,AnalyzeUrl.kt L245)。
   语料里 option 用单引号的写法极多,这一条独占 69 个 FAIL。
3. **嵌套 `AnalyzeUrl` 的日志要汇回外层**。`java.ajax(url)` 会拿 url 串再建一个
   `AnalyzeUrl`,那一次构造自己也会记上面那条日志;真身落的是**全局 `Debug`**,
   外层看得见,而被测侧的嵌套宿主是另一个实例 —— 少一条。
   修法:`NetProvider::take_logs`,js-host 在每次网络调用**之后**取走(顺序对得上)。
4. **header 的值是对象/数组时不是「整块跳过」**。legado 注册了自己的
   `StringJsonDeserializer`(GsonExtensions.kt L135):基元取 `asString`,
   JSON null 给 Java null,**对象/数组给 `toString()`**。实测一条 header 写成
   `{"headers":{"User-Agent":…}}`,真身给的是键 `headers` 配一整段 JSON 串。
5. **`source` 绑定要带 BaseSource 的方法面**(见上「第三个宿主」末尾)。
6. `java.HMacBase64` 缺失(JsEncodeUtils.kt L506)。

## 网络面(**已接**,走 HTTP 录放)

`java.ajax` 是 B 层第一名(123/803 例踩到它),M2c 起两侧都真的发请求 ——
发到 **HTTP 快照**上,不是外网。契约 `docs/http-snapshot.md`。

| | 裁判 | 被测 |
|---|---|---|
| `ajax` / `connect` | `AnalyzeUrl` **真身** + `jsharness/ReplayHttp` | `net::AnalyzeUrl` + `difftest::js_net::ReplayNet` |
| `get` / `head` / `post` | jsoup `Connection` → `jsharness/ReplayUrlStream` | 同一 transport,直接打一跳 |
| 快照根 | `fixtures/http-js-host`(`tools/js_diff.sh` 传第三个参数) | 同左 |

**两条路不一样**,别按同一条实现:
- `ajax`/`connect` 走 okhttp —— 有拦截器链(补 UA / Keep-Alive)、跟进重定向、
  按 urlOption 解析 `,{"method":…}`;
- `get`/`head`/`post` 走 **jsoup 的 `Connection`** —— `followRedirects(false)`、
  `ignoreContentType(true)`、**不过**那条拦截器链,头是调用方给的原样。

### 回落页

语料里的 URL 指向真实站点,不可能有录制。故两侧都配**回落页**
(`docs/http-snapshot.md` §6):未命中的请求一律返回它。用例的 `fallback` 字段挑
`json`(缺省)还是 `html`,两侧读同一个字段、拿到同一份字节 —— 差异仍然只来自实现。

`fixtures/http-js-host/_fallback/{json,html}.json` 与 pipeline-corpus 那两张
是同一份合成页(**复制**,不共享)。

### 裁判侧「一个字节都不许出网」

`ReplayUrlStream` 装了一个全局 `URLStreamHandlerFactory`,把 JVM 的 http/https
`URL.openConnection()` 整条路接到录放上。**这不是锦上添花,是判据的前提** ——
接网络面之前,`java.get/head/post` 走 jsoup,而 jsoup 底下是 `HttpURLConnection`,
那条路当时**没人拦**:实测 `js-corpus-ea97f51e04` 把 `https://www.35xss.com/` 的
**实时 302 Location** 当成了裁判结论。那不是判据,是当天的外网。

宿主自己的 okhttp 不受影响(它走裸 socket,本来就已经由 shims 的 `newCall*` 转到
ReplayHttp)。**但书源可以自己建一个**:`new Packages.okhttp3.OkHttpClient()` 是
LiveConnect 直接拿真身的类,既不经 shims 也不经 URLStreamHandler —— 这是 M2e 之前
最后一个洞。实测那两例(`js-corpus-5b65a8a137` / `f73025bf96`)的裁判结论是
**timeout**(单 case 5 秒),也就是「当天那台主机连不连得上」,不是语义。

M2e 起 `ReplayUrlStream.install()` 顺手把默认 `ProxySelector` 换成「一律走
127.0.0.1:1」:那个端口没人听,连接**立刻**被拒,书源自己的 `try/catch` 照常吞掉。
被测侧没有 `okhttp3` 这个类(`Packages.okhttp3` 不存在 → TypeError),同样被吞 ——
两侧收敛到同一个结论。这不是伪造语义,是把裁判放进一个「什么都连不上」的网络里,
而那正是本节这条前提本来就要求的。

### webView 三兄弟(**已接**,M3c)

`java.webView` / `webViewGetSource` / `webViewGetOverrideUrl`
(JsExtensions.kt L245-328)与 `java.ajax` 里 `{"webView": true}` 的链接:
两侧都走 `BackstageWebView` 的**策略层**(裁判是真身 —— :jsharness 从这一版起
按原路径挂载它;被测侧是 `net::webview`),平台那一半换成**剧本** —— case 的
`webview` 字段,契约见 `fixtures/cases/webview/README.md`。**不配剧本**就是缺省
剧本:页面加载得完、每次求值都回 `"null"` → 重试梯子跑满 →「js执行超时」。

失败**不吞**(真身那三处都没有 runCatching),异常穿到 JS;标签按裁判的**异常
类名**比:

| 情形 | 标签 |
|---|---|
| 一直取不到结果(`retry > 30`) | `host:NoStackTraceException`(真身的「js执行超时」) |
| 到点(缺省 60 s;含「页面一直加载不完」) | `host:WebViewTimeout` |
| `url` 与 `html` 都是 null | `host:NullPointerException` |
| 嗅探正则编译不了 | `host:PatternSyntaxException` |

> `host:WebViewTimeout` 是**垫片的类名**:真机上那是 `withTimeout` 抛的
> `TimeoutCancellationException`,而两侧的差分垫片都把 `withTimeout` 换成了
> 自己的虚拟时钟(裁判是 `jsharness.WvStage` 的内联泵,被测侧是策略层自己推的
> `now`)。被测侧由 `js_case_runner` 把 `«webview:timeout»` 归到同一个标签。

### 请用户出手那三件(**已接**,M3j)

`java.startBrowser` / `startBrowserAwait` / `getVerificationCode`
(JsExtensions.kt L352-400)底下都是 `SourceVerificationHelp`:
**裁判从这一版起挂真身那 300 行**(此前是 shims 里一个直接抛「未接」的桩),
被测侧是 `net::verification` 的移植。两侧共用同一份策略面 —— 挂号表、
界面互斥锁、**同一个盾只弹一次**的飞行表、轮询等待、取消、`验证结果为空`。

界面那一头两侧都换成**剧本用户**(case 的 `verify` 字段):

```json
"verify": { "result": "<html>过了</html>", "url": "https://a.example/ok" }
"verify": { "close": true }      // 用户把界面关掉 → 「验证结果为空」
```

- 裁判侧:`jsharness.VerifyStage` 顶掉 `appCtx.startActivity<…>`(垫片把
  `Intent` 的 extras 交给它),照剧本回头调 `SourceVerificationHelp.setResult`;
- 被测侧:`difftest::verify_script::ScriptedUser` 实现 `net::verification::VerifyUi`。

**不带 `verify` 字段 = 这一侧没接界面** → 被测侧报 `unsupported:net`
(与接进来之前逐字同一档,所以老 case 一例不动);裁判那边则**没有人答** ——
调这三件的 case 必须带剧本,否则真身会挂在轮询里。

观察面除了返回值,还有 **`verifyOpens`:界面被要求弹什么**(地址 / 标题 /
哪一种 / 要不要存结果 / 书源那三位 / 要不要等结果)。`startBrowser` 那条不等
结果,返回值只有 `undefined` —— 没有这一面就等于没比。挂号 key 本身不比
(真身是 UUID),只比「要不要等结果」那一位。

#### `browserLoad`:可见浏览器**到底怎么加载这一页**

`verifyOpens` 里嵌的一位,`kind: browser` 才有。真身在**界面层**算
(`WebViewModel.initData` + `WebViewActivity` 那三行):`AnalyzeUrl` 拆开
`url,{…}` 得到 `baseUrl` 与 `headerMap`,`toWebViewRequestConfig` 把
**UA 从头里单拎出来**(它属于 WebSettings,跳转与子资源才会一致;`CookieJar`
与 `proxy` 是内部网络选项,**不许发给网站**),`isPost()` 时先用 okhttp 那条路
(`useWebView = false`)抓一份 HTML **覆盖**入参那份。

我们的界面在 Dart(拿不到书源与网络),同一份算在宿主网络面
(`js-host::net_face::browser_load`),随「弹界面」一起交给平台 —— 于是这一面
两侧比得了:裁判由 `jsharness.VerifyStage.browserLoad` 照**同一份真身**算。

**为什么值得单开一位**:UA 是**过盾的关键** —— Cloudflare 的 clearance 认它,
可见窗口若用系统 WebView 的缺省 UA,用户点过之后书源再 `java.ajax`,会被当成
另一位客户端重新拦下。`webview` 套里 `toWebViewRequestConfig` 早就进了差分,
但那是**后台**那半;可见窗口这半在此之前一位都没比过。

三处按差分口径归一(改之前先读):

- 入参 html **不注入** `JS_INJECTION2`(页面里的 `java.*` 整族在砍单里);
- `initData` 出错 → `browserLoad` 是 **null**,而不是把异常抛回 JS:真身那句
  `execute {}` 走 `onError`(弹个 toast),**界面照开、书源那侧毫不知情**
  (探针 `js-api-verify-browser-await-bad-url`);
- POST 那一支两侧各多**一跳**(`hops` 里看得见),被测侧算在弹界面之前、
  裁判算在 Activity 被拉起之后 —— 一个 case 里只有一条线程,顺序等价。
  **已知不等价的一处**:飞行表里的第二个调用者(不开界面、等别人的结果)
  在被测侧仍会先算一次计划;真身是界面自己算,第二位根本不算。语料那 16 处
  `startBrowserAwait` 全是 GET,算计划不发请求,于是这一处照不出分歧。

标签按裁判的**异常类名**:

| 情形 | 标签 |
|---|---|
| 用户把界面关掉(`checkResult` 塞空结果) | `host:NoStackTraceException`(「验证结果为空」) |
| 飞行表等满 5 分钟 | `host:NoStackTraceException` |
| 地址超过 64 KiB(`require`) | `host:IllegalArgumentException` |
| 用户点了停 | `host:CancellationException` |

> **这一族没进差分的一支**:`VerificationResult.Refetch`(第二个撞上同一个盾的
> 调用**不用别人的结果、自己重抓一次**)要两个调用同时在飞 —— 一个 case 里
> 只有一条线程。它由 `net::verification` 的单测钉
> (`second_caller_joins_the_flight_and_refetches`,编排是确定的:界面等到
> 第二个真的挂上飞行表才答)。

### 仍然「未接」的

压缩包 / `queryTTF` / 文件系统 —— 裁判侧抛
`NoStackTraceException("«net-unsupported»")`(文件系统是 `«fs-unsupported»`),
被测侧同串,两侧收敛成 `unsupported:net` / `unsupported:fs`,
比的是「两边都到不了这里」。

> M2p 记一笔:`startBrowserAwait` 曾被记成本套的一条欠账(js-corpus-6e125c3ad3),
> 那是**归因错了** —— 那个 case 里它写在一条**没走到**的分支里,真因是完成值口径
> (见下)。它到 M3j 才第一次真的被执行。

### LiveConnect 的 crypto 一支(**已接**,M2e)

书源绕开 `java.*` 直接调 Java 类,语料里 ~27 个 B 层源这么写。**这一支不经 hutool**,
所以错误类别是 JCE 自己的,与 `java.aes*`(一律 `CryptoException`)不是一套。

被测侧的对象面在 `js-host/src/live_connect.rs`(类、静态成员、`with` 的名字解析
全在 JS 里搭,Rust 只出原语),分组密码本体与 `java.aes*` **共用**
`java_api::SymmetricCrypto`,差的只有 `CryptoFlavor` 那层皮。

已接的类:`java.lang.String`、`java.util.{Base64,Arrays}`、`android.util.Base64`、
`javax.crypto.{Cipher,Mac,SecretKeyFactory}`、`javax.crypto.spec.{SecretKeySpec,
IvParameterSpec,DESKeySpec,DESedeKeySpec}`、`java.security.{MessageDigest,Signature,
KeyFactory}` + `PKCS8EncodedKeySpec`(RSA 一支,M2p 接的)。
探针在 `hand.json` 的 `js-lc-*` 一族。

**这一面从 M3q 起在移植对照台账里有一行**(`docs/port-ledger.md` 的 `liveconnect` 面):
它的枚举源不是真身源码而是**语料** —— Rhino 的白名单是「整个 classpath 减掉
`RhinoClassShutter` 那张黑名单」,能枚举的只有需求侧。跑
`tools/port_audit.py --face=liveconnect` 看现算的分母。

钉住的几条:

- **`JavaImporter` 的名字解析是懒的**:`with (javaImport)` 里取到某个名字时才去
  各已 import 的包里找;同名类跨包冲突(同时 import `java.util` 与 `android.util`
  后取 `Base64`)是 Rhino 的 `EvaluatorException` → 与裁判 errorTag 同归
  **`js:SyntaxError`**(被测侧抛 `«java-ambiguous-import»`)。
  被测侧用 `Proxy` 的 `has`/`get` 实现,故 `with` 块里**没 import 的名字**照常
  落回全局(`result` / `java` / `JSON` 都还在)。
- **`String` 在 with 块里被 `java.lang.String` 遮住**:`String(byte[])` 是
  「按默认字符集解码」,不是 JS 的字符串化。Java 类**不带 `new` 直接调也是构造**。
- **Java 数组的形态**:`Array.isArray` 为 **false**(原型仍是 `Array.prototype`,
  `join`/`map`/下标/`length` 都能用)、元素**有符号**
  (`String('中').getBytes()[0] === -28`)、`String(arr)` 给 `[B@…`(归一成 «identity»)、
  GSON 序列化成数字数组。装箱层(`java_proxy.rs` 的 `javaArray`)按这四条建。
- **两个 Base64 解码器又多了一层**:`android.util.Base64` 在裁判侧是垫片,用
  `java.util.Base64.getMimeDecoder()` 复刻 —— 于是「表外字符跳过」,但 `=` 的
  **位置**仍按 JDK 那台状态机校验。`js-corpus-c9d744b7c7` 把一段中文喂给它:
  中文的字节全在表外被跳过,剩下的有效字符恰好凑满 4 的倍数,垫片补的 `==`
  落在「一个新组的开头」→ `IllegalArgumentException`。被测侧照抄那台状态机
  (`java_api::base64_decode_bytes`),**按字符数取模**(垫片那边是 Kotlin 的
  `String.length`,串是按 ISO-8859-1 从 byte[] 造的)。
- **错误落在哪一步要对**:transformation 不认识是 `getInstance` 抛
  (`NoSuchAlgorithmException`,**补码名不认识时抛的也是它**);密钥长度、IV 长度是
  `init` 抛(`InvalidKeyException` / `InvalidAlgorithmParameterException`);
  块大小、补码是 `doFinal` 抛(`IllegalBlockSizeException` / `BadPaddingException`)。
- **ECB 收到 `IvParameterSpec` 是错,不是忽略**(M3q,探针 `js-lc-err-ecb-iv` /
  `js-hut-ecb-iv-des`):hutool 只要 iv 非空就传 params,而 ECB 的 `init` 收到 params
  当场抛 `InvalidAlgorithmParameterException: ECB mode cannot use IV`。
  被测侧此前一路忽略 IV、照样算得出结果。
- **三条挨着的「空」边界**(M3q,`pb02492` 逼出来的):
  ① `Cipher.doFinal(new byte[0])` **解密**交回**空数组**(JCE 先判
  `totalLen % blockSize`,0 过得去,再让 `PKCS5Padding.unpad` 对 len==0 直接返回 0)——
  不是 `IllegalBlockSizeException`,探针 `js-lc-dec-empty` / `js-lc-dec-empty-nopadding`;
  ② 同一位**加密**要补出整整一个分组(`js-lc-enc-empty`);
  ③ hutool 那条路收的是**串**:`HexUtil.decodeHex("")` 与 `Base64.decode("")`
  **都返回 null** → `doFinal(null)` 抛 `Null input buffer` → CryptoException
  (`js-hut-dec-empty` / `js-hut-dec-empty-shell`),而 `" "` 这种**空白非空**的串
  走宽松表 → 0 字节 → 交空串(`js-hut-dec-blank`)。**三条不能合并。**
  书源真的会走到:拿明文页喂 `Base64.decode(…, 2)`,表外字符被全跳过就是 0 字节。
- **`java.getElement` 打不中 JSONPath**:jayway 的 `PathNotFoundException` 原样穿到
  JS(`host:PathNotFoundException`)—— 与 `getString` 那条路(吞掉、给空串)不同。
  被测侧在 `rule-engine` 的 `js_facing_error` 那一层翻回类名。

**没接的**:`Packages.okhttp3.*`(见上「一个字节都不许出网」,语料 1 源)、
`cn.hutool.core.util.ZipUtil`(压缩包那一支,砍单 §6,语料 1 源)。台账里这三行记 `不做`。

**故意不遮的两个名字**(台账记 `不做`,豁免带理由):`with` 块里真身还会把
`Object`(java.lang)与 `Date`(java.util)也遮住,于是 `Object.keys(...)` 在裁判那边是
Rhino 的 InternalError、`new Date().getFullYear` 是 undefined。被测侧只遮 `String`
(那条是**必须**的:`String(byte[])` 是解码)——复刻裁判等于让写 `Object.keys` 的
**5 个源**在 Rubato 上也失效。探针 `js-lc-shadow-object-keys` /
`js-lc-shadow-date-getfullyear`,对照组 `js-lc-shadow-math-floor` 两侧一致、不豁免。

### hutool 那条对称加解密路(`js-hut-*`,M3q 才有手写探针)

`java.aes*` / `des*` / `createSymmetricCrypto` 与上面**同一份算术、两套皮**
(这边一律 `CryptoException`)。这一族此前**一条手写探针都没有** —— 只有 corpus 里
几条真书源顺带走到,而它们两侧都落在 `catch` 里,「一致」得毫无信息量。补上之后
当场照出一处**裁判环境差**:

> hutool 5.8.22 的 `KeyUtil.generateKey(algorithm, key)` 在非 PBE / 非 DES 分支上把
> **整条 transformation 当算法名**塞进 `SecretKeySpec`(字节码可查,没有
> `getMainAlgorithm`)。于是密钥的 `getAlgorithm()` 是 `AES/CBC/PKCS5Padding` ——
> 裁判跑在 JVM 上,**SunJCE 的 AESCipher 查这个名字**并拒收;真身跑在 Android 上,
> **Conscrypt 只查密钥长度**、照跑。语料 **17 个源**就是这么写的。

于是 `js-hut-aes-full-transformation` 记豁免(**第三类豁免:裁判环境差** ——
既不是欠账也不是被测侧多做,是**这一位在这台裁判上问不出真身的答案**),
`pipeline-corpus-b` / `top100` 两套里它在四步上的投影同理。DES / DESede 那条路
不受影响(hutool 的 DES 分支走 `SecretKeyFactory` + `getMainAlgorithm`),
故 `js-hut-ecb-iv-des` / `js-hut-des-cbc-enc` 照比不豁免。

### 几处固定值

`AppConst.androidId` = `rubatodifftest16`(**正好 16 字符** —— `BaseSource` 的
登录信息面拿它前 16 字节当 AES 密钥,短一个字裁判侧就恒抛 IndexOutOfBounds,
比出来的会是垫片长度而不是语义)、`WebSettings.getDefaultUserAgent` =
`rubato-difftest-ua`、`AppConfig.userAgent` = `rubato-judge`。被测侧照抄同一常量。

## 确定性垫片(两侧共享一份文本)

书源里大量写 `Math.round(new Date()/1000)`,把时间戳拼进待签名串再
`java.md5Encode(...)`。两侧不可能在同一毫秒跑,时间戳与由它派生的签名必然不同 ——
**那不是实现差异,是差分自身的噪声**(实测 803 例里有 17 例每次运行结果都在变,
且全部落在 FAIL 里,永远不可能绿)。

**随机源现在是个开关**(M2h)。`java.randomUUID()` 从前无条件走定死的那条字节流,
连产品里也是 —— 每次跑、每个用户拿到的都是同一串 UUID,书源拿它当设备号/会话号
拼签名时就是所有人共用一个指纹。现在由 `HostConfig::random`
(`RandomSource::{System, Difftest}`)决定,**缺省是真随机**,`Difftest` 只由
`js_case_runner` 与 `ReplayNet` 的嵌套宿主显式选。嵌套宿主这一条要紧:
`ajax`/`connect` 会拿 url 再建一个 `AnalyzeUrl`,那里的 `<js>` 同样能调
`randomUUID` —— 忘了传下去,整套差分就会逐次变。

故 `fixtures/cases/js-host/determinism.js` 冻两样东西:
`Date.now()` / `new Date()`(无参)→ 固定纪元 `1788001200000`;
`Math.random()` → 固定种子 LCG。`tools/js_diff.sh` 把它复制到工作目录,
**两侧读同一份文本**(不靠两边手抄),裁判前置拼到用例代码前、被测侧单独求值。

垫片整段是一条 `var` 语句 —— VariableStatement 的完成值为空,拼接后脚本的
完成值仍旧来自用例自己。**改它的时候别写成 IIFE 表达式语句**,那会污染完成值。

宿主侧的非确定性分两处走:
- `androidId` / WebView UA / `AppConfig.userAgent` —— 上面「几处固定值」的常量;
- **`java.randomUUID()`** —— 冻的是 **JCA 的随机源**(`jsharness/DeterministicRandom`
  插一个优先级最高的 Provider,`SecureRandom` 吐定死的字节流
  `SHA-256("rubato-difftest" ‖ be_u64(i))`)。为什么不像上面那样给个常量:
  `UUID.randomUUID()` 在真身 `JsExtensions` 里,改不了;从随机源下手才拦得住。
  实测两个语料用例(`js-corpus-2033d8071d` / `d3c3112495`)把 UUID 拼进签名串,
  **裁判每次跑结果都不同** —— 建套时只查了被测侧的可重现性,漏了裁判侧。

**垫片只对 `op: "js"` 生效** —— `op: "analyzeUrl"` 那条路的 JS 是 AnalyzeUrl 真身
自己 eval 的,垫片进不去,故那边两侧都用真时钟、摸时钟的书源整条不进套
(见上「这个 op **不装确定性垫片**」)。

判据:`js_case_runner` **与 `jsharness`** 各自连跑两次,输出**逐字节相同**。
(建套时只写了前半句。裁判侧同样会飘,而且飘得更隐蔽 —— 它是"标准答案"。)

## `result` 的形态(`js-bind-*`,六十八条探针)

`result` 绑的**不一定是串**。产品里 `init: $.xxx` 从 JSON 页选出对象之后,
下一步 `@js:` 里的 `result` 就是 **jayway 读出来的 Java 对象本身**,Rhino 的
`WrapFactory` 把它**逐层**包成 `NativeJavaMap` / `NativeJavaList`。
下面每一格都是拿真 Rhino 探出来的(`resultRule` 走真身 `getElement`),
被测侧照着建:`rubato_core::host::JavaValue` → js-host 的
`nativeJavaMap` / `nativeJavaList` / `num` / `bool`。

| 表达式(`resultRule: "$.obj"`) | 真身 |
|---|---|
| `typeof result` | `"object"` |
| `result.anchor` | **键值**(不是 `String.prototype.anchor`)|
| `String(result)` | `{anchor=玄幻, n=书名A, …}`(Java 的 toString)|
| `JSON.stringify(result)` | 真 JSON |
| `for..in` / `Object.keys` | **只给键**,插入序 |
| `typeof result.hasOwnProperty` | `"undefined"` —— **没有 Object.prototype** |
| `result instanceof Object` | `false` |
| `result.get('n')` / `result.size()` | Java 方法面还在 |
| `typeof result.length` | `"undefined"`(Map 没有 length)|
| `typeof result.n` | **`"object"`** —— 取出来的值**又装了一次箱** |
| `result.n === '书名A'` / `== ` | **`false`** / `true` |
| `result.n.length` | **方法对象**(`result.n.length()` 才是数)|
| `typeof result.cnt` / `result.cnt + 1` | `"object"` / `8` |
| `typeof result.ok` | `"object"`(装箱的 Boolean)|
| **`result.no ? 'T' : 'F'`(值是 `false`)** | **`'T'`** |
| 完成值就是 `result` | `type: other`、`str` 是 toString、`norm` 是 GSON pretty |

`resultRule: "$.arr"` 那一半是 `NativeJavaList`:有下标 / `length` / `size()`,
**没有** `map`(没有 `Array.prototype`),`for..in` 只给下标;
`String(result[0])` 是**项自己的** Java toString(`{n=A}`)。

**两条推不出来、只能探的**(推错就是产品 bug):

1. **装箱的 `Boolean` 在 `if` 里恒为真 —— 连 `false` 也是**
   (探针 `js-bind-map-bool-if-false`,裁判给 `T`)。Rhino 的
   `ScriptRuntime.toBoolean` 对 `Scriptable` 在 ECMA1 版本上直接 `return true`,
   不解包。JS 的 ToBoolean 对任何对象同样给真,于是「装箱」这个做法在这一格上
   自动对上;若当初图省事绑成 JS 原生 `false`,`if (result.ok)` 会走反分支。
2. **顶层绑定的标量不装箱,容器里取出来的才装** —— `Context.javaToJS` 对
   `String`/`Number`/`Boolean` 原样交回(探针 `js-bind-txt-typeof` / `-num-typeof`
   / `-flag-typeof`:`typeof result` 分别是 `string`/`number`/`boolean`),
   而 `NativeJavaMap.get` 走 `WrapFactory.wrap`(`javaPrimitiveWrap` 默认 true)
   → 装箱。**两处口径相反**,故 `bind_value` 对标量与容器分两支。

**容器的 `toString()` 随出身**:json-smart 的 `JSONArray` 是 JSON 文本、
gson 的 `ArrayList` 是 `[a, b]`,而**同一棵 `serde_json::Value` 判不出来** ——
故由 rule-engine(它手上是 `RuleValue`)算好带着走(`value.rs` 的 `JavaFlavor`),
不在 JS 侧重算一份。

**对照的两族**(同一批探针里,防止「一刀切」):

- `js-bind-native-*`:`@js:({a:'A',n:7})` 交回来的是 Rhino 的 **NativeObject**
  —— **真 JS 对象**,`String(result)` 是 `[object Object]`、`hasOwnProperty` 在、
  完成值 `type: object`。走 `BoundValue::Native`,**不装箱**。
  (这一族照出一处旧缺口:`<js>` 的单个对象此前落 `RuleValue::Json`,
   即当成了 jayway 容器。)
- `js-bind-jx-*` / `js-bind-els-*`:XPath 的 `List<JXNode>` 是 NativeJavaList
  而**项是元素** → 整表**过不了 GSON**(`normError: host:JsonIOException`);
  jsoup 的 `Elements` 仍走元素代理那条路,一个字节不动。

**没钉的一处**:装箱方法对象自己的 `toString()` —— 真身是 Rhino 的 Java 签名形态
`function length() {/*\nint length()\n*/}\n`,被测侧是那个 JS 函数的源码。复刻要
给每个装箱方法带上 Java 签名(还得把重载全列出来),而语料里 **0 处**观察它;
探针只钉「是方法对象」与「调得出值」这两位。

## 已归一的差异(**不比较**,理由写在这里)

1. **数字的 int/double 表示**。Rhino 什么时候给 `Integer`、什么时候给 `Double`
   不可预测(`'abc'.length` → Integer 而 `1+1` → Double;`parseInt('7')` → Double
   而字面量 `1.0` → Integer),gson 于是输出 `"3"` 与 `"2.0"` 两种形态。这层是引擎
   内部表示,书源观察不到,故 `norm` 里的数字**两侧一律按 Java `Double.toString`**
   输出(被测侧 `java_double_to_string`,逐条对齐 Java 的
   「`1e-3 ≤ |d| < 1e7` 用普通小数、否则 `d.dddEn`」规则)。
2. **异常的栈字符串**。`ajax`/`connect` 失败时真身**吞掉异常**、返回
   `it.stackTraceStr`(`java.lang.IllegalArgumentException: …\n\tat okhttp3.…`)——
   全是 Java 的类名与行号,被测侧不可能也不该逐字复现。两侧归一成 `«java-stack»`
   (裁判 `Main.kt` 的 `JAVA_STACK`,被测 `host_env.rs` 的 `NET_ERROR`)。
   **比的是「失败了」,不是「怎么描述失败」**;「是不是在同一处失败」由 `hops`
   那一面钉(发没发出请求、发了几跳)。
   日志里那条 `ajax(<url>) error\n<栈>` 只归一换行之后的半截 —— **URL 那半要比**。
3. **对象的身份哈希**。Rhino 的 `NativeArray.toString()` 给
   `org.htmlunit.corejs.javascript.NativeArray@2f23b431`,每次运行都不同,
   被测侧也不可能一致 → 两侧归一成 `«identity»`。
   （`NativeObject` 给的是稳定的 `[object Object]`,照常比较。)

## 已知差异清单(FAIL 的分类 + 豁免的分类)

**2081 例 2070 PASS / 0 FAIL(豁免 11)**(M3q:LiveConnect / hutool 两族补了
十二条探针 2069 → 2081,见上「三条挨着的『空』边界」与「hutool 那条路」,
豁免 9 → 11 里新增的三条全是**裁判环境差 / 被测侧是超集**,不是欠账;
M2q:`js-bind-*` 六十八条形态探针
1975 → 2043,见上「`result` 的形态」;M2p 1975 例 1968/0 豁免 7,M2o 1894 例 1878/8 豁免 8,
M2n 1871/23 无豁免,M2m 1865/29,
M2j 1884 例 1856/28,M2h 1854/30,M2f 1875 例 1844/31,M2e 838 例 801/37,
M2d 803 例 747/56,M2c 730/73,M2b 709/94,建套首轮 513/290)。

### M2o:把「两个引擎的方言」那一堆定了调(23 → 8)

M2n 收盘时这一堆有 14 条,README 里写着「收敛方式待定(收紧被测侧、还是记豁免)」。
定调的依据只有一条:**这个差异会不会让书源在 Rubato 上退化**。
先把语料数出来(1704 源),再按数字分:

| 档 | 语料里几个源 | 判 | 落在哪 |
|---|---|---|---|
| **Rhino 收、规范不收**:没加括号的解构箭头参 `[a,b]=>` | **5** | **收紧被测侧** | `js-host/src/dialect.rs` |
| **quickjs-ng 收、Rhino 不收**:`class` / 展开 `...` / 正则 `\p` / Java 重载歧义 | 0 / 2 / 2 / — | **豁免** | `exemptions.json` |
| **Rhino 独有**:`for each` / E4X | **0 / 0** | **豁免**(探针留着) | 同上 |
| **完成值口径**:裁判在这一条上不合规范 | 1 | **豁免**(唯一一条) | 同上 |

M3q 又添了**第三类**(上面那张表是「两个引擎的方言」,这一类不是):
**裁判环境差** —— 裁判跑在 JVM 上、真身跑在 Android 上,同一份代码走的是两套 JCE
provider。它既不是欠账(我们没少做)也不是超集(我们没多做),是**这一位在这台
裁判上问不出真身的答案**。目前三条:`js-hut-aes-full-transformation` +
`js-corpus-e1d3254e62`(hutool 把整条 transformation 当算法名,SunJCE 查、Conscrypt 不查),
以及两条 `with` 块遮蔽(`js-lc-shadow-object-keys` / `-date-getfullyear`,那两条算超集)。
判法与前两类一样:**先把语料数出来再判**(17 个源 / 5 个源 / 1 个源)。

两句话概括:**被测侧是超集的,不跟**(复刻裁判等于主动把能力删掉,书源只会更跑不动);
**被测侧是子集、而且语料里真有源在用的,收紧**(不接就是那些源在 Rubato 上直接失效)。
「语料 0 个源」的那两条既不是超集也不值得接,豁免时**把探针留着** ——
哪天语料里出现 `for each`,它当场从豁免变成要还的账。

- **收紧的那条怎么落地**(dialect.rs):`list.map([title,url]=>…)` —— Rhino 先把
  `[title,url]` 当数组字面量解析,见到 `=>` 再改判成参数模式;规范要求
  `([title,url])=>`。**只在已经报了 SyntaxError 之后**重写一次再跑:正常路径一个
  字节都不动,重写完还是错就报**原来那条错**(垫片不许把真因盖住)。扫描要认得
  字符串 / 模板串 / 正则 / 注释,不然 `/[ab]=>/` 这种正则会被改写 —— 单元测试
  里那三组就是钉这个的。命中 5db449eef0 / 6689925092 / a08f654b9a / b67c744eaa /
  e302aeb992,全是「探索页 JSON 用 `.map` 拼出来」那一批源。

- 同一轮顺手修掉两条**不属于方言**的:
  - **`[native code]` 的缩进**(js-corpus-503b71f5bc):Rhino 用制表符、quickjs-ng
    用四个空格,而书源把函数对象直接拼进了字符串(`"主播:"+result.anchor` ——
    `anchor` 取不到就落到 `String.prototype.anchor` 上)。在 PRELUDE 里包一层
    `Function.prototype.toString`,**只归一内建函数那一行**,用户函数的源码形态不动。
  - **`hexDecodeToString` 的错误类别**(js-corpus-ef85223dda):hutool 把 `HexUtil`
    的一切包成 **UtilException**;而且归一标签**要带书名号**(`«host:…»` 才是
    js_case_runner 认的那一份),此前写成裸的 `host:HexException`,两处都错,
    于是落到了 `js:throw` 那一档。

### M2p:那 8 条欠账逐条还清(8 → 0),并照出四处此前没测的面

还账的顺序是「先探再改」:每一条都先拿 jsharness 打一组探针把真身的口径钉住,
再动被测侧,探针留在 `gen_js_cases.py` 里当判据(`js-dialect-cv-*` /
`js-api-select-*` / `js-els-*` / `js-lc-rsa-*` / `js-api-cache-file-*` /
`js-api-ajaxall-*` / `js-api-htmlformat-*` 几族)。

1. **`java.htmlFormat`**(js-api-htmlformat):本体是 `HtmlFormatter.formatKeepImg`。
   被测侧那份长在 `pipeline` crate 里、js-host 够不着 —— 下沉成 **`html-format`**
   crate(依赖 net + regex-compat + rubato-core),pipeline 与 js-host 共用一份。
2. **`java.ajaxAll`**(js-corpus-88f93cd3ff):网络面缺的一支。与 `ajax` 有三处不同,
   一处都不能省:**不吞异常**(那边整段包在 runCatching 里)、**不带 ruleData**、
   返回的是 **`StrResponse` 的 Java 数组**(书源写 `res[i][j].body()`)。
3. **`cache.putFile/getFile`**(js-corpus-02f6203511 / 90341bf923):
   **这条是裁判垫片的契约缺口,不是被测侧的实现缺口** —— 真身
   `CacheManager` 有这一支(打 ACache),而 `jsharness` 的垫片整支缺席,于是裁判
   在 `cache.getFile` 上直接 TypeError、被测侧照常往下走多发一跳。补法是两边一起:
   垫片补上这一支(**另一张表**,不是同一张),被测侧也拆成两张表
   (`QuickJsHost::cache_file`),并加了 `cacheFile` 观察面。
4. **`getElement(s)` 交回来的不一定是 jsoup `Elements`**(js-corpus-7db10b9743 /
   6e125c3ad3 的一半):容器形态由**规则模式**决定,不由内容 —— `Mode::Default`
   走 `AnalyzeByJSoup` 给 `Elements`,而 jayway 的 `getList`、XPath 的
   `List<JXNode>`、正则、JS 那几条给的都是普通 `ArrayList`。两者书源看得见的差别
   有三处:`toString()`(各 outerHtml 换行相连 vs `[a, b]`)、**过不过得了 GSON**
   (`Elements` 抛 JsonIOException,ArrayList 给得出)、项能不能**按键取属性**
   (`c[0].n`)。被测侧此前一律按 Elements 建箱,JSON 页上这一整片是错的。
   落地:`RuleHost::rule_get_elements` 交回 `ElementList { items, jsoup }`,
   新增 `RuleHost::el_json`(过 GSON 的形态;`None` = GSON 在它上面抛)。
   XPath 那条是一条用例同时钉两面:容器 `[a, b]` 而项是 jsoup 元素 → norm 仍是
   JsonIOException(`js-els-xpath-list`)。
5. **jsoup 选择器的错误通道**(js-corpus-5175e212a0):`Element.select` 在选择器
   不合法时**抛**,不是返回空。`el_select` 从前没有 Err、调用方一路
   `if let Ok(..)` 吞掉 —— 「`if let Ok(...)` 就是一处静默吞异常」又中一次。
   两个类名书源分得出来:空串在 `Selector.select` 头一句 `Validate.notEmpty` 上炸
   (**ValidationException**),其余在 `QueryParser` 里炸,而 QueryParser 把
   IllegalArgumentException 一律重抛成 **SelectorParseException**。
   顺带补了 html-compat 的一处宽松:`[k=]`/`[k$=]`/`[k^=]`/`[k*=]`/`[k!=]` 的**值
   不许为空**(jsoup 的 `AttributeKeyPair` 构造头两句是 notEmpty),而 `[k~=]`
   底下是 `AttributeWithValueMatching`、**不走** notEmpty —— 空正则是合法的。
   书源真的踩得到:`"a[href$=" + book.tocUrl + "]"`,而 `Book.tocUrl` 默认是空串。
6. **LiveConnect 的 RSA 一支**(js-corpus-2ed7808a7e):
   `PKCS8EncodedKeySpec` + `KeyFactory.getInstance("RSA").generatePrivate` +
   `Signature.getInstance("SHA256WithRSA")`。语料 1704 源里 **2 个**这么写。
   实现走「先自己算摘要、再拿裸摘要按 PKCS#1 v1.5 签」——`rsa` crate 绑 digest 0.10
   而本 crate 的 sha1/sha2 是 0.11,两套 trait 对不上;摘要那一半复用
   `java_api::jce_digest`(全 crate 唯一一份摘要表),只补 RFC 8017 的 DigestInfo 前缀。
7. **完成值口径**(js-corpus-6e125c3ad3 的真因):**上一轮的归因是错的** ——
   它记的是「空 Elements 过 GSON」,而那个 case 的 `if` 分支根本没走到,
   完成值来自**上一条语句**。规范 §16.1.7 `UpdateEmpty` 说:一条语句的完成值为空时
   整段留用上一条(`x=1; if(false){2}` → `1`),Rhino 与 V8 都照办,
   **不照办的是 quickjs-ng** —— 它在 `if`/`for`/`while`/`try` 上把 eval 结果寄存器
   置成 undefined(`;` / `{}` / `var` / `function` 四种它是对的,留作对照组)。
   这一档**不是**「裁判不合规范」,方向反过来:被测侧少一块能力,书源会因此空手 ——
   `<js>` 以 `if` 收尾、条件不成立时,legado 交回上一句的值(常常正是那张元素表),
   Rubato 交回 null。补法在 `js-host/src/dialect.rs::split_value_tail`:把脚本按
   顶层语句边界切成几段(末尾几段各是一条「完成值可能为空」的语句),顺序求值、
   取最后一个非 undefined 的完成值。三条约束写死在那里 ——
   ① **切之前先把每段用 `if(0){…}` 编译一遍**:语法错当场退回原来的单次求值
   (那时一个字节都还没跑,不会重复副作用),而且顺带把 `var` 提升做了;
   ② 段与段之间是同一 realm 的多次全局 eval,`var`/`let`/`function`/未声明赋值
   四种绑定都跨 eval 共享(单测 `cross_eval_scope_is_shared` 钉住);
   ③ `}` 之后的 `else`/`catch`/`finally` 是上一条的续、`}` 之后的 `while` 是
   do-while 的收尾 —— 都不算新语句的开头。
   连带作废了 `js-corpus-ff6d88b7c0` 那条豁免(它把方向判反了)。
8. **响应对象上返回 String 的成员是装箱的**(第 2 条的探针照出来的):
   `StrResponse.body()` / `url()` / `raw().body()` 与 jsoup `Response.body()` 在
   真身里声明的是 Kotlin `String`,过 LiveConnect 就是 `java.lang.String` ——
   `typeof r.body()` 是 **"object"**、`r.body().length` 是**方法对象**而不是数字。
   被测侧此前给的是裸 JS 串。

**本套现在 0 FAIL**(`fixtures/diff-baseline.json` 的 `js` 已经拧到 0,棘轮只许往下)。
剩下的 7 条**豁免**全部是「被测侧是超集」或「语料 0 个源、复刻不划算」,逐条带语料
数字,清单在 `exemptions.json` —— 每轮回头问一遍「这条今天还成立吗」:M2p 就作废了
一条(`js-corpus-ff6d88b7c0`,它把「谁不合规范」判反了)。

定位用:**`JS_DEBUG=1`** 把归一之前的真因打到 stderr(`[js] …`)——
`js:SyntaxError` 这一档归一之后长得都一样,不打出来就分不清「引擎方言」
与「我们自己的缺口」。与 pipeline 两套的 `PIPELINE_DEBUG` / `JSHARNESS_DEBUG`
同一条规矩。

### 已探明的方言事实(jsharness 探针实测)

探针共 103 条,当前 91 条对上。结论都落到了代码注释里,别重新踩:

- **绑定是 JS 原生 string**:`typeof result === "string"`、`result.match(...)` 可用
  —— legado 的 `RhinoWrapFactory` 会把绑定的 Kotlin String 转成 JS 字符串。
  (`typeof result === "object"` 只出现在 result 为 **null** 时。)
- **`java.*` 的返回值不转**,规律是**Java 方法优先、JS 原型兜底**:
  包装对象的原型链接到 `String.prototype`,所以 `slice`/`match` 这些 Java 没有的
  名字能用;而 `replace`/`split`/`length` 这些两边同名的走 **Java** 那一份
  (`replace` 全部替换且按字面量、`charAt` 返回**数字**、`length` 是**方法对象**)。
  细节与全表见 `js-host/src/java_proxy.rs`。
- **`java.ajax` 也装箱**(M2h 补的洞)。`JsExtensions.ajax` / `AnalyzeRule.ajax`
  的返回类型都是 Kotlin `String?`,`RhinoWrapFactory` 没覆写 `wrap()`、
  `javaPrimitiveWrap` 是默认的 true —— 和 `md5Encode` 走**完全同一条**路,
  必然是 `NativeJavaObject`。于是 `java.ajax(u).replace(a,b)` 在真身里是 **Java 的
  replace**(全部替换、按字面量),`.length` 是方法对象而不是数字。
  **这个洞差分套本来照不到**:顶层完成值会解包,而 `hand.json` 里 ajax 只有一条
  裸完成值的用例;语料 683 段里只有 10 段在 ajax 结果上直接调方法,且全是
  `.match` —— 恰好落在 JS 原型兜底那一支,两边同结果。补的探针是
  `js-api-ajax-boxed-{typeof,length,replace}`。
  同一批补的还有 `HMacBase64` 与 BaseSource 那一面的 `getKey`/`getTag`/
  `getVariable`/`getLoginHeader`/`getLoginInfo` —— 老用例一律写成 `String(...)`,
  把装箱盖住了。
- **完成值会解包**:`RhinoScriptEngine.eval` 顶层解 `Wrapper`,所以
  `java.md5Encode('a')` 作为整段 JS 的完成值又是普通字符串。装箱只在表达式中间可见。
  解包后是 List/数组时裁判侧 `type` 落到 `other`、`norm` 走 GSON pretty;
  是 jsoup 的 Elements 时 GSON 抛 → `normError: host:JsonIOException`。
- **`importClass` / `importPackage` 在真身里是 `undefined`**(别自作主张补);
  `JavaImporter` 是 function、`Packages` 与 `org.jsoup` 是 object。
- **`java.lang` 是 `undefined`** —— `java` 是 AnalyzeRule 不是 Java 包;
  但 **`Packages.java.lang.String` 是 function**(那条路通)。
  语料里 26 个 B 层源写 `java.lang.*`,它们在真身上同样会炸。
- **Rhino 不支持 `class` / 展开语法 `...`**;**支持 `for each`、E4X、八进制转义**。
- `java.getStringList(r).join('|')` 是 **TypeError**(Java List 没有 Array.prototype),
  但 `.forEach` 可以(`Iterable.forEach`);`for (var i in els)` 只给下标。
- **`ajax` 有两份实现,别当成一个**(M2c 接网络面时踩出来的,此前这里写错了):
  - `JsHost::Url` 上 `java` 是 AnalyzeUrl,`ajax` 是 `JsExtensions.ajax`
    (JsExtensions.kt L134):`AnalyzeUrl(url, source=…, callTimeout=…)`,
    **不带 ruleData**;失败走 `AppLog.put` —— **不进** Debug 日志。
  - `JsHost::Rule` 上 `java` 是 AnalyzeRule,而 **AnalyzeRule 覆写了 `ajax`**
    (AnalyzeRule.kt L951):`AnalyzeUrl(url, source=…, **ruleData = book**)`,
    没有 callTimeout;失败走 **`Debug.log("ajax(<url>) error\n<整个栈>")`**
    —— **进** Debug 日志,而且带的是**完整栈**(不是 localizedMessage)。
  带不带 ruleData 决定 url 里的变量能不能解出值,落不落日志决定 `logs` 那一面。
- **`url` 宿主的 `baseUrl` 绑定不是你传进去的那个串**。真身
  `AnalyzeUrl.analyzeUrl()` 在 `evalJS` 之前把它改写成
  `NetworkUtils.getBaseUrl(url)`(AnalyzeUrl.kt L236)—— 只剩 `scheme://host`,
  尾部斜杠没了。`rule` 宿主没有这一步(`AnalyzeRule` 绑的就是 `setBaseUrl` 收到的串)。
  差一个 `/` 就会让 `baseUrl + "/so/…"` 两侧走岔(实测 `js-corpus-59feb39b5d`)。
- **base64 有两个解码器,书源分得出来**:
  `java.base64Decode` 走 **hutool** `Base64.decodeStr` —— **宽松**,表外字符直接
  跳过、末组多出来的位直接丢,坏输入也不抛(`YWJj_-` → `abc\u{FFFD}`);
  `java.base64DecodeToByteArray` 走 **android.util.Base64**(经
  `EncoderUtils.base64DecodeToByteArray`)—— **严格**,坏输入抛
  `IllegalArgumentException`。混用会把错误类别钉错。
- **对称加解密的错误类别看走哪条路**:`java.aes*`/`des*`/`createSymmetricCrypto`
  经 hutool,`SymmetricCrypto` 把 JCE 的一切异常都包成 **`CryptoException`**
  (密钥长度、块大小、补码全都是它);而 LiveConnect 直接调 JCE 的那一支不经
  hutool,类别是 JCE 自己的(`IllegalBlockSizeException` 等)。
- **构造 `AnalyzeUrl` 抛的异常不被吞**。真身是
  `val analyzeUrl = AnalyzeUrl(...)` 在 `runCatching` **之外**,只有
  `getStrResponse()` 在里面。所以 url 里写 `{{$.id}}` 这类东西时,
  `initUrl → replaceKeyPageJs` 把它当 JS 跑、`$` 未定义 → ReferenceError →
  `com.script.ScriptException` **直接穿到 JS**,归一标签是 `host:ScriptException`
  (不是「返回栈字符串」那条路)。被测侧用 `NetError::Construct` / `Request`
  把这两条分开。

## 判据自己会骗人的地方(先读这段再信绿色)

**凡是真身从 assets / 文件系统加载的东西,先确认裁判侧到底加载没有。**
裁判是 vendor 来的引擎 + 一层垫片,垫片把某个能力悄悄关掉时,差分**结构上**
不可能发现被测侧也缺它 —— 两边一样缺,一样绿。

实例(M2h 查出来的):**`CryptoJS` 在真身里是可用的,这一套永远测不到**。
真 legado 的 `SharedJsScope.getCryptoScope` 会把 `scripts/cryptojs.min.js`
(assets)编译进共享作用域,于是书源 JS 里 `CryptoJS.*` 有值;而
`jsharness/shims/ModelShims.kt:50` 把 `getCryptoScope` 整个垫成 `return null`
—— 裁判自己也没有 CryptoJS。语料里 2/1704 源用到,影响小,但**性质**与 M2f
踩到的「裁判垫片 androidId 只有 15 字符」是同一族:比出来的是垫片,不是真身。

**第二条(M3q):「一致」不等于「同一个理由」。**
`java.aes*` 那一族此前只有 corpus 里几条真书源顺带走到,而它们**两侧都落在
`catch` 里** —— 绿了三个月,一个字节的信息量都没有。补上七条手写探针的当天
就照出:① 空输入解密两侧都报错,但一侧是 `IllegalBlockSizeException`、
另一侧是 `Null input buffer`;② `AES/…` 全式 transformation 在这台裁判上
**根本走不通**(SunJCE 查 key 算法名,Android 的 Conscrypt 不查)——
语料 17 个源天天在用的写法,在判据里从来没被真的执行过。
**看见「一致」先问一句:两侧是同一个理由吗。**

**第三条(M3q):豁免比它的理由活得久。**
LiveConnect 那条 B 层豁免写着「面还没接」,而面在写下那句话之前就接上了;
72 个挂着它的 case 里真在分岔的只有 1 个。**豁免要按轮回头问一遍
「它今天还成立吗」** —— 不然它既盖着一块早绿的面,也盖着后来长出来的分岔。

同族的还没排查完的面:`SharedJsScope` 的 **jsLib**(语料 0 源,故未接)、
`getShareScope` 在**调用超过 16 次后复用 topScope**(全局会开始跨次求值持久化,
被测侧现在每次都是全新 Context)。后者是**语义**差异不是能力差异,接
pipeline-corpus B 层前要先拿探针钉住。

## 语料来源

- `corpus.json`:683 段 —— `tools/extract_js.py` 从 `fixtures/sources` 的 1704 个
  书源里抽出的去重 JS 片段。绑定按片段的**来源规则位**挑(`searchUrl` 给 key/page,
  `ruleContent.*` 给 result/src/title/nextChapterUrl,列表规则给 JSON result,
  见 `tools/gen_js_cases.py` 的 `bind_for`)。
- `url.json`:1014 例 —— 1704 源里去重后的真实 `searchUrl`(带各自的 `header`),
  `op: "analyzeUrl"`。排除条件见上「这个 op **不装确定性垫片**」。
- `hand.json`:384 例 —— 方言探针(完成值形态 / 字符串与正则 / ES 版本面 /
  Rhino 专属 / JSON / 全局对象)+ `java.*` 宿主 API 面(按语料频次:
  `ajax` 116 源 / `put` 92 / `getString` 87 / `get` 76 / `timeFormat` 68 / …)
  + **LiveConnect 面**(`js-lc-*` 48 条:名字解析 / java.lang.String / Java 数组 /
  两个 Base64 / Cipher-Mac-MessageDigest / 八种 JCE 错误类别 / 三条「空」边界 /
  三条 `with` 遮蔽)
  + **hutool 对称加解密面**(`js-hut-*` 7 条:ECB 收到 IV、空串与空白串、
  以及那条裁判环境差)
  + **第三个宿主面**(`js-src-*` 38 条:绑定面 / CacheManager 那层变量 /
  源变量 / 登录头 / 登录信息 AES 往返 / 规则反调面不在)。
  **数字会漂,以 `python3 tools/gen_js_cases.py` 现算的为准。**

重新生成:`python3 tools/gen_js_cases.py`。跑差分:`tools/js_diff.sh [--show=N]`。
