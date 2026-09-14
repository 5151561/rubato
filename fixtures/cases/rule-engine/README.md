# rule-engine 差分用例契约

两侧执行器与比较口径同 `fixtures/cases/rule-syntax/README.md`。入口:`tools/rule_diff.sh`。

- 裁判:`judge/harness` 里**原样挂 `judge/engine/.../AnalyzeRule.kt`**(973 行,一行未改);
  它的 Android/Room/OkHttp/Rhino 依赖由 `judge/harness/src/main/kotlin/harness/shims/`
  的垫片顶上,垫片只提供 AnalyzeRule 触及的最小面。
- 被测:`rust/crates/rule-engine` + `difftest` 的 `case_runner`。

## 用例格式

```jsonc
{
  "id": "re00001", "op": "analyzeRule",
  "content": "<html>…",                  // setContent 的内容
  "contentType": "string" | "map",       // 默认 string;map 见下
  "baseUrl": "http://www.example.com/a/b.html",   // 可省
  "redirectUrl": "http://www.example.com/a/b.html", // 可省;走 setRedirectUrl
  "rule": "class.item@text##(\\d+)##N",
  "mode": "string" | "stringList" | "element" | "elements",
  "isUrl": false,                        // 仅 string / stringList
  "ruleData": "plain" | "book" | "none", // ruleData 层类型(默认 plain)
  "bookName": "书名A", "bookAuthor": "作者",       // ruleData=book 时生效
  "chapter": true, "title": "章节T",     // 是否挂 BookChapter 层
  "vars": {"v1": "初值"},                 // ruleData 层初始变量
  "chapterVars": {"v1": "章节值"},        // chapter 层初始变量
  "nextChapterUrl": "…"
}
```

## 输出

```jsonc
{"id":"…","result": <按 mode 而定>, "vars": {"data": {…}, "chapter": {…}}}
{"id":"…","error":"rule_error"}        // 任何 Throwable / RuleError
```

- `string` → 字符串;`stringList` → 字符串数组或 null;
  `element` → `toString()` 或 null;`elements` → 各元素 `toString()` 的数组。
- `elements`(LegadoTeam 基准语义):末态结果按 `List` / `Array` /
  `NativeArray` 三分支展开,并过滤 `null` 与 `Scriptable.NOT_FOUND`;
  **不是列表的一律给空数组**(String / Map / 标量都落这里)。旧基准在这里
  `result as List<Any>` 强转抛 ClassCastException → `rule_error`,是本次基准
  切换的语义漂移点(94 例 FAIL 由此而来)。
- `vars` 两层都按**键排序**输出(裁判用 TreeMap),避开 HashMap 遍历序。
  规则内部 `@put` 的**执行顺序**仍按 Java HashMap 遍历序,由
  `rule-engine::java_util::hash_map_order` 复刻(桶下标 + 插入序)。

## JS 桩指令表(两侧必须逐条一致)

Phase 1 的被测对象是 AnalyzeRule 的切分/分发/拼接,不是 JS 引擎;接真 Rhino
会把方言差异混进来(那是 Phase 2 js-host 的差分范围)。因此 `evalJS` 两侧都换成
确定性桩:整段 JS 源码 **trim 后**当作一条指令。

| 指令 | 返回 |
|---|---|
| `#null` | null |
| `#result` | `result` 绑定的 `toString()`(绑定为 null 时返回 null) |
| `#baseUrl` / `#src` / `#title` / `#nextChapterUrl` | 对应绑定(可为 null) |
| `#fromBookInfo` | `"true"` / `"false"`(字符串) |
| `#num:<字面量>` | Double(解析失败 → 抛) |
| `#bool:<字面量>` | Boolean(Kotlin `toBoolean()` 口径:仅 `true` 忽略大小写为真) |
| `#echo:<文本>` | 该文本(String) |
| `#list:<a>\|<b>` | `List<String>`(按 `\|` 切;空载荷 → `[""]`) |
| `#get:<键>` | `java.get(键)` |
| `#put:<键>=<值>` | `java.put(键, 值)`,返回值(无 `=` → 抛) |
| `#err` | 抛异常 |
| 其它 | **未 trim 的原始源码**(String) |

`@webjs:` **不再是桩**:两侧都走 `BackstageWebView` 的**策略层**(裁判是真身,
被测侧是 `net::webview`),平台那一半换成**剧本** —— case 的 `webview` 字段,
契约见 `fixtures/cases/webview/README.md`。`getWebJsResult` 的那几个常量
(`cacheFirst = true` / `timeout = 10000` / `isRule = true` /
`result = GSON.toJson(result)`)两侧逐字对齐。

生成器给每条 `@webjs:` 用例配的剧本是「**把那段 js 源码本身回给你**」
(`tools/gen_rule_cases.py::webview_script`)—— 与从前那个桩的口径逐字一致,
于是 `fromJsonArray` / `##替换` / 四个 mode 的下游覆盖一条没丢。
**不配剧本**就是缺省剧本:页面加载得完、每次求值都回 `"null"` → 重试梯子跑满 →
「js执行超时」→ `rule_error`。

`.getStrResponse().body.toString()` 那处漂移(LegadoTeam 基准;旧基准是
`body ?: ""`)照旧钉着:剧本让页面上那段 js 回一个**字符串** `"null"`
(从前是桩指令 `#nullbody`),body 于是是 `"null"` 而不是空串。
`java.ajax` 桩:回传 `"ajax:<url>"`(当前 op 触达不到,留给 Phase 2)。

## Phase 1 未建模(出现即两侧一起跳过或记豁免)

> **XPath 已于 Phase 2 (M2g) 接上**:`@XPath:` 与 `/` 开头的规则此前两侧都
> 预探测 `Mode.XPath` 并输出 `xpath_unsupported`(避免把"未实现"混进 pass 率),
> 现在两侧都真的跑 —— 被测侧是 `xpath-compat`,契约见
> `fixtures/cases/xpath/README.md`。这 105 例自那时起是真结果。

- content 是 Rhino `NativeObject` 的分支(要真 Rhino 才造得出对象,随 Phase 2
  的 js-host 一起补)。**gson `LinkedTreeMap` 那条已补**,见下节。
- `reGetBook` / `refreshTocUrl`(preUpdateJs 专用,依赖 pipeline)。
- `source` 层恒为 null(BaseSource 的 put/get/getHeaderMap 属于 net 的活;
  getHeaderMap 的契约见 `fixtures/cases/fetch/README.md`)。

## 已知近似

- jayway 的 JSON provider 是 json-smart **PERMISSIVE**,对非 JSON 文本(HTML 页面)
  不抛异常而是当成裸字符串;Rust 侧用 `AnalyzeByJSonPath::parse_permissive` 近似
  (以 `{`/`[` 开头却解析不了的才算坏 JSON)。真实脏 JSON 的差异见 plan 风险 4。
- `@put:{…}` 的宽松 JSON 档是手写扁平解析器(gson lenient 的常见面:裸键、单引号);
  超出这一面的写法会落到"解析失败 → 忽略该 put",与裁判可能不一致。
- 数字字面量按 serde_json 归一化(`1.50` → `1.5`),gson 保留原字面量。

## content 是 gson `LinkedTreeMap`(`contentType: "map"`)

`AnalyzeRule.getString` / `getStringList` 对 `result is LinkedTreeMap` 有一条
**专用短路分支**(AnalyzeRule L242-244 / L334-336):直接拿
`ruleList.first().rule` 当**键**去查这张表,`putRule` / `makeUpRule` /
`replaceRegex` / mode 分发**统统不跑**。用例把它铺满:

- 裁判侧内容用 `GSON.fromJsonObject<Map<String, Any?>>(content)` 构造;
  被测侧对应 `RuleValue::GsonMap`(`rubato_core::gson::parse_lenient`)。
- `getString` 取到的值走 `?.toString()`(Java 集合口径:`{k=v}` / `[a, b]`);
  `getStringList` 保留原对象,由尾巴的 `result as? List<String>` 定死活 ——
  值是 `String` 就按 `\n` 切,是 `ArrayList` 就逐元素 `toString()`,
  其余(嵌套 Map / 数字 / 布尔)一律 `null`,值缺失或为 JSON null 也是 `null`。
- 后处理写法(`##正则`、`{{}}`、`@get`/`@put`、`$.jsonpath`)在这条分支下
  **整串当键**,因此基本都查不到 —— 用例专门钉住这个反直觉点。
- `getElement` / `getElements` **不**走短路,落到通用循环;此时
  `content.toString()` 形如 `{k=v}` 让 `isJSON` 为真,规则被切成 `Mode.Json`,
  裁判 `JsonPath.parse(Object)` 直接把 Java 集合当 JSON 模型:
  - **definite 路径**(`$.list`)读回来的是模型里**原样的 `ArrayList` /
    `LinkedTreeMap`**,`toString()` 是 `[a, b]` / `{k=v}`;
  - **indefinite 路径**(`$.list[*]`)读回来的是 jayway **新建的 json-smart
    `JSONArray`**,`toString()` 是 JSON 文本 `["a","b"]`。
    被测侧靠 `AnalyzeByJSonPath::get_object_scalar` 的第二个返回值区分,
    分别包成 `RuleValue::Java` 与 `RuleValue::Json`。
  - `AnalyzeByJSonPath.getObject(rule): Any` 的返回类型**非空**,读到 JSON null
    时 Kotlin 的空检查抛 NPE → `rule_error`(这条对普通 JSON 文本内容同样成立,
    是本轮顺带钉住的一处)。

## 实测修正的两处「照源码读会读错」的地方

- `Gson.fromJsonArray<String>`(`@webjs:` 在 `getStringList` 里的那支):源码写着
  「列表含 null 元素就抛 JsonSyntaxException」,**实测 null 元素是被静默丢弃的**,
  列表照常返回(探针:`@webjs:[null]` → `[]`、`["a",null]` → `["a"]`、
  `["a", , "b"]` → `["a","b"]`)。以裁判实测为准。
- `INITIAL_GSON` 注册的 `MapDeserializerDoubleAsIntFix`(整数化 double)在
  **本工程触达的每一个 Kotlin 调用点上都不生效**,数字一律是
  `ToNumberPolicy.LONG_OR_DOUBLE` 的结果(`3.0` → Double `3.0`,不是 Long `3`)。
  实测见 `fixtures/cases/analyze-url/README.md` 同名小节。
