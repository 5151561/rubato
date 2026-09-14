# pipeline-corpus-b 差分用例契约(B 层真实书源 × WebBook 四步,**JS 是真引擎**)

入口:`tools/pipeline_corpus_b_diff.sh`(先跑 `tools/gen_pipeline_corpus_cases.py --tier=B`
重建用例、豁免清单与 `fixtures/http-pipeline-corpus-b/` 的回落页)。

**用例与输出格式和 `fixtures/cases/pipeline-corpus/README.md` 逐字一致**
(同一个 `op: "pipeline"`、同一份回落页机制、同一张投影)。这份 README 只写
**不一样的那部分**,别的去读那一份。

## 这一套与 pipeline-corpus 的唯一区别:裁判进程

| | pipeline-corpus | **pipeline-corpus-b** |
|---|---|---|
| 语料 | A 层 908 源(无 `<js>`/`@js:`) | **B 层 647 源**(684 减去摸时钟/随机的 37) |
| 裁判 | `:harness` —— com.script 是**确定性桩** | `:jsharness` —— **真 Rhino** |
| 被测 | `difftest::case_runner`,`StubHost` | `difftest::js_case_runner`,**真 QuickJS + 网络面** |

两个 harness 的 com.script 只能有一份(桩与真 Rhino 互斥),所以分模块、
也因此分两套。**被测侧没有这个约束**:两套共用 `difftest::pipeline_case`
一份执行器,`Js::Stub` / `Js::Real` 只挑 JS 走哪条路 —— 投影、错误归一、
观察面因此天然一致,不会各写各的。

裁判侧的挂载面见 `judge/jsharness/build.gradle.kts`:`io/legado/app/model/webBook/**`
四步真身(此前那里是 `LegadoWebBookShims.kt` 垫片,已删)+ 已有的 AnalyzeRule /
AnalyzeUrl / JsExtensions / BaseSource 真身。

## 为什么不冻时钟

`js-host` 那套两侧都前置拼一段 `determinism.js`(冻 `Date` 与 `Math.random`)。
**这一套不能**:四步内部的 `<js>` / `@js:` 是真身自己 eval 的,裁判侧没有
「先在作用域里跑一段」的入口(与 `op: "analyzeUrl"` 是同一条边界,见
docs/plan.md M2f)。单边冻比不冻更糟 —— 故**两侧都用真时钟**,
生成器把摸时钟/随机的书源整条剔除:

    NONDET = /\bDate\b|Math\.random|randomUUID/   扫**整份书源**

扫整份而不是只扫规则段:书源常把 JS 藏在 `bookSourceComment` 里再 `eval` 它。
`randomUUID` 也在内 —— 两侧虽然都接在定死的字节流上,但被测侧一条规则换一个
宿主、各自从 block 0 起,而裁判是每 case 一个全局计数器,消耗序对不上。
剔除 29 源。判据仍然成立的验法照旧:**两侧各连跑两次,输出逐字节相同**。

## 一次 case 内的共享:hop 表与 cookie 库

裁判侧 `ReplayHttp.hops` 与 `appDb.cookieDao` 都是**进程级单例** —— 四步发的请求
和书源 JS 里 `java.ajax` 发的请求落在同一张 hop 表上,存下的 cookie 也是同一份。

被测侧原本不是:`PipelineEnv` 握着 `&mut` 的 transport/cookies,而
`js_net::ReplayNet` 自己另建一份。于是 `java.ajax` 的那一跳**不进 `requests`**、
它存下的 cookie **不进 `cookiesDb`**、也不会被后续请求带上 —— 三处都是静默的
单边差异。现在两边接在同一份 `Rc<RefCell<…>>` 上(`difftest::shared_io`)。
`CacheManager`(`java.put` / `sourceVariable_*` / `loginHeader_*`)同理,
一次 case 内的全部宿主共用一张表,终态出在 `cache` 字段上。

## 观察面

与 pipeline 套相同(`books` / `book` / `chapters` / `content` / `chapter` /
`requests` / `cookiesDb`),**多一个 `cache`**(CacheManager 终态,空则不出字段)。

**不比 Debug 日志**。`java.log` 与 ajax 失败的栈会落进裁判的 `Debug.logs`,
那是个真实的可观察面,但 pipeline 套从来没有它 —— 保持一致,别在这里单开。
要比日志去 js-host 套(那边 `logs` 是钉住的)。

## 这一套抓出来的东西(都是**产品路径**上的洞,A 层差分结构上照不到)

A 层那套两侧都是确定性桩,压根不碰绑定面 —— 于是下面四处在 pipeline 里
**从建起来那天就是空的**,而它们在产品里天天被走到:

1. **`source` 绑定一直是 None**:`AnalyzeUrl` 与每一个 `AnalyzeRule` 都没填。
   书源 `@js:` 里但凡读 `source.getKey()` / `source.bookSourceComment`
   (再 `eval` 它是常见写法)就是 TypeError。现在 `pipeline::source_binding`
   带上整份原始 JSON(`BookSource::raw`,新增字段)。
2. **`book` 绑定一直是 None**:详情/目录/正文三步的 ruleData 就是 Book 实体
   (`AnalyzeRule.kt L68` 的 `ruleData as? BaseBook`),`book.name` / `book.intro`
   拼串很常见。搜索/发现两步真身也是 null(那里的 ruleData 是裸 `RuleData`),
   保持 null。
3. **`chapter` 绑定被写死成 null**(`js-host/host_env.rs` 里一行注释「本套永远是
   null」漏进了产品)。正文规则读 `chapter.title` 很常见。现在有
   `ChapterBinding`,并且是**活的** —— 真身绑的是实体本身,目录循环里
   `chapterUrl` 的 JS 读得到刚算出来的 `chapter.title`,故每改一次重绑一次。
4. **`UrlArgs.rule_host` 一直是 None**:`org.jsoup.Jsoup.parse(...)` 是 Rhino 的
   LiveConnect **全局面**,与 `java` 是谁无关。M2f 只在 js-host 套的
   `op: "analyzeUrl"` 那条路上注入过它,pipeline 这条路漏了 → `«no-rule-host»`。

首轮 2650/127;修完这四处 **2698 PASS / 79 FAIL / 6 豁免(共 2783)= 96.9%**。
(M2n 收盘 **2775 PASS / 1 FAIL / 7 豁免 = 99.7%**;
**M2o 收盘 2779 PASS / 0 FAIL / 4 豁免 = 100.0%**;
**M3q 收盘 3040 例 3030 PASS / 0 FAIL / 10 豁免** —— 例数涨是语料入册面变宽,
豁免从 4 涨到 10 是「LiveConnect 那条欠账豁免删掉、裁判环境差那 6 条露出来」,
逐条见下面的「豁免」。)

(第五处顺带修的:`cache` 终态此前只在**成功分支**挂,而裁判侧那一段在
try/catch **之外** —— `java.put` 之后才抛的 case 会现形。3 例。)

## M2j:`result` / `src` 绑定走真对象(79 → 66)


上一轮把「~25 例」记在这一条名下,实测**只有 13 例是它**;剩下的一半是另一件
(见下面清单第一条)。**判据自己会骗人的又一例**:同一个 `pipeline_error`
标签底下,「被测抛而裁判不抛」与「裁判抛而被测吞了」长得一模一样,
不打 stderr 就分不开(裁判侧的真因打印是这一轮加的,`JSHARNESS_DEBUG=1`,
对侧是 `PIPELINE_DEBUG=1`)。

落地的:`JsBindings.result` / `src` 从 `Option<String>` 换成
`rubato_core::host::BoundValue`(`Str` / `StrList` / `Element`;M2q 又添了
`Java` / `Native` 两支,见下「这一轮照出的欠账」)——
真身 `bindings["result"] = result` 绑的是**对象本身**,jsoup 的 `Elements`
在 Rhino 那边是 NativeJavaList。连带的三件:

1. **元素能穿过 JS 回来**(`JsValue::Elements`):`class.x\n@js:result.toArray()
   .map(…)` 交回一批元素,`getElements` 按元素继续跑下游规则 —— 此前
   `to_js_value` 把数组元素 `Coerced<String>` 成串,下游只能当 HTML 文本再解析。
2. **`seal_proxy` 接上了**。它此前挂着不接,理由是「语料里没有对 Elements 用
   `for..in` 的写法」—— 那是因为**元素根本进不了 JS**。绑定一走真对象,
   `for (i in result)` 立刻就有(pb01214),不接就会把 `text`/`select`/`toString`
   这些方法名一起枚举出来。
3. **`Elements.text()` / `html()` / `toString()` 的拼接口径**(`jsoup_join`):
   jsoup 是 `if (sb.length() != 0) sb.append(sep)` —— **空项不产生分隔符**,
   而 `join(sep)` 会。`select('h2')` 命中三个空 `<h2>` 时 jsoup 给 `""`,
   `join(" ")` 给 `"  "`(pb00024 的标题前多两个空格)。反编译 jsoup 1.16.2
   的 `Elements.text()` 逐条对过。

还没走真对象的形态:jayway 值(`Json`/`JsonArray`)、`Num`、`Bool` —— 它们在
真身那边分别是 NativeJavaList/Map 与 JS 数字,被测侧仍按 `toString()` 过。
语料里还没有用例踩到,记在这里当欠账。

## M2k:一轮吃掉 50 例(66 → 16)

按上面那份清单从上往下做的,顺序与结论:

1. **`nextTocUrl` / `nextContentUrl` 吞 `Err`(19)**。两处 `?` 上去。真身
   `BookChapterList.kt` L214 / `BookContent.kt` L262 没有 try/catch。
2. **JS 返回对象数组(11)**:新 `JsValue::Native` / `RuleValue::Native`
   —— 真身那边是 NativeArray 装 NativeObject,`getString` 对 `result is
   NativeObject` 有**专用分支**(AnalyzeRule.kt L321-331:键值直接访问,
   putRule/makeUpRule/replaceRegex 都跑)。元素全是标量的数组仍走 `List`,
   那条路十四套都绿着,不动。
3. **`loginCheckJs`(7)**:`analyzeUrl.evalJS(checkJs, res) as StrResponse`
   接上了 —— `result` 绑一个 StrResponse 形态的对象
   ([`BoundValue::Response`]),JS 把它原样交回来就算强转成立,交别的就是
   ClassCastException。**没做的一支**:JS 返回**另一个** StrResponse
   (`java.connect(u)` 的结果)认不出来;还有 fetch 自己失败时真身会拿
   `getErrStrResponse` 再跑一遍 checkJs(WebBook.kt L84-98)。
4. **`java.get(u,h).body()`(2)**:装箱层**只装标量**了 —— `java.get` 按
   arity 分派,两个实参那一支返回的是 jsoup `Connection.Response`(对象),
   照装就拍成 JavaString,`.body()` 成了 not a function。
5. **`connect(u).raw().request().url()`(4)**:补上 `raw()` 那一层。
6. **搜索/发现两步的 `book` 绑定不是 null(4)**:`getSearchItem` 一上来
   `analyzeRule.setRuleData(searchBook)`,而 `book get() = ruleData as? BaseBook`
   —— SearchBook **就是** BaseBook。而且是**活的**(`coverUrl` 规则里
   `book.origin + …`)。上一轮 README 写的「搜索/发现真身也是 null」只对
   BookList 自己那个 AnalyzeRule 成立,`setRuleData` 之后就不是了。
7. **详情步的 `book` 绑定要跟着实体走(4)**:`analyze_book_info_with` 里
   每填一个字段就重绑 —— `tocUrl: {{book.kind}}` 读的是**刚填好的** kind,
   快照版本给的是空串。
8. **`putVariable` 的 10000 分流(1)**:`RuleDataInterface.putVariable` 以
   `value.length < 10000`(**UTF-16 码元数**)分流,够大的走
   `putBigVariable`(另一张按 bookUrl/chapterUrl 键的表)并**从 variableMap
   里删掉**;读取 `variableMap[key] ?: getBigVariable(key)`。可观察面:
   实体的 `variable` JSON 里没有这一条(`java.put('real_chapter', src)`
   存整页是常见写法,必然超过 10000)。
9. **`cookie` 绑定接真 CookieStore(1)**:真身的 `cookie` 就是那个单例,
   与网络层同一张表 —— JS `cookie.setCookie(...)` 存下的要进 cookiesDb、
   也要被后续请求带上。此前 js-host 自己攥着一张按域名键的 `SharedMap`,
   **写了等于没写**;差分与产品同病(engine 那边一并接上了)。
   `CookieEnv` 的四个新方法**故意不给缺省实现**:漏接一处就是静默单边差异。

顺带修的一处 abort:**Rust 闭包里捕获 `Object<'js>` 会让它逃出 QuickJS 的
GC 记账**,`JS_FreeRuntime` 那里 `gc_obj_list` 非空直接 abort
(`headers_fn` 与 `raw().request()`)。规矩:**闭包只捕获 Rust 数据,
JS 对象每次调用现建**。

## M2n:回落页补上 `<td>` + 按源合成(16 → 1)

上一轮那 15 例记的是「`<tr>` 里的格式化元素,jsoup 留在行内而 html5ever 挪出
表外」——**那一半是回落页自己造出来的**:`tools/fallback_page.py` 给
`bookList: tbody tr` / `tag.tbody@tag.tr@tag.a` 这类规则合成的是
`<tr><a>…</a></tr>`(**没有 `<td>`**),而真实站点是 `<tr><td><a>`,
后者两个解析器一致。生成器补上这一层(`PAYLOAD_WRAPPER` / `CHILD_OK`,两个
方向都补:payload 装不进 `<tr>`、`<a>` 也装不进 `<tr>`)之后,这 15 例从
「照不到」变成**真的在测规则**,其中 14 例当场 PASS,剩下 1 例
(pb00407)是**真缺口**:`BookList.getInfoItem` 里 `@put:` 存进 ruleData 的
变量没写回实体 —— 真身的 ruleData **就是** book(`setRuleData(book)`),
`toSearchBook()` 把 `variable` 带走;被测侧 ruleData 是拷贝,不写回就丢
(`web_book.rs` 里那一行,与 `analyze_book_info_entry` 同源)。

html5ever 与 jsoup 在**畸形** `<tr>` 上的分歧仍然存在(jsoup 那边是它自己的
bug:嵌套的 `processEndTag("a")` 把 `fosterInserts` 置回了 false,机理在
`html-compat/src/lib.rs` 的头注释),只是不再有语料 case 打在它上面 ——
要复刻得改 html5ever 的树构造,风险落在 html 套的 23113 例上,别顺手动。

同一轮把 HTML 回落页整个改成**按「源 × 步」一张**(命中面 51% → 71%,
机制见 `fixtures/cases/pipeline-corpus/README.md`),新测到的面上又抓出两处:
**jsoup `hasClass` 两个代理都没有**(`result.select('em').hasClass('vip')`,
pb02708)、**XPath 选出来的 JXNode 交给下一条规则时丢了元素身份**
(pc03399,在 A 层那套照出来的;见 `RuleValue::JxList`)。

## 还没做的:0 例(M2o 起)

M2n 收盘时这里还挂着 1 例(pb02108,`\p` 那条)。**M2o 把它定了调,改判为豁免**:
Rhino 把 `\p`/`\P` 留给 Unicode 属性转义、不带 `u` 标志也直接 SyntaxError,
而 quickjs-ng 按 Annex B 当恒等转义 —— **被测侧是超集**,复刻裁判等于主动让这个源
在 Rubato 上也失效。判据与语料数字记在 `fixtures/cases/js-host/exemptions.json` 的
`js-dialect-regex-identity-escape-p`(那边是一行的最小复现,这边是它在四步上的投影),
**同一件事只判一次**。棘轮同步归 0。

### M2o 那一轮在这套上照出来的东西

回落页生成器认得的选择器形态补齐之后(索引先剥再认前缀、组合子、jsoup 伪类、
`class.` 名字里可以有空格),命中面 1994 → 2058,**一次冒出 53 个 FAIL** ——
而它们**全是同一处产品缺口**:jsoup 的 `hasClass` 有一条**等长快路**
(1.16.2:`if (len == wantLen) return className.equalsIgnoreCase(classAttr)`),
于是 `class="txt-list txt-list-row5"` 上 `hasClass("txt-list txt-list-row5")` 为真,
而被测侧两份实现都只按空白切 token。补上之后 53 条当场归零。
**这一课记住**:53 个长得完全不同的 FAIL(有的是 `toc_empty`、有的是
`book.kind` 少一节、有的是 `books` 空)可以是**一处**缺口 —— 别按症状分堆去猜,
先找那个能同时解释全部症状的口径差。

数字记在 `fixtures/diff-baseline.json` 的 `pipeline_corpus_b`(棘轮,只许往下调)。
定位用:被测侧 `PIPELINE_DEBUG=1`、裁判侧 `JSHARNESS_DEBUG=1`,两侧都把归一前的
真因打到 stderr(`[pipeline] <id>: …`)。

## 豁免

由生成器写进 `exemptions.json`(逐条带理由,入库可 review)。现有五类(第 1、4 条已还清 —— **还清的留在这里**,它们各自是一次判据教训):

1. ~~**`bookSourceUrl` 的 authority 含非 ASCII**(2 源 10 例)~~:
   **M2n 接上 IDN 之后不再豁免**,照常比、全 PASS(A 层同期收掉 17 条)。
2. **这一步用到的 `##` 正则在 regex 套里已经是已知回避面**(2 例,M2m 起):
   有界变长 lookbehind(`\h{0,4}`、`[”）】]?` 之类)Java 支持而 fancy-regex 0.14
   只支持定长,被测侧按编译失败降级为字面量替换 —— 在四步这边的形态是
   **裁判把正文清成 `content_empty` 而被测侧原样留着**(pb01528)。
   名单**直接读 `fixtures/cases/regex-compat/exemptions.json` 的 `pattern` 字段**,
   不在这里另抄一份:同一件事两处记,迟早漂;而漂掉的那天判据不会报错。

3. **引擎方言里「被测侧是超集」的那一档**(2 例,M2o 起):这一步的书源 JS 有
   `/…\p…/` 这样的正则字面量(pb02108 / pb02496)。**与第 2 条方向相反** ——
   第 2 条是我们差一块能力(**欠账**,总有一天要还),这一条是我们比裁判**更能跑**
   (复刻裁判等于删能力)。生成器的统计行把两档分开报,别混着读。
   判据同样只记一处:`fixtures/cases/js-host/exemptions.json`。

4. ~~**LiveConnect 加解密面未接**(M3a 起,1 例生效 / 72 例挂着)~~:
   **M3q 删掉了**。这条豁免的故事值得留着:面本身 **M2e 就接上了**
   (`js-host/src/live_connect.rs`),豁免却一直挂到 M3q —— **豁免比它的理由活得久**。
   真拆的时候,72 个挂着它的 case 里只有 **1 个**还在分岔(pb02492:空输入解密该给
   空数组,被测侧抛 IllegalBlockSizeException),修完就是 0。
   **教训**:豁免要按轮回头问一遍「它今天还成立吗」—— 不然它既会把一块早就绿了的
   面继续盖着,也会把后来才长出来的分岔一起盖掉。这一面现在在移植对照台账里
   有一整节(`docs/port-ledger.md` 的 `liveconnect`)。

5. **裁判环境差**(6 例,M3q 起 —— **既不是欠账也不是超集**):这一步走 hutool
   那条对称加解密路(`java.aes*` / `createSymmetricCrypto`)且 transformation 写的是
   `AES/…` 全式。hutool 5.8.22 把**整条 transformation 当算法名**塞进 `SecretKeySpec`,
   裁判所在的 JVM(SunJCE)会查 `key.getAlgorithm()` 并拒收,而真身所在的 Android
   (Conscrypt)只查密钥长度、照跑 —— 于是裁判整步 `pipeline_error`、被测侧出结果。
   语料 **17 个源**这么写。**这一档与前两档都不同**:第 2 条是我们少做、第 3 条是
   我们多做,这一条是**这一位在这台裁判上问不出真身的答案**。
   判据同样只记一处:`fixtures/cases/js-host/exemptions.json` 的
   `js-hut-aes-full-transformation`。**两套都给**(top100 也给,见那边的 README)——
   它不是「我们还没做」,换个层同样问不出答案。

   > 这 6 例从前是**碰巧一致**:被测侧那时对空输入解密抛 IllegalBlockSizeException、
   > 与裁判一起以 CryptoException 收场 —— 一致,但两个理由。M3q 把空输入那一位
   > 按真身修对之后,藏在下面的环境差才露出来。**「一致」不等于「同一个理由」。**

豁免的 case **仍然跑、仍然比**,只是差异不算 FAIL,数量在 diff 报告里可见。
每一类都只在**两侧真的不一致时**才生效(比较器先判等、再判豁免),
所以「多列了几条」不会把本来就一致的 case 变成未过。

## 命中面

`tools/hit_report.py` 每次跟着通过率一起打(见
`fixtures/cases/top100/README.md` 的同名段落——口径逐字相同)。
2026-08-31(**M3a 修分层口径**那一轮,B 层 593 → 647 源)收盘:

| 步 | 共 | 命中 | 源没配 | 配了没命中 | (html 页) | (json 页) |
|---|---|---|---|---|---|---|
| search | 647 | 572/643 (88%) | 4 | 71 | 453/509 (88%) | 119/138 (86%) |
| explore | 452 | 333/365 (91%) | 87 | 32 | 274/329 (83%) | 59/123 (47%) |
| info | 647 | 496/532 (93%) | 115 | 36 | 398/479 (83%) | 98/168 (58%) |
| toc | 647 | 567/645 (87%) | 2 | 78 | 457/505 (90%) | 110/142 (77%) |
| content | 647 | 565/647 (87%) | 0 | 82 | 446/481 (92%) | 119/166 (71%) |
| **合计** | **3040** | **2533/2832(89%)** | **208** | **299** | **2028/2303 (88%)** | **505/737 (68%)** |

**M2o 改了分母口径**:「源没配这一步的入口规则」(`ruleToc.chapterList` 为空、
整个 `ruleExplore` 缺席…)那一档裁判也拿不到东西,留在分母里会把「这个源没有
探索页」读成缺口 —— 2783 个 case 里有 **188 个**是它。
html/json 两列是**未扣**的原始数(按回落页口味分),两套口径并排放着,免得只看一个。

**这张表上一版(M2o 收盘)是 2058/2595 = 79%**,当时点名「真正的缺口集中在
toc(71%)与 content(75%)」。2026-08-31 那一轮就是照着这句做的:回落页生成器
认得的形态补齐(索引串/负索引照 `findIndexSet` 数全、`CHILD_OK` 只留 table 一族、
不再用白名单拦未知标签、切掉尾巴上的 `<js>` 后处理块)、口味改成**按哪张页造得出**
判而不是按 `$` 前缀 —— toc 71%→78%、content 75%→84%,合计 79%→83%,
未命中 537 → 437。

**未命中的那些为什么没命中 —— 有脚本,别按症状猜**:`tools/miss_report.py`
(用法 `tools/miss_report.py pipeline-corpus-b [--step=toc]`)。它挑出未命中的
case,单独喂给 `JSHARNESS_DEBUG=1` 的裁判 —— 裁判把一切异常归一成
`pipeline_error`,**一个标签盖住几十种因**,不问它就只能猜。

**猜会翻车,当轮就翻过一次**:先按 `chapterList` 的**长相**分堆,数出「33 例页已按
源造了却仍选不中」,断定是生成器造错了;真跑一遍才看清它们**根本没走到选择器
那一步**,是后面的 `@js:` 当场抛了。

2026-08-31 第八轮之后的 282 例(下表的分堆是第七轮那一版,量级仍然成立):

| 真因 | 例 | 能不能修 |
|---|---:|---|
| 没报错、就是空(选择器/JSONPath 没选中) | 109 | 一部分能:见下 |
| 书源的 JS 在合成页上跑不通(`match` 返回 null) | 47 | 一部分能:三条取值路已接,剩下的见第六轮 |
| **加密族**(base64→AES/3DES 34 + JSON 形状后接解密 30 + AES 19) | 83 | **不能**:要拿书源自己的密钥把 payload 加密回去 |
| 书源的 JS **自己在 Rhino 上就语法错误** | 11 | **不能**:真身也跑不动这个源 |
| Rhino `\p` 方言 17 / 没配 url 4 / 其它 9 / 页面类型 6 | 36 | 多数不用修 |

「没报错、就是空」那 109 例**再按生成器自己的视角分一次**(它造没造出结构,
是能直接算出来的,不用问裁判):**html 造出了结构却仍空 42**(最难的一档:
容器选中了、后面的 JS 又把它筛没了)、**html 没造出结构 46**(其中**整条是 JS
剥完什么都不剩 25**、**前导 `-` 12 已知不能修**、`@put:{…}` 整条 4、正则模式 4)、
**json 造出了结构却仍空 19**、json 没造出结构 2。

「没报错、就是空」里按**入口规则的形态**细分(第四轮量的,量级仍然成立):
生成器认得的一档最大,其次是**整条就是 `<js>`**(反推不出容器)、
**前导 `-`**(**不能修**:实测**裁判自己也抛** —— `select("-class.box")` 不是合法
jsoup 选择器,`dsl_error`)。

### 八轮做了什么

- **第一轮**(形态补齐 + 口味按「哪张页造得出」判):79% → 83%,未命中 537 → 437。
- **第二轮**(`rx_sample.py` JS 正则诱饵):「JS 跑不通」117 → 81,命中 2158 → 2175。
  **顺带照出一处产品缺口**:jayway 的 `JsonPath.parse(String)` 头一行是 `notEmpty` ——
  书源把正文解成**空串**再交给 `$.content_html` 时裁判**整步抛**,而被测侧一路宽松、
  给 content_empty。补在 `AnalyzeByJSonPath::parse_string`,边界(空串抛 / 全空白不抛 /
  字面量 `null` 另一条消息)拿 jayway 的 jar 实跑出来,探针留在 json 套的 `DSL_EDGE_DOCS`。
- **第三轮**(口味再认一条:JS 直接把页面 `JSON.parse` 掉):
  `Unexpected token: <` **52 → 4**,而命中只 +1。
  **这一轮的价值不在那 +1,在于它把 52 例的真因从「页面类型给错了」翻出来,
  露出后面真正挡路的是加密族** —— 在此之前那 52 例的账全记在「页面类型」上,
  照着它继续投工是白干。判据的含金量就是这么一层层剥的。
- **第四轮**(生成器的两处形态缺口)命中 2176 → **2203**,未命中 419 → 392:
  - **`@put:{…}` 要剥掉**(40 例的规则里有它):它是**副作用后缀**,真身
    `SourceRule.splitPutRule` 拿 `@put:(\{[^}]+?\})` 把它整段 replace 掉,剩下的
    才是选择器。此前见到它就整条不认 —— 于是 `name: "Name@put:{id:$.Id}"` 的源
    **页面上没有 `Name` 这个属性**,容器选中了、name 取到空串,而 legado 的
    `getSearchItem` 把没有 name 的项整条丢掉,`books` 就是空的。
  - **XPath 翻成选择器再合成**(`_xpath_to_css`):生成器从第一天起就整条跳过
    XPath(全库 213 处)。裁判跑的是 JsoupXpath(**在 jsoup 树上**跑的方言),
    所以只要造出「等价 CSS 选得中」的树它就选得中。只认语料里真出现的那一小撮形态,
    **谓词里有一样认不出就整条 None**(丢掉谓词凑合造 = 白造一张页还把底板挤掉)。
  这一轮**三套一起抬**:A 层 3162 → **3185**(97%→98%)、top100 399 → **405**(91%→92%)。
- **第五轮**(口味与选择器的三处)命中 2203 → **2239**(**86%**),未命中 392 → 357;
  A 层 3185 → **3198**(**99%**,未命中 61 → 48)。三件:
  - **`[name=og:novel:book_name]` 被截成 `[name=og]`** —— `_take_pseudos` 不看方括号,
    把属性值里的 `:novel` / `:book_name` 当成两个伪类摘走了。语料 `og:` 那一族 meta
    50+ 处。症状是 info 步「name 空、别的字段有值」(pb00399)。
  - **`[*]` / `[?(` 是 JSONPath 独有的写法** —— CSS 里 `[*]` 不合法。语料 **82 个
    「源×步」**的入口规则长这样却不带 `$` 前缀(`data[*]`、`DATA.BOOKCATA[*].CHAPTERS[*]`),
    此前一律当 CSS 发 HTML 页。只看 `<js>` 块**之前**那一截(JS 里的 `[*]` 是代码)。
  - **`info` 步 + 单段裸键 `name` ⇒ HTML 页永远取不到值**(见下面「探出来的一条口径」),
    改发 JSON 页 —— info 命中 **422 → 452(86% → 92%)**。

- **第六轮**(**按 JS 的三条取值路放诱饵**)命中 2239 → **2289(88%)**,未命中 357 → 307;
  top100 405 → **412(94%)**;A 层不动(那一层按定义没有 `<js>`/`@js:`)。十六套仍 0 FAIL。

  这一轮的起点是**上一轮记的归因被推翻**:未命中里「JS 跑不通」那 93 例记着
  「`rx_sample` 再多认几种取值来源」,真去看才发现 **71 例的正则本来就造出了样例**
  —— 诱饵在页尾,而 JS 读的**根本不是页面**。三条取值路各修各的:

  1. **字段规则自己取出来的值**(`fallback_page._fit_value`,34 例)——
     `bookUrl: "tag.a.0@href@js:result.match(/_(\d+)/)[1]"` 里的 `result` 是
     **前一截选择器取出来的那个值**,不是页面。诱饵要放进合成页上那个 `href`
     的值里。先试「原值 + 样例」(原值别的规则也在读,换掉会把命中的字段弄空),
     不行才退到「样例 + 原值」、最后「只要样例」,每一步拿 `re` 验一遍;
     **属性位上不放 `"` / `<` / `&`**(`_emit` 拼属性时不转义,放进去就是畸形 HTML,
     两个解析器在那里分歧,测的就不是规则了)。
  2. **JS 自己写的那条规则**(`fallback_page._js_reads`)——
     `chapterList: "@js: var data=java.getString(\"@@.last_page@href\"); …"`:
     `.last_page` 不在任何**规则字段**里、只在 JS 的字符串里,生成器从来没造过它。
     现在把这些规则串也当规则反向合成,值同样要满足随后作用在它上面的正则。
     两条口径照真身分开:`getElement(s)` 收**列表规则**(每段都是元素步)、
     `getString(List)` 收**字段规则**(末段是动作)。**只在整条是 `@js:`/`<js>`
     的规则上认** —— 那时 `java.getString` 面对的 content 才是整张页;字段规则
     尾巴上的 `@js:` 里 content 是当前那个元素,落点不一样。
     顺带解锁了「**整条是 `<js>` 的容器反推**」那一档:
     `chapterList = java.getElements(".section_list li a")` 读到的那批元素**就是**
     这一步的容器,字段规则从此相对它们造(此前容器一支都合不出就整条退回底板)。
  3. **请求地址本身**(`gen_pipeline_corpus_cases.base_url_fit`,19 例)——
     `n=baseUrl.match(/book_id=(\d+)/)[1]`。`baseUrl` 是 info 的 `book.bookUrl`、
     toc 的 `book.tocUrl`、content 的章节 url(真身 `setBaseUrl` 的那三处);
     search/explore 的地址由书源自己的 `searchUrl` 算出来,不认。
     **补在查询串上**(`?…`)而不是直接拼在路径后面 —— 后者会把地址弄成
     `t1.htmlbook_id=3` 这种不像地址的东西。

  顺带扩了 `rx_sample`:**正则先存进变量再用**的写法(`var re=/…/; src.match(re)`)。

  **剩下的 41 例**(分堆见 `tools/miss_report.py`):JSON 页 13(诱饵与形态这两件
  至今只造在 HTML 页上)、值来自 `java.ajax` 取回的另一张页 8、DOM 上下游导航
  (`parentNode` 链)3、其它 17。

- **第七轮**(**`<js>…</js>` 是有界的**)命中 2289 → **2309(88%)**,未命中 307 → 287;
  A 层与 top100 不动。十六套仍 0 FAIL。

  **这一条是照着裁判的源码定的,不是猜的**:真身 `AnalyzeRule.splitSourceRule` 拿
  `JS_PATTERN = <js>([\w\W]*?)</js>|@js:([\w\W]*)` 把一条规则切成**一串**子规则,
  段与段之间的文本各自是一条规则、按顺序作用在上一条的结果上。两处不一样,而生成器
  此前把它们混为一谈(见到 `<js>` 就把后面全丢掉):

  - `@js:` **吃到结尾**(`[\w\W]*`),后面确实没有规则了 —— 这一半此前是对的;
  - **`<js>…</js>` 是有界的**,`</js>` 之后那截是**下一条规则**。语料里 **44 个
    「源×步」**的入口规则长这样(`<js>eval(String(source.bookSourceComment))</js>`
    这种前奏块打头,真正的选择器/JSONPath 在后面),此前**一律整条丢掉** ——
    既没造结构,口味也判错(`$..list[*]` 打头的 12 例一直在发 HTML 页)。

  口径只记一处:`fallback_json.rule_segment`,`fallback_page._normalize_sel` 与
  `gen_pipeline_corpus_cases._looks_json` 都调它。这一改**同时动了「认不认」和
  「口味怎么判」**(58 个步从 html 口味翻到 json),正是 README 里记着的那个跷跷板 ——
  所以两件一起复验,净 +16。

  顺带补了 **CSS 选择器组**(`#a .x, #a .y`,4 例):jsoup 的 `select` 收它,
  「或」的关系,造页面满足**第一支**就够。**只在段内切**(`.list@span,li` 是
  `.list` 和 `span,li` 两段)—— 跨 `@` 切会把 `A@B,C` 切成 `A@B` 与 `C`,
  后者落到根上就是造错地方;方括号/圆括号/引号里的逗号不算。

- **第八轮**(**JSON 页也按 JS 反推形状**)命中 2309 → **2314(89%)**,未命中 287 → 282;
  top100 412 → **413**。十六套仍 0 FAIL。

  HTML 那张页有三条「按 JS 反推」的路(第六轮),**JSON 这张一条都没有** ——
  `content: "@js: JSON.parse(result).data.page.map(i => …)"` 这一档合成页上没有
  `data.page`,`.map` 当场 TypeError。`fallback_json._js_shapes` 反推三件:
  ① **链本身**就是路径(`$.data.page`);② **链后面那一下**说它是什么
  (`map/forEach/reduce/length/[i]` ⇒ 数组,`replace/split/match` ⇒ 字符串);
  ③ **数组的项长什么样由回调的形参说**(`.map(i => … i.image …)` ⇒ 项里有 `image`),
  认不出形参才退回这一步自己的项目底板。别名也认一层
  (`$ = JSON.parse(result).data;` 之后的 `$.chapters.forEach(…)`)——
  语料里这是最常见的写法;名字可能就叫 `$`,所以边界用 `(?<![\w$])` 不用 `\b`。

  **候选 39 例只换来 5 例命中** —— 与第三轮量出的天花板一致:这一档后面接着解密。

  **试过但没做的:JSON 页上的正则诱饵**。HTML 那张缀在 `</body>` 前买到 36 例;
  JSON 挂不了裸文本、只能加一个根键,实测**只 +1**,而代价是每一张有 JS 的 JSON 页
  都多一个根键(书源 `for (k in obj)` 就会看见它)。**不值,故不做** ——
  理由写在 `fallback_json.build` 上面,免得下一轮又试一遍。

### 探出来的一条口径:`init` 之后上下文变回 Document

真身这条路是 `BookInfo` 先 `setContent(analyzeRule.getElement(init))`,而
`getElement` 走 `getAnalyzeByJSoup(content).getElements(rule)` —— 交回来的是
**`Elements`(列表),不是 `Element`**;`AnalyzeByJSoup(doc: Any)` 对非 `Element`
的入参一律 `Jsoup.parse(doc.toString())`,**又变回一个 Document**。于是单段字段
规则 `name: "Name"` 不走选择器、直接 `element.attr("Name")`,而 Document(`#root`)
的属性在 HTML 里根本写不出来 —— **这一档书源只能跑在 JSON 页上**。

四条探针(拿 pb00515 的页面与规则实跑,不是读源码推的):

| | 规则 | 结果 |
|---|---|---|
| t1 | `init="data"` + `name="Name"` | **空** |
| t2 | 无 init + `name="data@Name"` | `书名一` |
| t3 | 整页 + `name="Name"` | **空** |
| t4 | `init="data"` + `name="text.书名一"` | **空**(init 之后整个上下文都不对了) |

**`init` 是 `<js>` 的除外**:那时 `setContent` 收到的是 JS 造出来的对象,字段规则
对着对象求值,HTML 页照样成立 —— 实测 3 例正是这样命中的,规则里必须把它们排掉。

### 这一轮照出的欠账(棘轮 0 → 1)—— **同日已还清,拧回 0**

**pb01652**:书源 `init: results` 从 JSON 页选出一个对象,随后 `@js:` 里写
`result.anchor`。真身 Rhino 把 Java Map 包成 **NativeJavaMap**,键直接当属性读,
拿到 `玄幻`;被测侧 `AnalyzeRule::bind_value` 对 jayway 值那几支按
`to_java_string()` 过,于是 `result` 是个字符串、`.anchor` 撞上
`String.prototype.anchor`,读出 `function anchor() { [native code] }`。

**这不是新缺口,是被新照到的旧缺口**(plan §4 M2f 就点过名的「Rhino NativeObject
那半」)。当轮**没有当场改**:换成 JS 原生对象只在这一格上对,`typeof` /
`String(result)`(Java Map 的 toString 是 `{k=v}`)/ `for..in` 全是面,凭「看着该
这样」直接改会把别处已经绿的例子弄红。

**还法(M2q)**:照着记下的顺序 —— **先探再改**。js 套新开 `resultRule` 字段
(两侧都走真身 `getElement` → `evalJS` 这条产品路径),`js-bind-*` 六十八条探针把
真 Rhino 的形态逐条钉住,再动 `BoundValue`。探针买到的口径见
`fixtures/cases/js-host/README.md` 的「`result` 的形态」——**其中两条推不出来**:
装箱的 `Boolean` 在 `if` 里连 `false` 都算真;而**顶层绑定的标量不装箱**、
只有从容器里取出来的才装。落地是 `rubato_core::host::JavaValue` +
`BoundValue::Java` / `BoundValue::Native` 两支。

两列都是反向合成来的,机制见 `fixtures/cases/pipeline-corpus/README.md` 的
「回落页怎么来的」:json 那一列在 M2m 之前是个位数百分比(手写的固定形状),
html 那一列在 M2n 之前是 47%(共用一张、payload 写死)。
