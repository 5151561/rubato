# analyze-url 差分用例契约

两侧执行器与比较口径同 `fixtures/cases/rule-syntax/README.md`。入口:`tools/analyze_url_diff.sh`。

- 裁判:`judge/harness` 里**原样挂载 `judge/engine/.../AnalyzeUrl.kt`**(992 行,一行未改)。
  okhttp 是纯 JVM 库直接当依赖用;WebView/网络/限流/cookie 由 shims 顶成确定性桩。
  私有字段(`body`/`encodedForm`/`encodedQuery`/`charset`/`method`/`retry`/…)用反射读出。
- 被测:`rust/crates/net`(+ `rubato-core::gson` 的 gson 等价面)。

## 被测面 = **离线的请求构造**

`initUrl()` 全链:`analyzeJs`(`<js>`/`@js:` + `@result` 占位)→ `replaceKeyPageJs`
(`{{js}}` 内嵌 + `<a,b,c>` 页码)→ `analyzeUrl`(切 option JSON、绝对化、套用
option、按 method 编码 query / form)。**真正发请求不在本套里**——那要等 HTTP 录放
与 store 的 CookieStore(`setCookie` / `getStrResponse` 因此不进差分)。

## 用例格式

```jsonc
{
  "id": "ah00001", "op": "analyzeUrl",
  "mUrl": "/s?q={{#key}},{\"method\":\"POST\",\"charset\":\"gbk\"}",
  "baseUrl": "http://www.example.com/a/b.html",
  "key": "关键字", "page": 2, "speakText": "…", "speakSpeed": 1,
  "headers": {"User-Agent": "UA"},        // 构造器的 headerMapF
  "ruleData": "plain" | "book" | "none",  // 变量层,同 rule-engine
  "bookName": "…", "chapter": true, "title": "…",
  "vars": {…}, "chapterVars": {…}
}
```

## 输出

构造成功时给出全部要素(`ruleUrl` / `url` / `urlNoQuery` / `method` / `headers` /
`body` / `encodedForm` / `encodedQuery` / `charset` / `proxy` / `type` / `retry` /
`useWebView` / `webJs` / `bodyJs` / `dnsIp` / `serverID` / `webViewDelayTime` /
`userAgent` / `isPost`)与变量终态;任何异常 → `{"error":"url_error"}`。
`headers` 按 **LinkedHashMap 插入序**输出(顺序本身也进差分)。

JS 桩指令表与 rule-engine 共用,另有两条 AnalyzeUrl 专属绑定:
`#key` → `key` 绑定、`#page` → `page` 绑定的 `toString()`。

## 口径要点(差分抓出来的)

- **gson 是按字段反射填充 data class 的,不走 setter**。所以 `UrlOption` 那些
  「空白归 null」的 setter 逻辑在链接 option 这条路上**不生效**:`"charset": ""`
  会原样留着空串(再传给 `encodeParams` 时才按 `isNullOrEmpty` 走 UTF-8 分支)。
- gson 即使在 lenient 档也**不接受尾逗号**;`{"a":1,}` 会让整个 option 解析失败,
  于是 method 退回 GET、body 丢失——真实书源里这种写法不少。
- `getBody()` 对非字符串 body 走 `GSON.toJson`,而 GSON 开了 `setPrettyPrinting`,
  所以 body 是**带两空格缩进的多行文本**。
- `retry`/`serverID`/`webViewDelayTime` 是 Integer/Long 字段:数字串可以,
  其它类型会抛 JsonSyntaxException 让**整个 option** 变 null。
- `useWebView()`:只有 null / `""` / `false` / `"false"` 为假,数字 0 也算真。
- **urlOption 新增字段(LegadoTeam 基准)**,投影里新增 `readTimeoutMs` /
  `urlTimeoutConfigured` / `followRedirects` 三项:
  - `timeout` → `parseRequestTimeoutMillis`:字段类型是 `Any?`,gson 把 JSON 数字
    填成 Double;只有**落在 `1..=Int.MAX_VALUE` 的整数值**算数(小数 / 0 / 负数 /
    超界 / 非数字串 / 布尔一律 null)。命中时同时置 `urlTimeoutConfigured`;
  - `followRedirects` → `parseBooleanOption`:Boolean 直用;数字只认 0/1;
    字符串 trim + 小写后只认 `true`/`1`/`false`/`0`;其余 null;
  - `dnsIp` 有 `@SerializedName(alternate = ["resolveIp"])` —— 两个名字绑同一
    BoundField,**都出现时后出现的赢**(gson 按流顺序覆盖写入)。
- 编码三条路:query 且有 charset → 先看整串是否已编码,否则走 hutool
  `queryEncoder`;`charset=="escape"` → `EncoderUtils.escape`(`%uXXXX`);
  其余 → 按 `&`/`=` 切段后 `URLEncoder.encode`(空格→`+`)。
- **query 编码器换血(LegadoTeam 基准)**:`queryEncoder =
  RFC3986.UNRESERVED.orNew(PercentCodec.of("!$%&()*+,/:;=?@[\]^`{|}"))`。
  安全字符集与旧基准的 `queryEncoderSafeChars` 表一致,**但代理对语义变了**:
  hutool 逐 UTF-16 码元喂 `OutputStreamWriter` 再 flush,而 `StreamEncoder`
  把高代理项留在 leftoverChar 里(flush 不吐),等低代理项到齐才整体编码 ——
  净效果是**按码点编码**。`😀`:UTF-8 → `%F0%9F%98%80`、gb18030 →
  `%94%39%FC%36`、GBK/big5/latin1 → 单个 `%3F`。旧基准逐码元编码给两个 `%3F`,
  这是本次基准切换的语义漂移点(10 例 FAIL 由此而来)。
- `EncoderUtils.escape` 仍是 Kotlin `for (char in src)`,**依旧按 UTF-16 码元**,
  增补平面字符拆成两个孤立代理项,Rust 侧照做。

## 已知近似

- `charset(name)` 在 Rust 侧用 `encoding_rs::Encoding::for_label`(WHATWG 标签),
  与 JDK 的 charset 别名表不完全重合:WHATWG 把 `iso-8859-1` 映到 windows-1252、
  把 `gb2312` 映到 GBK。差分覆盖了 utf-8/gbk/gb2312/gb18030/big5/iso-8859-1/escape,
  更冷门的编码出现即记豁免。
- `NetworkUtils.getSubDomain`(cookie 域名)依赖 okhttp 的 PublicSuffixDatabase,
  Rust 侧走 psl crate;差分在 fetch 套(见下)。

## `MapDeserializerDoubleAsIntFix` 实测结论(drift.json)

`INITIAL_GSON` 给 `object : TypeToken<Map<String?, Any?>?>() {}.type` 注册了
`MapDeserializerDoubleAsIntFix`(把 `ceil(d) == toLong().toDouble()` 的 double
整数化成 Long)。切换基准时把它列进了「可能的漂移点」,**差分实测结论是:
在本工程触达的每一个调用点上它都不生效**。

`drift.json` 用四类形态钉住这一点(`3.0 / 2.5 / -2.5 / -0.0 / 1e3 / 1E-3 /
9007199254740993 / 1.0e20 …` 逐个铺):

| 路径 | 声明类型 | 实测数字语义 |
|---|---|---|
| `urlOption.headers` 是 JSON **对象** | 字段 `Any?` → ObjectTypeAdapter | `LONG_OR_DOUBLE`(`3.0` → `"3.0"`) |
| `urlOption.headers` 是 JSON **字符串** | `GSON.fromJsonObject<Map<String, Any>>` | 同上,**没走那个反序列化器** |
| `urlOption.body` 是 JSON 对象 | 字段 `Any?` | 同上(`GSON.toJson` 回吐 `3.0`) |
| `AnalyzeRule` 的 `Map<String, Any?>` 内容 | 见 rule-engine README | 同上 |

所以被测侧**不实现**这条整数化:`gson::parse_lenient` 的 serde 数字分档
(有小数点/指数 → f64,否则 i64)与 `LONG_OR_DOUBLE`(先 `Long.parseLong`
原串、失败再 `Double.parseDouble`)逐例等价。

同一批用例还钉住了 `GSON.toJson` 的另一条:gson **默认 `serializeNulls = false`**,
`getBody()` 回吐 JSON 时**对象里值为 null 的键整条丢掉**(数组里的 null 照写,
因为 `JsonWriter.nullValue` 只在有 deferredName 时才跳过)。
被测侧对应 `rubato_core::gson::to_json_pretty`;而 `StringJsonDeserializer` 走的
`JsonElement.toString()` 是另一个 writer(`serializeNulls = true`),**保留** null。

## cookie 域名归一

`NetworkUtils.getSubDomain` 已在 `rubato_core::net_utils` 移植(PSL 走 psl crate),
`domain` 字段的取法见 `fixtures/cases/fetch/README.md` 的同名小节。
