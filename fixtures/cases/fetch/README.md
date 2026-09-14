# fetch 差分用例契约(HTTP 录放全链路)

入口:`tools/fetch_diff.sh`(先跑 `tools/gen_fetch_cases.py` 重建用例与
`fixtures/http/` 快照)。回放客户端与快照 key 的契约:**docs/http-snapshot.md**。

- 裁判:原样挂载的 `AnalyzeUrl.kt` + `CookieStore.kt` / `CookieManager.kt` /
  `StrResponse.kt` / `EncodingDetect.kt`(含 icu4j)+ okhttp 真身
  (`Cookie.parseAll` / `HttpUrl` / `PublicSuffixDatabase`);回放循环在
  `harness/ReplayHttp.kt`,appDb/CacheManager/webkit 由 shims 顶掉。
- 被测:`net`(http_url / cookie / client / response / encoding_detect / fetch)
  + `store::CookieStore`(内存 SQLite)+ `difftest::replay`。

## 被测面

`getStrResponse` 全链:setCookie(库 cookie 并入请求头)→ okhttp URL 规范化 →
回放循环(UA 默认头 / CookieJar 逐跳注入回存 / 重定向跟进 / 重试)→
`ResponseBody.text()` 三级编码判定 → xml 补头 / bodyJs → StrResponse。
`type` 字节路径(data URI / 网络字节 → hex)。cookie 终态(库 + session)。

## 用例格式

```jsonc
{
  "id": "ft0001", "op": "fetch",
  "mUrl": "...", "baseUrl": "...", "key": "...", "page": 1,
  "headers": {"User-Agent": "..."},          // headerMapF
  "source": {"url": "http://...", "enabledCookieJar": true,
             "header": "{\"Referer\":\"...\"}"},        // BookSource
  "ruleData": "plain" | "none", "vars": {...},
  "cookies": {"example.com": "k=v"},          // 预置库 cookie(按 domain)
  "sessionCookies": {"example.com": "s=1"}    // 预置 session cookie
}
```

每个 case 执行前重置 cookie 存储与请求序列。

## 输出

成功:`url`(终态跳的 okhttp 规范化 URL)、`status`、`body`(text() 结果,
经 xml 补头 / bodyJs)、`requests`(每跳 `[method, url]`,跨重试累计)、
`headers`(AnalyzeUrl.headerMap 终态,含 setCookie 写入的 `Cookie`/`CookieJar`)、
`cookiesDb` / `cookiesSession`(按 domain 排序)。

错误(两侧字符串必须逐字一致):
- `url_error`——AnalyzeUrl 构造抛;
- `snapshot_miss:<key>:<规范化URL>`——快照未命中。**这是合法结果**:两侧串一致
  即证明「离线构造 + okhttp 规范化 + key 计算」整条链一致,语料用例大量依赖
  这一点(不配快照);入口脚本单列 miss 数;
- `too_many_redirects`——超 20 跳;
- `fetch_error`——其余一切异常的归一化(非法 URL / 非法 Content-Type /
  编码名不识别等)。

## 桩与契约

- JS:两侧同一指令表桩(`fixtures/cases/rule-engine/README.md`);`bodyJs`
  / `{"js":...}` / webView 的 `webJs` 都走它。
- webView:**不再是桩**。`{"webView": true}` 两侧都走 `BackstageWebView` 的
  **策略层**(裁判是真身,被测侧是 `net::webview`),平台那一半换成**剧本** ——
  case 的 `webview` 字段,契约见 `fixtures/cases/webview/README.md`
  (**不配剧本**就是缺省剧本:页面加载得完、每次求值都回 `"null"` →
  重试梯子跑满 → 「js执行超时」→ `fetch_error`。那是「这一页录不下来」的
  诚实结局,不是假装取到了内容)。本套钉的是 **AnalyzeUrl 怎么进出它**:
  `tag = source?.getKey()`(cookie 回抄的键,进 `cookiesDb`)、
  `javaScript = webJs ?: jsStr`、`delayTime = webViewDelayTime`、
  GET 那条**不发请求**(`requests` 为空)、POST 那条先走一次真实(回放)请求
  再把 `res.url` / `res.body` 交给 webView,而
  `followRedirects=false` 且这一跳是 3xx 时**不进 webView**
  (`shouldReturnRedirectBeforeWebView`)。
  策略本身的每一条(补 900 / 重试梯子 / 超时 / 嗅探 / UA 过滤)在 webview 那套钉。
- UA 默认值两侧固定 `rubato-judge`(judge 侧 AppConfig 垫片)。
- **`source.header` 进入契约(LegadoTeam 基准起)**:`headerMapF` 缺席时走
  `BaseSource.getHeaderMap()` —— header 规则 JSON 宽松解析(顶层非对象、值为
  对象/数组 → Gson 抛 → 整块跳过)+ 缺 `User-Agent` 时注入 `AppConfig.userAgent`。
  旧基准的「header 恒空」契约作废。被测侧等价物:`net::source_header`。
  内联 `<js>`/`@js:` 头两侧都走桩(真 JS 是 Phase 2)。
- **`followRedirects` 进入回放循环**:真身经 `buildRequestClient` 把它落到
  okhttp client 配置上,回放侧由 `newCallResponse` 从 client 读出传给
  `ReplayHttp.executeCall`;为 `false` 时不跟进重定向,终态就是 3xx 本身。
  `timeout` 只影响 okhttp 超时配置,对回放输出无副作用(有用例钉住这一点)。

## 已知回避面(用例不生成,生成器有过滤;命中记豁免)

(icu4j CharsetDetector 已移植,检测兜底路径在测;独立差分见
`tools/charset_diff.sh` 与 `fixtures/cases/charset/`。)

- ~~**IDN / 非 ASCII host、`xn--` 标签**~~:**M2n 已接** —— `net::http_url`
  照 okhttp 5 的 `idnToAscii`(UTS-46 映射 → NFC → 逐标签 punycode)走,
  交给 `idna` crate(同一份 Unicode 数据、non-transitional、
  UseSTD3ASCIIRules=false);**全 ASCII 且不含 `xn--` 的 host 仍走原来的快路**
  (映射表在那一档只做大小写折叠)。语料生成器不再按 authority 过滤,
  那批 url 照常入册。**IPv6 字面量 host** 仍判非法。
- Cookie 串超 4096:真身**随机**删键,不可差分。
- `postMultipart` / `upload`:Phase 2+。
- charset 标签集:Java `Charset.forName` vs encoding_rs 标签(如 gb2312 的
  真 GB2312 与 GBK 超集之差)只在罕见字节上不同。
- PSL 版本:okhttp 内嵌表 vs psl crate,罕见后缀可能漂移。

## cookie 域名键(`ft0035`–`ft0043`)

`AnalyzeUrl` 里决定「这次请求带哪份 cookie」的是 `domain` 字段
(AnalyzeUrl L149-151):

```kotlin
val sourceKey = source?.getKey()
domain = sourceKey?.takeIf { NetworkUtils.getBaseUrl(it) == null }
    ?: NetworkUtils.getSubDomain(url)
```

两条反直觉的地方,用例逐条钉住:

- sourceKey **不是** http(s) 链接(书源名、`legado://…`、空串、host 为空的坏
  URL)→ **原样当键**,不再做任何域名归一;
- sourceKey **是**链接时,取的是**本次请求 url** 的子域名,而**不是**
  sourceKey 的 —— 同域时看不出区别,跨域请求(`source.url = http://www.a.com/`
  而请求打到 `http://b.com/…`)才照得出来:带上去的是 `b.com` 那份 cookie。
  `getBaseUrl` 的 `http(s)://` 前缀判定忽略大小写,`HTTP://…` 也算链接。

写回那侧不受这条影响:`CookieManager.saveResponse` 用的是**响应 url** 的子域名。

被测侧对应 `net::AnalyzeUrl::new` 里的 `domain` 计算。把它退回旧口径
(`getSubDomain(sourceKey ?: url)`)会让 ft0036/0037/0041/0043 四例 FAIL。
