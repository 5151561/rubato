# HTTP 录放快照契约

> 状态:**契约先行**(docs/plan.md §3 要求「先写文档再写代码」)。两侧实现都以本文为准;
> 改规则要先改这里,并重新生成受影响的快照。

差分体系里两边都不许直连网络:裁判(Kotlin)与被测(Rust)各自实现一个**回放客户端**,
从同一份快照目录取响应。快照的 **key 规范化规则**是两边共享的唯一契约——只要 key 算得不一样,
差分就会退化成"两边都 miss",毫无意义。

## 1. 目录布局

> **这些目录都是生成物,不入库**(`fixtures/http*`,`fixtures/http-js-host`
> 除外——那份回落页是手维护的)。快照由 `tools/gen_fetch_cases.py` /
> `gen_pipeline_cases.py` / `gen_pipeline_corpus_cases.py` 各自 rmtree 再重建,
> 每个 `*_diff.sh` 跑之前都会调自己的生成器。判据见 `tools/check_generated.sh`。

```
fixtures/http/
├── <host>/                 # 规范化后的 host(小写,含端口时写成 host_port)
│   └── <key>.json          # 一次请求/响应
└── index.json              # key → 摘要(便于人工检索,不参与匹配)
```

`<key>` 是下面算出来的 **32 位小写十六进制**(SHA-256 前 16 字节)。

## 2. key 的输入(严格按此顺序拼接,分隔符 `\n`)

1. `METHOD`——大写(`GET`/`POST`/`HEAD`)。
2. **规范化 URL**(见 §3)。
3. **参与哈希的请求头**(见 §4),逐行 `名: 值`,名按 ASCII 小写排序。
4. **请求体哈希**:无请求体写空串;否则写请求体字节的 SHA-256 十六进制全长。
   注意这里用的是**实际发出的字节**——即 `encodedForm` / `body` 编码后的结果,
   不是规则里的原文。

拼好的字符串按 UTF-8 取 SHA-256,取前 16 字节的十六进制即 key。

## 3. URL 规范化

| 项 | 规则 |
|---|---|
| scheme | 小写;`https` 与 `http` **不合并**(是不同的 key) |
| host | 小写;末尾的 `.` 去掉;IDN 保持**已有形态**(不做 punycode 互转,录制时是什么就是什么) |
| port | 默认端口(http 80 / https 443)去掉,其余保留 |
| userInfo | **丢弃**(不进 key,也不进快照文件) |
| path | 原样保留,**不做百分号编码的规范化**——`%2F` 与 `/` 是不同的 key |
| 空 path | 补成 `/` |
| query | 按 `&` 切段,段内不切 `=`;整段按**字节序排序**后用 `&` 重连;空 query 整体省略 |
| fragment | **丢弃** |

排序只影响 key,不影响回放时实际写进快照的原始 URL(快照文件里存原样 URL 供审计)。

## 4. 参与哈希的请求头

**白名单(参与)**:`user-agent`、`referer`、`content-type`、`x-requested-with`。

**不参与**:其余全部,特别是 `cookie`、`accept`、`accept-encoding`、`accept-language`、
`connection`、`host`、`content-length`,以及引擎内部标记(`CookieJar`)。

理由:UA / Referer / Content-Type / XHR 标记会真实改变响应形态,必须进 key;
Cookie 每次会话都不同,进 key 会让快照永远命中不了——它的影响改为在**流水线级差分**里
比对「发出的请求序列」与 cookie 终态(plan §3 第 2 级),而不是靠 key 区分。

值的规范化:去首尾空白,内部原样;头名一律小写。

## 5. 快照文件格式

```jsonc
{
  "key": "3f2a…",                       // 冗余,便于校验
  "request": {
    "method": "GET",
    "url": "https://www.example.com/s?q=%E4%B8%AD",   // 原样(未排序)
    "headers": {"User-Agent": "…", "Referer": "…"},   // 原样全量,审计用
    "bodyBase64": null                                 // 有请求体时填
  },
  "response": {
    "status": 200,
    "headers": {"content-type": "text/html; charset=gbk", "set-cookie": ["…"]},
    "bodyBase64": "…"                    // **原始字节**,不做解码
  },
  "recordedAt": "2026-08-29T00:00:00Z",  // 仅审计
  "note": "手工合成 / 真实录制"
}
```

响应体必须存**原始字节**:charset 检测(EncodingDetect)与 `##` 正则的输入都依赖它,
存成文本会把这一层的差异抹掉。

## 6. 回放语义

- **命中**:按 key 找到文件 → 原样返回状态码、响应头、响应体字节。
- **未命中**:两侧都必须报**同一种可比对的错误**(`snapshot_miss`),而不是去联网。
  差分比较器把两边的 miss 当作一致(但会在报告里单列计数,提醒补录)。
- **回落页(fallback)**:case 带 `"fallback": "<名字>"` 时,**未命中**不报错,
  而是读 `<快照根>/_fallback/<名字>.json` 的 `response` 段原样返回(格式同 §5,
  `request` 段可省)。名字只允许 `[A-Za-z0-9_-]+`。两侧实现必须一致:
  裁判 `ReplayHttp.fallback`、被测 `ReplayTransport.fallback`,由各自的 case
  执行器在跑每个 case 前按 `fallback` 字段设置(不带该字段就是 null = 老语义)。

  用途:**真实书源语料**跑四步流水线时,真实站点快照不可得(也不该进仓库),
  但「四步的调度、字段装配、翻页、错误分类」在真实规则上的一致性仍要差分。
  给每一跳发同一张合成页面,两侧看到的输入完全相同,差异就只可能来自实现。
  代价是 `requests` 序列不再受快照约束(URL 构造仍逐跳比对),
  且页面与规则不成对——命中率低的源大量落在「空结果 / toc_empty」等分支上,
  这本身也是要对齐的语义。见 `fixtures/cases/pipeline-corpus/README.md`
  (A 层,JS 是确定性桩)与 `fixtures/cases/pipeline-corpus-b/README.md`
  (B 层,两侧都是真 JS 引擎;快照根 `fixtures/http-pipeline-corpus-b/`)。

  **回落页不止一张**:名字是 case 自己带的,所以「一张页回答所有请求」只是
  最初的用法,不是机制的限制。现在三套 pipeline 各写 `html-<内容哈希>` 与
  `json-<内容哈希>` 若干张(**都是一个「源 × 步」一张**,分别由
  `tools/fallback_page.py` / `fallback_json.py` 按那个源自己的规则反向合成,
  相同形态按内容哈希去重),外加合不出结构时退回的两张**底板** `html` / `json`。
  JSON 从一开始就只能按源一张(JSONPath 除 `$..` 外**从根锚定**,
  `$.data[*]` 与 `$.data.content` 在一份文档里互斥);HTML 共用了很久,
  因为 CSS **位置无关**、一张 DOM 摆得下几十种**容器**形态 —— 漏掉的是**字段**:
  payload 只能有一份,于是「容器选中了、`name` 规则对不上」,那一步照样什么都
  装配不出来(M2n:改成按源一张之后 html 页的命中面 47% → 70~85%)。
- **重定向**:录制时**逐跳存**(每跳一个快照);回放时由客户端按 `Location` 自己跳,
  这样 `redirectUrl` 的记录语义(相对 URL 解析依赖它)才能被差分覆盖。
- **多次请求同一 key**:同一份快照可被重复命中(无状态),不做"第 N 次返回不同响应"。
  需要状态的场景(登录、翻页 token)靠 URL/请求体的差异自然分开。

## 7. 回放客户端语义(两侧共享)

回放客户端 = 把 okhttp 真实调用链里**影响请求形态与响应处理**的部分摊平成一个循环。
裁判侧(harness/ReplayHttp)与被测侧(net::client)都按本节实现;本节即 okhttp 5.4.0
相应拦截器的摘录,存疑时以 okhttp 源码为准。

### 7.1 一次 call 的流程

1. **应用层预处理**(每 call 一次,对应 HttpHelper 的 UA 拦截器):
   - 无 `User-Agent` 头 → 加默认 UA(`AppConfig.userAgent`,`net::client::user_agent()`)。
     **产品与差分是两个值**:产品用 `net::client` 的 `PRODUCT_UA`(照真身的形状,
     Chrome UA),差分侧两个 runner 在 `main` 头一行 `set_user_agent("rubato-judge")`
     钉成裁判垫片的同值 —— UA 进 key(见 §3),两侧对不上就命中不了录好的快照。
     实测过这条判据:把那一句去掉,`pipeline` 那套当场 2 PASS / 86 FAIL,
     全是 `snapshot_miss`;
   - `User-Agent` 值为字面量 `"null"` → 删除该头;
   - 追加 `Keep-Alive: 300`、`Connection: Keep-Alive`(不参与 key,仅为对齐审计输出)。
2. **逐跳循环**(对应 RetryAndFollowUpInterceptor + 网络拦截器,最多 **20** 跳):
   1. 若请求带 `CookieJar` 头(AnalyzeUrl.setCookie 在 enabledCookieJar 时设置):
      本跳**去掉该头**,并把 CookieStore 的 cookie 合并进 `Cookie` 头
      (CookieManager.loadRequest:`mergeCookies(requestCookie, store)`;merge 右侧覆盖左侧)。
   2. **生效 Content-Type** = 显式 `Content-Type` 头(若有),否则请求体的 media type
      (对应 BridgeInterceptor 只在头缺失时按 body 补头);以此参与 §4 的 key。
   3. 按 §2 计算 key、查快照;未命中 → `snapshot_miss`(带 key 与规范化 URL)。
   4. 若本跳启用了 CookieJar:按快照响应的全部 `set-cookie` 头执行
      CookieManager.saveResponse(okhttp `Cookie.parseAll` 语义:域不匹配丢弃;
      有 `expires`/`max-age` 记 persistent 入库,否则记 session 入内存)。
   5. **重定向跟进**(okhttp followUpRequest 摘录;`followRedirects=true`):
      - 状态码 300/301/302/303:任何方法都跟进;307/308:仅 GET/HEAD 跟进,否则本响应即终态;
      - 无 `Location` 头 → 终态;`Location` 按 okhttp `HttpUrl.resolve` 相对解析,
        解析失败或 scheme 非 http/https → 终态;
      - 非 307/308 跟进时方法一律改 **GET**、丢请求体、删 `Content-Type`/`Content-Length`/
        `Transfer-Encoding` 头;
      - 跨 origin(scheme/host/port 任一不同)删 `Authorization` 头;
      - 超过 20 跳 → 错误 `too_many_redirects`。
3. **重试**(OkHttpUtils.newCallResponse):整个 call(含逐跳循环)最多执行 `retry+1` 次,
   响应码非 2xx 才重试,取最后一次响应。每次都会重复 cookie 存取。
4. **响应文本化**(`ResponseBody.text()`,StrResponse.body):
   去 UTF-8 BOM → 响应 `content-type` 的 charset 参数(Charset 不识别则跳过)→
   EncodingDetect.getHtmlEncode(字节窗口找 `<head>…</head>`,jsoup 解析 meta 的
   charset / http-equiv content-type;失败则 icu4j CharsetDetector 检测)→ 兜底 UTF-8。
5. **StrResponse.url** = 终态那一跳的请求 URL(即重定向解析后的 okhttp 规范化 URL)。

### 7.2 请求 URL 的规范化

发出的 URL 一律经 **okhttp `HttpUrl` 规范化**(GET 为 `toHttpUrl().newBuilder()
.encodedQuery(encodedQuery)`,POST/HEAD 为 `toHttpUrl()`):百分号编码按 okhttp
各组件的 encode set、host 小写、默认端口省略、路径段 `.`/`..` 归一。被测侧移植
okhttp 的 canonicalize;**快照 key 里的 URL 是规范化后的**。Phase 1 用例回避
非 ASCII host(IDN/punycode 不在被测面)。

### 7.3 差分口径内外

- 在内:请求序列(每跳 method + 规范化 URL)、终态 URL、状态码、body 文本、
  cookie 终态(库 + session)。
- 在外:超时、TLS、代理、DNS(dnsIp)、限流(ConcurrentRateLimiter 两侧放行)、
  压缩(快照体一律存解压后字节;`Accept-Encoding` 不参与 key)、
  webView(Phase 3)、`Cookie` 头超 4096 的**随机**裁剪(不可差分,用例回避)。

## 8. 合成快照

Phase 1 的流水线差分**优先用手工合成的快照**(`note: "手工合成"`):
不依赖外网、可进版本库、CI 可重复。真实录制留给 Phase 2 之后按需补,
并且录制脚本必须复用本文的 key 算法,不得另写一份。
