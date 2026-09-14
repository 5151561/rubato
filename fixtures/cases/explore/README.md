# explore 差分用例契约(发现页分类列表)

入口:`tools/explore_diff.sh`。生成器:`tools/gen_explore_cases.py`
(输出 `01-probe.json` / `02-corpus.json`,**生成物,不入库**;
`exemptions.json` 是手写的,入库)。

- **裁判**:`judge/jsharness`,挂的是 **`help/source/BookSourceExtensions.kt` 真身**
  (`exploreKinds()`)。此前那份文件在 jsharness 里整块是垫片
  (`shims/LegadoSourceShims.kt`:`getBookType` 逐字复刻 + `exploreKindsJson` 恒空),
  于是**这一面从来没人比过** —— 台账 `field` 面的 `ExploreKind::*` /
  `FlexChildStyle::*` 十四行记的就是它。
- **被测**:`rust/crates/pipeline/src/explore_kinds.rs` +
  `rubato_core::entities::{ExploreKind, FlexChildStyle}`,执行器是
  `difftest` 的 `js_case_runner`(op `exploreKinds`)。

## 为什么与 js 套分开

裁判进程是同一个(真 Rhino —— `@js:` / `<js>` 那两支的发现规则非它不可),
但**分母不同**:js 套的分母是「书源里的 JS 片段」,这一套的分母是
「有 `exploreUrl` 的书源」(语料 1704 源里 977 个,去重后 960 例)。
混在一起会让两个分母互相盖住。

## 用例格式

```jsonc
{ "id": "ex-…", "op": "exploreKinds", "source": "<整份书源 JSON(裁剪过)>" }
```

用例**只给书源** —— 走哪条路由 `exploreUrl` 自己决定,这正是要比的那一位:

| `exploreUrl` 的形态 | 真身走哪支 |
|---|---|
| 空 / 全空白 / 字段缺席 | `isNullOrBlank()` → 空表 |
| `[` 开头 `]` 结尾(**只看首尾**) | `GSON.fromJsonArray<ExploreKind>` |
| `@js:` / `<js>` 开头(忽略大小写) | `BaseSource.evalJS` 再看结果是不是 JSON 数组 |
| 其余 | `split("(&&|\n)+")` → `title::url` |

## 观察面

`kinds` 数组,逐格出:

- `title` / `url` / `type` / `action` / `default` / `viewName`:**可能是 null** ——
  Kotlin 那边 `title` 与 `type` 是**非空** `String`,但 gson 的反射适配器对
  非基元字段照样写得进 null(`{"title":null}`),真身于是出现「非空类型持有 null」。
  被测侧用 `Option<String>` 表示同一个状态。
- `chars`:**缺席时不出这个键**(与裁判 `k.chars?.let { … }` 同口径)——
  出成 `null` 会把「没有这个字段」与「有但是 null」照成一致。
- `hasStyle` 与 `style`:**分两位**。前者是「给没给 `style`」,后者是
  `style()` 的**取值**(没给就是 `FlexChildStyle.defaultStyle`)。合成一位会把
  「没给」与「给了一份与默认相同的」照成一致。
- `style` 里的四个浮点位**出成串**(`Float.toString` 的形态):两侧 JSON
  序列化器对 float 的字面形态口径不同(gson 走 `Float.toString`,serde_json 会
  先加宽到 f64,`0.29f` 出成 `0.28999999165534973`)—— 那是报表的差,不是语义的差。
- `alignSelf()`:字符串 → FlexboxLayout 常数那张表,认不出给 -1。

出错时**不出 `kinds`,只出 `error`**:真身出错交回的是
`ExploreKind("ERROR:${localizedMessage}", stackTraceToString())`,消息与栈两侧
不可能逐字一致 —— 只比**哪一类错**(`js:<ECMA 错名>` / `js:throw` / `json` /
`host:<类名>`,与 js 套 `errorTag` 同一条口径)。

`hops`(发出的请求序列)与 `cache`(CacheManager 终态)沿用 js 套的口径。

## 三处抄漏了就会静悄悄错掉的

1. **不是 `BaseSource.extractInlineJs`**:`exploreKinds()` 用的是
   `startsWith` + `substring`,`<js>` 那支取 `substring(4, lastIndexOf("<"))`
   —— **最后一个 `<`**,不是 `</js>`。脚本里再出现 `<` 就会被截短;
   `<js>` 后面没有第二个 `<` 时真身抛 `StringIndexOutOfBoundsException`
   (探针 `ex-jstag-inner-lt` / `ex-jstag-no-close`)。
2. **`isJsonArray()` 只看首尾字符**,不校验内容:`[这不是 json]` 会进 gson
   那一支再抛(探针 `ex-json-not-really-json`);而 `[{"title":"甲"}`
   首尾不成对,反倒走了 `split` 那一支(探针 `ex-json-unclosed`)。
3. **Kotlin 的 `Regex.split` 保留首尾空片**(Java 的 `String.split` 会丢尾部
   空片):规则以换行收尾时真身实实在在多出一格空标题(探针
   `ex-plain-trailing-newline`)。

## 剔除与豁免

- **摸时钟 / 随机的书源整条不进**:裁判侧 `exploreKinds()` 里的 `evalJS` 是
  真身自己调的,没有「先在作用域里跑一段」的入口 —— 确定性垫片进不去
  (与 `gen_js_cases.py` 的 `op="analyzeUrl"` 同一条理由)。
- 没有 `exploreUrl` 的源不进语料例(两侧都是空表,没有信息量);这一支由
  手写探针 `ex-empty-*` 钉住。
- `exemptions.json`:眼下 1 条,理由与分母写在文件里。

## 缓存

真身有两层(进程内 `exploreKindsMap` + 落盘 `ACache`,键
`md5(bookSourceUrl + exploreUrl)`)。**判据面一律冷算**:裁判每例开跑前
`clearExploreKindsCache()` + `ACache.clearAll()`(落盘那层在 jsharness 里是
`shims/ACacheShim.kt` 的内存表),被测侧根本没有缓存层。缓存是**产品面**的事。
