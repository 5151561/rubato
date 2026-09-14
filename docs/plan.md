# Rubato 计划书:Rust 引擎 + Flutter UI

> 状态:**Phase 0 已达标;Phase 1 三条判据全部闭合**——裁判基准 LegadoTeam/legado@3046111c,
> **十二套差分 92362 例 0 FAIL**(规则级 + A 层四步),
> store / engine / ffi / Flutter 三页落地,**Android 真机 + macOS 端到端验收通过**。
> **Phase 2 三条判据全部闭合(2026-08-30 M2l)**;此后的每一轮动的都是
> **判据的含金量**,不是判据的数字。
> **当前(2026-09-01,Phase 3 M3a–M3s):十八套差分 101221 例
> 101130 PASS / 0 FAIL(豁免 91)**;**移植对照台账 `--todo` 已清空**
> (`tools/port_audit.py --check` → 错 0 条、欠账 0 条)。
> **最新三件**:**M3q** LiveConnect 加解密面结案(拆掉的豁免比接上的面还多);
> **M3r** 三条尾巴线索判完、**webView 线结案**,判的路上掉出两处引擎分岔
> (JsoupXpath 的 `@attr` trim、`chapter` 的变量绑定整块没接);
> **M3s** 发现页分类列表 —— 第十八套 `explore`(1017 例)+ 产品的发现页,
> 台账最后一族欠账还清。
> 此前 **M3n 让可见浏览器按书源的
> UA / 转发头加载**(真身 `WebViewModel.initData`:`url,{…}` 拆开、UA 交给
> WebSettings、POST 先抓;**这一面同时进了差分**,js 套 2062 → 2066 例)——
> 过盾靠的就是 UA 一致,此前可见窗口用的是系统缺省 UA,用户点过之后
> 书源再 `java.ajax` 会被当成另一位客户端重新拦下。**当天真机验收(Android 16 /
> WebView 151):八一中文的盾放行、50 命中出书**(M3m 那次是「能弹能回传但盾没过」;
> 同轮换了 UA 与设备两个变量,不单独归因)。**M3o 把 `webview:timeout` 那一堆
> 判完**:五个是站点从手机连不上(报表此前从 Mac 探,判反了 —— 现在
> `--device` 走 adb),两个是真缺口(DOM 齐了但 load 事件被吊住、
> `onPageFinished` 不来)—— 补 `readyState` 看门狗后 webview 线走到底 **16 → 19**。此前 **M3m 补齐非搜索入口并把
> `startBrowserAwait` 跑上 Android 真机**(webView 清单 98 search + 2 explore;
> 两条仅发现源都走到目录;八一中文真实 Turnstile 能弹、能交互、能回传,
> 但旧 Android WebView 被再次挑战,诚实记“盾未通过”)。此前 **M3l 把登录线
> 接进产品**(`loginUrl` 可见 WebView + cookie 回抄、`loginUi` RowUi 表单 +
> AES 持久化 + `login()`/按钮 action;`getLoginInfoMap` 加两条 Rhino 探针)。
> 此前:**M3d 把 webView 的平台那一半
> 接上了**(PlatformHooks 过 FFI 到 Dart:`ffi::platform` 的桥 +
> flutter_inappwebview 的 headless 实例;端到端由
> `app/integration_test/webview_test.dart` 钉住 —— 一个书名**由 JS 写进 DOM**
> 的页,`,{"webView":true}` 的书源整条走通,macOS 实测过。差分一例不动);
> **M3c 把 webView 的策略层接进了引擎**(`{"webView": true}` / `@webjs:` / `java.webView*` 三处的确定性桩两侧
> 一起拆掉,见下);**M3a 修分层口径**
> (`ContentRule` 默认带的两个空键把 92 个源误判成 C 层、整源缺席 A/B 两套 ——
> A 879→908 / B 622→684 / C 203→112,两套扩到 4027 / 3040 例),
> **M3b 新开第十七套 `webview`**(现 98 例:`BackstageWebView` 的**策略层**,
> 裁判是第三个 harness `:wvharness`,挂真身那 394 行)。
> 此前当日还有:命中面照出一处旧缺口 0 → 1(pb01652 / `BoundValue`)、
> **同日按记下的顺序还清、棘轮拧回 0**(M2q,见下);js 套因形态探针 1975 → 2043 例。
> 判据实测:**B 层 100.0%**(线是 80%)、**自选 top-100 按源 100/100**(线是 90)、
> **差分 CI 常绿**(`tools/all_diff.sh` + `fixtures/diff-baseline.json`,基线只许往下调)。
> **命中面**(裁判侧真的装配出东西的比例,判据要连着它读):
> A 层 **98%**(3198/3246)、B 层 **89%**(2314/2596)、top100 **94%**(413/437);
> **未命中那一半为什么没命中**由 `tools/miss_report.py` 分堆(它去问裁判要真因,
> 别按症状猜)。
> 合计**由 `all_diff.sh` 的汇总行打出来,不要手加**——此前这里写的 92119 就是手加飘掉的。
> 已闭合的架构件:**三个宿主**(AnalyzeRule / AnalyzeUrl / **BaseSource**)、
> Java 对象代理层、RuleHost 反调面、五个绑定、org.jsoup LiveConnect、确定性垫片、
> **网络面**(AnalyzeUrl 真身 + ReplayHttp 进 jsharness,两侧共用 HTTP 录放)、
> **`java.*` 按频次补齐 + 对称加解密**、**LiveConnect 的 crypto 一支**、
> **`@js:` url + 登录 header**(真实 `searchUrl` 整条走 AnalyzeUrl,1014 例全绿)。
> **XPath**(M2g:照着 JsoupXpath 移植,1726 例 0 FAIL;顺带解锁 rule 套 105 例
> 与 pipeline-corpus 的 33 个 XPath 源)。**M2h:产品路径补课**——`java.ajax` 在产品里
> 真的发请求了(此前 `net: None`,返回字面串被当正文拼进去)、`androidId` 落库复用、
> cache/cookie 跨规则共享、执行闸(30 秒 / 256 MiB / 取消钩子)、差分 CI。
> **M2i:pipeline-corpus 扩 B 层**——新套 `pipeline-corpus-b`(裁判 `:jsharness`
> 挂 WebBook 四步真身 + 真 Rhino,被测侧真 QuickJS + 网络面),一上来就抓出
> **四处产品路径上的空绑定**(`source` / `book` / `chapter` / `rule_host`)。
> **M2j:`result` / `src` 绑定走真对象**——`JsBindings` 那两个绑定从
> `Option<String>` 换成 `BoundValue`(`Str`/`StrList`/`Element`):真身绑的是
> **对象本身**(jsoup 的 Elements 在 Rhino 那边是 NativeJavaList)。连带三件:
> 元素能穿过 JS 回来(`JsValue::Elements`)、`seal_proxy` 接上(`for..in` 只给下标)、
> `Elements.text()/html()/toString()` 的拼接按 jsoup(**空项不产生分隔符**)。
> **79 → 66,js 套顺带 30 → 28**,别的十三套零变化。
> **M2k:把 B 层那份清单做完**(66 → 16,B 层 **99.2%**)。
> **M2l:最后一条判据 `top100`**——自选 100 个常用源(A 52 / B 40 / **C 8**,
> C 层第一次进流水线差分),**按源** 100/100、按 case 446/446、0 豁免。
> 名单是一条写死的可复算规则(不是手挑),落在 `selection.json` 入库可 review。
> 顺带修好两处:**回落页口味按「步」算而不是按源**(一整批书源搜索走 JSON API、
> 目录/正文是 HTML 页,按源取味必然有一半的步骤拿到永远匹配不上的那张页 ——
> 改完 A 层套的 toc 命中 43%→50%、content 78%→85%,**FAIL 数零变化**),
> 与 **`source.setVariable(0)` 把变量删掉了**(真身只有 null 才删)。
> **M2m:JSON 回落页改成按源反向合成**(`tools/fallback_json.py`,一个「源 × 步」
> 一张、内容哈希去重)——判据的数字一个没动,动的是**底下有多少面是真被测过的**:
> top100 的命中面 **251/446(56%)→ 331/446(74%)**,其中 JSON 页那一列
> **~15% → 92%**;A 层 2163/3894(55%,JSON 列 91%)、B 层 1447/2783(51%,
> JSON 列 75%)。新测到的面上露出六处缺口,五处当场修掉(`{{result}}` 里整数
> Double 的格式化、jayway 路径不 trim、`book.kind` 该是 null、合成页自己造出的
> 分歧、过滤器谓词要认全比较运算符),第六处是已知回避面进豁免。
> 命中面由 `tools/hit_report.py` 打,三套的 `*_diff.sh` 都跟着通过率一起报 ——
> **各 README 里的表别手抄**。
> **M2n(2026-08-30 收盘):HTML 回落页也改成按源反向合成 + 把它照出来的缺口修完**。
> 十六套 **99372 例 99279 PASS / 24 FAIL(豁免 69)**——FAIL 只在两套:
> **js-host 23**(1894 例)与 **pipeline-corpus-b 1**(2783 例,就是那条
> Rhino/`\p` 的方言)。**A 层与 top100 现在是 0 FAIL、0 豁免**。
> 命中面 top100 **74% → 87%**、A 层 55% → 77%、B 层 51% → 71%
> (html 页那一列 47% → 70~85%)。抬上来的面照出**七处产品缺口**,全部修完:
> `getInfoItem` 的 `@put:` 变量没写回实体、jsoup `hasClass` 没接、
> **XPath 选出来的 JXNode 交给下一条规则时丢了元素身份**、java_proxy 的箱交给
> String 形参要解包、**IDN host**(接上 okhttp 的 idnToAscii,连带收掉 27 条豁免)、
> `Elements.remove()/.not()/.eachText()`、`java.getString(rule, mContent)` 的第二
> 个实参。**两侧各连跑两次逐字节相同**;`tools/check_generated.sh` 绿。
> **M2o(2026-08-30 收尾):把命中面的最后两块与 js 套的方言那一堆都收掉**。
> 十六套 **99372 例 99286 PASS / 8 FAIL(豁免 78)**——FAIL **只剩 js-host 一套**,
> 而且 8 条全是「面还没接」(RSA 1、startBrowserAwait 1、htmlFormat 1、
> 错误类别 2、网络面影子 3);**另外十五套 0 FAIL**。
> 三件事:
> ① **回落页生成器认得的选择器形态补齐**(`tools/fallback_page.py`):索引要
> **先剥再认前缀**(`tag.tr!0` 此前整条作废,语料 130+ 处)、组合子 `>`/`+`/`~`、
> jsoup 伪类(`:eq()`/`:has()`/`:matches()`/`:contains()`)、`*`,
> 以及 **`class.` 的名字里可以有空格**(语料 146 处)。B 层 HTML 口味的步
> **1760/2473 → 1872/2473 合成出结构**,命中 1994 → 2058。
> ② 抬上来的面**一次照出 53 个 FAIL,而它们全是同一处产品缺口**:
> jsoup 的 `hasClass` 有一条**等长快路**(`if (len == wantLen) return
> className.equalsIgnoreCase(classAttr)`),于是 `class="a b"` 上 `hasClass("a b")`
> 为真 —— 被测侧两份实现都只按空白切 token。补上之后 53 条当场归零。
> 口径现在只有一份(`rubato_core::host::jsoup_has_class`,html-compat 调它)。
> ③ **js 套的「两个引擎的方言」定了调**(23 → 8 FAIL + 8 豁免),判据只有一条:
> **这个差异会不会让书源在 Rubato 上退化**。`[a,b]=>`(Rhino 收、规范不收,
> 语料 **5 个源**)→ **收紧被测侧**(`js-host/src/dialect.rs`,只在 SyntaxError
> 之后重写一次再跑);`class`/展开/`\p`/Java 重载歧义(被测侧是超集)→ **豁免**,
> 复刻裁判等于主动删能力;`for each`/E4X(语料 **0 个源**)与完成值口径 → 豁免,
> **探针留着**。逐条理由与语料数字在 `fixtures/cases/js-host/exemptions.json`。
> 顺带修掉 `[native code]` 的缩进与 `hexDecodeToString` 的错误类别。
> ④ **命中面的分母改了口径**:`hit_report.py` 单列「源没配这一步的入口规则」
> 那一档(B 层 725 个未命中里有 **188 个**是它)—— 那是「这个源没有探索页」,
> 不是缺口。扣掉之后 B 层 73% → **79%**、top100 87% → **90%**。
> 判据要连着**命中面**读。
> **M2p(2026-08-31):js-host 那 8 条欠账逐条还清,十六套 99453 例
> 99376 PASS / 0 FAIL(豁免 77)—— 全绿**,`diff-baseline.json` 的 `js` 拧到 0。
> 顺序是「先探再改」,探针留在 `gen_js_cases.py` 当判据(js 套 1894 → 1975 例)。
> 八条:① `java.htmlFormat`(HtmlFormatter 从 pipeline **下沉成 `html-format` crate**,
> 两条路共用一份);② `java.ajaxAll`(不吞异常 / 不带 ruleData / 返回 StrResponse 数组);
> ③ `cache.putFile/getFile` —— **是裁判垫片的契约缺口**:真身有(打 ACache,与
> `put/get` 的 cacheDao 是**两张表**),垫片整支缺席,于是裁判先 TypeError、被测侧
> 多发一跳;两边一起补,新增 `cacheFile` 观察面;④ **`getElement(s)` 交回来的
> 不一定是 jsoup `Elements`** —— 容器形态由**规则模式**决定(jayway `getList` /
> `List<JXNode>` / 正则 / JS 给的是普通 `ArrayList`),`toString()`、过不过得了 GSON、
> 项能不能按键取属性三处都不同,被测侧此前整片按 Elements 建箱;
> ⑤ **jsoup 选择器的错误通道**(`el_select` 从前没有 Err,调用方 `if let Ok(..)`
> 吞掉):空串是 `ValidationException`、其余是 `SelectorParseException`,
> 顺带补 html-compat 的 `[k$=]` 空值校验;⑥ **RSA 一支**(语料 2 源:
> `PKCS8EncodedKeySpec` + `KeyFactory` + `Signature`);
> ⑦ **完成值口径** —— 原先记的「startBrowserAwait」与「空 Elements」两条**归因都是错的**,
> 真因是规范 §16.1.7 的 `UpdateEmpty`(空完成值留用上一条):Rhino 与 V8 都照办,
> **不照办的是 quickjs-ng**。补法 `dialect::split_value_tail`(按顶层语句边界切段、
> 顺序求值、取最后一个非 undefined;切之前先用 `if(0){…}` 把每段编译一遍),
> 连带作废 `js-corpus-ff6d88b7c0` 那条**方向判反了**的豁免;
> ⑧ **响应对象上返回 String 的成员是装箱的**(`body()`/`url()`/`raw().body()`)。
> `startBrowserAwait` 到今天**一次都没被真的执行过**(它在那个 case 没走到的分支里),
> 仍属 Phase 3。**两侧各连跑两次逐字节相同**;`tools/check_generated.sh` 绿。
> **2026-08-31(命中面那一轮):不改任何被测语义,只让判据照得到更多面**。
> 十六套仍是 99453 例 0 FAIL,而裁判侧真装配出东西的比例:
> A 层 3088/3247(95%)→ **3162/3246(97%)**、B 层 2058/2595(79%)→
> **2158/2595(83%)**、top100 395/437(90%)→ **399/437(91%)**。四件:
> ① **XPath 的 ANTLR 单记号删除**(`xpath-compat/src/parse.rs`):裁判对
> `/pc/book/{$.id}/catalog` **不报错、返回空**(词法丢掉 `{}`、语法把 `$` 删掉再来一次、
> 尾巴被「文法没有 EOF」无视),语料 **20 条**规则长这样(其实是嵌了 `{{$.id}}` 的相对
> URL,以 `/` 开头被真身当 XPath),被测侧此前一律硬报错、整步当场死掉。
> **只在 `/` 之后的 step 上恢复,不在头一个 step 上** —— 头部是 ANTLR 的预测点,
> NoViableAlt 不做单记号删除,放宽了方向就反了(每条期望值都拿 JsoupXpath 的 jar 实跑)。
> ② **回落页生成器认得的形态再补一轮**(`tools/fallback_page.py`):`CHILD_OK`
> **只留 table 一族**(多补 `<dd>` 那一层是**有害**的 —— `#list dl>a` 的直接子代当场断)、
> 索引照 `findIndexSet` **数全**(`!0:1:…:8` 这种串与 `!-1` 这种负数)、
> 不再用白名单拦未知标签(`data.chapter_lists` 选的是 `<data>`)、
> 切掉尾巴上的 `<js>`/`@js:` 后处理块(前面那截仍是元素规则)。
> ③ **口味按「哪张页造得出」判,不按规则前缀**:真身 `isJSON || startsWith("$.")`
> —— 页面是 JSON 时**任何**规则都走 jayway,所以 `chapterList: data.list[*]` 满语料都是。
> **顺序要紧**:必须先修好 ② 再上 ③,否则「html 造不出」里混的是生成器自己的缺口。
> ④ **请求基址与主键分开**:语料 **13 个源**的 `bookSourceUrl` 根本不是 URL
> (`小说合集`、`API-1`、`🐧` —— 真身只拿它当主键),照抄就造出没有 scheme 的地址,
> 两侧**双双**在发请求之前就抛、一起「一致」而一个字段都没测(60 例里 43 例)。
> `bookSourceUrl` 一个字节不改(cookie/变量表全挂在它上面),只换生成器自己编的那几个基址。
> **2026-08-31(命中面第二轮):按「JS 对页面的期待」反推,顺带照出一处产品缺口**。
> 十六套 **99565 例 0 FAIL**(json 套 +112 条边界探针);B 层命中 2158 → **2175**,
> 未命中 437 → 420。三件:
> ① **先有报表再动手**:新 `tools/miss_report.py` —— 挑出未命中的 case 单独喂给
> `JSHARNESS_DEBUG=1` 的裁判,按**真因**分堆(裁判把一切异常归一成 `pipeline_error`,
> 一个标签盖住几十种因)。**当轮就证明了「按长相猜」会翻车**:先数出「33 例页已按源
> 造了却仍选不中」并断定是生成器造错,真跑一遍才看清它们根本没走到选择器那一步。
> ② **JS 正则诱饵**(新 `tools/rx_sample.py`):把 JS 里**用来 match 的**正则字面量
> (只认 `.match`/`.exec`/`.test`/`.search`/`.split` 那几个位置)抽出来,造一段能被它
> 匹配的文本原样缀在 `</body>` 前 —— 语料里大量书源是
> `chapterList: "@js:\nvar n = src.match(/共(\\d+)页/)[1]…"` 这种写法,合成页里没有
> 那段字样就 TypeError,**整步死在选择器之前**。生成完一定拿 `re` 自己验一遍,
> 验不过整条丢掉;环视/反向引用/`\p{…}` 一律不生成。这一档 117 → 81。
> ③ **它照出的产品缺口**:jayway 的 `JsonPath.parse(String)` 头一行是 `notEmpty` ——
> 书源把正文解成**空串**再交给 `$.content_html` 时裁判**整步抛**,被测侧一路宽松、
> 给 content_empty。补在 `AnalyzeByJSonPath::parse_string`;**边界拿 jar 实跑出来**
> (空串抛、全空白**不**抛、字面量 `null` 是另一条消息),探针留在 json 套的
> `DSL_EDGE_DOCS`。**这两道只加在「真的是 Java String」那一档** —— 裁判
> `AnalyzeByJSonPath(json: Any)` 只有 String 走 `parse(String)`,别的对象走 Object
> 重载没有这两道;第一版搬到 `parse_permissive` 上,当场把 16 例本来两侧一起空手
> 而归的 case 变成我们单边抛。
> **2026-08-31(命中面第三轮):口味再认一条 —— JS 直接把页面 `JSON.parse` 掉**。
> 十六套仍是 99565 例 0 FAIL;`Unexpected token: <` 那一档 **52 → 4**,而命中只 **+1**
> (2175 → 2176)。规则:这一步的 JS 里有 `JSON.parse(result)` / `eval(src)`
> (**参数必须整个就是 `result`/`src`** —— `JSON.parse(result.match(…)[1])` 是
> 「页面里夹着一段 JSON」,那种页面本身仍是 HTML,认错了是往下砸命中面),
> 这一步就发 JSON 页 —— 那一档规则**整条都是 JS**,两个生成器谁都反推不出形状,
> `_json_only` 判 False 于是发 HTML 页,书源头一件事就 `JSON.parse(result)`、
> 当场 SyntaxError,**整步死在这里**。另有一条**带条件**的:JS 吃的是
> `java.ajax(...)` 取回来的文本时,只有 HTML 那张也造不出来才翻(页面只有一张)。
> **这一轮的价值不在那 +1**:它把那 52 例的真因从「页面类型给错了」翻出来,
> 露出后面真正挡路的是**加密族** —— 在此之前那 52 例的账全记在「页面类型」上,
> 照着它继续投工是白干。
> **顺带量出了这一套的天花板**:419 例未命中里,加密族(base64→AES/3DES 37 +
> JSON 形状后面接解密 29 + AES 15 = **81**)要拿书源自己的密钥把 payload 加密回去,
> 加上书源自己就在 Rhino 上语法错误的 21 例 —— **102 例原理上抬不动**,
> 即 B 层命中面的上限约 **96%**,不是 100%。真能修的 299 例:形态缺口 203、
> 正则诱饵扩面 92。分堆一律用 `tools/miss_report.py`。
> **2026-08-31(命中面第四轮):补生成器的两处形态,三套一起抬**。
> 十六套仍是 99565 例 0 FAIL;A 层 3162 → **3185**(97%→**98%**,未命中 84 → 61)、
> B 层 2176 → **2203**(**84%**,419 → 392)、top100 399 → **405**(91%→**92%**,38 → 32)。
> ① **`@put:{…}` 要剥掉,不是整条不认**:它是**副作用后缀**,真身
> `SourceRule.splitPutRule` 拿 `@put:(\{[^}]+?\})` 整段 replace 掉,剩下的才是选择器。
> 此前见到它就整条不认 —— 于是 `name: "Name@put:{id:$.Id}"` 的源**页面上没有
> `Name` 这个属性**,容器选中了、name 取到空串,而 legado 的 `getSearchItem`
> 把**没有 name 的项整条丢掉**,`books` 就是空的(203 例「没报错、就是空」里 40 例有它)。
> 与 `@get:{…}` 不同,那一个是取值占位、替换出来的选择器随运行时状态变,仍然不认。
> ② **XPath 翻成选择器再合成**(`fallback_page._xpath_to_css`):生成器从第一天起
> 就整条跳过 XPath(`sel[0] == '/'` 直接 return None),全库 **213 处**规则的源一律
> 退回底板。裁判跑的是 JsoupXpath(**在 jsoup 树上**跑的方言),所以只要造出
> 「等价 CSS 选得中」的树它就选得中。只认语料里真出现的那一小撮形态,
> **谓词里有一样认不出就整条 None** —— 丢掉谓词凑合造等于白造一张页,还把底板挤掉。
> ③ **顺带钉死一档「不能修」**:前导 `-` 的规则(12 例)拿 jsoup 的 jar 实探,
> **裁判自己也抛**(`select("-class.box")` 不是合法选择器)—— 坏书源,不是生成器缺口。
> **2026-08-31(命中面第五轮):口味与选择器的三处,info 从 86% 到 92%**。
> A 层 3185 → **3198**(**99%**,未命中 61 → 48)、B 层 2203 → **2239**(**86%**,392 → 357)。
> ① **`[name=og:novel:book_name]` 被截成 `[name=og]`**:`_take_pseudos` 不看方括号,
> 把属性值里的 `:novel` / `:book_name` 当成两个伪类摘走了(语料 `og:` 那族 meta 50+ 处)。
> ② **`[*]` / `[?(` 是 JSONPath 独有的写法**(CSS 里 `[*]` 不合法):语料 **82 个
> 「源×步」**的入口规则长这样却不带 `$` 前缀,此前一律当 CSS 发 HTML 页。
> ③ **`info` 步 + 单段裸键 `name` ⇒ HTML 页永远取不到值** —— 真身
> `setContent(getElement(init))` 交回来的是 **`Elements`(列表)不是 `Element`**,
> `AnalyzeByJSoup` 对非 Element 一律 `Jsoup.parse(toString())`、**又变回 Document**,
> 而单段字段规则直接走 `element.attr(...)`,Document 的属性在 HTML 里写不出来。
> 四条探针实跑钉住(`init` 是 `<js>` 的除外:那时上下文是 JS 对象,HTML 页照样成立)。
> info 命中 **422 → 452**。
> **这一轮照出一处旧缺口**(pb01652,棘轮 0 → 1):`init` 从 JSON 页选出对象后
> `@js:` 里 `result.anchor` —— 真身 Rhino 把 Java Map 包成 **NativeJavaMap**、键当属性读,
> 被测侧 `bind_value` 对 jayway 值仍按 `to_java_string()` 过,于是 `result` 是字符串、
> `.anchor` 撞上 `String.prototype.anchor`。**没有当场改**:这是 M2f 就点过名的
> 「Rhino NativeObject 那半」,`typeof` / `String(result)` / `for..in` 全是面,
> 得**先在 js 套用真 Rhino 打一组 NativeJavaMap 的形态探针**再动 `BoundValue`。
>
> **M2q(2026-08-31):`BoundValue` 那条欠账还清,棘轮拧回 0**(十六套 99633 例 0 FAIL)。
> 顺序照上一条写的走:**先探再改**。js 套新开 `resultRule` 字段(两侧都走真身
> `getElement` → `evalJS` 这条产品路径),`js-bind-*` 六十八条探针把真 Rhino 的形态
> 逐条钉住。**其中两条推不出来、只能探**:① **装箱的 `Boolean` 在 `if` 里恒为真**
> —— 连 `false` 也是(Rhino 的 `toBoolean` 对 Scriptable 直接 return true);
> ② **顶层绑定的标量不装箱**(`Context.javaToJS` 原样交回),只有从容器里取出来的
> 才装 —— 两处口径相反。落地:`rubato_core::host::JavaValue`(逐层的 Java 形态,
> 每个节点自带 `toString()`,因为**容器的 toString 随出身** —— json-smart 的
> `JSONArray` 是 JSON 文本、gson 的 `ArrayList` 是 `[a, b]`,同一棵 JSON 树判不出来)
> + `BoundValue::Java` / `BoundValue::Native` 两支 + `RuleHost::el_java`
> (`java.getElement` 那条路共用同一份建树)。顺带照出并修掉两处旧缺口:
> `<js>` 交回来的**单个对象**此前落 `RuleValue::Json`(该是 `Native`:
> `String(result)` 是 `[object Object]`)、`List<JXNode>` 当绑定时整条被拍成串。
>
> **2026-08-31(命中面第六轮):按 JS 的三条取值路放诱饵**。十六套仍是
> 99633 例 **0 FAIL**;B 层 2239 → **2289**(86% → **88%**,未命中 357 → 307)、
> top100 405 → **412**(92% → **94%**,32 → 25);A 层不动(那一层按定义没有
> `<js>`/`@js:`)。**起点是上一轮记的归因被推翻**:「JS 跑不通」那 93 例记着
> 「`rx_sample` 再多认几种取值来源」,真去看才发现 **71 例的正则本来就造出了样例**
> —— 诱饵缀在页尾,而 JS 读的**根本不是页面**。三条路各修各的:
> ① **字段规则自己取出来的值**(`fallback_page._fit_value`,34 例):
> `bookUrl: "tag.a.0@href@js:result.match(/_(\d+)/)[1]"` 里的 `result` 是前一截
> 选择器取出来的那个值 —— 诱饵要放进合成页上那个 `href` 的值里,**先试「原值 +
> 样例」**(原值别的规则也在读,直接换掉会把命中的字段弄空),每一步拿 `re` 验一遍,
> **属性位上不放需要转义的字符**(`_emit` 拼属性时不转义)。
> ② **JS 自己写的那条规则**(`fallback_page._js_reads`):
> `var data=java.getString("@@.last_page@href")` 里的 `.last_page` 不在任何**规则
> 字段**里、只在 JS 的字符串里,生成器从来没造过它。两条口径照真身分开
> (`getElement(s)` 收列表规则、`getString(List)` 收字段规则),**只在整条是
> `@js:`/`<js>` 的规则上认**(那时 content 才是整张页)。顺带解锁「**整条是
> `<js>` 的容器反推**」那一档:`chapterList = java.getElements(".section_list li a")`
> 读到的那批元素**就是**这一步的容器,字段规则从此相对它们造。
> ③ **请求地址本身**(`gen_pipeline_corpus_cases.base_url_fit`,19 例):
> `n=baseUrl.match(/book_id=(\d+)/)[1]` —— `baseUrl` 是 info 的 `book.bookUrl`、
> toc 的 `book.tocUrl`、content 的章节 url(真身 `setBaseUrl` 那三处),**补在查询
> 串上**而不是拼在路径后面。剩下的 41 例:JSON 页 13(诱饵与形态至今只造在 HTML
> 页上)、`java.ajax` 取回的另一张页 8、DOM 导航 3、其它 17。
>
>
> **2026-08-31(命中面第七轮):`<js>…</js>` 是有界的**。十六套仍 99633 例 **0 FAIL**;
> B 层 2289 → **2309**(未命中 307 → 287);A 层与 top100 不动。
> **照着裁判的源码定的**:`AnalyzeRule.splitSourceRule` 拿
> `JS_PATTERN = <js>([\w\W]*?)</js>|@js:([\w\W]*)` 把一条规则切成**一串**子规则,
> 段与段之间的文本各自是一条规则。`@js:` 吃到结尾(这一半生成器本来就对),
> 而 **`<js>…</js>` 有界** —— `</js>` 之后那截是下一条规则,语料里 **44 个「源×步」**
> 的入口规则长这样(`<js>eval(source.bookSourceComment)</js>` 这种前奏块打头),
> 此前一律整条丢掉:既没造结构,口味也判错(`$..list[*]` 打头的 12 例一直发 HTML 页)。
> 口径只记一处(`fallback_json.rule_segment`)。这一改**同时动了「认不认」与「口味
> 怎么判」**(58 个步从 html 翻到 json),正是那个跷跷板,两件一起复验、净 +16。
> 顺带补了 **CSS 选择器组**(`#a .x, #a .y`):**只在段内切**,跨 `@` 切会把
> `A@B,C` 切成 `A@B` 与 `C`,后者落到根上就是造错地方。
>
>
> **2026-08-31(命中面第八轮):JSON 页也按 JS 反推形状**。十六套仍 99633 例 **0 FAIL**;
> B 层 2309 → **2314(89%)**、top100 412 → **413**。HTML 那张页有三条「按 JS 反推」的路
> (第六轮),JSON 这张一条都没有 —— `JSON.parse(result).data.page.map(…)` 这一档
> 合成页上没有 `data.page`。`fallback_json._js_shapes`:链本身是路径、链后面那一下说
> 它是数组还是字符串、**数组的项长什么样由回调的形参说**(`.map(i => … i.image …)`),
> 别名也认一层。**候选 39 例只换来 5 例命中** —— 与第三轮量出的天花板一致(后面接着解密)。
> **试过但没做**:JSON 页上的正则诱饵(只能加一个根键,实测 +1,代价是每张有 JS 的
> JSON 页都多一个根键)—— 理由写在 `fallback_json.build` 上面,免得下一轮又试一遍。
>
> 本文是项目的权威计划;改方向先改这里。

## 0. 背景与决策

前身是 legado-with-MD3(Kotlin/Compose,25 万行)的精简重写。约束:不再写 Kotlin、不依赖 JVM、不手写 Gradle;目标平台 **Android + 桌面(macOS/Windows)**;在线书源是核心功能。

**为什么是 Rust + Flutter**:Flutter + Kotlin 引擎 AAR 的黑盒方案在桌面端站不住(桌面 Flutter 挂不了 AAR,只能拖 JVM sidecar,等于没离开 JVM)。Rust 引擎经 flutter_rust_bridge 双端复用,是这组约束下唯一自洽的路线。代价:引擎重写,书源兼容性从 100% 变为**分阶段逼近**。

**裁判(judge)**:vendor 进仓库的 Kotlin 引擎(`judge/engine` + `judge/rhino`)不作为产品代码,而是**可执行的行为规范**——同一书源两边跑、diff 输出。冻结不开发;它是整个重写的对答案机制。

**裁判基准切换(2026-08-29 决策)**:最初的裁判 vendor 自 legado-with-MD3。实际使用发现 MD3 版对书源生态的兼容本身就有缺口(不少书源它自己都跑不了),而 [LegadoTeam/legado](https://github.com/LegadoTeam/legado) 才是 legado 的正统续作、书源生态的事实标准。据此把裁判基准整体切换为 **LegadoTeam/legado @ `3046111c`(2026-08-28)** 的快照:兼容目标从「复刻 MD3 fork」改为「复刻正统续作」。切换是一次性锁定,不做持续同步(见 §6);日后若再升基准,重演本次的切换流程——差分体系保证每一处语义漂移都会以 FAIL 的形式显式暴露。

## 1. 技术选型(已论证 + Phase 0 实证)

| 项 | 选择 | 理由 / 实证 |
|---|---|---|
| JS 引擎 | rquickjs(quickjs-ng,**必须开 `bindgen` 特性**才能交叉编译 Android) | ES2020 覆盖足够;~1MB;NDK/桌面交叉编译无痛。实证:16 方言片段桌面+真机全过,中文标识符原生支持,Annex B(escape)俱全。boa 太慢,V8 太重 |
| HTML | html5ever(经 scraper)+ 自研 jsoup 1.16.2 兼容层 | 同为 WHATWG 解析,树主干一致(实证 12/13);**text()/html() 序列化与伪类方言必须自写**——这是 HTML 侧的真实工作量。libxml2 的 HTML4 解析器否决 |
| XPath | ~~序列化成 XML 交 libxml2~~ → **照着 JsoupXpath 2.5.5 移植**(`xpath-compat`)| **Phase 2 M2g 推翻了原选型**:裁判的 `AnalyzeByXPath` 底下不是标准 XPath 引擎,是 JsoupXpath —— 跑在 jsoup 树上的方言(`allText()`/`num()` 等节点测试、`^=`/`~=` 等运算符、`[n]` 按上下文集合内同标签序号计数还支持 `[-1]`、文本节点包成合成元素、`main` 无 EOF)。换任何标准引擎都对不上逐字节判据。解析树仍只有 html5ever 一份 |
| JSONPath | jsonpath-rust 包在自有接口后,对齐 jayway 3.0.0 行为,准备好 fork | jayway 的 definite/indefinite 返回形态、异常吞并等按差分打补丁 |
| 规则层正则 | fancy-regex + java.util.regex 兼容垫片 | `##` 正则大量用 lookaround/反向引用,regex crate 不支持;编译失败降级为字面量替换的行为要复刻 |
| HTTP | reqwest + rustls;自建 CookieStore(原样字符串合并语义,非 RFC jar) | per-source 代理/dnsIp/不安全 TLS;redirectUrl 记录语义要复刻(相对 URL 解析依赖它) |
| 存储 | rusqlite(bundled),数据全在 Rust 侧 | JS 宿主的 cache/cookie/variable 是同步热路径,不能跨 FFI 找 Dart;表名对齐裁判 schema 便于日后导入 legado 备份 |
| 桥 | flutter_rust_bridge v2.13 + cargokit | panic 不穿 FFI;StreamSink 推进度;**FRB 无自动取消**——长任务显式 task_id + cancel();Rust→Dart 闭包做 PlatformHooks |
| webView/验证码 | Rust 不持有 webview;PlatformHooks 回调 Dart(Android 用 flutter_inappwebview headless;桌面降档走可见窗口) | Phase 3 落地;桌面 webView 不做任何早期 Phase 的验收项。**M3b 起接口面已钉死**:平台只剩「加载 / 求值 / 等 / 还」四件原语(`rubato-core::host::WebViewHost`),策略全在 `net::webview` 且**进了差分**。**M3c 起接口已接进引擎**:`AnalyzeUrl`(`{"webView": true}`)、`AnalyzeRule`(`@webjs:`)、`JsExtensions`(`java.webView*`)三处都走它,host 由调用方给;**M3d 起产品侧那四件原语也接上了**:`ffi::platform` 的桥 + `app/lib/platform/web_view.dart` 的 headless 实例(flutter_inappwebview,Android/iOS/macOS/Windows;别的平台不登记,于是诚实地报 `webview_unsupported`)。桌面**不必降档走可见窗口** —— macOS 上 headless 实测跑通(集成测试 `app/integration_test/webview_test.dart`) |

## 2. 目录与 crate 边界

```
rubato/
├── rust/crates/
│   ├── rubato-core    实体、错误、HostEnv trait(打破 rule↔js↔net 环依赖的接口层)、java.net.URL 语义
│   ├── rule-syntax    RuleAnalyzer(377 行零依赖切分器)逐行移植 + 属性测试
│   ├── html-compat    html5ever + jsoup 伪类方言 + jsoup 风格 text()/html() 序列化 + AnalyzeByJSoup DSL
│   ├── json-compat    jayway 行为对齐的 JSONPath
│   ├── xpath-compat   JsoupXpath 2.5.5 移植(词法/文法/求值器 + AnalyzeByXPath 壳),跑在 html-compat 的树上
│   ├── regex-compat   fancy-regex 的 java 方言垫片
│   ├── js-host        rquickjs:java.* 宿主 API(~130 方法,重载按 arity 分发)、Rhino 方言 prelude、crypto、QueryTTF
│   ├── net            AnalyzeUrl 移植:option JSON/charset/限流/CookieStore/EncodingDetect
│   │                  + webview:BackstageWebView 的**策略层**(webView 可差分的那一半)
│   ├── rule-engine    AnalyzeRule(973 行调度器)移植
│   ├── pipeline       WebBook 四步:搜索/详情/目录/正文(explore 与 search 同路径顺手做)
│   ├── store          rusqlite:book_sources/books/chapters/caches/cookies
│   ├── engine         组装(store+net+pipeline+js-host)、并发搜索、任务取消
│   ├── ffi            引擎单例 + 任务注册表 + **PlatformHooks 的桥**(`platform.rs`:
│   │                  webView 四件原语过 FFI 到 Dart,M3d)。**无 C ABI**;
│   │                  DTO 在 app/rust,因为 FRB 只扫它那棵树
│   └── difftest       差分 harness + HTTP 录放
├── app/               Flutter
│   ├── lib/pages/     书架 / 搜索 / 阅读 三页 + 书源面板
│   ├── lib/engine.dart 起库、首次导入起步书源包、登记 webView 平台面
│   ├── lib/platform/  PlatformHooks 的 Dart 那一半(headless webview 的四件原语)
│   ├── assets/sources/ starter.json(实测能读的 40 源,来历见同目录 README)
│   ├── rust/          FRB 扫描的入口 crate(DTO + `#[frb]` 函数面)
│   └── rust_builder/  cargokit(**带两处本地补丁**,见 §4「Android 构建」)
├── judge/             Kotlin 裁判(冻结):engine = LegadoTeam/legado@3046111c 的 app/src/main/java
│                      快照;rhino = 其 modules/rhino(com.script,htmlunit-core-js 5.3.0-legado.4,
│                      构件 vendor 在 judge/third_party/maven);**三个 harness 都是纯 JVM
│                      模块、都是我们的**(不属于被冻结的引擎本体),按原路径挂载引擎文件,
│                      Android/Room/OkHttp/Rhino 依赖由各自的 shims/ 顶掉:
│                      `:harness`(com.script 是确定性桩)/ `:jsharness`(真 Rhino)/
│                      `:wvharness`(webView 策略那一套自己的执行器)——
│                      分模块是因为同一个 classpath 上 com.script 只能有一份。
│                      **BackstageWebView 三个模块都挂真身**(M3c 起):
│                      `:wvharness` 的时钟从**外面**推(它要把 `evals[].at` 记成
│                      观察面),另外两个是**内联泵**(那两边的入口都在
│                      `runBlocking` 里,挂起就没人喂事件循环)
├── fixtures/          sources(书源 JSON)/ corpus-js(去重 JS)/ pages(差分页面)/
│                      http(fetch 套录放快照)/ http-pipeline(pipeline 套录放快照)/
│                      cases(差分用例:每套一份 README 写死两侧契约)
└── tools/             gen_*_cases.py(用例生成)、*_diff.sh(各套差分入口)、case_diff.sh(通用比较)
```

依赖方向:`core ← {rule-syntax, html-compat, json-compat, regex-compat} ← rule-engine ← pipeline ← engine ← ffi`;`js-host`/`net` 依赖 core,经 `HostEnv` trait 被 rule-engine 反向调用。

UI:v2 重设计的**令牌值**(AppColorScheme/AppTypography/AppShapes/AppDimens/ReadingPalette)翻译成 Flutter ThemeData;组件照旧画板稿在 Flutter 重做(Compose kit 无法带走)。

## 3. 差分测试体系(成败所系)

- **裁判 harness**:`judge/harness` 纯 JVM 模块(Robolectric 最终没用上——把引擎文件按原路径
  挂进来 + 写垫片更轻),读 case 目录输出规范化 JSONL。
  ~~BackstageWebView/验证码不进自动差分(Phase 3 手工清单)~~ —— **这句话只对
  「真的去加载一个页面」那一半成立**(M3b 改判):`BackstageWebView.kt` 那 394 行
  里 WebView 之外全是策略(补 900ms、重试梯子、超时、重定向包装、嗅探、
  cookie 回抄),把 WebView 换成**两侧同一份剧本 + 虚拟时钟**之后策略全都可比
  —— 第三个 harness `:wvharness` 挂的就是真身那 394 行,判据见
  `fixtures/cases/webview/README.md`。**M3c 起那份剧本不止 webview 一套用**:
  `:harness` / `:jsharness` 也挂上了真身,于是 `{"webView": true}` 的书源、
  `@webjs:`、`java.webView*` 在 fetch / rule-engine / js-host / pipeline 那几套里
  走的都是真策略(case 带 `webview` 剧本;不带就是缺省剧本 —— 页面加载得完、
  每次求值都回 `"null"`,重试梯子跑满报「js执行超时」)。
  验证码/loginUi 仍是手工清单。
  JS 引擎两侧同换确定性桩,JS 方言差异留给 Phase 2 的 js-host 差分。
- **HTTP 录放**:两边都不直连网络。快照 key 的规范化规则(哪些 header 参与 hash、query 排序)**先写文档再写代码**,是两边共享的契约。
- **两级 diff**:
  1. 规则级(主力):`{source, ruleName, rule, content 快照, 变量初值}` → 双方 getString/getStringList → diff 输出与变量终态,失败直接定位到单个 crate;
  2. 流水线级:`{source, step, 输入}` → diff 搜索结果字段/详情/章节列表/正文文本/**发出的请求序列**/cookie 终态。
- **规范化器**:URL 按解码后等价比较(java.net.URL vs WHATWG 差异);`getAbsoluteURL` 按 java 语义移植。
- **口径**:diff 不一致默认裁判为准;裁判明显 bug 才进带注释的豁免清单。CI 按 A/B/C 层出 pass 率仪表。

**第三条判据:移植对照台账**(M3p,2026-09-01)。差分守的是「比过的面」,
台账守的是「**没进剧本的那一位**」—— 真身的每个面(方法 / URL 选项 / 书源字段)
机械枚举出来,逐条判 `移植 / 桩 / 不做 / 待做 / 未比 / 未判`,`--check` 拦
「真身有而台账没有」。见 `tools/port_audit.py` 与 `docs/port-ledger.md`。

## 4. 分期与验收判据

### Phase 0 — 技术验证 ✅(2026-08-29 达标,时间盒两周,实际一天)

| Spike | 达标线 | 实测 |
|---|---|---|
| S1 QuickJS 方言 | ≥70% 可收敛,无引擎级死结 | 16 方言片段 macOS 16/16、Android 真机 16/16;683 段真实书源 JS **92.4% 可收敛**(PASS 31.9% + 数据类 36.3% + 缺上下文/可 shim 20.4%+3.8%;SYNTAX 7.6% 抽查多为抽取伪影——`{{}}` 模板占位与 `##` 误抽) |
| S2 HTML 兼容 | 选择器命中一致,序列化差异是规则性的 | 对 jsoup 1.16.2 裁判 **12/13**;唯一 FAIL 是 `html()` pretty-print(树结构一致),Phase 1 自写序列化器收敛 |
| S3 FRB 全链路 | 双端跑通,构建零手工步骤 | macOS 集成测试绿 + Android 真机集成测试绿(sync/async/StreamSink);全程零手写 gradle |

语料分层(1704 源):**A 层 53% / B 层 40% / C 层 7%**(908 / 684 / 112)——
Phase 1 做完即点亮约一半生态。~~A 52% / B 36% / C 12%~~ 是 M3a 之前的数:
老 `classify` 把 `ContentRule` 默认带的两个**空键**(`"webJs": ""` /
`"sourceRegex": ""`)当成 C 层特征,**C 层被高估近一倍**(见 Phase 3 M3a)。
已知真方言点:书源里存在 `org.jsoup.Jsoup.parse` 式 LiveConnect 调用 → Phase 2 在 prelude shim `org.jsoup` 映射到 html-compat。

### Phase 1(6-8 周)— A 层源可用

rule-syntax / html-compat(DSL + jsoup 序列化器)/ json-compat / regex-compat / net(无 webView)/ rule-engine / pipeline / store / ffi + Flutter 最小三页(书架/搜索/阅读)+ 裁判 Robolectric harness + HTTP 录放。
**判据:A 层四步差分 pass ≥95%;规则级 ≥98%;Android 真机 + macOS 可搜书、加书架、翻页阅读、进度持久化。**

判据现状:
- **规则级 ≥98%** ✅ ——十套规则级差分 88613 例 0 FAIL(见下表);
- **A 层四步差分 ≥95%** ✅ ——`pipeline-corpus` 套 **3749 例 0 FAIL**(豁免 11
  = 4 个 authority 含非 ASCII 的源,与 fetch 套同一条 IDN 回避面)。
  口径要连着「命中面」一起读,见 `fixtures/cases/pipeline-corpus/README.md`:
  真实站点快照不可得,四步用 docs/http-snapshot.md §6 的**回落页**供页
  (页面由 `tools/fallback_page.py` 按语料自身的选择器分布反向合成),
  规则真实、页面合成。裁判侧的命中面:search 32% 出书 / explore 50% /
  info 30% 取到书名 / toc 43% 出章节 / content 78% 出正文,其余落在
  空结果与 `toc_empty`/`content_empty`/`pipeline_error` 分支(这些分支的
  一致性也在比,但不覆盖字段装配;`pipeline_error` 还是粗粒度归一)。
- **Android 真机 + macOS 端到端** ✅ ——`app/integration_test/app_test.dart`
  驱动真实 UI 走完「书架 → 搜索 → 加书架 → 阅读 → 翻页 → 进度落库回读」,
  **macOS 与 Android 真机(Samsung RFCY41BD54H)各跑通一次**;
  `engine_test.dart` 另有 6 条(建库、导源、开关源、任务号/取消、坏 JSON、
  联网四步)两端全绿。验收命令:
  `flutter test integration_test/app_test.dart -d macos|<设备号>`。

**进度(2026-08-29)**:裁判基准已切换到 LegadoTeam@3046111c,**十一套差分全绿**(十套复验 + 新建的 pipeline 套);
随后把 §4 里挂着的「已探明但未被差分覆盖」的漂移逐条补成用例(下文「漂移补覆盖」):
| 件 | 旧基准成绩 | 新基准复验 |
|---|---|---|
| rule-syntax(RuleAnalyzer 377 行) | 30326 例 0 FAIL | ✅ 30326 例 0 FAIL(零改动) |
| regex-compat(Java 正则转译 + replaceRegex 降级链) | 8071 例 0 FAIL(豁免 26) | ✅ 8071 例 0 FAIL(豁免 26,零改动) |
| json-compat(jayway 对齐求值器 + AnalyzeByJSonPath DSL) | 2307 例 0 FAIL | ✅ 2307 例 0 FAIL(零改动) |
| html-compat(jsoup 方言 + 序列化 + AnalyzeByJSoup DSL) | 23113 例 0 FAIL(豁免 36) | ✅ 23113 例 0 FAIL(豁免 36;jsoupxpath 降 2.5.3 无影响) |
| rubato-core(HostEnv、四层变量、java.net.URL 语义) | 2066 例 0 FAIL | ✅ 2066 例 0 FAIL(零改动) |
| rule-engine(AnalyzeRule 调度器) | 10649 例 0 FAIL | ✅ **10949** 例 0 FAIL(getElements 语义已移植;+300 例漂移补覆盖) |
| net(AnalyzeUrl 离线面) | 6556 例 0 FAIL | ✅ **6613** 例 0 FAIL(hutool query 编码器 + timeout/followRedirects/resolveIp 已移植;+36 例漂移补覆盖) |
| HTTP 录放 + 联网面 + store(CookieStore) | fetch 3045 例 0 FAIL | ✅ **3060** 例 0 FAIL(getHeaderMap + followRedirects 已移植;+9 例 cookie 域名键) |
| icu4j CharsetDetector | charset 293 例 0 FAIL | ✅ 293 例 0 FAIL(零改动) |
| BookSource 实体模型(gson 语义) | source 1727 例 0 FAIL | ✅ 1727 例 0 FAIL(**字段面已对齐**) |
| 差分基建(harness + case_runner + 豁免清单) | 十套 diff 入口 | ✅ harness 垫片修复完成(编译错误 386 → 0) |
| pipeline(WebBook 四步) | WIP 骨架(检查点 569dd15) | ✅ **88** 例 0 FAIL(新建套:手工合成书源 × 页面,铺满四步的调度与装配轴;+42 例 bookSourceType→BookType) |
| **pipeline-corpus(A 层真实书源 × 四步)** | ⬜ | ✅ **3749** 例 0 FAIL(豁免 11):新建套,A 层 846 源 × 四步,回落页供页 |
| store(books/chapters/caches/book_sources) | ⬜ | ✅ 落库面 + 8 条单测(列名对齐裁判 Room 实体) |
| engine(组装/并发搜索/取消) | ⬜ | ✅ 5 条单测 + 两个联网探针例程 |
| ffi + Flutter 三页 | ⬜ | ✅ 端到端两端验收通过(见上) |

新基准合计 **92362 例差分 0 FAIL**(豁免 73,全部带注释)。语料与用例在切换中**全部复用**,
每一处 MD3→LegadoTeam 的语义漂移都以 FAIL 的形式显式暴露后逐条移植 —— 差分体系在这次
基准切换里完成了它的设计目标验证。

**基准切换里程碑**:
1. ✅ vendor 换血:`judge/engine/src` 整树替换为 LegadoTeam@3046111c 快照(1012 个 .kt);
   `judge/rhino` 换为其 modules/rhino;htmlunit-core-js 5.3.0-legado.4 经仓库内置 maven
   (`judge/third_party/maven`)接入;版本锁对齐(jsoup 1.16.2、okhttp 5.4.0 不变;
   jsoupxpath 降 2.5.3;新增 hutool 5.8.22)。
2. ✅ harness 垫片修复(编译错误 386 → 0):真身挂载面扩大到 BookSource / rule 实体 /
   AnalyzeUrlNetworkOptions / MapExtensions / InfoMap / StringUtils / ContentHelp /
   help.book.BookContent;重灾区(BookHelp / Debug / BookExtensions 绑死文件缓存与 UI)
   改薄垫片,纯函数逐字复刻;com.script 桩按新 rhino 的 **VarScope** 体系重写
   (TopLevel 不再是 Scriptable);新增 media3 / glide / Cronet / jsSource / ExploreAdapter
   类型面垫片;旧的 `BookChapterModelRuntime` 间接层作废(BookChapter 真身自带
   getDisplayTitle / getAbsoluteURL)。
3. ✅ 十套差分逐套复验:FAIL 全部定位为语义漂移并移植(见下),复验矩阵清零。
4. ✅ 在新基准上建立 pipeline(WebBook 四步)差分(46 → 88 例 0 FAIL);
5. ✅ 建立 **pipeline-corpus**(A 层 846 真实书源 × 四步,回落页供页)——
   3749 例 0 FAIL,「A 层四步差分」判据闭合。这一套抓出三处**旧基准遗留的截断**:
   `BookInfo.kind` / `BookList.searchBook.kind` 截 1000 字符、`SearchBook` 构造
   init 块截 `kind`/`intro`/`latestChapterTitle` 到 1000/5000/200 —— LegadoTeam
   基准里这些截断都不存在,被测侧已删。手工套照不出来(合成页取不到超长串),
   真实规则打在合成页上(泛选择器一次选中几十个块)才炸出来。
   为它给回放层加了**回落页**机制(docs/http-snapshot.md §6),两侧同实现。
6. ✅ **端到端那条线**(store → engine → ffi → Flutter 三页),下文单列。

### Phase 1 端到端(2026-08-29 收尾)

差分钉的是「规则跑得对不对」,端到端钉的是「装配得起来、用得下去」。这一段**没有裁判可对**,
所以判据就是两端各跑通一次真实操作;它的正确性靠下面这条分工守住:
**四步语义永远由 pipeline / pipeline-corpus 两套差分负责,engine 只做装配、落库与并发。**

- **store**:`db.rs` 的 DDL 按裁判 Room 实体逐列抄(`books` / `chapters` / `caches` /
  `book_sources`,连 `books` 在 (name, author) 上的唯一索引都照建 —— 那条索引就是真身
  「同名同作者跨源互相顶掉」的换源语义)。规则组(`ruleSearch` 等)和 Room 的
  TypeConverter 一样存 JSON 串,读回时重新拼成一个 BookSource JSON 交
  `BookSource::from_value` —— 与导入书源**同一条解析路径**(source 套 1727 例钉的那条)。
  `Book` 实体只有流水线要的字段面(它被差分钉住,不动),阅读进度这些产品列放在
  `BookRow` 外层。
- **net::live**(feature `live`,默认关):reqwest blocking + rustls 的**单跳**执行器。
  重定向/重试/cookie 循环仍在 `client.rs`,与回放侧共用同一条链路,所以必须
  `redirect::Policy::none()`。未接的面:per-source 代理 / dnsIp-resolveIp / 不安全 TLS
  —— 它们要给 `execute_hop` 加参数,留 Phase 2。
- **js-host 的产品面(计划外,但必需)**:A 层的定义是「无 `<js>`/`@js:`/jsLib」,可
  `searchUrl` 里的 `{{key}}`/`{{page}}` 在真身里同样走 `evalJS`
  (AnalyzeUrl.replaceKeyPageJs → innerRule → evalJS)——**没有 JS 引擎就搜不了书**。
  于是提前落了 `js_host::host_env::QuickJsHost`:绑定 key/page/result/baseUrl/src/title/
  nextChapterUrl,`java.put/get/log` 打到 rule-engine 的变量层,md5/base64/hex 若干纯函数;
  `java.ajax`/`webView`/`cookie`/`cache`/`book`/`source` 等未接能力**显式抛异常**,
  不静默给空串。**它不参与差分**(差分侧永远是 difftest::StubHost),Rhino 方言差异
  仍是 Phase 2 建 js-host 差分套时才算数。
- **engine**:两把锁(stores / cookies)各自短持;搜索开 6 个 worker,每个 worker
  自带一份 transport(HttpTransport 是 `&mut self`),cookie 走
  `SharedCookies`(每次读写短暂持锁)。结果经 mpsc 边收边回调,UI 因此「搜到一条显示一条」。
  **取消不是错误**:被取消就提前收工并照常返回跑过的源数(用户主动停搜不该在 UI 上冒红);
  粒度是**步间/源间**,单次 HTTP 调用不可中断。
- **ffi**:只有引擎单例 + `task_id ↔ CancelToken` 注册表,**一行 C ABI 都没有** ——
  桥全是 flutter_rust_bridge 生成的。DTO 与 `#[frb]` 函数面被迫留在 `app/rust/src/api/`
  (FRB 只扫 `crate::api` 那棵树),所以 `app/rust` 那层只做形状转换。
- **Flutter 三页**:书架 / 搜索 / 阅读(书源管理是书架页上的辅助面板,不算一页)。
  阅读页是**真分页**:用 `TextPainter` 按当前视口切页,落库的 `durChapterPos` 就是
  「本页首字在本章里的字符下标」——与裁判 `Book.durChapterPos` 同语义,换字号/转屏
  按这个锚点重排。左/中/右三个热区是**真实 widget**(带 Key),不是「按 localPosition
  算三分之一」:后者要在 MediaQuery 宽度与手势子树局部坐标之间做假设,桌面窗口有偏移
  时会错位,集成测试也没法按 finder 去点。
- **起步书源包**(`app/assets/sources/starter.json`,首次启动自动导入):
  A 层 733 源 →(`probe` 例程联网跑搜索)123 源真出书 →(`probe_deep` 例程把四步走完)
  **68 源读得下去** → 取目录 ≥20 章的前 40 条。**这不是差分资产**,真实站点会挂会改版,
  名单只对生成当次成立;复现步骤写在 `app/assets/sources/README.md`。
- **Android 构建**:cargokit 只给 cc/ar/linker 配了 NDK,**没管 bindgen**
  (bindgen 直接调 libclang,不读 `CFLAGS_<target>`),于是 rquickjs-sys 会在
  `#include <stdio.h>` 上炸。已在 `app/rust_builder/cargokit/build_tool/lib/src/
  android_environment.dart` 打本地补丁补出 `BINDGEN_EXTRA_CLANG_ARGS_<target>`
  (target + NDK sysroot),**升级 cargokit 时要重新打**。另外补了 `INTERNET` 权限与
  `usesCleartextTraffic`(大量书源仍是 http)、macOS 两个 entitlements 的
  `com.apple.security.network.client`。
- **flutter_inappwebview 钉在 `^6.2.0-beta.3`**(M3d):6.1.5 的
  `flutter_inappwebview_android` 在 `build.gradle` 里用
  `getDefaultProguardFile('proguard-android.txt')`,新 AGP **直接拒**
  (「no longer supported since it includes `-dontoptimize`」),
  APK 一行代码都编不出来;beta 那一支已经换成 `proguard-android-optimize.txt`。
  正式版发出来之后回退这条 beta 约束。
- **macOS 构建**(M3d 时踩到,与本仓库代码无关):Xcode 会把
  `MACOSX_DEPLOYMENT_TARGET` 传进 cargo,而 rustc 1.97 + macOS 26 SDK 一旦看见它,
  **宿主 proc-macro 的 dylib** 就被链成 dyld 拒收的形态(dlopen:
  `mis-aligned LINKEDIT string pool`),整个构建停在
  `error[E0463]: can't find crate for phf_macros` 这一类错上。任何值
  (12.0 / 14.0 / 26.0)都复现,空串被 rustc 拒掉,**只有不设才行** ——
  于是在 `builder.dart` 打了**本地补丁 ②**:macOS 上借 `env -u` 起 cargo。
  **代价记在这里**:cc-rs 编的那几个 C 文件(`dart_api_dl.c` / quickjs wrapper)
  因此按宿主 OS 版本打标,链接时有一条
  `object file was built for newer 'macOS' version` 警告 —— 桌面发行版要在
  Phase 4 收这一条(精确解是 `RUSTC_WRAPPER` 只对 `--crate-type proc-macro`
  摘变量)。**升级 cargokit 时两处都要重新打**。

**语义漂移清单**(切换时源码对比探明,复验时逐条踩实;✅ = 已移植且被差分钉住):
- ✅ **AnalyzeUrl 的 query 编码器**换成 hutool `RFC3986.UNRESERVED.orNew(PercentCodec.of(…))`。
  安全字符集不变,但代理对语义变了:hutool 逐 UTF-16 码元喂 `OutputStreamWriter` 再 flush,
  `StreamEncoder` 把高代理项留在 leftoverChar 里,等低代理项到齐才整体编码 —— 净效果是
  **按码点编码**(`😀` UTF-8 → `%F0%9F%98%80`、gb18030 → `%94%39%FC%36`、GBK/big5/latin1 →
  单个 `%3F`;旧基准逐码元编码给两个 `%3F`)。10 例 FAIL 由此而来。
- ✅ **urlOption 新增 `timeout`/`followRedirects`**,`dnsIp` 多了 `resolveIp` 别名
  (gson 同一 BoundField,两键都在时后出现的赢)。旧投影没这三项 → 是**覆盖盲区**而非 FAIL,
  已补进 analyze-url 投影与用例;`followRedirects=false` 的「不跟进重定向」也补进 fetch 回放。
- ✅ **AnalyzeRule.getElements**:不再 `result as List<Any>` 强转,改按 List/Array/NativeArray
  三分支展开 + 过滤 null 与 `Scriptable.NOT_FOUND`,其余类型给空表(旧基准抛
  ClassCastException)。94 例 FAIL 由此而来。
- ✅ **BaseSource.getHeaderMap**:`source.header` 不再被差分忽略 —— JSON 解析 + 默认 UA 注入
  + 登录头合并进入契约(旧契约「header 恒空」作废)。7 例 FAIL 由此而来;Rust 侧对应
  `net::source_header`。
- ✅ **BookSource 实体字段面**:`BookInfoRule` 去掉 `relatedBooks`(1580 例 FAIL 由此而来);
  新增 `mainJs`(JS 单文件书源)—— 功能砍单但 `isJsSource()` 决定四步入口,已纳入两侧解析。
- ✅ **漂移补覆盖**(原「尚未被差分覆盖」的六条,逐条落成用例;+387 例):
  - ✅ **cookie domain 键规则**:`sourceKey?.takeIf { getBaseUrl(it) == null } ?: getSubDomain(url)`
    —— sourceKey 非链接时原样当键;**是**链接时取的是**本次请求 url** 的子域名而非
    sourceKey 的。Rust 侧原先是 `getSubDomain(sourceKey ?: url)`,跨域请求会带错 cookie,
    已移植;fetch 套 +9 例(ft0035–0043),退回旧口径 4 例 FAIL。
  - ✅ **`getBookType()` 的 video 分支**:`bookSourceType == 4 → BookType.video(0b100)`,
    且 `allBookType` 含 video 位(决定 `resetType` 清不清它)。Rust 侧两处都漏了,已补;
    pipeline 套 +42 例,退回旧口径 16 例 FAIL。
  - ✅ **`<js></js>` 空捕获组 group-null / group-empty**:裁判 `group(2) ?: group(1)`
    只判 null 不判空。因两个组分属互斥的择一分支,**实测两侧等价**(Rust 用
    「g2 非空则取 g2」逼近);rule-engine 套铺了 `<js></js>` / `@js:` / 大小写 /
    前后缀混排 / `@webjs:` 的 `{5,}` 不足等 20 串 × 5 模式钉死。
  - ✅ **内容为 Map(gson `LinkedTreeMap`)时的 getString/getStringList**:新增
    `contentType: "map"` 用例面 + Rust 侧 `RuleValue::GsonMap` 短路分支。顺带踩实了
    `getElement/getElements` 落到通用循环后 `JsonPath.parse(Object)` 的返回类型语义
    (definite 路径给原样 Java 容器、indefinite 给 json-smart `JSONArray`)与
    `getObject(): Any` 读到 JSON null 时抛 NPE —— 后者对普通 JSON 内容同样成立,是本轮
    顺带修的一处真 bug。**Rhino `NativeObject` 那半仍留给 Phase 2**(要真 Rhino 才造得出)。
  - ✅ **webJs 空 body 变 `"null"`**:`getWebJsResult` 是 `.body.toString()`(旧基准
    `body ?: ""`)。BackstageWebView 桩加 `#nullbody` 指令(两侧同契约)才照得出这条,
    已钉住。(**M3c 起桩没了**:同一处改由剧本钉 —— 让页面上那段 js 回一个
    **字符串** `"null"`,观察面一模一样,而路上跑的是真策略。)
  - ✅ **INITIAL_GSON 的 MapDeserializerDoubleAsIntFix**:差分实测结论是**它在本工程
    触达的每一个 Kotlin 调用点上都不生效**(注册的 TypeToken 与调用点的 reified 类型
    对不上),数字一律是 `ToNumberPolicy.LONG_OR_DOUBLE`。所以被测侧**不实现**这条整数化;
    analyze-url 套 +36 例(`3.0 / -2.5 / -0.0 / 1e3 / 9007199254740993 / 1.0e20` ×
    headers 对象/字符串 × body)把它钉死,免得日后又照着源码"补"错。
    同一批用例顺带抓到 `GSON.toJson` 的 `serializeNulls = false`(对象里 null 值的键
    整条丢掉,数组里的 null 照写)—— Rust 侧 `to_json_pretty` 已对齐。
- ⬜ **仍未覆盖**(需要真 JS 引擎,属 Phase 2):header 里的内联 `<js>`(语料 1704 源中
  14 条);content 是 Rhino `NativeObject` 的那条分支(`Mode.Js` / `Mode.Json` /
  `getParamSize() > 1` / 键值直接访问四支)。
- 🔨 ContentProcessor/BookHelp/HtmlFormatter/WebBook 四步:与 MD3 版差异大(每处几十至数百行),
  语义面在 pipeline 差分里逐步踩实。**已在 pipeline 套里钉住并移植的**:
  - `HtmlFormatter.formatIntro` 的段首缩进串是 **`""`**(正文的 `format` 才是两个全角
    空格)——简介不再缩进;裁判也**不截断**简介长度(旧实现有 5000 字截断,已删);
  - `formatImagePattern` 从三选一变**四选一**:新增 `data-original` / `data-srcset`
    与「`src="…"` 只认双引号」两条分支,第四条兜底允许空值;
  - `BookChapter.getAbsoluteURL()` 改用 `AnalyzeUrl.paramPattern` 切 option
    (旧实现是朴素 `indexOf(',')`),并对「二级目录空卷链接」直接返回 baseUrl;
  - `BookChapterList` 的 `tocCountWords` 分支:非卷章节从 updateTime 信息里按
    `AppPattern.wordCountRegex` 抽出字数写进 `BookChapter.wordCount`,并把匹配串从
    `tag` 里 `replaceFirst` 掉(Rust 侧原本整块未实现,已补;chapter 投影新增 wordCount)。

  另外抓出一处**被测侧自身的时序错误**(不是基准漂移,是新用例才照出来):
  `BookList.analyzeBookList` 里 `ruleData.getVariable()` 是在 `getElements` **之后**、
  逐项 `setRuleData` 之前取的快照,所以 bookList 规则里的 `@put` 对每一项都可见;
  Rust 原先取的是 AnalyzeUrl 的旧副本,put 写入丢失。

联网面的差分姿势:裁判把 CookieStore/CookieManager/StrResponse/EncodingDetect
按原路径挂真身(okhttp 的 Cookie.parseAll/HttpUrl/PublicSuffixDatabase 免费拿到),
只写「快照回放循环」(harness/ReplayHttp);被测侧对称移植 okhttp 5.4.0 的
HttpUrl 规范化与 Cookie.parse(net::http_url / net::cookie)。**snapshot_miss
串(key + 规范化 URL)两侧一致即 PASS**——语料用例(3000 条真实书源 url)
不配快照,专门差分「构造 + 规范化 + key」整条链。已知回避面记录在
fixtures/cases/fetch/README.md(IDN/IPv6 host、icu4j 兜底、>4096 随机裁剪等)。

rule-engine 的差分能成立,靠的是 harness **原样挂载 AnalyzeRule.kt**(一行未改),
用 `judge/harness/.../shims/` 顶掉 Android/Room/OkHttp/Rhino 依赖;JS 与 WebView 两侧
都换成同契约的确定性桩(指令表见 `fixtures/cases/rule-engine/README.md`),把
JS 方言差异留给 Phase 2 的 js-host 差分。XPath 规则曾由两侧**同步预探测**后一起跳过
(不把"未实现"混进 pass 率),**M2g 起两侧都真的跑**。

gson 的等价面(严/宽两档解析、pretty toJson、StringJsonDeserializer)沉到
`rubato-core::gson`,`@put` 与链接 option 共用;新基准补 MapDeserializerDoubleAsIntFix。

### Phase 2(6-8 周)— B 层源 + XPath

js-host 补全 JsExtensions/JsEncodeUtils(除 UI 类)、Rhino prelude(LiveConnect shim、jsLib/SharedJsScope)、`@js:` url、登录 header、XPath、书源调试页(复用 difftest 执行器)。
**判据:B 层 ≥80% ✅(M2i 达标:`pipeline-corpus-b` 2783 例 2698 PASS =「96.9%」;
M2j 2711 =「97.4%」;M2k 2761 =「99.2%」;M2n 2781 =「99.9%」;
**M2o 2782 = 100.0%**,唯一那条 `\p` 从欠账改判为豁免 —— 被测侧是超集);
**自选 top-100 常用源 ≥90% ✅**(M2l 新建 `top100` 套,**按源** 100/100、
按 case 446/446、0 豁免 —— 口径与命中面见
`fixtures/cases/top100/README.md`,那份 README 要连着读);
差分 CI 常绿 ✅(M2h 已就位:`.github/workflows/ci.yml` 调 `tools/all_diff.sh`,
基线在 `fixtures/diff-baseline.json`)。**

**Phase 2 三条判据至此全部闭合。**

**M2o 收尾**(2026-08-30):判据的**含金量**再补一轮,收在这三条上 ——
① 命中面(B 层 79% / top100 90%,口径见下),② **十六套里只剩 js-host 一套有 FAIL**,
③ 那 8 条全是「面还没接」的欠账,没有一条是「口径没对齐」。
**命中面的分母口径**从 M2o 起扣掉「源没配这一步的入口规则」那一档
(`tools/hit_report.py` 的 `ENTRY_RULE`):B 层 725 个未命中里 188 个是它,
留在分母里会把「这个源没有探索页」读成缺口。真正的缺口当时集中在
**toc 71% / content 75%**,下一轮就是从这两步入手的 —— 见上面 2026-08-31
那一轮(toc 71%→78%、content 75%→84%,B 层合计 79%→**83%**)。

**再下一轮从哪儿下手**(B 层还剩 357 例「配了却没命中」;分堆用
`tools/miss_report.py pipeline-corpus-b`,别手抄下面这张表)。
**先看「不能修」有多少**:加密族 81 + 书源自己语法错 21 + 前导 `-` 12 = 114 例
原理上抬不动,这一套的天花板约 95%。
**下一档是「没报错、就是空」里入口规则生成器**认得**的那 75 例** ——
页造了、容器也该选得中却仍是空,多半是**字段规则**没造出来、项被 legado 丢掉
(`@put:` 那一批就是这么找到的),得逐条探。
先按规则长相分过一轮,**那次归因是错的**(又一次踩「上一轮的归因不能信」):
数出「33 例页已按源造了却仍选不中」,以为是生成器造错了 —— 拿
`JSHARNESS_DEBUG=1` 把真因打到 stderr 才看清,它们**根本没走到选择器那一步**,
是后面的 `@js:` 当场抛了。按裁判打出来的**真因**分是这样:

  160  没报错、就是空 —— 再细分:入口规则**生成器认得**的 **75**(← 下一档,得逐条探)、
       整条就是 `<js>` 的 53、前导 `-` 的 12(**不能修**:裁判自己也抛)、其余 20
   92  书源的 JS 在合成页上跑不通(match 返回 null)   ← **能修**:rx_sample 再多认几种取值来源
   37  源要 base64 解码,页面是明文                   ← 多半不能:后面接着 AES/3DES
   29  JSON 页的形状对不上(`找不到函数 replace`)     ← 多半不能:28/29 之后拿去解密
   21  书源的 JS **自己在 Rhino 上就语法错误**        ← **不能**:真身也跑不动这个源
   19  Rhino `\p` 方言                               ← 不用修:已知豁免族
   15  源要 AES 解密,页面是明文                      ← **不能**:要拿书源的密钥加密回去
   19  口味 / 没配 url / 其它

一句话:**还能修的是 75 + 53 + 92 ≈ 220 例,另外 114 例是加密族、坏书源与前导 `-`**。
全表与脚本在 `fixtures/cases/pipeline-corpus-b/README.md` 的「命中面」段。

**语料实测的加权**(1704 源:A 879 / B 622 / C 203 —— 这三个数**后来发现是错的**,
真数是 908 / 684 / 112,理由见 Phase 3 M3a;下面按加权排的清单不受影响,
因为多出来的那 92 个源本来就分布在 A/B 两层里)。B 层里:
`<js>` 392 源、`@js:` 363、`loginUrl` 352、`header` 274;LiveConnect
(`Packages.`/`JavaImporter`/`org.jsoup`)仅 ~27;**`jsLib` 0 源**;
XPath(裸 `//` 开头的规则)全库 80 源、`@XPath:` 前缀 0 源。B 层真正被调用的
`java.*` 只有 26 个名字,头部是 `ajax(116) / put(92) / getString(87) / get(76) /
timeFormat(68) / log(36) / md5Encode(31) / aesBase64DecodeToString(19)`。
据此把 plan 的清单按加权排序:**js-host 差分套(地基)→ JsExtensions 按频次移植
→ LiveConnect 方言 → `@js:` url + 登录 header → XPath(4.7%,排后面)
→ **产品路径补课(M2h)** → pipeline-corpus 扩 B 层收口判据**。
前五件已落地(M1 / M2a-g),M2h 把「差分绿了但产品其实没接」的三处填上
(网络面 / HostConfig / cache 共享)—— 那是扩 B 层的前置;**M2i 把 B 层扩进去了**
(新套 `pipeline-corpus-b`,B 层 96.9%,判据线是 80%);**M2j 把 `result` / `src`
绑定改成真对象**(79 → 66);**M2k 把那份清单做完**(66 → 16,B 层 99.2%,
详见该套 README 的「M2j」「M2k」两节)。
剩下的 16 例当时记的是「两个引擎层面的分歧」(html5ever 的 foster parenting 15、
正则严格度 1)—— **M2n 证明那 15 例里的分歧是回落页自己造出来的**(`<tr>` 少了
`<td>`),补上之后 14 例 PASS、1 例是真缺口;只剩正则严格度那 1 例。

**M2l(2026-08-30):最后一条判据「自选 top-100 常用源 ≥90%」闭合**。
新套 `top100`(`tools/top100_diff.sh`,契约 `fixtures/cases/top100/README.md`),
**446 例 446 PASS,按源 100/100,0 豁免**;两侧各连跑两次逐字节相同。三件事:

1. **名单是一条写死的规则,不是手挑的清单**。「自选」如果等于手挑,就能挑软
   柿子、判据当场作废。规则(`select_top100()`)只用**包方自己的元数据**排序:
   入池取 `fixtures/sources/` 里**人工策展的那两份**(aoaostar 精品包 128 +
   XIU2 22 = 150 源),剔掉「包方自己标了暂不可用/人机验证」10 条与摸时钟/随机
   5 条,余 135 按「XIU2 优先 → 也在大包『校验可用』组里(双重背书)→ 精品标签
   → customOrder → url 字典序」取前 100。实得 **A 52 / B 40 / C 8** ——
   **C 层第一次进流水线差分**(A/B 两套按层全量入册,哪一套都不收 C)。
   名单落在 `selection.json`,**入库、可 review**。
2. **判据按源算**。按 case 算时一个源错一步只丢 1/446,通过率天然好看;
   「这个源能用」的意思是它的四五步全都对。故:源通过 ⟺ 它名下每一条 case 都
   PASS,≥90/100。**豁免不算通过**(但仍占分母),否则把跑不动的那支逐条豁免
   掉,判据就被躲掉的部分注水了。报表 `tools/top100_report.py` 的比较与豁免
   判定**直接复用 `compare_jsonl.py` 的函数**,不另写一份。
3. **顺带修好两处**:
   - **回落页口味按「步」算,不是按源**(`flavor()`)。一整批书源搜索走 JSON
     API、目录/正文是 HTML 页 —— 按源取一个口味,必然有一半的步骤拿到它永远
     匹配不上的那张页,两侧一起空、一起「一致」。改完 top100 的
     info 37%→69% / toc 41%→73% / content 53%→86%,**A 层套同期
     30%→38% / 43%→50% / 78%→85%,而两套 FAIL 数零变化** ——
     抬上来的这部分本来就是对的,只是三年没被测到。
   - **`source.setVariable(0)` 把变量删掉了**(产品缺口)。真身
     `setVariable(variable: String?)` 只有 null 才是删除,非 null 由 Rhino 按
     Java String 形参转换(= JS 的 ToString);被测侧只认 JS 字符串,数字/布尔/
     对象一律走 None 分支删键 —— 与真身正好相反。八条探针
     `js-src-setvar-*` 把这组转换逐条钉住(含 Rhino 的怪癖 `undefined`→"undefined")。

**这条判据的含金量要连着命中面读**:M2l 收盘时 446 例里只有 **251 例(56%)**
裁判侧真的装配出了东西,最弱的一环是 **JSON 回落页**(命中 4~13%)——
这一条已由 M2m 做掉,见下。
另外 **「0 豁免」不等于 webView 做好了**:名单里 8 个 C 层源带 webView/
startBrowser,但四步这条路一次都没走到它(它们落在登录流程与发现页里)——
webView 仍是 Phase 3 的零信心区。

**M2m(2026-08-30):JSON 回落页按源反向合成 —— top100 的命中面 56% → 74%**。
M2l 结尾点名的那一件,单独立项做完。判据的数字一个没动
(top100 **446/446、按源 100/100**;A 层 0/3894;B 层 16/2783),
**动的是这些数字底下有多少面是真的被测过的**。

1. **机制**:新 `tools/fallback_json.py` 按**每个源自己的 JSONPath** 反向合成
   JSON 回落页 —— 入口规则(`bookList`/`chapterList`/`content`/`init`)定容器
   形状,字段规则相对列表项合成进 item,`{{$.x}}` / `{$.x}` 这类**内嵌规则**
   (`SourceRule.makeUpRule` 里 `isRule()` 认的那一档)与 `java.getString('$.x')`
   一并收。**一个「源 × 步」一张**,相同形态按内容哈希去重
   (三套各 56 / 57 / 186 张)。合成不出结构的源拿到的是**底板**,即改造前那份
   手写的通用形状 —— 逐字节没变,所以这一改**只可能往上抬命中面**。
2. **为什么 JSON 不能像 HTML 那样全套共用一张**(**M2n 起 HTML 那张也按源了**,
   理由见下面 M2n 第 2 条):CSS 选择器**位置无关**,
   一张 DOM 摆得下几十种形态;JSONPath 除 `$..` 外**从根锚定**,而语料里最常见
   的容器名恰恰互相打架 —— `$.data[*]`(搜索列表,要 data 是数组)、
   `$.data.entry`(目录列表,要 data 是对象)、`$.data.content`(正文,要对象)
   三选一。而回落页的名字本来就是 case 自己带的(docs/http-snapshot.md §6),
   「一张页回答所有请求」只是最初的用法,不是机制的限制。
3. **命中面**(`tools/hit_report.py`,三套的 `*_diff.sh` 现在都跟着通过率一起打;
   README 里的表**别手抄**——手算的数字会和代码悄悄漂):

   | 套 | 合计 | JSON 页(改前 → 改后) |
   |---|---|---|
   | top100 | 251/446 (56%) → **331/446 (74%)** | ~16/104 (15%) → **96/104 (92%)** |
   | pipeline-corpus(A) | → 2163/3894 (55%) | → **171/186 (91%)** |
   | pipeline-corpus-b(B) | → 1447/2783 (51%) | → **350/465 (75%)** |

   `info` 那一行只看 `book.name` 非空:`BookInfo.kt L158` 把空 tocUrl 兜底成
   baseUrl,拿 tocUrl 当命中信号会把 top100 读成 96%(实测 96/100 而 name 79/100)。
4. **抬上来的面上露出六处缺口**,五处当场修掉:
   - **`{{result}}` 里的整数 Double 格式化**(6 例):`<js>` 返回的数字再进
     `{{}}` 时,真身走 `String.format("%.0f")` 给 `1100000001`,而被测侧把
     `result` **绑成了串**(Java `Double.toString` → `1.100000001E9`)。
     Rhino 的 `Context.javaToJS` 对 Number/Boolean 原样交回 —— `BoundValue`
     因此补了 `Num` / `Bool` 两个变体,绑成 JS 原生 number/boolean。
   - **jayway 的路径**:`JsonPath` 构造**不 trim**。`$.a&&\n$.b` 这种写法
     (`&&` 由 AnalyzeByJSonPath 自己拆)拆出来的第二支是 `"\n$.b"`,真身把
     `\n$` 当**属性名**读(`Missing property in path $['\n$']`)—— 被测侧原本
     两头都 trim,于是「我们读得到而裁判读不到」。只有**尾部空格**被属性名
     读取器吃掉,而且只在「后面到末尾全是空格」时;中间的空格是 InvalidPath。
     十条探针进了 json 套(`jh049`~`jh058`)。
   - **`book.kind` 该是 null 不是空串**:Kotlin 的 `Book.kind` 是 `String? = null`
     (name/author/bookUrl/tocUrl 都是非空 `""`,只有它可空),
     `JSON.stringify({BookID: book.kind})` 这类写法上真身给 `null`。
   - **合成页自己造出来的分歧**(M2k 那一课的复发):`java.getString('$.modifiedOn')`
     被塞进 `timeFormat(...)`,而页面给的是展示串 `2026-08-29` → Rhino
     「无法将 2026-08-29 转换为 java.lang.Long」直接抛。**真站点那里是数字** ——
     故键名像时间戳/id 的、以及被塞进 JS 算术的路径,一律给数字。
   - **过滤器谓词要认全比较运算符**:`[?(@.type != 1)]` 只写个 `type` 是不够的,
     键缺失时 jayway 认为 `!=` 成立(实测),塞个 `1` 反而把这一支变成不成立。
   - 第六处**不是欠账而是已知回避面**(pb01528):同一条 `##` 正则在 regex 套里
     早就记着豁免(**有界变长 lookbehind**,Java 支持而 fancy-regex 0.14 只支持
     定长),在四步这边的形态是「裁判把正文清成 `content_empty` 而被测侧原样
     留着」。故进 `exemptions.json`,**名单直接读 regex 套那份的 `pattern` 字段**,
     不另抄一份 —— 同一件事两处记迟早漂,而漂掉的那天判据不会报错。
5. **顺带**:`js` 套 28 → 29,那一条是**给已知缺口补的最小探针**
   (`js-dialect-regex-identity-escape-p`:Rhino 把 `\p`/`\P` 留给 Unicode 属性
   转义,**不带 `u` 标志也直接 SyntaxError**,quickjs-ng 按 Annex B 当恒等转义)。
   这条缺口本来就在 pipeline-corpus-b 的 16 里(pb02108),现在它有了一行的
   最小复现,不用再拿一条一千字的语料 case 当证人。
6. **「生成物一律不入库」立成判据**(顺手把这一类关掉)。此前
   「先入库好 review」与「这是生成物删掉」来回拉锯过四次
   (`d086af6` / `dc2f4b3` / `e6ee878` / `ebd8f17`)—— 同一批文件加进来又删掉,
   因为规则只活在人的脑子里。现在它由 CI 判:

       tools/check_generated.sh —— 把 tools/gen_*.py 全跑一遍,
       工作区必须没有任何变化(既不改到入库文件,也不冒出未跟踪文件)

   据此清掉 **109 个入库的生成物**(12 份用例/名单/豁免清单 + 97 份合成 HTTP
   快照),`fixtures/` 里留下的 728 个文件**全部是输入**:`sources`(1704 源
   语料 8 MB)、`corpus-js`(683 段真实书源 JS)、`pages`、`spike-js`、
   `http-js-host`(手维护的回落页)、`diff-baseline.json`(手维护的棘轮)、
   各套 README(契约)与手写的用例/豁免。**验过一遍**:把那 109 个全删掉再跑
   `all_diff.sh`,十六套逐套与改动前逐字节同结果(连 fetch 的 `snapshot_miss
   2932` 都一样)—— 也就是 fresh clone 跑得起来。
   同一条判据顺带抓到一个真 bug:`gen_icu4j_tables.py` 的输出比入库的
   `rust/crates/net/src/icu4j_data.rs` **多一行空白**,谁跑一遍生成器产品源码
   就脏一行;已修。那份 `.rs` 是**产品构建的输入**(不入库就编不过),
   是这条规则唯一的例外,现在由这条判据钉住不许漂。

**M2n(2026-08-30):HTML 回落页也按源反向合成 —— 命中面 74% → 87%,
FAIL 45 → 24,豁免 86 → 69**。M2m 结尾点名的那一件。

1. **先补那一层 `<td>`**(pipeline-corpus-b 16 → 2)。上一轮把那 15 例记成
   「html5ever 与 jsoup 在 `<tr>` 里的分歧」,**其中一半是回落页自己造出来的**:
   生成器给 `bookList: tbody tr` / `tag.tbody@tag.tr@tag.a` 合成的是
   `<tr><a>…</a></tr>`,而真站点是 `<tr><td><a>` —— 没有 `<td>` 的那份会被
   HTML 解析器 **foster parenting** 把内容挪出表外,于是那批 case 测的不是规则,
   是两个解析器对畸形 HTML 的分歧。补上之后 14 例当场 PASS,剩下 1 例是真缺口
   (见下面第 3 条的第一处)。两个方向都要补:payload 装不进 `<tr>`,
   `tag.tbody@tag.tr@tag.a` 的 `<a>` 也装不进 `<tr>`(`PAYLOAD_WRAPPER` /
   `CHILD_OK`)。
2. **再把「一张页」这个约束去掉**(与 M2m 同一课)。共用的那张只反推了**容器**
   规则,字段规则(`name` / `bookUrl` / …)拿的是一份**写死的 payload** ——
   容器选中了、`name` 对不上,`getSearchItem` 照样把整条丢掉。实测 B 层 search
   的 486 条 html 用例里,**183 条是「容器认得、仍然没命中」**。改成
   **一个「源 × 步」一张**(`fallback_page.build_source`,内容哈希去重,
   合不出结构的退回底板),照真身 `AnalyzeByJSoup` 的两条口径合成:
   列表规则每一段都是元素步,字段规则**末段永远是动作**
   (text/textNodes/ownText/html/all 取文本,其余一律 `element.attr(末段)`)。
   命中面:**top100 74% → 87%**、A 层 55% → 77%、B 层 51% → 71%;
   html 页那一列 A 53%→76% / B 47%→70% / top100 68%→85%。
   顺带三件:页面小了十几倍,三套 pipeline 差分**快了一半**(all_diff 2:48);
   合成时不再造畸形属性名(书源的末段常是半截 JS);裸文本不再落进 `<tbody>`。
3. **抬上来的面照出七处产品缺口,全部修完**:
   - **`BookList.getInfoItem` 的 `@put:` 变量没写回实体**(pb00407):真身的
     ruleData **就是** book(`setRuleData(book)`),`toSearchBook()` 把 variable
     带走;被测侧 ruleData 是一份拷贝,不写回就丢。
   - **jsoup 的 `hasClass` 两个代理都没有**(pb02708):
     `result.select('em').hasClass('vip')` —— Element 看自己,Elements 是
     「其中任意一个」,大小写不敏感(拿 jsoup 1.16.2 的 jar 探过)。
   - **XPath 选出来的 JXNode 交给下一条规则时丢了元素身份**(pc03399):
     真身 `AnalyzeByJSoup.parse(doc)` 对 `doc is JXNode && isElement` 走
     `doc.asElement()`,**不重新解析**;被测侧把 `getElements` 的结果整体做成
     `StrList`,下一条规则于是 `Jsoup.parse("<tr>…</tr>")` —— 又是 foster
     parenting,`tag.tr` 一条都选不中。新增 [`RuleValue::JxList`]:
     toString 仍是 Java 集合口径 `[a, b]`,而**每一项若在真实树上就保住元素**。
   - **java_proxy 的箱交给 String 形参要解包**、**IDN host**、
     **`Elements.remove()/.not()/.eachText()`**、**`book.setReverseToc`**、
     **`java.getString(rule, mContent)` 的第二个实参** —— 这五处在 js 套里,
     逐条见 `fixtures/cases/js-host/README.md` 的「已知差异清单」。
     其中 IDN 是接上 okhttp 的 `idnToAscii`(UTS-46 映射 + NFC + punycode,
     交给 `idna` crate;全 ASCII 的 host 仍走原来的快路),**连带把三套 pipeline
     的 27 条 IDN 豁免全部收掉** —— A 层与 top100 因此变成 **0 FAIL、0 豁免**。
4. **判据现状(M2n 当时)**:十六套 99372 例 **99279 PASS / 24 FAIL(豁免 69)**;
   FAIL 只在 js-host(23,其中 14 是两个引擎的方言)与 pipeline-corpus-b(1,
   就是 Rhino 的 `\p`)。两侧各连跑两次逐字节相同,`check_generated.sh` 绿。
   (**今天是十六套 99453 例 0 FAIL、豁免 77** —— 见文首。)

**M2n 当时点名的下一个动作**:命中面还剩两块 —— B 层的 explore(35%)与 info
(70%)那两列里,`不认`的形态(`||` 之外的组合子、`:has()`/`:matches()` 这类
jsoup 扩展选择器)还没合成;以及 js 套那 14 条方言到底收紧还是记豁免。
**两件都已做掉**:形态在 M2o 与 2026-08-31 那一轮补齐,方言在 M2o 定调。

**M1 已落地(2026-08-29):js-host 差分套建立**,首轮 803 例 **513 PASS / 290 FAIL**,
补完纯函数面后 **584 PASS / 219 FAIL**。
契约见 `fixtures/cases/js-host/README.md`,入口 `tools/js_diff.sh`。
- 裁判是**新模块 `judge/jsharness`**:挂 `judge/rhino` 真身 com.script(**真 Rhino**)
  + `JsExtensions` / `JsEncodeUtils` / `AnalyzeRule` / `BaseSource` / `JsSourceEngine`
  真身。`:harness` 那边的 com.script 是有意为之的确定性桩(92362 例绿建立在它上面),
  同一 classpath 上只能有一份,故分模块;jsharness 的垫片从 harness **复制**而非共享。
- 被测侧 `difftest` 新增 `js_case_runner` bin,走 `js-host::QuickJsHost`。
- 语料 683 段真实书源 JS(`fixtures/corpus-js`,绑定按来源规则位挑)+ 手写 120 例
  (方言探针 + `java.*` 宿主 API 面)。
- **290 个 FAIL 全是被测侧的实现缺口**,即 M2 的工作清单:① `java.*` 未实现
  (timeFormat/htmlFormat/encodeURI/hex/crypto 家族…);② `java.getString/
  getElements/setContent` 未接 —— 真身的 `java` **就是 AnalyzeRule**,JS 能反调
  规则求值,被测侧的 `HostEnv` 只有 `VarStore`(put/get),要扩成完整回调面;
  ③ `cookie`/`cache`/`source`/`book` 绑定未接;④ LiveConnect 方言。
- **本套抓到的方言事实**(探针实测,写进 README):绑定是 JS 原生 string,但
  **`java.*` 的返回值不转** —— `typeof java.md5Encode('a') === "object"`,是包着
  Java String 的 `NativeJavaObject`,于是 `java.get('k').replace('a','b')` 走
  **Java 的 replace(全部替换)** 而不是 JS 的(只换第一个);`importClass` 在真身里
  **是 undefined**(Phase 1 的 prelude 自作主张补了,已删);`java.lang` 是 undefined
  (`java` 是 AnalyzeRule 实例不是 Java 包,语料里 26 个源写 `java.lang.*`,
  它们在真身上同样会炸);Rhino **不支持 `class` 与展开语法**,却支持 `for each`、
  E4X 与八进制转义。
- **M2a(同日)**:按频次补 `timeFormat`/`timeFormatUTC`/`encodeURI`/`hex*`/
  `base64*`(flags 与 charset 重载按实参类型分派)/`androidId`/`getWebViewUA`,
  并修三处口径:① 求值改**非严格模式**(rquickjs 默认 strict,Rhino 是 sloppy);
  ② `ajax`/`connect` 未接时**吞异常返回标记串**而不是抛(真身就是吞);
  ③ `java.get` 按 arity 分派(`get(key)` 是变量读取,`get(url, headers)` 是请求 ——
  一处就吃掉 29 例)。剩余 219 例的最大一块是**规则回调面**(`java.getString` 49 例
  等):真身的 `java` 就是 AnalyzeRule,JS 能反调规则求值,被测侧的 `HostEnv` 只有
  `VarStore`,要扩成 `RuleHost`;再往下是 **Java 对象代理层**(List/Element/String
  在 JS 里都是 NativeJavaObject,`typeof` 为 object、`toString` 是 Java 形态)。
- 本套**不接网络与文件系统**:两侧同一标记 `«net-unsupported»` / `«fs-unsupported»`
  收敛成 `unsupported:net|fs`。要让 `java.ajax`(B 层第一名,116 源)进差分,
  得把 `AnalyzeUrl` 真身 + `ReplayHttp` 挂进 jsharness。

**M2b(2026-08-29 收盘):584 → 709 PASS / 94 FAIL**。五件事,按落地序:

1. **补上「两个宿主」维度**(+8,但改的是判据本身)。首轮 64 例裁判报
   `ReferenceError: "key" 未定义` —— 不是实现缺口,是**差分套挑错了宿主**:
   整套跑在 `AnalyzeRule` 上,而它根本不绑 `key`;那些片段来自 `searchUrl`,
   产品里跑的是 `AnalyzeUrl.evalJS`。照着这批 FAIL「实现」会把搜索做死。
   两个宿主的 `java` 指向不同对象、绑定面不同、变量层查找链也不同。
   落地:用例加 `host` 维度、裁判侧 AnalyzeUrl 垫片补真身的 evalJS/put/get、
   被测侧 `rubato_core::host::JsHost`。
2. **Java 对象代理层**(+4)。规律是**Java 方法优先、JS 原型兜底**:包装对象的
   原型链到 `String.prototype`,`slice`/`match` 能用,但 `replace`/`split`/`length`
   走 Java 那一份。另一条关键实测:**完成值会解包**,装箱只在表达式中间可见。
   见 `js-host/src/java_proxy.rs`。
3. **五个绑定**(+47):`book` / `source` / `cookie` / `cache` / `chapter`。
   cookie/cache 现在是进程内表,**产品侧后续要接 store 的两张表**。
4. **RuleHost 反调面**(+45):`java` 就是 AnalyzeRule,JS 能反过来跑规则。
   元素**不跨接口传值**(jsoup 类型属 html-compat),只过 `ElementHandle` 号;
   `AnalyzeRule::eval_js` 用 `mem::replace` 把 host 换出去打破自借用。
5. **确定性垫片 + org.jsoup LiveConnect + 三处口径**(+21):
   冻 `Date`/`Math.random`(此前 17 例每次跑都在变,是差分自身的噪声);
   `org.jsoup.Jsoup.parse` 走 `RuleHost::rule_parse_html` 反调,与 getElement(s)
   共用句柄;真身 `ajax` 失败走 **AppLog.put 不进 Debug 日志**(未接的网络面
   不能落日志);gson 没填的实体字段是 **Java null 不是 undefined**;
   ~~元素代理的方法要**不可枚举**(`for (var i in els)` 只给下标)~~
   —— **这一条当时只写了一半,2026-08-30 查出来**:`sealProxy` 两侧都写好了却
   没人调,被测侧 `for (var i in els)` 实测吐 `0,1,__javaKind,length,size,
   toString,text,attr,html,select,first,last,eq,isEmpty,get,toArray`,
   真身只给 `0,1`。仍未接,见本文末「留着没做」。

**剩下 94 例分六堆**(详表在 `fixtures/cases/js-host/README.md`),最大一堆
(~25)是网络面没挂 —— 那就是 M2c。

**M2c(2026-08-29 收盘):接上网络面,709 → 730 PASS / 73 FAIL**。
`java.ajax` 是 B 层第一名(803 例里 123 例踩到),此前两侧都只是返回同一个
「未接」标记 —— 比的是标记本身,不是实现。现在 **52 例真的发请求**。

- **裁判侧**:`judge/jsharness` 挂 `AnalyzeUrl` **真身** + `StrResponse` /
  `CookieStore` / `CookieManager` 真身,okhttp 那条路由 shims 的 `newCall*` 转到
  **`jsharness/ReplayHttp`**(复制自 `:harness` 那份,契约 `docs/http-snapshot.md`)。
  此前那份 AnalyzeUrl 垫片作废。
- **被测侧**:`rubato-core::host` 新增 `NetProvider` / `NetResponse` / `NetError`
  接口(**网络的提供方与 JS 宿主不是一个东西**:产品侧注真 okhttp,差分侧注录放),
  `js-host` 按接口装 `java.ajax/connect/get/head/post`,
  `difftest::js_net::ReplayNet` 把 `net::AnalyzeUrl`(Phase 1 已 0 FAIL 的那份移植)
  接到同一份快照上。
- **快照**:语料的 URL 指向真实站点,没有录制可言 → 两侧都配**回落页**
  (`fixtures/http-js-host/_fallback/{json,html}`,用例的 `fallback` 字段挑,缺省 json)。
  两侧拿到同一份字节,差异仍然只来自实现。
- **新增观察面 `hops`**:本 case 发出的 `(method, 规范化 URL)` 序列。URL 拼装、
  方法、跳数是 `ajax` 类书源最容易走岔的地方,**出错路径上也记**。

**顺带堵掉两个判据本身的窟窿**(都不是「实现缺口」,是差分自己不可信):

1. **裁判在偷偷连外网**。`java.get/head/post` 走的是 **jsoup** 而不是 okhttp,
   jsoup 底下是 `HttpURLConnection` —— 那条路当时没人拦。实测
   `js-corpus-ea97f51e04` 把 `https://www.35xss.com/` 的**实时 302 Location**
   当成了裁判结论。修法:`jsharness/ReplayUrlStream` 装全局
   `URLStreamHandlerFactory`,把 JVM 的 http/https 整条 URL 路接到录放上,
   **裁判进程再也发不出真实请求**。
2. **裁判自己不可重现**。`java.randomUUID()` 每次都变(2 个语料用例把它拼进
   签名串)。建套时只查了被测侧连跑两次一致,漏了裁判侧 —— 而裁判是"标准答案",
   飘得更隐蔽。修法:`jsharness/DeterministicRandom` 插一个优先级最高的 JCA
   Provider,`SecureRandom` 吐定死的字节流(被测侧照同一条定义实现)。
   **判据改成两侧各自连跑两次逐字节相同**,已通过。

**踩实的语义**(都写进了 README 与代码注释):
- **`ajax` 有两份实现**:`AnalyzeRule` **覆写**了它(AnalyzeRule.kt L951)——
  带 `ruleData = book`、失败**记 Debug 日志且带完整栈**;
  `JsExtensions.ajax`(L134)不带 ruleData、失败走 AppLog(不进 Debug)。
  此前 README 只记了后者,是错的。
- **构造 `AnalyzeUrl` 抛的异常不被吞**:它在真身的 `runCatching` **之外**。
  url 里写 `{{$.id}}` 会在 `replaceKeyPageJs` 里当 JS 跑并抛,归一标签
  `host:ScriptException`,不是「返回栈字符串」那条路。
- **异常栈字符串两侧归一**成 `«java-stack»`(全是 Java 类名行号,不该逐字比);
  日志里 `ajax(<url>) error\n<栈>` 只归一换行之后那半 —— **URL 那半要比**。

**M2d(同日):按频次补 `java.*`,730 → 747 PASS / 56 FAIL**。

- **摘要 / HMAC**:`digestHex`、`HMacHex`(hutool `DigestUtil` / `HMac`,
  按 RFC 2104 直接算,分组长按各摘要算法取)。
- **对称加解密**:`aes*` / `des*` 那一排薄壳 + `createSymmetricCrypto`。
  真身里它们**全汇到 `createSymmetricCrypto(transformation, key, iv)` 一处**,
  故被测侧也只有一份核心(AES/DES/DESede × ECB/CBC × PKCS5/7、NoPadding、
  ZeroPadding)。CFB/OFB/CTR 显式报错而不是静默算错 —— 差分上会现形。
- **字节数组面**:`strToBytes` / `bytesToStr` / `base64DecodeToByteArray`
  (装箱层的 `bytes` 一档早就在,只是没实现)。
- **`randomUUID`**:按 `DeterministicRandom` 那条**同一个字节流**实现,
  每 case 从 block 0 起,每次调用吃一个 block。
- **`getCookie(tag[, key])`**、**`toNumChapter`**(连带
  `StringUtils.fullToHalf` / `chineseNumToInt` / `stringToInt` 的移植)。

**顺带钉住三条语义**(都写进了 README 与代码注释):
- **`url` 宿主的 `baseUrl` 绑定不是传进去那个串** —— `AnalyzeUrl.analyzeUrl()`
  在 `evalJS` 之前把它改写成 `NetworkUtils.getBaseUrl(url)`(只剩 `scheme://host`)。
  差一个 `/` 就让 `baseUrl + "/so/…"` 两侧走岔。
- **base64 有两个解码器**:`base64Decode` 走 hutool(**宽松**,坏输入不抛,
  末组多出来的位直接丢),`base64DecodeToByteArray` 走 android(**严格**,抛
  `IllegalArgumentException`)。混用会把错误类别钉错。
- **对称加解密的错误类别看走哪条路**:经 hutool 的一律 `CryptoException`;
  LiveConnect 直接调 JCE 的是 JCE 自己的类。

**剩下 56 例**(详表在 README):**LiveConnect 的 crypto / okhttp 一支 20**(书源
绕开 `java.*` 直接调 Java 类,是剩余最大一堆)、网络面余量 7(多数是别的缺口的
影子;真的一条是 **IDN host 未做 punycode**)、仍未接的 webView / 验证码 6、
被测侧 SyntaxError 而裁判能跑 5(探索页 JSON 里内嵌 `@js:`)、Rhino 更严 3、
DOM 改写 2、XPath / `for each` / `htmlFormat` 各 1、零碎 10。
**下一件是 LiveConnect 那一支。**(→ M2e)

**M2e(2026-08-29 收盘):LiveConnect 的 crypto 一支,747 → 801 PASS / 37 FAIL**
(用例同时从 803 涨到 838 —— 补了 35 条探针,全绿)。

书源绕开 `java.*` 直接调 Java 类,语料里 ~27 个 B 层源这么写:
`new JavaImporter()` + `Packages.javax.crypto.*`,`with (javaImport)` 里
`SecretKeySpec` / `Cipher` / `Base64` 直接当名字用。落地的是**一层对象面**
(`js-host/src/live_connect.rs`),不是第二份密码学 —— 分组密码本体与 `java.aes*`
共用 `java_api::SymmetricCrypto`,差的只有新加的 `CryptoFlavor`(hutool 一律
`CryptoException`,JCE 是自己那六个类)。

- **名字解析**:`JavaImporter` 用 `Proxy` 的 `has`/`get` 实现,与 Rhino 一样是
  **懒的**;同名类跨包冲突(`java.util` 与 `android.util` 都有 `Base64`)是
  `EvaluatorException` → 与裁判 errorTag 同归 `js:SyntaxError`。
  `with` 块里没 import 的名字照常落回全局。
- **`java.lang.String` 遮住 JS 的 `String`**:`String(byte[])` 是按默认字符集
  **解码**;Java 类不带 `new` 直接调也是构造。
- **Java 数组的形态改对了**:`Array.isArray` 为 false、元素**有符号**、
  `String(arr)` 给 `[B@…`、GSON 出数字数组 —— 装箱层照这四条重建
  (顺手修好了 `java.strToBytes('中')[0]` 一直给 228 而真身给 -28 的老账)。
- **错误落在哪一步**:`getInstance` / `init` / `doFinal` 三步各抛各的,与真身同行。
- **android.util.Base64 的解码器复刻到状态机**:裁判垫片用
  `java.util.Base64.getMimeDecoder()`,表外字符跳过但 `=` 的**位置**照 JDK 校验。
  `js-corpus-c9d744b7c7` 把一段中文喂给它 —— 中文字节全被跳过,剩下的有效字符
  恰好凑满 4 的倍数,垫片补的 `==` 落在新组开头 → `IllegalArgumentException`。
  两侧对上要连「按字符数还是字节数取模」都一样(Kotlin 的 `String.length`)。

**顺带堵住裁判侧最后一个出网口**:书源可以 `new Packages.okhttp3.OkHttpClient()`
自己建客户端 —— 那是真身的类,既不经 shims 的 `newCall*` 也不经
`URLStreamHandler`,裁判会真的去连外网(那两例的裁判结论是 5 秒 **timeout**,
即「当天那台主机连不连得上」)。`ReplayUrlStream.install()` 现在把默认
`ProxySelector` 换成「一律走 127.0.0.1:1」,连接立刻被拒,书源自己的 `try/catch`
照常吞掉;被测侧没有 `okhttp3` 这个类,同样被吞 —— 两侧收敛。判据的
「一个字节都不许出网」这条前提到这里才真正成立。

**另外两处口径**(都由差分抓出来):`java.getElement` 打不中 JSONPath 时,jayway 的
`PathNotFoundException` 会原样穿到 JS(与 `getString` 吞掉给空串**不同**),
被测侧在 `rule-engine::js_facing_error` 那一层翻回类名;`java.bytesToStr` 收的是
装箱过的类数组,不能再走 rquickjs 的 `Vec<u8>` 转换。

**十二套复验**:syntax 30326 / regex 8045 / json 2307 / html 23077 / rule 10949 /
analyze_url 6613 / fetch 3060 / charset 293 / source 1727 / url 2066 / pipeline 88 /
pipeline-corpus 3738 —— **全部 0 FAIL**,无回归。两侧各连跑两次,输出逐字节相同。

**剩下 37 例**(详表在 README):LiveConnect 的 **RSA 一支 1**、网络面余量 7
(真的一条仍是 **IDN host 未做 punycode**)、webView / 验证码 6、被测侧 SyntaxError
而裁判能跑 5、Rhino 更严 3、DOM 改写 2、XPath / `for each` / `htmlFormat` 各 1、
零碎 10。**下一件不在这张表里**:`@js:` url 与登录 header 这两个面**还没进套**,
而它们在语料里的加权比表上任何一堆都大(363 / 274 源)。(→ M2f)

**M2f(2026-08-30 收盘):`@js:` url + 登录 header,801 → 1844 PASS / 31 FAIL**
(用例从 838 涨到 **1875**)。这一件是「补面」不是「修 FAIL」——上一轮的 37 个
FAIL 里一条都不属于这两个面,因为**它们根本没被测过**。

两件事:

1. **第三个宿主 `BaseSource.evalJS`**(`host: "source"`)。source 的 `header` /
   `loginUrl` / `loginCheckJs` 走它。绑定面最窄,而且差别不只是「少几个名字」:
   `java` **就是书源实体**(探针实测 `java === source === sourceApi` 为 true),
   `baseUrl` 是 `getKey()` 而不是页面地址,`result`/`book`/`chapter`/`page`/`key`
   是**未声明**(ReferenceError,不是 null),而 **`java.put/get` 打的是
   CacheManager**(`v_<sourceKey>_<key>`)不是那套四层变量。登录信息面是
   AES(androidId 前 16 字节)+ base64 的往返。新增观察面 `cache`(CacheManager 终态)。
   23 条 `js-src-*` 探针一次全绿。
   **顺手堵掉一个判据窟窿**:裁判垫片的 androidId 是 15 字符,而真身拿它的
   `[0,16)` 当 AES 密钥 —— 裁判侧的登录信息面因此**恒抛 IndexOutOfBounds**,
   比出来的会是垫片长度而不是语义。改成 16 字符。
2. **`op: "analyzeUrl"`:一条真实 `searchUrl` 整条走 AnalyzeUrl**(1014 例,
   去重后的真实规则 + 各自的 `header`)。`headerMapF` 不传 → `getHeaderMap()`
   真的跑,于是登录 header 那一面同期进套。与 `analyze-url` 那套(6613 例,
   两侧 JS 都是确定性桩)的分工:那边钉拼装与编码的全部分支,这边把 JS 打开。
   加权:1704 源里 `searchUrl` 带 `{{}}`/`@js:` 的有 **1456** 个。

**这 1014 例第一轮 932 绿,84 个 FAIL 全是被测侧的实现缺口,逐条修完后全绿**:

- **`org.jsoup.Jsoup.parse` 在 `url` 宿主上也得有**。它是 Rhino 的 LiveConnect
  **全局面**,与 `java` 是谁无关;被测侧却把元素面挂在 `RuleHost` 上,而
  AnalyzeUrl 那条路的求值环境只有 `RuleData` → TypeError。修法是 `net::SplitEnv`:
  变量层仍是 AnalyzeUrl 自己那份 RuleData(真身 `put/get` 打的就是它),
  **元素面另外注入**(`UrlArgs.rule_host`;net 建不出 AnalyzeRule,依赖方向不允许),
  而规则求值那几个**不转发** —— 真身在这个宿主上就没有。
- **option JSON 只有宽松档解得开时要记一条 Debug 日志**(AnalyzeUrl.kt L245)。
  语料里 option 用单引号的写法极多,这一条独占 69 个 FAIL。
- **嵌套 AnalyzeUrl 的日志要汇回外层**:`java.ajax(url)` 会拿 url 串再建一个
  AnalyzeUrl,它自己也记那条日志;真身落的是**全局 Debug**,外层看得见,
  而被测侧的嵌套宿主是另一个实例。修法 `NetProvider::take_logs`。
- **header 的值是对象/数组时不是「整块跳过」**:legado 注册了自己的
  `StringJsonDeserializer`,对象/数组给 `toString()`(实测一条 header 写成
  `{"headers":{"User-Agent":…}}`,真身给的是键 `headers` 配一整段 JSON 串)。
- **`source` 绑定要带 BaseSource 的方法面**:`source.getVariable()` /
  `setVariable()` 在 searchUrl 的 `@js:` 里是常见写法(书源用它记「选了哪个源站」)。
- `java.HMacBase64` 缺失。

**一处判据边界要记住**:确定性垫片**进不去** AnalyzeUrl 的内部求值(`@js:`/`{{}}`
是真身自己 eval 的,裁判侧没有「先在作用域里跑一段」的入口,jsLib 那条共享作用域
在 jsharness 是桩)。只在被测侧冻时钟会造成**单边**差异,反而更糟 —— 故这个 op
两侧都用真时钟,摸时钟/随机的书源整条不进套(扫的是 url **加整份裁剪后的 source**:
书源常把 JS 藏在 `bookSourceComment` 里再 `eval` 它)。语料里只排掉 2 条,
而它们的 JS 片段本来就由 `op: "js"` 覆盖。两侧各连跑两次逐字节相同,判据仍然成立。

**十二套复验**:syntax 30326 / regex 8045 / json 2307 / html 23077 / rule 10949 /
analyze_url 6613 / fetch 3060 / charset 293 / source 1727 / url 2066 / pipeline 88 /
pipeline-corpus 3738 —— **全部 0 FAIL**,无回归。

**剩下 31 例**(详表在 README):RSA 一支 1、网络面余量 7(真的一条仍是 **IDN host
未做 punycode**)、`startBrowserAwait` 1、被测侧 SyntaxError 而裁判能跑 5、
两个引擎严格度不同 4(含 **Java 方法重载歧义** —— Rhino 挑不出 `timeFormat` 的
重载而被测侧按 arity 分派就跑通了)、DOM 改写 2、XPath / `for each` / E4X /
`htmlFormat` 各 1、零碎 7。(**XPath 那 1 例在 M2g 已消** → 30 例。)
**下一件是 XPath**(全库 80 源),再往后是 pipeline-corpus 扩 B 层收口判据。(→ M2g)

**M2g(2026-08-30):XPath —— 新建 `xpath-compat`,1726 例 0 FAIL**。
这一件的头等收获不是代码,是**把 §1 的选型推翻了**。

**原计划「把 DOM 序列化成 XML 交 libxml2 求值」不成立**。裁判的
`AnalyzeByXPath` 底下是 **JsoupXpath 2.5.5**(`cn.wanghaomiao:JsoupXpath`,
ANTLR 文法 + 自写求值器,直接跑在 jsoup 树上),不是标准 XPath 引擎。差别不是边角:

- 多出 `allText()` / `html()` / `outerHtml()` / `num()` / `node()` 五个节点测试,
  `^=` `$=` `*=` `~=` `!~` 五个运算符,`following-sibling-one` /
  `preceding-sibling-one` 两个轴,`substring-ex` / `substring-after-last` 等函数;
- **`[n]` 不是文档位置**,是"在当前上下文集合里、同一个父下、同标签兄弟中的
  第几个",支持 `[-1]`、越界钳到 1;
- 文本节点被包成**合成元素**(`JX_TEXT`)继续参与后续步骤与谓词;
- **文法的 `main` 没有 EOF**,尾巴上的记号一律无视。

判据是与裁判逐字节相同,换任何标准引擎都对不上。故落地的是**照着 JsoupXpath
移植**:新 crate `xpath-compat`(词法 / 递归下降文法 / 求值器 / `AnalyzeByXPath`
壳),跑在 html-compat 那棵树上 —— **解析树仍然只有 html5ever 一份**,§2 的约束没破。
jar 用 `jadx` 反编译读的,52 个类逐个对着抄(轴 14 / 节点测试 6 / 函数 17 +
`CommonUtil` 的序号计算)。

**新差分套 `xpath`**(入口 `tools/xpath_diff.sh`,契约
`fixtures/cases/xpath/README.md`):1732 例 = 探针 549 + 语料 1179。
裁判是 harness 里**原样挂的 `AnalyzeByXPath.kt` + 真 JsoupXpath**,新 op `xpathDsl`,
三个 `contentType` 分支(string / document / element)× 三个 mode
(string / stringList / elements,elements 另出 `kinds` 钉住 `isElement()`)。
语料:1611 源里 107 个用 XPath,按 AnalyzeRule 的上游口径裁出 131 条表达式
(`##正则` / `<js>` / `@js:` 由上游切走,`{{}}` 换占位串)。
**首轮 1498/225,逐条修完 1726 PASS / 0 FAIL / 6 豁免。**

**顺带解锁两处判据**(此前是"两侧一起跳过",跳过不产生信心):
- **rule 套的 105 例 XPath 规则**:两侧的 `xpath_unsupported` 预探测双双删掉,
  10949 例仍 0 FAIL —— 这 105 例从此是真结果。
- **pipeline-corpus 的 33 个 XPath 源**:整源剔除的口子关上,
  3738 → 3894 例,0 FAIL。

**这 225 个 FAIL 教的方言事实**(全写进 README,这里只记最贵的四条):

1. **`/html` 是语法错误,不是"选不中"**。`html` / `text` / `node` / `num` /
   `comment` / `allText` / `outerHtml` 是**词法保留字**(NodeType),而文法的
   `nCName` 只收 `NCName | AxisName` —— 名字里没有 NodeType。`//htmls` 反而没事。
2. **`main` 没有 EOF**,吃完表达式就收工。语料里 20 多条"其实是相对 URL"的规则
   (`/api/book-info?id={{$.id}}`)靠这条静悄悄返回空串:`?` `&` 被词法丢掉,
   `/api/book-info` 当路径求值(选不中),剩下的 `id = X1 source_id = 1` 无视。
   **一条改动吃掉 200 个 FAIL** —— 之前被测侧一律报语法错。
3. **带冒号的 qName 不是名字测试**。`visitQName` 的 `size > 1` 分支返回
   `XValue.create(join(":"))` 而**漏了 `.exprStr()`**,于是 `visitStep` 把这个
   字符串**当结果抬走**:`//p:q` 的结果是字面量 `"p:q"`。语料里
   `//javascript:gotochapter('2332','577419')` 走的就是它。
4. **ANTLR 的 `sync()` 会静默删记号**。`DoFailOnErrorHandler` 只覆写了
   `recover` / `recoverInline`,没覆写 `sync` —— 可选块/循环入口上前瞻不匹配时
   `consumeUntil(follow set)` 只报 unwanted token 不抛。语料里 `[@href!~='…']`
   多出来的那个 `=` 就是这么被吞掉的。

**顺手修好 html-compat 的一个老账**:它的 `INLINE_TAGS` 多列了
`text` / `mi` / `mo` / `msup` / `mn` / `mtext` 六个,jsoup 1.16.2 的 Tag 表里
**根本没有它们**(逐个探针实测:`<div><text>x</text></div>` 裁判给
`<div>\n <text>\n  x\n </text>\n</div>`,即按未注册标签缩进)。html 套 23077 例
从没碰到这些标签,所以一直没现形 —— 是 XPath 的 `following-sibling` 把文本节点
包成 `Element("text")` 才把它顶出来。改完 html 套仍 0 FAIL。

**判据边界:裁判自己不可复现的那一处**。`descendant::` /
`descendant-or-self::` 与 `..` 都用 **`HashSet<Element>`** 归并,而 jsoup 的
`Element` 没覆写 `hashCode`(identity hash)—— **命中多于一个元素时迭代序跨进程随机**。
实测同一条 `//dl/descendant::a`,`string` 与 `stringList` 两个 case 顺序就不一样。
这类用例**不进套**(探针只探单命中形态);被测侧实现成文档序去重,
与裁判的一致**只在单命中时成立**。老规矩仍然成立:**两侧各连跑两次逐字节相同**
—— 上面这条就是这么照出来的。

**两条已知近似进豁免**:`<?xml` 开头的内容走 `Parser.xmlParser()`(html-compat
只有 HTML 解析器;该分支由**内容**触发而非规则,语料里没有规则依赖它);
wiki 页的无值属性(与 html 套同一条)。

**十四套复验**(数字是各套的 **PASS**):syntax 30326 / regex 8045 / json 2307 /
html 23077 / rule 10949 / analyze_url 6613 / fetch 3060 / charset 293 / source 1727 /
url 2066 / pipeline 88 / pipeline-corpus 3883 / **xpath 1726** —— **这十三套 0 FAIL**;
js-host 1845 PASS / **30 FAIL**(它从来没有 0 过,别把它算进上一句),无回归。

**M2h(2026-08-30):产品路径补课 + 判据自身的鲁棒性**。这一件不加新语义,
只把「差分绿了但产品其实没接」的三处填上 —— 它们才是 pipeline-corpus 扩 B 层的
**真正前置**,权重高于 js-host 剩下的 30 个 FAIL。

**① 产品侧的三处空壳(都在 `engine`,从前差分照不到)**

- **网络面整个是空的**:`host_factory` 每次 `AnalyzeRule::new` 造一个新
  `QuickJsHost`,`net: None` —— `java.ajax` 返回**字面串** `«net-unsupported»`,
  不报错、不落日志,**会被当正文拼进去**。差分侧的标记契约漏进了产品行为,
  而 `ajax` 是 B 层第一名(116 源)。现在有 `engine::js_net::LiveNet`
  (与差分侧的 `ReplayNet` 同一条路,换成真 transport)。
- **`HostConfig` 全空**:`androidId` / `getWebViewUA` 是空串,于是登录信息那条的
  AES 密钥长度为 0 —— `putLoginInfo` 恒 false、`getLoginInfo` 恒 null,**静默失效**。
  现在 `androidId` **开库时**生成 16 个十六进制字符**落库复用**(设备号不能每次变,
  书源拿它拼签名),UA 给一个真实形态。
- **`cache` / `cookies` 是 per-host 的 `HashMap`**:真身那两个是**进程级单例**,
  而这边一条规则换一个宿主 —— `java.put` / `sourceVariable_*` / `loginHeader_*`
  写完就丢(352 源有 loginUrl、274 源有 header)。现在是 `SharedMap`
  (`Rc<RefCell<…>>`),同一次引擎调用内共享,并在 `caches` 表的 `js:` 段
  seed / flush,跨调用也活着。

顺带:`host_factory` 的签名从 `Fn()` 变成 **`Fn(&RuleData)`** —— `AnalyzeRule.ajax`
的覆写要把 ruleData 传给嵌套的 `AnalyzeUrl`,url 里的 `{{…}}` 靠它解值;
`CancelToken` 接到了 QuickJS 的**中断钩子**上,取消不再只是步间粒度。

**② 判据自身**

- **差分 CI**(`.github/workflows/ci.yml` + `tools/all_diff.sh`):判据里白纸黑字
  写着「差分 CI 常绿」,而此前十四套全靠手跑,改 A 套时 B 套回归没人知道。
  `fixtures/diff-baseline.json` 记每套允许的 FAIL 上限(只许往下调),
  全 0 之前也守得住「不新增」。
- **两侧鲁棒性对称**:裁判每个 case 跑在可弃线程上、5 秒闸 + `ExecutionException`
  兜底;被测侧的主循环从前既无超时也无 `catch_unwind` —— 一次 panic 或死循环会废掉
  **整轮**输出,而裁判报 `error: "timeout"` 的 case 被测侧永远拿不出同样的值。
  现在 `js_case_runner` 同形。

**M2i(2026-08-30):pipeline-corpus 扩 B 层 —— 新套 `pipeline-corpus-b`**。
这一件收口 Phase 2 判据里的「B 层 ≥80%」。

**为什么必须新开一套而不是把 B 层塞进 pipeline-corpus**:裁判那边
**com.script 只能有一份** —— `:harness` 是确定性桩(A 层 3894 例的绿建立在它上面),
而 B 层要的恰恰是真 Rhino,那在 `:jsharness`。于是裁判分两个进程;
**被测侧没有这个约束**,两套共用 `difftest::pipeline_case` 一份执行器
(`Js::Stub` / `Js::Real` 只挑 JS 走哪条路),投影、错误归一、观察面因此
只有一份,不会各写各的。落地面:
- 裁判 `:jsharness` 挂上 `io/legado/app/model/webBook/**` 四步真身
  (此前那里是 `LegadoWebBookShims.kt` 垫片,已删)+ 新 `jsharness/Pipeline.kt`;
- 被测侧 `js_case_runner` 加 `op: "pipeline"`;
- **一次 case 内共享 hop 表 / cookie 库 / CacheManager**(`difftest::shared_io`)——
  裁判那三样本来就是进程级单例,而被测侧此前 `ReplayNet` 自建一份,
  于是 `java.ajax` 的那一跳不进 `requests`、cookie 不进 `cookiesDb`、
  也不会被后续请求带上,三处都是静默的单边差异;
- 语料:B 层 622 源减去**摸时钟/随机的 29 源**(与 `op: "analyzeUrl"` 同一条边界 ——
  确定性垫片进不去四步内部的求值,单边冻比不冻更糟),入册 593 源 2783 例。

**首轮 2650 / 127。127 个 FAIL 里最大的一堆不是「实现缺口」,是产品路径上的
四处空绑定 —— A 层那套两侧都是确定性桩,压根不碰绑定面,结构上照不到**:

1. **`source` 绑定一直是 None**:`AnalyzeUrl` 与每个 `AnalyzeRule` 都没填。
   书源 `@js:` 里读 `source.getKey()` / `source.bookSourceComment`(再 `eval` 它
   是常见写法)就是 TypeError。修法:`rubato_core::entities::BookSource` 新增
   `raw` 字段(解析它的那份原始 JSON),`source_binding` 带着走。
2. **`book` 绑定一直是 None**:详情/目录/正文三步的 ruleData 就是 Book 实体
   (`AnalyzeRule.kt L68` 的 `ruleData as? BaseBook`)。搜索/发现两步真身也是 null,
   保持 null —— 别顺手都填上。
3. **`chapter` 绑定被写死成 null**:`js-host/host_env.rs` 里「本套永远是 null
   (用例不带章节实体)」那行**差分套的注释漏进了产品**。新增 `ChapterBinding`,
   而且是**活的** —— 真身绑的是实体本身,目录循环里 `chapterUrl` 的 JS 读得到
   刚算出来的 `chapter.title`,故每改一次重绑一次。
4. **`UrlArgs.rule_host` 一直是 None**:`org.jsoup.Jsoup.parse(…)` 是 Rhino 的
   LiveConnect **全局面**。M2f 只在 js-host 套的 `op: "analyzeUrl"` 那条路上注入过,
   pipeline 这条路漏了 → `«no-rule-host»`。

修完(外加一处 `cache` 终态只在成功分支挂的单边差异)**2698 PASS / 79 FAIL /
6 豁免 = 96.9%**。十五套复验无回归,两侧各连跑两次逐字节相同。

**剩下 82 例的头一条值得写在这里**:**`result` 绑定的保真度**(约 25 例,
最大一堆)。真身 `bindings["result"] = result` 绑的是**对象本身**(jsoup
`Elements`/`Element`/Java String),被测侧 `JsBindings.result` 是 `Option<String>`
—— 规则值被拍平成串。于是 `result.toArray()` / `result.parentNode()` 一律
`TypeError: not a function`,而 `result[0]`、`src.match(…)[1]` 这类**在裁判侧抛、
在被测侧不抛**(Java String 的 `[0]` 是 undefined,JS 字符串的 `[0]` 是字符)。
完整清单在 `fixtures/cases/pipeline-corpus-b/README.md`。

**这一件的通用教训**(与 M2h 同族,再记一次):**「两侧都不碰的面」不是绿,
是没测**。A 层那套两侧的 JS 都是桩,绑定面从来没被读过 —— 三年也照不出
`chapter` 恒 null。**换掉一侧的桩,才知道另一侧空了多少。**
- **CI 的格式与警告闸**(`.github/workflows/ci.yml` 的 `rust` job):此前只跑
  `build` + `test`,不带 `-D warnings`(16 条警告)也没有 `cargo fmt --check`
  (64 个文件不是 fmt-clean),闸立不起来。现在先做一次全量格式化
  (`rust/rustfmt.toml` 只写 `use_small_heuristics = "Max"` —— 仓库本来就是
  「能一行放下就一行」的写法;手排的标签表/实体表/charset 探测表带
  `#[rustfmt::skip]`),再把两道闸加上。`build` 带 `--all-targets`,
  不然 tests/examples 里的警告漏得掉。
- **分母**:`compare_jsonl.py` 的 total 从前取自两份**输出文件**的并集 ——
  两侧同时漏掉某个 case 时它静默地从分母里消失。现在 `--cases` 一律传,缺席按 FAIL 记。
- **产品侧的执行闸**:书源是用户从网上导入的第三方代码,`while(true)` 会把引擎线程
  挂死(`CancelToken` 只在步与步之间生效,救不了)。`QuickJsHost` 现在有
  `JsLimits`(缺省 30 秒 / 256 MiB,走 QuickJS 的中断钩子与内存上限,单测在
  `js-host/tests/limits.rs`);差分侧走 `JsLimits::difftest()` —— 口径上的闸在 runner
  上(5 秒,与裁判同层),宿主里那道 60 秒只是**泄漏兜底**:runner 杀不掉超时的那条
  线程,宿主自己不设闸的话它会空转到进程退出,白占一个核。

**③ 顺手修掉的语义缺口**(差分套照不到的那种)

- **`java.ajax` 没装箱**:返回类型是 Kotlin `String?`,和 `md5Encode` 走同一条
  LiveConnect 路,真身里必然是 `NativeJavaObject` —— `java.ajax(u).replace(a,b)`
  在裁判侧是 **Java 的 replace**(全部替换、字面量)。差分照不到是因为顶层完成值
  会解包,而唯一一条 ajax 用例正是裸完成值。补了 9 条装箱探针
  (`ajax` / `HMacBase64` / BaseSource 那一面),1875 → 1884 例,FAIL 31 → **30**。
- **`java.randomUUID()` 在产品侧也是定死的**:`Date`/`Math.random` 的冻结挂在
  `determinism` 上、产品为空,唯独随机源无条件走差分那条流 —— **每个用户的 UUID 都一样**。
  现在是 `HostConfig::random` 开关,缺省真随机。
- **`java_double_to_string` 有四份实现**,`xpath-compat` 那份漏了**科学计数**一支
  (`1e7` 给 `"10000000.0"`,Java 给 `"1.0E7"`)。下沉到
  `rubato_core::java_num`,四处共用,单测钉住。
- **`num()` 用 f64 而真身用 BigDecimal**:19 位整数(`1234567890123456789` →
  被测侧变 `…768`)、超 Long 范围时 Java 的 `longValue()` 截低位而 Rust 的
  `as i64` 饱和。按 BigDecimal 语义重写。
- **`xpath_on` 少了兄弟们都有的内容缓存**(`jsoup_for` / `json_for` 都有,真身的
  `getAnalyzeByXPath` 也有)—— `ruleBookInfo` 八个字段全 XPath 的源要把整页解析八遍;
  `sel_n` 每次重新 parse,而真身有静态 `PARSE_TREE_CACHE`。两处都补上。
- **清掉 `js-host/src/lib.rs` 里的 Phase 0 spike**(477 行 + `bin/s1`、`s1_corpus`):
  那份 PRELUDE 补的 `importClass` / `Packages` / `JavaImporter` 正是 M1 实测**证伪**的。

**④ 评审带出来的四处**(`SharedMap` 把「表要共享」在宿主层解决了,
但它的外沿没跟上)

- **并发搜索会互相抹掉 CacheManager**(数据丢失):`flush_js_env` 从前是
  `DELETE js:% + INSERT 全量`,而每个 worker 各自 seed 各自收工 ——
  **读-改-写没有原子性,粒度还是整个命名空间**。B 线程 seed 的时候看不见 A 刚写的
  `sourceVariable_A`,收工时就把它删了;而 searchUrl 的 `@js:` 里
  `source.setVariable(v)` 正是常见写法,搜索又正是唯一多线程的那一步。
  现在按**差量**写(`BookStore::update_cache_prefix`:只 UPSERT 变过的、
  只 DELETE 掉 JS 里 `remove` 的,一个事务),跨源不再互踩。
- **嵌套宿主没接上刚共享出来的表**:`LiveNet::nested_host` 只传了 config,
  `cache` / `cookies` 是**全新的空表** —— `java.ajax(url)` 里那条 url 的
  `@js:` / `{{}}` 求值时读到空表、写完出门就丢。中断钩子同样没传(用户点「停」,
  卡在 ajax url 里的那段 JS 要等 30 秒默认闸)。三样并成 `NestedEnv` 一起传,
  少传一样在类型上就过不去。`ReplayNet` 同形。
- **androidId 首次生成有竞态**:「查不到就生成 + `INSERT OR REPLACE`」跑在 6 个
  线程上,各自拿到不同 id。若某源在本次就 `putLoginInfo`,密文是它那个 id 加的密、
  库里存的是别人的 —— 下次 `getLoginInfo` 解不开,而那条路**静默吞异常给 null**。
  只影响首次运行,代价不可逆。现在 `Engine::open` 里就定死。
- **`PARSE_TREE_CACHE` 无上限**:注释说「键来自书源规则,数量天然有界」,但走到这里的
  是 `makeUpRule` **插值之后**的串 —— `//div[@id='{{key}}']` 每换一次关键词就是一个
  新键,而 worker 线程是长命的。真身那份也无界(JsoupXpath 的老问题),
  但没有义务连这个一起复刻:256 条上限 + 最久未用淘汰,判据不受影响。

**留着没做、写在这里免得忘**

- **`SharedJsScope` 的语义**:真身 `getShareScope` 在**同一段 JS 被调用超过 16 次后
  复用 topScope**,全局从此跨次求值持久化;被测侧每次都是全新 Context。
  这是**语义**差异不是能力差异,接 B 层前要先拿探针钉住(见
  `fixtures/cases/js-host/README.md`「判据自己会骗人的地方」)。
  **`evalJS` 每次建 Runtime + 跑四段 prelude 的开销也压在这一件上**:release 实测
  一次求值 0.97 ms,其中 Runtime+Context 只占 0.09 ms,剩下的都是 prelude ——
  想省它就得复用 Context,而那正好把上面那条语义一起改了。所以**先钉语义,再谈性能**。
- **`AnalyzeRule.evalJS` 的重入**:真身能递归(`java.getString('<js>…')`),
  被测侧的 `NoHost` 报错。本套还没有用例踩到,接 B 层时会变成真路径。
- ~~**`net::client::DEFAULT_UA` 是 `"rubato-judge"`**:差分口径直接当成了产品的默认 UA。~~
  **2026-08-31 已还**:拆成两个值 —— 产品用 `net::client` 的 `PRODUCT_UA`
  (照真身 `AppConfig.getPrefUserAgent()` 的形状:`Mozilla/5.0 (Windows NT 10.0; Win64;
  x64) … Chrome/… Safari/537.36`;真身的版本号来自它自己的 `BuildConfig.Cronet_Main_Version`,
  复刻不了也不该猜,形状照抄、版本我们钉一个),差分侧两个 runner 在 `main` 头一行
  `set_user_agent("rubato-judge")` 钉成裁判垫片的同值。
  **做成进程级单例(`OnceLock`)而不是逐层传参**:真身 `AppConfig` 本来就是单例,
  两处消费同一格(`BaseSource.getHeaderMap()` 的补 UA、okhttp 拦截器的兜底)。
  **漏钉不会静默**:UA 进 HTTP 快照的 key,实测把那一句去掉,`pipeline` 那套
  当场 2 PASS / 86 FAIL、全是 `snapshot_miss`。
- ~~**`java.htmlFormat`**:被测侧那份在 `pipeline` crate 里,js-host 够不着,
  要先把 `html_formatter` 下沉。~~ **M2p 已还清**:下沉成 `html-format` crate,
  pipeline 与 js-host 共用一份。
- ~~**JS 的 `cookie` 绑定与 HTTP 层的 `CookieStore` 是两张表**:真身那里 `cookie`
  绑定就是 `CookieStore` 单例,okhttp 的 CookieJar 写的就是它读的那一张。
  这边 `JsEnv::js_cookies` 与传给 `LiveNet` 的 store 互不相通,于是
  `cookie.getCookie(url)` 拿不到 HTTP 层刚存下的 cookie、`setCookie` 也影响不了
  后续请求 —— `enabledCookieJar` + 登录那一类源整条链断在这。差分套照不到
  (那边不联网,只走「读到空串」)。~~ **已还清**:`QuickJsHost::cookie_store`
  接 `CookieEnv`,产品侧(`engine/src/shared.rs`)两处建宿主都挂 `SharedCookies`
  包着的真 `CookieStore`;`JsEnv::js_cookies` 那张 `SharedMap` 退化成
  **js-host 差分套的老口径**(没有 `cookie_store` 时才用)。
- ~~**每个源新建一个 reqwest blocking Client**:`JsEnv::new` 里一个,而 `JsEnv` 是
  **每源**建一次 —— reqwest 0.12 的 `blocking::Client` 每个都带一条 current-thread
  runtime 的后台线程,一轮 1704 源就是 1704 次建/拆,且 JS 网络面每源都从冷连接
  开始。~~ **已还清**(随 `HttpTransport::execute_hop` 改 `&self` 一起):
  `JsEnv` 收的是 engine 开库时建的那一份 `SharedTransport`,与 pipeline 同一个。
- ~~**`__java.sealProxy` 没接上**:`els_proxy` / `el_proxy` 建完的代理对象,
  `size`/`text`/`attr`/`select`/`toString` 都是**可枚举**的,而真身的
  NativeJavaList `for (var i in els)` 只给下标。~~ **M2j 已还清**(随
  `result`/`src` 绑定走真对象一起):`host_env.rs` 三处 proxy 出口都过
  `seal_proxy`,`for..in` 只给下标。

**下一件是 pipeline-corpus 扩 B 层**(收口判据:B 层 ≥80%)。

### Phase 3(持续)— C 层长尾

PlatformHooks 落地(webView 过盾、验证码弹窗、loginUi)、QueryTTF 字体反混淆、importScript/cacheFile。
**判据:指定清单源逐个验收,不设总 pass 率。**

**M3a(2026-08-31):先把分母落库 —— 能力清单 + 分层口径修正**。
「指定清单源逐个验收」得先有那份清单,而且要能 review、能被下一轮重算。
新 `tools/phase3_caps.py`(能力探测,**分层与清单共用的唯一口径**)+
`tools/gen_phase3_checklist.py` → `fixtures/phase3/checklist.json`,
口径与六条验收线写在 `fixtures/phase3/README.md`。

1. **语料把这一期的清单重排了**(1704 源,按值判定):
   webView 选项 **98 源** / `startBrowser` 11 / 登录三件套 26 /
   `getVerificationCode` 1;而 **`queryTTF`、`replaceFont`、`importScript`、
   `cacheFile`、`downloadFile`、`getZipStringContent`、`@webjs:`、
   `ruleContent.webJs`、`java.webView(...)` —— 一次调用都没有(0 源)**。
   本节开头那句把 QueryTTF 与 importScript/cacheFile 跟 webView 并列,
   按语料它们**没有分母**:移植了也没有任何一个源能验收它。故排到最后,
   动手前先回答「判据从哪来」(见 `fixtures/phase3/README.md`)。
   §5 风险 5 里 QueryTTF 的「工作量风险而非可行性风险」那句仍成立,
   但它现在多了一条前置:**先有判据**。
2. **分层口径是个 bug,C 层此前被高估近一倍**。老 `classify`(`extract_js.py` 与
   `gen_pipeline_corpus_cases.py` 各抄一份)拿**整份书源的 JSON dump** 找裸子串,
   而 `ContentRule` 默认就带 `"webJs": ""` 与 `"sourceRegex": ""` 两个**空键**
   —— **92 个源**(29 个本该 A、63 个本该 B)被这两个空键踢进 C 层、
   **整源缺席 A/B 两套流水线差分**。改成按值判定后
   **A 879→908 / B 622→684 / C 203→112**,两套差分扩到 A **4027** 例 /
   B **3040** 例(十六套合计 99633 → **100023**)。
3. **扩进来的那 63 个 B 层源当场照出四件**,当日全部结账(见下)。

**M3a 顺带还清的四件**(都是被误分层挡了三年、没被测到的):

- **裁判 harness 的请求记账串台**(19 个 case 的假 FAIL)。被判 `timeout` 的
  case,`future.cancel(true)` 杀不掉它那条线程 —— 中断只在阻塞点生效,而
  `nextContentUrl` 从 `baseUrl` 自增页码的书源(**回落页每张都带「下一页」**,
  `BookContent` 的 `while (nextUrl.isNotEmpty() && !nextUrlList.contains(nextUrl))`
  于是永不收敛)一次都不检查中断。那条僵尸线程继续发请求,把 hop 记到
  **后面 25 个 case 的账上**。修法:`ReplayHttp` 每个 case 换一份新表并绑在
  跑它的线程上(`newCase()`)—— 僵尸写进它自己那份旧表、没人读;
  `mapAsync` 的并发翻页工作线程没绑,回落到当前那份,正是它该记的地方。
  两个 harness 同改。
- **fancy-regex 的回溯预算对整页输入太小**。默认 1_000_000 是给「短输入 + 病态
  正则」定的,而规则层的输入是**整张网页**:420 KB 的回落页上,一条**没有病态
  回溯**、只是从头扫到尾的书源正则(`(?<!\d,\h)"name":"…"` 那一族)就要
  1~4 M 步,实测 136 ms 后报 `Max limit for backtracking count exceeded`;
  而 **Java 的 Matcher 根本没有这种预算**,同一条正则在裁判侧顺利扫完、报
  「没匹配到」。于是同一步一个 `toc_empty`、一个 `pipeline_error`(3 个 case)。
  改成 `1 << 26`(≈6700 万,按实测 ~8 步/字符够扫 8 MB 的页面),
  病态回溯时它仍是兜底刹车(~30 ms/百万步,最坏约 2 s)。
- **jayway 读 Rhino 对象树的渲染口径**。`<js>` 交回来的是 NativeObject/NativeArray,
  jayway 读得动(Rhino 的 NativeObject 实现了 `java.util.Map`),但读出来的
  **还是 Rhino 的对象** —— `getString` 那一步的 `toString()` 是
  `[object Object]`,不是 json-smart 的 `{k=v, k2=v2}`。新
  `json-compat::ModelFlavor::Rhino`(`AnalyzeByJSonPath::from_native`),
  `new_json` 对 `RuleValue::Native` 走它。
- **LiveConnect 加解密面未接**(记了账,**没修**)。书源用
  `new JavaImporter(Packages.javax.crypto, …)` 直接拿 JDK 的 `Cipher` /
  `SecretKeySpec` / `IvParameterSpec`(外加 `Packages.android.util.Base64`)
  自己做 DESede/AES,而我们只移植了 `java.aesBase64DecodeToString` 那组宿主方法。
  被测侧抛 ReferenceError、被书源自己的 `catch(e){result}` 吞掉 → 返回原文;
  裁判真跑了一遍解密 → 空值。语料实测 **`javax.crypto` 28 源 / `JavaImporter`
  29 源 / `Packages.android.util` 21 源** —— 这个量级**压过本期清单里除 webView
  之外的每一条**,是下一件该做的 js-host 活。在接上之前进第三档豁免(欠账,
  **只在 B 层给**:top100 的判据是按源算且一直 0 豁免,那边就让它以 FAIL 的
  形式出现),做完一起删。
  **（M3q 后记:这条豁免比它的理由活得久 —— 面其实 M2e 就接上了,72 例里真在
  分岔的只有 1 例。见下面的 M3q。）**

**M3b(2026-08-31):webView 的两半 —— 策略那一半进了差分**。
§3 那句「BackstageWebView 不进自动差分(Phase 3 手工清单)」对 **WebView 本身**
成立,对**它外面那层策略**不成立:
`BackstageWebView.kt` 那 394 行里,WebView 之外全是策略 —— 默认 JS 是
`document.documentElement.outerHTML`;`javaScript == null && delayTime == 0` 时
delay 补 900 ms;`onPageFinished` 之后 `100L + delayTime` 起跑;取不到结果按
`200/400/600/800/1000` 的梯子重试、`retry > 30` 报「js执行超时」;结果过
`unescapeJson` 再剥掉首尾引号;跟过重定向就把 `StrResponse` 包成带
`priorResponse(302)` 的那种(`res.url` 与 `isRedirect` 书源读得到);
`sourceRegex`/`overrideUrlRegex` 非空时换 `SnifferWebClient`,回的是**命中的
资源 URL 本身**而不是页面;页面加载完把 `CookieManager` 的 cookie 抄进
`CookieStore`。**这一半已经进了差分**,进不去的只有「真的去加载一个页面」
那一半 —— 那一半才是手工清单的分母。

做法(判据 `fixtures/cases/webview/README.md`,新的第十七套 **87 例 0 FAIL**):

1. **第三个 harness `:wvharness`**,挂**真身那 394 行**。为什么非得再开一个模块:
   `:harness` 与 `:jsharness` 里的 `BackstageWebView` 都是**确定性桩**
   (回传 javaScript 原文),而 fetch / analyze-url / rule-engine / js-host
   四套的判据全建在那个契约上;同一个 classpath 上这个类只能有一份。
   挂真身的还有 `WebViewRequestConfig`(UA 与请求头的取法本身就是判据面)
   与 `StrResponse`(`url()` 走 networkResponse / priorResponse 的取法);
   `CookieStore` 只做**录调用**的垫片 —— 二级域名归一那套语义 fetch 套已经
   逐字节钉过,再挂一遍真身只是把同一件事判两次。
2. **真 WebView 换成剧本 + 虚拟时钟**。剧本 = case 里写清「加载这一页会依次
   发生什么」(`shouldOverrideUrlLoading`* → `onLoadResource`* →
   `onPageFinished`,外加每次 `evaluateJavascript` 的回值);
   `Handler.postDelayed` 与「加载完成」全排进虚拟时钟,**不真的睡**。
   于是 `900 + 100`、重试梯子、`retry > 30`、60 s 超时**逐毫秒可比**,
   87 例跑完不到一秒 —— 真睡一遍要几分钟且不稳。
3. **移植版不需要时钟**:排期算在策略层(`net::webview` 自己推 `now`),
   平台只负责「等到那一刻」。这条线正好把接口面切干净 ——
   `rubato-core::host::WebViewHost` 只剩**加载 / 求值 / 等 / 还**四件原语,
   凡是能算的都留在可差分的这一侧。产品侧(Android headless webview /
   桌面可见窗口)实现这四件就行,不碰任何策略。
4. **写用例时才看清的两处**:`javaScript` 是**空串**时 `javaScript == null`
   为假 → **不补 900**,但 `getJs()` 仍回默认那句 —— 两条分支由两个不同的
   判断决定;嗅探客户端跑 JS 用的是 `loadUrl("javascript:…")` 而**不是**
   `evaluateJavascript`,**结果根本不收**,所以 `sourceRegex` 没命中的源
   只能等到超时。

**M3c(2026-08-31):把它接上引擎 —— 三个入口的桩一起拆掉**。
M3b 之后 `net::webview` 还只有 webview 那一套用;引擎里真正会走 webView 的三处
仍是 Phase 1 的确定性桩(与裁判侧的桩**成对**,所以两侧只能一起换):

| 入口 | 真身 | 从前两侧的桩 |
|---|---|---|
| `{"webView": true}` 的书源 | `AnalyzeUrl.executeStrRequest`(L464-500) | 回传 `javaScript` 原文 |
| `@webjs:` 规则 | `AnalyzeRule.getWebJsResult`(L179-196) | 同上(`#nullbody` 指令另给一档) |
| `java.webView` / `webViewGetSource` / `webViewGetOverrideUrl` | JsExtensions.kt L245-328 | 抛「未接」标记 |

做法:

1. **裁判侧**:`:harness` 与 `:jsharness` 也按原路径挂 `BackstageWebView.kt`
   (外加 `WebViewRequestConfig.kt` —— UA 与转发头的取法是判据面),
   剧本 WebView 与 `:wvharness` 同契约。**时钟换了个推法**:那两边的入口
   (`AnalyzeUrl.getStrResponse()` / `AnalyzeRule.getWebJsResult()` /
   `JsExtensions.webView()`)全在 `runBlocking { … }` 里 —— 协程一挂起就没人再喂
   那个事件循环,`:wvharness` 那种「从外面 drain」在这里直接死锁。于是改成
   **内联泵**(`harness.WvStage` / `jsharness.WvStage`):入队即就地按虚拟时间
   跑完,回调在 `suspendCancellableCoroutine` 那个块**返回之前**就 resume,
   协程根本不挂起。`withTimeout` 的那个数由剧本 WebView 顺着
   `HtmlWebViewClient` 的 `this$0` **反射**读回来(`AnalyzeUrl` 那条是 60000,
   `getWebJsResult` 是 10000,差三倍 —— 写死一个会让重试梯子在其中一条上跑错)。
2. **被测侧**:`AnalyzeUrl::get_str_response` 多一个 `Option<&mut dyn WebViewHost>`;
   `HostEnv::web_js` 改成带 `base_url` / `content` 的 `WebJsRequest`,书源那半边
   (`headerMap` / `tag`)由宿主的网络面补;`NetProvider` 添 `web_view`
   (三个 JsExtensions 入口只差哪一个正则非空)。产品侧一律传 `None` ——
   PlatformHooks 落地前 `{"webView": true}` 的书源报 `WebViewUnsupported`,
   与从前「回传 js 原文」的假成功相比是**往诚实里改**。
3. **口径变化写进各套 README**:case 可以带一个 `webview` 剧本;**不带就是缺省
   剧本** —— 页面加载得完、每次求值都回 `"null"`,于是重试梯子跑满、报
   「js执行超时」。那是「这一页录不下来」的诚实结局,不是假装取到了内容。
   语料里 `@webjs:` **0 源**、pipeline 两套的 webView **0 例**,受影响的只有
   fetch / rule-engine / js-host 三套里手写的那些;`@webjs:` 那 139 例的剧本
   钉成「把那段 js 源码本身回给你」——与从前那个桩逐字同口径,下游
   (`fromJsonArray` / `##替换` / 四个 mode)一条覆盖没丢。
4. **顺手补上的一处单边差异**:嗅探命中(`onLoadResource` 出结果)之后真身
   `WebViewPool.release` 会 `stopLoading()` 并换掉 `webViewClient` ——
   后面的 `onPageFinished` 根本送不到 BackstageWebView 手上,**cookie 也就不抄**。
   `:wvharness` 的剧本此前照送(87 例没踩到,是因为生成器恰好没写「命中 + tag」
   那一组),而被测侧是命中即返回。三份剧本现在都按「还回去就不再送事件」走,
   webview 套新增两例专钉这一条。

全量 **17 套 100129 例 0 FAIL**(命中面 A 98% / B 89% / top100 94%,与 M3b 持平
—— 语料里那 98 个 webView 源本来就到不了内容,现在只是**报错报得诚实**)。

**M3d(2026-08-31):PlatformHooks 过 FFI 到 Dart —— 那四件原语真的接上了**。
M3c 之后引擎里三处都会走 `net::webview`,但**产品侧一律传 `None`**
(`webview_unsupported`)。M3d 把平台那一半接上,分三段:

1. **桥**(`rust/crates/ffi/src/platform.rs`,能在 workspace 里跑单测):
   `WebViewHost` 的调用编成 JSON 从一条 FRB 流出去,取值的两件
   (`eval` / `page_cookie`)在条件变量上等 Dart 用 `#[frb(sync)]` 的
   `web_view_reply` 交回来;WebViewClient 的三个回调由 `web_view_event`
   送上来进队列。**只有取值的两件要回复** —— `create` / `load` /
   `eval_void` / `destroy` 在真身那边本来就是异步的,发了就走(流有序,
   Dart 侧按 session 串行,顺序不丢)。
2. **供给方**(`rubato_core::host::WebViewProvider`,`available` 与 `acquire`
   分开):Dart 登记那条流之前一个都要不到,engine 的四处 `PipelineEnv`
   与 `js_net` 据此决定给不给 host ——**要不到就还是 `WebViewUnsupported`**,
   而不是给一个必然超时的壳。`Engine::open_with` 注入,`ffi::init_engine` 开库时给。
3. **Dart 那一半**(`app/lib/platform/web_view.dart`,几十行):
   flutter_inappwebview 的 `HeadlessInAppWebView`,只做加载 / 求值 / 还。

**三处口径是这一段的全部含金量**:

- **虚拟原点是「页面加载完成那一刻」**,不是 `loadUrl` 那一刻。策略层的时钟
  从 0 起算且事件不占虚拟时间,而真身 `postDelayed(100 + delayTime)` 的基准是
  `onPageFinished` —— 差分侧两种锚法等价(事件都在虚拟时刻 0),产品侧只有
  后者对得上真身:锚错了页面加载耗掉的那几秒会把重试梯子挤成瞬间跑完。
- **`eval` 交回的是「原样串」**:Android 的 `evaluateJavascript` 给的是 JSON
  编码后的值,而插件替我们解了一层 —— Dart 侧 `jsonEncode` 编回去,
  策略层的 `unescapeJson` + 剥首尾引号要的正是那个串。
- **等待有上限**:`next_event` 最多等 60 000 ms(真身 `withTimeout` 的缺省),
  等不到就 `None` → `WvError::Timeout`。已知偏差:`@webjs:` 那条路真身的预算是
  10 000 ms,而平台原语面里没有这一位(`WebViewSettings` 是差分判据面,不能为它
  加字段),于是那条路的超时**晚到**,报的错仍是同一档(语料里 `@webjs:` 0 源)。

判据:① `ffi::platform` 六条单测(挂号/回复/掉线唤醒/事件次序与重锚/
`destroy` 幂等 + `Drop` 补发);② **端到端**
`app/integration_test/webview_test.dart` —— 本进程起一个 HttpServer 供一张
**书名由 JS 写进 DOM** 的页(静态抓取拿不到),一个
`,{"webView":true}` 的书源整条走通并出书(macOS 实测通过);
③ 十七套差分**一例不动**(策略层没碰,`web_view` 仍由调用方给)。

**记在这里的两处欠账**:① 取消进不到 webView 里 —— 用户点「停」时,卡在
`next_event` 上的那条 worker 线程要等到 60 秒的上限才回来(取消是**步间粒度**,
而这一步现在可以很长);`WebViewHost` 面上没有取消这一位,补它要么加原语、
要么把令牌交给 `acquire`。② 桌面发行版的 `MACOSX_DEPLOYMENT_TARGET`
(见 §4「macOS 构建」的本地补丁 ②)。

**M3e(2026-08-31):逐源验收探针 —— 判据那条线自己跑起来**。
Phase 3 的判据是「指定清单源逐个验收,不设总 pass 率」:M3a 落了分母
(`checklist.json`),M3b/M3c/M3d 把 webView 从策略到平台接通,**这一件是把
「逐个验收」这个动作做成可重复的**。三段(口径写在 `fixtures/phase3/README.md`):

```bash
tools/gen_phase3_checklist.py                    # 分母
tools/phase3_probe.sh --from=0 --limit=10        # 逐源跑真实抓取
tools/phase3_report.py tools/out/phase3-*.log    # 按真因分堆(多份日志一起喂)
```

- `tools/phase3_plan.py` 把「清单 ∩ 语料」摊成 plan.json;
- `tools/phase3_probe.sh` 起一个**本地 http 服务**把 plan 递给 app ——
  **沙箱读不到仓库的文件**(macOS 实测:stat 得到、`open` 不得,真机更是两台
  机器),走 http 之后**同一份探针真机也能跑**(`--host=<局域网 IP>`);
- `app/integration_test/phase3_probe_test.dart` **一次只启用一个源**
  (搜索是「跑遍所有启用的源」),照 `steps` 里最深那一步跑
  搜索 → 加书架(详情 + 目录)→ 正文,每源打一行 `PHASE3 {json}`(每步的结果、
  **错误原串**、耗时)。

**它不打分**,报表也不打:失败有三族完全不同的因 —— **过盾没过**
(`webview:js_timeout` / `webview:timeout`,这一族才是 Phase 3 的正题)、
**站点没了/拒了**(DNS、连不上、4xx/5xx)、**规则不匹配**(`toc_empty` /
`content_empty`)。混成一个 pass 率就等于把「站点三年前就关了」读成「过盾失败」。
所以报表按族分堆 + 打原串,人逐条看;全量落 `fixtures/phase3/acceptance.json`
(生成物,不入库)。

**头一遍(macOS,98 源,45 分钟)照出三件,当轮全部结账**:

1. **形状就是速度**。头一版「一次亮一个源」串行跑,死站点的超时是**加起来**的。
   改成**三步都在池子里并发**(`--pool`,缺省 12)之后,100 个源
   **45 分钟 → 约 3 分钟**(搜索 150 s + 目录/正文 21 s)。
   **它仍然不是开发回路** —— 改代码跑离线判据(`all_diff.sh` +
   `webview_test.dart`),盯一个源用 `--only=`。
2. **`--step-timeout` 必须大于 webView 自己的预算(60 s)**,否则探针的超时
   **盖住**真因:那一遍图快调到 45 s,15 个源本该报 `webview:timeout`
   却全落进「探针超时」。缺省 90 s 就是为这个。
3. **「search 0 命中」不等于「站点没书」** —— `Engine::search` **逐源吞异常**
   (产品该有的行为:一个源炸了不该让整次搜索失败),代价是书源当场炸了与
   站点真没结果同形,65 个源落在这一堆、**全是「还没查明」**。
   于是引擎多了一个诊断口:**`Engine::search_source`(FRB `searchOne`)——
   一次一个源、失败原样交回**,并发交给调用方;将来书源调试页要的也是它。
   探针换上它之后,那一堆从 65 落到 40,剩下的分成了
   「网络发不出去」21 / **「webView 超时」15** / 「书源 JS 出错」2 /
   「源根本没有搜索规则」2 —— **webView 那一族这才第一次看得见**。

**这条路验收不了的一族**:探针只有「搜索」一个入口,而有的源根本没有搜索规则
(webView 踩在目录/正文上)。报表单列它,不混进失败堆 —— 要验它们得另给入口
(已知的 bookUrl 或发现页)。

**M3f(2026-08-31):把那份报告逐条判了 —— `webview:timeout` 那 15 个,
过盾失败一个都没有**。

- **12 个是我们这一侧的 bug,当场修掉**:真身跑在 Android 的 WebView 上,
  连不上 / 解不出的页面**照样**走完 `onReceivedError` → `onPageFinished`
  (**错误页**),策略层于是当场求值、当场有结论;而 WKWebView(macOS/iOS)
  只发 `didFailProvisionalNavigation`、**没有 didFinish**,平台那一侧
  (`app/lib/platform/web_view.dart`)从前只接 `onLoadStop` —— 于是
  `net::webview` 每次都等满 60 s 的 `withTimeout`,**站点连不上被读成过盾超时**。
  补法:接上 `onReceivedError`(**只认主框架**),**等一个宽限期**、真的
  `onLoadStop` 没来才补发 pageFinished —— Android 上仍然是真身那一下先到,
  真的那一路一个字节不改,这一侧因此不必分平台。
  判据是 `webview_test.dart` 新增的那条(打一个**关着的本机端口**,不联外网:
  从前 60 s 超时,现在 1 s 内有结论)。**Rust 一行没动,十七套差分不受影响**。
- **剩下 3 个是站点连 SYN 都没人回**(`m.121du.net` / `m.leanclo.com` /
  `m.yunhai9.com`,curl 同样连不上)——真身在那儿同样只能等到超时。
- **修完之后那 12 个落进「0 命中」**,这是真身的行为(错误页也是页面),
  于是**报表这一侧另探一次站点**:没走到底的源逐个 curl 书源自己的地址,
  连不上打 `✗`、回 4xx/5xx 打 `⚠`(`--no-net` 关掉)。
  **别拿 python 的 `urllib` 探** —— macOS 上它没有系统根证书,几乎每个 https
  站点都报 `CERTIFICATE_VERIFY_FAILED`,头一版就这么把 40 个活着的站点判成
  「连不上」;curl 用系统信任库,与浏览器同一套。**判据本身会骗人,这是第三次。**

**判下来的账**(98 个源):走到底 18、站点从这台机器连不上 27、站点回 4xx/5xx 11、
这条路验收不了 2(源没有搜索规则),**过盾失败 0**。
**还没判明的是「站点回 2xx 而搜索 0 命中」那一族**:里头既有真的没这本书的,
也有 `Redirecting…` 那种**要执行完才跳走的广告墙**(`m.quge9.cc`、
`www.qqduw.com`、`m.bqg227.com`)—— 现在这条路**看不见 webView 到底把哪张页
交了回来**。判它得再开一个「把这一步抓回来的页面原样交出」的诊断口
(书源调试页要的也是它),那是下一件。

**M3g(2026-08-31):第二个诊断口 —— 把搜索那一步抓回来的页面原样交出**,
「0 命中」那一堆这才判得动。

`Engine::search_source_page` → `ffi::search_source_page` → FRB `searchOnePage`,
与搜索**共用同一条抓取路径**(`pipeline::search_book` 拆出 `search_fetch`,
两条路都走它 —— **分成两份实现就会出现「诊断说抓到了、搜索却没有」**)。
探针对每个 0 命中的源再抓一次,只留一句话摘要(长度、`<title>`、正文头 120 字、
最终 url),报表打在那一行下面;页面本身**不截断地**交给调用方,留多少是它的事。

一句话就把 49 个「0 命中」拆开了:**39 字节且没标题** = webView 的空文档
(`<html><head></head><body></body></html>`,页面根本没加载上,与「站点连不上」
同一件事);`Redirecting…` / `请稍候…正在进行安全验证` / `Just a moment…` = **盾**;
`域名可以转让` / `This domain is for sale` / `404` / `The region has been denied`
= 站点没了或拒了(书源过期);**真的结果页**才轮到问「规则为什么没选中」——
`基友书屋` 那条查下来是容器在、里面空的(站点真没这本书),不是我们的缺口。

**判下来:webView 这条线上,我们这一侧的过盾失败一个都没有。** 余下十来个是
**站点的盾**(Cloudflare / 安全验证 / 广告过渡页)—— 真身的排期我们逐字节照搬
(第十七套 89 例钉着),过不过取决于网络环境与书源自己的 `delayTime`/`sourceRegex`,
**那一族要上真机才判得了**。

**M3h(2026-08-31):上真机跑了一遍 —— 照出一处真的移植缺口(求值排期没被重排)**。

真机(HITV205N / Android 11)与 macOS 各跑一遍同一份清单,**47 / 98 个源结论不同**。
两件事情因此看清了:

- **macOS 那一遍的「站点连不上」有一半是这台机器的出口**:同样的源在手机上
  出书(`笔趣阁8.net` 50 命中、`笔墨看书` 50 命中、`115文学` 15 命中、
  `笔趣阁③` 24 命中×2、`乐乐小说` 1 命中)。**验收要两边都跑**,一台机器的
  网络不是判据。
- **`m.bqg225.com` 在 macOS 上出书、在真机上只拿回 538 字节的 `<head>`** ——
  顺着这条查到真身 `HtmlWebViewClient.onPageFinished` 那三行:
  `setCookie(url)` → `result?.let { evaluateJavascript("window.result = …") }` →
  **`mHandler.removeCallbacks(runnable)` + `postDelayed(runnable, 100 + delayTime)`**。
  **每来一次加载完成,还没跑的求值就被撤掉、重排**;跳转站(`Redirecting…`
  那种页面自己 navigate 走)一次加载有两次 `onPageFinished`,真身取的是
  **最后一次**之后的页面。移植版此前**在第一次 `onPageFinished` 就 break 出事件循环**
  —— 于是把加载到一半的 DOM 交了上去,后面的 cookie 也不再抄。

**为什么 89 例差分没照出来**:剧本里 `pageFinished` 是个 **bool**,一次加载
只能有一次「加载完成」——这条策略两侧**从来没被比过**。
**「差分全绿」不等于「面都比过了」,只等于「比过的面一致」**(这是第三次栽在
剧本的表达力上,前两次是「命中 + tag」与「JSON 页的口味」)。

补法(两侧一起):
- **剧本加一位** `laterPageFinished: [{at, url}]`(契约见
  `fixtures/cases/webview/README.md`),三个 harness 的剧本 WebView 都按虚拟时间
  补送;
- **`WebViewHost` 的两件原语合成一件**:`next_event()` + `wait(t)` →
  **`next_event_until(t) -> Option<(时刻, 事件)>`** ——「等到某刻,期间来了事件就
  先交事件」。事件与排期必须在**同一根轴**上,否则「后来的加载完成把排期重排」
  没法表达;顺带把平台侧那个写死的 60 s 等待去掉了(时间轴原点改回
  **`loadUrl` 那一刻**,不再是页面加载完那一刻);
- **策略层重写成一条循环**(`net::webview`):每轮「等到排期那一刻,期间来了事件
  就先处理」。连带补齐三处**每次** `onPageFinished` 都做的事:抄 cookie、
  跑 `window.result = …`、嗅探客户端排一发 `LoadJsRunnable`(那边**没有**
  removeCallbacks,所以 JS 会跑好几次)。

判据:webview 套 **89 → 98 例**(九条新用例逐条钉重排:求值从 1000 挪到 1700、
三次加载完成取最后一次、`retry` 不归零、地址仍是第一次那张、重排之后越过
deadline 就超时、嗅探那边跑两次),**十七套 100138 例 0 FAIL**;
`webview_test.dart` 在 **macOS 与真机上各跑一遍**(真机那一遍顺带证明:Android 上
真的 `onPageFinished` 先到,M3f 那个 300 ms 宽限期不会抢在它前面)。

**M3i(2026-08-31):取消进 webView —— M3d 记的那条欠账还清**。

从前:用户点「停」,卡在等页面加载完的那条 worker 线程要等满 **60 秒**才回来 ——
取消在别处是「步与步之间」的粒度,而 webView 那一步本身就能有一分钟长。

补法三段(**差分一例不动**:剧本 host 的 `cancelled()` 是缺省实现、恒假):
- `rubato_core::host` 添 `CancelFn`(`Arc<dyn Fn() -> bool + Send + Sync>` ——
  等事件的 worker 与点停的 UI 不是同一条线程)、`WebViewHost::cancelled()`
  **带缺省实现**、`WebViewProvider::acquire(cancel)`;
- `net::webview` 那条循环每一轮问一次,真了就 `destroy()` + `WvError::Cancelled`
  (真身那边对应的是协程被取消、`invokeOnCancellation` 把 WebView 还回池子);
- `ffi::platform` 的等待**切成 100 ms 一片**:令牌是别的线程置位的,条件变量
  这边没有它的通知口 —— 不分片就看不见。引擎侧 `Engine::web_view(cancel)` 与
  `js_net::live_net` 把本次调用的令牌交下去(`CancelToken::as_fn()`)。

判据:`net` 与 `ffi` 各一条单测(策略层:取消当场收摊**且把 WebView 还回去**;
平台层:等 60 秒的调用在令牌置位后 120 ms 就回来),十七套 100138 例仍 0 FAIL。
**没动的一处**:`eval`/`page_cookie` 那两件仍是 30 秒兜底(它们是毫秒级的,
只有「Dart 那侧死了」才会等满)——真正的一分钟在等事件那里。

**M3j(2026-08-31):「请用户出手」那三件的策略层进了差分** ——
`java.startBrowser` / `startBrowserAwait` / `getVerificationCode`。

**为什么是它们**:M3h 之后剩下的那一族是**站点的盾**(Cloudflare /
安全验证 / 广告过渡页),我们过不去。而 legado 对这一族的答案就在语料里 ——
`startBrowserAwait` 16 处,写法几乎只有一种:

```js
if (result.match(/Just a moment/)) {      // 盾拦下来了
    cookie.removeCookie(source.bookSourceUrl)
    java.startBrowserAwait(url, "验证")     // 弹内置浏览器,人工过盾
    result = java.ajax(url)                // 过完再抓一次(带上新 cookie)
}
```

也就是说:**盾那一族的正解不是「过得更巧」,是「让用户点一下」**。

底下是 `SourceVerificationHelp` 那 300 行,与 `BackstageWebView` 同类 ——
**界面之外全是策略**:挂号表、界面互斥锁(并发搜索六个 worker 同时撞上盾,
不锁就是六个窗口一起弹)、**同一个盾只弹一次**的飞行表(第二个进来的等它,
等到了**不用它的结果、自己重抓** —— cookie 是共享的)、轮询等待、取消、
`验证结果为空`。移植在 `net::verification`。

**两侧都换了**:裁判 `:jsharness` 从这一版起**挂真身那 300 行**(此前是 shims
里一个直接抛「未接」的桩 —— 那正是 js 套里 `startBrowserAwait` 到今天一次都
没被真的执行过的原因);界面那一头两侧都是**剧本用户**(case 的 `verify` 字段:
`{"result":…,"url":…}` 或 `{"close":true}`),裁判那边由
`jsharness.VerifyStage` 顶掉 `appCtx.startActivity<…>`。

观察面除了返回值还有 **`verifyOpens`:界面被要求弹什么**(地址/标题/哪一种/
要不要存结果/书源那三位/要不要等结果)—— `startBrowser` 不等结果,返回值只有
`undefined`,没有这一面就等于没比。

判据:js 套 **2053 → 2062 例**(九条:等回结果 / 用户没给地址就回退到入参 /
用户关掉界面 / 三参与四参重载 / 验证码两条 / 不等结果那条两条),
**十七套 100147 例 0 FAIL**;`net::verification` 五条单测(其中
「第二个撞上同一个盾自己重抓」那条编排是确定的 —— 界面等到第二个真的挂上
飞行表才答,不靠 sleep 猜)。

**产品侧现在是「没接界面 → 报未接」**:`VerifyUi::available()` 为假时
`java.startBrowser*` 与 `getVerificationCode` 当场报 `«net-unsupported»`,
与接进来之前逐字同一档 —— **弹不出窗还去等,就是让书源等一个永远不来的答复**。
界面那一半(Dart:内置浏览器页 + 验证码对话框)是下一件。

**M3k(2026-08-31):界面那一半接上了 —— 「可见窗口 → 用户操作完 → 回四步」
真的走得通**。

`app/lib/platform/verify.dart`(一百多行):`verifyOpen()` 那条流上来一件就弹
一个 —— **内置浏览器页**(AppBar + `InAppWebView` + 「完成」)或**验证码对话框**
(图 + 输入框)。做完调 `verifySetResult(key, url, result)`,与真身
`WebViewActivity` / `VerificationCodeActivity` 回头调
`SourceVerificationHelp.setResult` 同形。

**三处口径**(照真身,别回退):
1. **每加载完一张页就把 cookie 抄回引擎**(真身 `onPageFinished` 里那句
   `CookieStore.setCookie(url, CookieManager.getCookie(url))`)——
   **过盾靠的就是这一下**:书源随后 `java.ajax(url)` 走的是引擎自己的 cookie 表,
   不抄过来等于白过。新 FFI `verifySaveCookie` → `Engine::set_cookie`。
2. **Cloudflare 过了自己收摊**:盯 `!!window._cf_chl_opt`,true → false 且这一趟
   要存结果就当场存了关掉(真身 `isCloudflareChallenge` 那一段),不必用户再点。
3. **关掉界面 = 空结果**:退出这一页的每条路都交回一个值,策略层据此报
   「验证结果为空」而不是傻等。

**一处没照搬,理由写清**:`refetchAfterSuccess` 那一支真身在界面层用
`AnalyzeUrl(useWebView=false)` **再抓一遍**(它在 Android 上拿得到书源与 okhttp);
我们界面在 Dart、网络在 Rust,交回的是页面 DOM —— 而语料里那 16 处
`startBrowserAwait` 全是「过完自己再 `java.ajax` 一遍」的写法,拿到的是同一件东西。

判据:`app/integration_test/verify_test.dart` 两条**端到端**(不联外网,页面由
本进程 HttpServer 供):① 书源 JS 调 `startBrowserAwait` → 页面弹出来 →
**等页面加载完**(标题从「验证」换成页面自己的 `<title>`)→ 按「完成」→
DOM 交回策略层 → 书源接着选 → 出书;② `getVerificationCode` → 对话框 →
打进 `8Q2K` → 按「确定」→ **那串一路走到规则层当了书名**。
> 按的是按钮自己的 `onPressed` 而不是 `tester.tap`:内置浏览器是**平台视图**,
> 在测试环境里盖住整块画布,命中测试落不到 AppBar 上。

**M3l(2026-08-31):登录线接进产品 —— `loginUrl` / `loginUi` / `loginCheckJs`**。
清单里这条线是 26 个源(`loginUrl` 16 / `loginUi` 2 / `loginCheckJs` 9,有重叠);
`loginCheckJs` 此前已经在四步抓取里跑真 JS,这一轮补的是用户能真正完成登录的入口。

1. **引擎语义**:`BookSource` 补回此前只留在 raw JSON 的 `loginUi`;
   `Engine::source_login_spec` 按真身 `SourceLoginActivity` 分流(表单非空且不是 `[]`
   → RowUi,否则可见 WebView),动态 `<js>` / `@js:` 的 loginUi 仍走 BaseSource
   第三宿主。`source_login_action` 把当前表单以 Java Map 形态绑成 `result`,先用
   androidId 前 16 字节的 AES 口径持久化,再跑 `login()` 或按钮 action。
   普通 BaseSource 求值**不多绑** `result/book/chapter` —— 新 `source_login` 形态位
   只给登录对话框,所以原本的 ReferenceError 契约没有被悄悄放宽。
2. **补齐真实源踩到的方法面**:`番茄小说2` 会在后续规则里调
   `source.getLoginInfoMap()`。实现返回 Rhino 的 NativeJavaMap 形态(属性访问与
   `.get()` 都可用),js 套新增 `empty` / `roundtrip` 两条真 Rhino 探针,
   **2062 例 2055 PASS / 0 FAIL(豁免 7)**。
3. **可见界面**:`app/lib/pages/source_login_page.dart`。
   `loginUrl` 用 `InAppWebView`,每次开始/完成加载都按真身把浏览器 cookie 写回
   **书源 key**;`loginUi` 支持 text/password/button/label/select/toggle,
   按钮 action 仍能复用 M3j/M3k 的 `startBrowserAwait`。书源管理页只给有登录
   能力的源显示登录按钮。

判据:`app/integration_test/login_test.dart` 两条 macOS 端到端(本地 HttpServer,
不联外网):① 登录页下发 cookie → Dart 回抄 → Rust 后续搜索带上 cookie 才出书;
② RowUi 输入账号/密码 → AES 落库 → 执行 `login()` → 再读登录配置能解回同一份值。
另有 engine 两条单测(表单持久化/动作、相对 loginUrl),workspace 全测通过;
十七套 **100144 例 100065 PASS / 0 FAIL(豁免 79)**。

**这轮明确没冒充做完的面**:`loginUi` V2 / mainJs 纯 JS 源与
`SourceLoginJsExtensions.upLoginData/reLoginView` 动态改表单仍未接;当前 1704 源
的 Phase 3 清单里没有 V2/mainJs 分母,不拿自造用例宣称逐源验收完成。

**M3m(2026-09-01):非搜索入口 + `startBrowserAwait` 真机验收。**

1. **探针不再强迫所有源先搜索**:`pipeline::explore_fetch` 与
   `Engine::explore_source/explore_source_page` 共用发现抓取路径,FRB 暴露
   `exploreOne/exploreOnePage`;`phase3_plan.py` 按源摊 `entry`。webView 线现在是
   **98 search + 2 explore**,没有静态入口的动态发现会明确记 `unreachable`,不把
   JS 源码猜成 URL。两条原本永远不可达的源已跑真站点:
   `9sct` 发现出 20 本 → 目录 **6948 章**;“嘉丽美文学”发现出 20 本 →
   目录 **64 章**。两条都实际穿过“发现 → 加书架 → 目录 WebView”,不是只改报表。
2. **browser 线成为可运行入口**:`phase3_probe.sh --line=browser`,真机仍走
   `adb reverse`;探针挂 `VerifyPlatform` 并持续 pump,否则 Rust worker 在等用户时
   Flutter 路由永远画不出来。Android 平台视图会盖住测试坐标点击,故另有显式
   `--auto-verify` 只调用“完成”按钮的同一个回调;外部的 Turnstile 操作仍由用户做。
   顺手补 `pipefail`,Flutter 测试失败不会再被 `tee` 吞掉后继续生成假报告。
3. **真机结论(设备 c6bbcb77 / Android 11)**:本地判据
   `integration_test/verify_test.dart` 的 `startBrowserAwait` 与图片验证码 **2/2**;
   真实“八一中文”书源确实进入 `startBrowserAwait`、显示 Cloudflare Turnstile、
   接受点击并把页面交回书源。此设备/出口被 Cloudflare 再次挑战,最终仍是
   `Just a moment...`、搜索 0 命中 —— 这是**盾没过**,不是链路未接。
   “起点中文”的真实 cookie 验证分支也弹出并完成回传,但源站仍回 `var buid`
   探针页。另两条分别照出源站问题:`m.50zww.net` TLS 握手失败;
   `hetushu.com` 已从源规则只认的 `Just a moment...` 变成
   `Attention Required!`,所以旧规则根本不会触发浏览器。

**M3n(2026-09-01):可见浏览器按书源的 UA / 转发头加载 —— 「盾没过」那一半里
我们这侧能修的那一处。**

M3m 的真机结论是「链路接通、盾没过」。逐条看那一路:用户在可见窗口点完
Turnstile,Cloudflare 发的 `cf_clearance` 是**绑 UA 的**;而可见窗口此前用的是
系统 WebView 的缺省 UA(Android 上是设备自己那串),引擎随后 `java.ajax` 用的
是 `AppConfig.userAgent`(桌面 Chrome 140 那串)—— **两位客户端**,用户点过
照样被重新挑战。后台那半(`{"webView":true}`)早在 M3b 就照
`toWebViewRequestConfig` 把 UA 交给 WebSettings 了,**可见窗口这半一直没有**。

真身把这件事放在**界面层**(`WebViewModel.initData` + `WebViewActivity` 那三行:
Activity 拿得到书源与 okhttp);我们的界面在 Dart,于是算在宿主网络面、随
「弹界面」一起交过去。三段:

1. **算加载计划**(`js-host::net_face::browser_load` → `net::verification::BrowserLoad`):
   `AnalyzeUrl` 拆开 `url,{…}` 得 `baseUrl` 与 `headerMap`;
   `net::webview::to_web_view_request_config`(M3b 就有的那份移植)把 UA 单拎
   出来交给 WebSettings,`CookieJar` / `proxy` 是内部网络选项、**不许发给网站**;
   `isPost()` 先用 okhttp 那条路(`useWebView = false`)抓一份 HTML **覆盖**入参
   那份。**出错交回 `None`** —— 真身那句 `execute {}` 走 `onError`(弹个 toast),
   界面照开、书源那侧毫不知情,不能抛成 JS 异常。
2. **过 FFI 到界面**:`VerifyOpen.browser_load` → `ffi::platform` 的 `request` 位 →
   `app/lib/platform/verify.dart` 按它 `loadUrl(url, headers)` /
   `loadDataWithBaseURL`,UA 进 `InAppWebViewSettings`(登录线 M3l 早就这么做,
   这条线此前漏了)。
3. **进差分 —— 这一面此前一位都没比过**:裁判 `jsharness.VerifyStage` 照
   **同一份真身**算(它与被测侧一样拿得到书源与 HTTP 录放),观察面新增
   `verifyOpens[].browserLoad`(地址 / UA / 转发头 / 要加载的 HTML)。
   js 套 2062 → **2066 例**,四条探针各钉一位:UA 与转发头(`CookieJar` 被挡)、
   POST 先抓(两侧各多一跳,`hops` 看得见)、`startBrowser` 那条也带计划、
   `initData` 出错 = `browserLoad: null`(书源那侧仍然只看见用户的答复)。

判据:十七套 **100148 例 0 FAIL**;`verify_test.dart` 增第三条端到端 ——
本进程 HttpServer **验收它真的收到了**书源写的 UA 与 Referer、且**没有**收到
`CookieJar`。探针那侧顺手补一位:`phase3_probe_test.dart` 打开
`shouldPropagateDevicePointerEvents` —— 不打开,真机上的触摸只会被
`LiveTestWidgetsFlutterBinding` 当成定位提示(日志里一句
`Some possible finders …`),人与 `adb shell input tap` 都点不动 Android 平台
视图里的 Turnstile。

**当天补上的真机验收(设备 RFCY41BD54H / SM-S9310 / Android 16 /
System WebView 151.0.7922.200 —— M3m 那次是 Android 11 + 旧 WebView)**:

- 离线判据先过:`verify_test.dart` 在真机上 **3/3**(含新增那条 UA/转发头),
  说明这条链路在 Android 上也接对了,不只是 macOS;
- **「八一中文」过了,而且出书**:`--line=browser --only=81zw2 --auto-verify`,
  内置浏览器弹出 → 盾放行 → cookie 抄回引擎 → 书源接着抓,**50 命中、
  第一条「剑来」**(20.4 s)。M3m 那次是「能弹能点能回传、随后 `java.ajax`
  又被 `Just a moment...` 拦下、搜索 0 命中」。整条 browser 线复跑(11 源)
  同一个源再次 50 命中,走的是**另一条退出路径**(45 s 后按「完成」)——
  自动收摊(`window._cf_chl_opt` 那一支)与用户按完成两条都验过了。
- **归因说清楚**:这一轮同时换了两个变量(UA 一致 + 设备/WebView 版本),
  单次实验分不开,老实记成「两者叠加之后过了」。顺带核过:我们的默认 UA
  与真身 `AppConfig.getPrefUserAgent()` 是**逐字同一串**桌面 Chrome UA,
  所以这不是「我们比 legado 更宽」。

**browser 线其余 10 个源逐条判完,没有一个是我们这侧的缺口**:站点连不上 4
(TLS 握手 / 连接超时)、站点回 4xx/5xx 3(和图书是 `Attention Required!`
被 Cloudflare 拉黑,而源规则只认 `Just a moment...`、压根不会触发浏览器)、
起点搜索页只回 209 字节、文学吧正文 0 字节但只花 2 ms(**没发请求,更像目录
第一项取到的是卷头** —— `chapterUrl: tag.strong@text||href`,是线索不是结论)。
还关掉一条 M3m 留的旧疑问:**69书吧的 `invalid regular expression flags` 不是
我们的** —— searchUrl 是 `<js>/modules/article/search.php,{…}</js>`,`/modules/`
被当成正则字面量、`article` 成了非法 flags;差分里那条(`js-url-5d706d203a`)
**两侧都报 `js:SyntaxError`**,真身 Rhino 一样跑不动这个源。

**同机复跑主线 webview(98 源)**:走到底 **16 / 98**(entry 5 / toc 9 /
content 2 —— 与 M3h 的 macOS 那遍同为 16,但**构成不同**,再次印证「一台机器
的网络不是判据」);站点连不上 25、站点回 4xx/5xx 11。剩下**归到我们这侧要
单看的只有四条**,都记在这里备查、本轮没动:`webview:timeout` **7**
(其中 `kdzw.net` ×2 与 `m.bqg119.com` 站点本身是通的 —— 页面加载得完、
嗅探正则一直没命中,重试梯子跑满 60 s;要判是「源过期」还是「我们的嗅探
少了一面」,得 `--only=` 加 `--dump` 逐张页看)、`期刊杂志` 的
`rule:Js("SyntaxError: unexpected token: ']'" … at parse (native))`、
`就去看网` 的 `TypeError: not a function`(M3h 就照出来过)、
`塔读文学` 目录为空 / `奇漫屋` 下载链接为空。

**M3o(2026-09-01):把 `webview:timeout` 那一堆判完 —— 一处真的平台缺口 +
一处报表口径错**。

**七个源,拆成两堆**(判法:先问「这台**手机**够不够得着这个站」,再问我们):

1. **五个是「手机那条出口够不着」**(`kdzw.net` ×2、`m.bqg119.com`、
   `m.yunhai9.com`、`m.121du.net`):`adb shell curl` 实测 http=000
   (DNS 解不出 / 握手失败 / 超时都有)。**站点本身没死** —— 从 Mac 全是 200,
   因为**开发机常年挂着 TUN 代理,而手机是裸网**。于是「站点是通的却 webview
   超时」看着像我们的缺口,真因是那条出口下的黑洞式超时(连 RST 都没有,
   60 秒内自然没有 `onPageFinished` —— 真身同理)。
   **两件事要分开记**:
   - **报表口径是错的,已修**:`phase3_report.py` 的 ✗/⚠ 标记从前一律**从 Mac**
     curl,拿开发机的网络给真机的结论打标记。现在 `--device=<id>` 走
     `adb shell curl` **在跑验收的那台机器上**探(`phase3_probe.sh` 自动传),
     标签也带上是谁连不上(`✗ 设备 <id> 连不上站点:…`)。同一份日志换口径重判:
     **26 → 38**。判据会骗人,这是第四次。
   - **同一个坑,第二次**:头一版的设备探测**没加 `-k`**,而 Android 自带的
     curl **没有根证书库** —— 于是几乎每个 https 都报
     `curl: (60) unable to get local issuer certificate`,活着的站点被整片判成
     「连不上」(实测 `kdzw.net` / `m.bqg119.com`:不加 `-k` 是 (60),加了是 200)。
     这与本仓库早就记着的「别拿 python 的 urllib 探」(M3f)是同一件事换了机器。
     探的是「站点还在不在」,证书可信与否是**引擎自己那条路**的事(rustls +
     webpki),不该由这一探替它下结论。
   - **标签摆正 ≠ 验收作数**:这一堆源这一轮**只能记成「没测到」**,不是
     「站点没了」,更不是「我们过不了」。
   - **「让两边走同一个出口」是个错主意,实测推翻了它**:手机挂上代理再跑一遍,
     走到底 **19 → 16** ——**四个源反而没了**(乐乐小说 / 115文学 / 笔趣阁8.net /
     连尚读书:清一色国内站,境外出口够不着或按地区拒),只换回两个
     (9sct / 笔趣阁225)。**这一族书源本来就是给国内网络看的**,拿代理出口跑
     并不更「正确」,只是把能测到的换了一批。
     > 结论写死在这里:**验收的分母跟着网络走,没有一条「正确的网络」**。
     > 报表能做的是**记下是从哪台机器探的**(`--device`);能当判据用的是
     > **同一台机器、同一条网络下的前后对比**(M3o 那个 16 → 19 就是这么来的:
     > 两遍都是手机裸网、`--pool=12`)。「走到底 N/98」这个绝对数从来不是判据
     > —— 报表自己每次都印着那句「这不是判据,判据是逐条看上面的堆」。
   - **同一份日志,三种口径**(数字差得很难看,所以口径必须写死在工具里):
     从 Mac(常年 TUN)**26** / 从手机但没 `-k`**38** / 从手机 + `-k` + 手机挂
     代理 **22**。
2. **两个是真的**(「笔趣阁③」`www.boquku.com` 两条):站点从手机 200、
   子资源(百度 CDN / Google Ads)也全都 200,却照样 60 秒超时。**一次性诊断
   脚本**(HeadlessInAppWebView 挂产品那份设置,逐条打事件)钉死了现象:
   **`onProgressChanged(100)` 在第 2 秒、`outerHTML` 已有 75 KB,而
   `onLoadStop` 90 秒里一次都没来** —— 页面的 load 事件被某个吊着的子资源
   拖住了。策略层只认 `onPageFinished`,于是等满 `withTimeout(60000)`。

**补法(平台那一半,Dart 一处)**:`web_view.dart` 加一条看门狗 ——
每 250 ms 问一次 `document.readyState`,`interactive`(DOMContentLoaded,
「DOM 齐了」)之后再等 **3 秒**宽限期,真的 `onLoadStop` 还没来才**补发**一次
`pageFinished`。形状与 M3f 那条(`onReceivedError` + 300 ms 宽限)一模一样:
真的那一下来了就照真身走,补发只发生在压根没有它的时候;策略层「每次
onPageFinished 都重排求值」(M3h)保证补早了也不会错。

- **为什么问 `readyState` 而不是听 `onProgressChanged(100)`**:头一版就是听进度,
  macOS 上那条端到端**仍然等满 60 秒** —— WKWebView 的 `estimatedProgress` 被
  吊着的子资源卡在 100 以下。`readyState` 是页面自己的状态,两个平台同一套;
  卡在 `loading` 说明是**同步脚本**吊住了解析,那时 DOM 还不全、本来就不该补发。
- **这是一处有意的超集,不是对齐**:真身在这种页上同样卡死(它也只接
  `onPageFinished`)。写在 `_readyGrace` 的注释里,免得日后被当成「对齐真身」改回去。

判据:`webview_test.dart` 增第四条(本进程 HttpServer 供一张**末尾挂着永远不回
的 iframe** 的页:DOM 齐了、load 事件吊着),**macOS 与真机各 4/4**;
Rust 一行没动,十七套差分不受影响。

**同机复跑 webview 线(98 源)**:走到底 **16 → 19**(entry 5 / toc 9→12 /
content 2,原有的一条没丢),**`webview:timeout` 那一堆清零**。
> 这一轮的 98 源真机数**只当回归看,不当验收结论**:手机裸网、开发机常年 TUN,
> 38 个源根本没走到我们这一侧。**16 → 19 是同一台机器、同一个网络下的前后对比**,
> 那一位是作数的。

**M3p(2026-09-01):方法改动 —— 驱动力从「探针」换成「对照台账」**。

**为什么改(三条都是实测,不是感觉)**:

1. **原清单的分母已经空了**。M3a 那份能力探测的结论到今天没变:webView 之外
   (`queryTTF` / `importScript` / `cacheFile` / `@webjs:` / `java.webView`)
   **一个源都没有**;而 webView 那条线 M3o 之后剩下的失败全落在**站点的盾 /
   站点没了 / 那台机器的出口够不着**。「走到底 N/98」本来就不是判据
   (同一份日志三种口径:Mac 26 / 手机没 `-k` 38 / 手机挂代理 22)。
   继续跑逐源验收,**每轮一小时、产出趋近 0** —— 「做一点吐一点」是这个循环的
   必然形状,不是手慢。
2. **三次真缺口里,两次本来是「读得出来」的**:M3h 那处求值重排明写在
   `HtmlWebViewClient.onPageFinished` 的三行里,M3n 的可见浏览器 UA 明写在
   `WebViewModel.initData` 里。真身 1159 个 kt 文件一直躺在 `judge/engine/`,
   **缺的从来不是源码**。(M3f/M3o 那两处是 WKWebView/Flutter 的平台差,
   真身没有可抄的 —— 那一类只能靠探针,但它只占三分之一。)
3. **差分守得住「比过的面」,守不住「没进剧本的那一位」**。M3h 那句
   「差分全绿 ≠ 面都比过了」到现在只是一句教训,**没有任何机制**。

**新增第三条判据:移植对照台账**(`tools/port_audit.py` + `docs/port-ledger.md`,
口径写在工具的文件头):

- **面**从真身源码里**机械枚举**(v1 三个面;M3q 加了第四个 `liveconnect`,
  它的枚举源是**语料**而不是 kt 文件 —— 理由见 M3q:`JsExtensions` 的方法面、
  `AnalyzeUrl.UrlOption` 的选项面、`BookSource` + 规则实体的字段面,
  字段面按 `实体::字段` 分行 —— `title` 在 `ExploreKind` 与 `ContentRule` 里
  各有一个,合成一行就没法分别判);
- **分母**从语料实算(按源计;`exploreUrl`/`loginUi` 那种「字符串包 JSON」要
  钻进去,不然那两族的用量恒为 0);
- **状态**是人写的六档:`移植` / `桩` / `不做` / `待做` / `未比` / `未判`;
- `--check` 拦三件:**真身有而台账没有**、台账有而真身没了、状态与证据矛盾。
  「生成物不入库」那条规则对它不适用 —— **台账是判据本身**,`--sync` 只补空行。
- 选项面的解析口径与 `phase3_caps.py` **逐源对上**(0 分歧):键可以不加引号
  (真身用 Gson 解那段尾巴,缺省宽松),一个值里可能有好几个 `,{...}`
  (`exploreUrl` 一行一个 URL)—— 两条都补齐之后 `webView` 才从 63 回到 98。

**头一遍跑下来,当日结账**:

- **`urlopt` 一个面不缺**(唯一的 `origin` 是真身只有界面读的,记 `不做` + 理由);
- **`jsext` 71 个面**:33 `移植`、38 `不做` —— `不做` 那 38 个里**有用量的只有
  `toast`/`longToast`**(砍单内的空实现,两侧同为 undefined),其余 36 个语料 0 源。
  **这一面就此关掉,不必再逐个猜**;
- **`field` 127 个面**:76 `移植`、37 `不做`(段评 `ReviewRule` 全族 + 书源扩展位
  + loginUi 表单的样式/倒计时那几位)、**14 `待做`**。

**台账第一跑就照出一件谁都没提过的**:`ExploreKind` + `FlexChildStyle` 那两族,
也就是**发现页的分类列表**(`title` / `url` / `style.layout_*`)。引擎只接
「用户点中的那条 URL」(`Engine::explore_source`),**分类 JSON 至今没人解**,
而语料里 `layout_flexGrow` 那一族是几百个源的量级(数字跑
`tools/port_audit.py --face=field`,别手抄)。真身的答案现成:
`BookSourceExtensions.exploreKinds()` + `ExploreKind`/`FlexChildStyle` + Flexbox。

> 这正是「借鉴 legado」该有的形状:**不是探针撞出来的,是对着源码枚举出来的**。

**判据改法(三条,写给未来的自己)**:

1. **「走到底 N/98」永远不作数**(分母跟着网络走,没有「正确的网络」);
   作数的只有同机同网的前后对比那一位、差分套数、以及台账。
2. **探针从「验收手段」降级为「抽样体检」**:一个批次跑一轮,不再是每轮的驱动力。
3. **webView 线按台账结案,不按探针跑绿结案。**

**排下来的活(按分母排,不按好奇心)**:

1. ~~**LiveConnect 加解密面**~~ —— **M3q 收了**,见下。
2. ~~**三条尾巴线索**~~ —— **M3r 判完了,webView 线结案**,见下。
   (`期刊杂志` 站点加壳、`奇漫屋` 源把漫画填成 `bookSourceType=3` —— 两条真身同错;
   `塔读文学` 今天走得到底。判的路上掉出两处引擎分岔:`@attr` 的 trim、
   `chapter` 的变量绑定整块没接。)
3. ~~**`loginUi` V2 / mainJs / `upLoginData`+`reLoginView`:砍**~~(M3p 已写进 §6)。
4. ~~**发现页分类列表**(台账那 14 条 `待做`)~~ —— **M3s 做完了**,见下:
   第十八套 `explore` + 产品的发现页。**台账 `--todo` 从此是空的**
   (`对账:错 0 条、欠账 0 条`)。

**M3q(2026-09-01):LiveConnect 加解密面结案 —— 拆掉的豁免比接上的面还多**

按分母排的第一件。动手前以为是「整个面没接」,拆开发现**面 M2e 就接上了**
(`js-host/src/live_connect.rs`),**只有豁免留在原地**:M3a 给 B 层挂的那条
第三档豁免写着「被测侧抛 ReferenceError 被 catch 吞掉」——那句话在写下的时候
就已经过期了。72 个挂着它的 case 里,真在分岔的只有 **1 个**。

**这一件的教训写在最前面**:

> **豁免比它的理由活得久。** 它既会把一块早就绿了的面继续盖着(72 例里 71 例
> 早就一致),也会把后来才长出来的分岔一起盖掉。**每一轮都要回头问一遍
> 「这条豁免今天还成立吗」** —— 这和 M2o 那句「豁免也会过期」是同一件事,
> 只是这次是自己撞上的。

拆开之后逐条落地的:

1. **`pb02492` 是真缺口:空输入解密该给空数组**。书源拿明文页喂
   `Base64.decode(…, 2)`,表外字符被 AOSP 的解码表全跳过 → **0 字节** → 再进
   `doFinal`。JCE 那边先判 `totalLen % blockSize`(0 过得去)、再让
   `PKCS5Padding.unpad` 对 `len == 0` 直接返回 0,于是**交回空数组**;
   被测侧抛 `IllegalBlockSizeException`。修在 `java_api::SymmetricCrypto::decrypt`。
2. **顺着这一位量出三条挨着的边界**(探针都在 js 套,别合并):
   ① LiveConnect 的 `doFinal(new byte[0])` 解密 → 空数组;
   ② 同一位**加密**要补出整整一个分组;
   ③ hutool 那条路收的是**串**:`HexUtil.decodeHex("")` 与 `Base64.decode("")`
   **都返回 null** → `doFinal(null)` 抛 `Null input buffer` → CryptoException,
   而 `" "` 这种**空白非空**的串走宽松表 → 0 字节 → 交空串。
3. **ECB 收到 `IvParameterSpec` 是错,不是忽略**:hutool 只要 iv 非空就传 params,
   ECB 的 `init` 收到 params 当场抛 `InvalidAlgorithmParameterException:
   ECB mode cannot use IV`。被测侧此前一路忽略 IV、照样算得出结果。
4. **`java.aes*` 那一族此前一条手写探针都没有** —— 只有 corpus 里几条真书源顺带
   走到,而它们**两侧都落在 catch 里**,「一致」得毫无信息量。补上 `js-hut-*`
   七条之后当场照出**第三类豁免:裁判环境差**(见下)。

**第三类豁免:裁判环境差**(既不是欠账,也不是被测侧多做)

hutool 5.8.22 的 `KeyUtil.generateKey(algorithm, key)` 在非 PBE / 非 DES 分支上把
**整条 transformation 当算法名**塞进 `SecretKeySpec`(字节码可查,没有
`getMainAlgorithm`)。于是密钥的 `getAlgorithm()` 是 `AES/CBC/PKCS5Padding` ——
**裁判跑在 JVM 上,SunJCE 的 AESCipher 查这个名字并拒收**;**真身跑在 Android 上,
Conscrypt 只查密钥长度、照跑**。语料里 **17 个源**就是这么写的:复刻裁判等于让这
17 个源在 Rubato 上也失效,而失效的理由在真身上根本不存在。

这一档**不是欠账**(还不了,让它红着只会把棘轮钉死在一个永远修不掉的数字上),
**也不是超集**(我们没多做什么)。它是**这一位在这台裁判上问不出真身的答案**。
判法与前两类一样:**先把语料数出来再判**。

> **这 6 例从前是「碰巧一致」**:被测侧那时对空输入解密抛
> `IllegalBlockSizeException`、与裁判一起以 CryptoException 收场 —— 一致,
> 但两个理由。把空输入那一位按真身修对之后,藏在下面的环境差才露出来。
> **「一致」不等于「同一个理由」**,这是「判据会骗人」那份清单上的新一条。

**代价记在明处**:`top100` 从此不再是 0 豁免 —— `[B] 读书阁` 一个源**仅因豁免支
未过**,按源判据 100/100 → **99/100**(判据线 90/100,仍在线上)。这一分记在
`fixtures/cases/top100/README.md` 里,**不许再往下滑而不写理由**。

**台账的第四个面:`liveconnect`**(18 行,13 `移植` / 5 `不做`)

它的枚举源**不是真身源码,是语料** —— Rhino 的白名单是「整个 classpath 减掉
`RhinoClassShutter` 那张黑名单」,枚举它既不可能也没意义;**能枚举的是需求侧**:
书源真的引到了哪些类。两条路都认:全限定名(`Packages.javax.crypto.Cipher`)与
`importPackage` 进来的简单名(**只在 `with (javaImport)` 里解析得到**,故只认
`with` 之后、且用在调用位的)。

**这一面一枚举出来就照出两处没人比过的**:`with` 块里真身还会把 `Object`
(java.lang)与 `Date`(java.util)也遮住 —— `Object.keys(...)` 在裁判那边是 Rhino 的
InternalError(**语料 5 个源**这么写)、`new Date().getFullYear` 是 undefined。
被测侧只遮 `String`(那条是必须的:`String(byte[])` 是解码)。判 `不做` + 豁免:
复刻裁判等于让那 5 个源在 Rubato 上也失效。

**收盘**:十七套 100160 例 **0 FAIL**;js 套 2078 → 2081 例(补十二条探针)、
pipeline-corpus-b 豁免 4 → 10(删一档欠账、露一档环境差)、top100 按源 99/100。

**M3r(2026-09-01):三条尾巴线索判完,webView 线结案 —— 判「是不是站点的事」的路上掉出两处引擎分岔**

按 M3p 定的规矩收尾:**webView 线按台账结案,不按探针跑绿结案**;剩下的三条
尾巴线索一条条判,判完这条线就收。判法是那句写在前面的话——**`--only= --dump`
先看页,别按症状猜**。

**这一件的教训写在最前面**:

> **「站点的事」这个结论也要拿证据换。** 三条线索最后有两条确实是站点/源那边
> 的事,但**判的过程**里掉出两处货真价实的引擎分岔 —— 一处是属性值两头的空白,
> 一处是整块没接的绑定。**先看页**这条规矩省下的不是时间,是两次误判。

逐条:

1. **`期刊杂志` 的 `SyntaxError: unexpected token: ']'` —— 站点侧,真身同错**。
   照着规矩把搜索页取回来看:620 字节,一个 `<script>` 往 `8.153.105.188/jlp`
   打点、一个 `<iframe>` 带着令牌参数把自己再套一层 —— 站点加了跳转壳。
   规则第一步 `result.match(/JSON.parse\('([^']+)'/)[1]` 因此拿到 null。
   **重跑时报错还换了个样子**(`TypeError: cannot read property '1' of null`,
   位置从第二段 JS 挪到第一段):同一件事在站点漂移下的两张脸 —— 这正说明
   **按症状猜会猜到两个不同的「因」上去**。真身同一条路、同一个 UA,同样拿不到。
2. **`就去看网` 的 `TypeError: not a function` —— 症状在盾后面,因不在**。
   入口确实是盾:站点回 403,搜索经 webView 拿回 Cloudflare 的拦截页,
   `hits:1 / first:"Attention"`。但**那个 TypeError 不是盾的产物** ——
   按位置(`eval_script:1:265`)拆开 `ruleToc.chapterUrl`,踩的是
   `chapter.putVariable("next", …)`:**`chapter` 上根本没有这个方法**。
   详见下面「整块没判过的面」。**同机同网的前后对比那一位**(这是探针唯一
   作数的读法):修之前 `toc: TypeError: not a function`,修之后**走到底、
   content 出结果** —— 盾还在,入口仍是 `first:"Attention"`,而那条 JS
   自己的回退分支本来就绕得过去,是我们把它拦在了绑定上。
3. **`塔读文学` 目录为空 —— 今天走得到底,规则本身两侧一致**。把真目录页
   (`/book/catalogue/955858`)取回来对着规则跑:`//*[@class="chapter clearfix"]/a`
   在**裁判与被测两侧都选中同一个 `<a>`**;重跑探针 `chapters:1`,走到底。
   那一次的空目录是内容侧的事(这本书只有一章免费章节,catalogue 页上没有
   那个块时两侧同样是空)。**但对着这张真页跑的时候露出了属性 trim 那处分岔**。
4. **`奇漫屋` 下载链接为空 —— 源自己写坏了,真身同错**。它是漫画源,
   `bookSourceType` 却填了 **3**(`BookSourceType.file`)。真身
   `BookSourceExtensions.getBookType()` 把 3 映成 `text or webFile`,于是
   `BookInfo.analyzeBookInfo` 走 `isWebFile` 那一支要 `downloadUrls` ——
   而它的 `ruleBookInfo` 里**压根没有这个键**,`BookInfo.kt` L168 当场抛
   「下载链接为空」。被测侧 `web_book.rs` L793 是同一句、同一个位置。
   **这个源在 legado 上也进不去详情页**,不是分岔。

**掉出来的第一处:`@attr` 的值,真身两头是削过的**

`JXDocument.selN` 只有 **`isString()` 那一支**过 `XValue.asString()`,而
`asString()` 的兜底是 `String.valueOf(value).trim()`(JsoupXpath 2.5.5 的字节码
可查);`isList()` 那支是逐项 `JXNode.create(item)`,**不过 asString、不 trim**。
于是同一条 `@href`:

- 命中**一个**元素 → `XValue::Str` → 两头按 **Java `trim()`**(`c <= ' '`,
  NBSP 与全角空格**不**算)削掉;
- 命中**两个** → `XValue::List` → 原样;`//@href` 这种递归属性恒走 List,也原样。
- **谓词里比的是原值**:`[@id="p"]` 选不中 `id=" p "`,`[@id=" p "]` 才选得中。

被测侧此前一律不削。塔读的目录页 `href="  /book/955858/99266714/ "` 就是这个形状
(那个源自己写了 `##\s` 把空白洗掉,所以这一处**没有**表现成它的故障 —— 它是
**顺手照出来的**,不是那条线索的因)。修在 `xpath-compat` 的 `render()`;
xpath 套补 24 条探针把上面三个岔口逐一钉住。

**掉出来的第二处:`chapter` 的变量面整块没判过**

`chapter.putVariable` 在被测侧**不存在** —— `install_chapter` 只摊字段,没有
方法。真身那里 `BookChapter` 实现 `RuleDataInterface`,而目录那一步
(`BookChapterList.kt` L241)是**逐章 `analyzeRule.setChapter(bookChapter)`**
之后才求 `chapterName` / `chapterUrl` 的,所以书源写得出来、也真有人这么写。

**为什么差分是绿的**:js 套的裁判 `host:"rule"` 从来**不 setChapter**,被测侧
照抄了这一条(`js_case_runner` 里那句注释白纸黑字写着「裁判侧同样不 setChapter」)。
于是两侧的 `chapter` 都是 null、`chapter.putVariable(...)` 两侧都抛,
**一致得毫无信息量**——又一条「判据会骗人」:

> **两侧都没绑的面,不叫比过了。** 这和 M3q 那条「一致≠同一个理由」是同一族:
> 那条是理由不同而结论相同,这条是**分母压根没建起来**。找它的办法也一样 ——
> **按真身的调用点核对绑定面**,而不是看差分的颜色。

两侧一起补:裁判 `jsharness/Main.kt` 认用例里的 `chapter` 对象并 `setChapter`,
被测 `js_case_runner` 认同一个键;js 套加 **17 条探针**(`js-ch-*`)。补出来当场
又照出两处:

1. **`putVariable` 落哪一层**:真身 `BaseBook.putVariable` / `BookChapter.putVariable`
   各写**自己**那一层,而 `java.put` 写的是优先级链的**第一层**。章节在场时
   两者不是一回事(`book.putVariable` 该进书那层,`java.put` 进章节那层)。
   被测侧此前把 `book.putVariable` 直接转接成 `java.put` —— 章节一在场就写岔。
   修法:`VarStore` 上开 `put_entity` / `get_entity` 两个按层的口,
   `RuleData` 与 `AnalyzeRule` 各转一手。两个方法都**无条件返回 `true`**
   (真身如此,不是把值还回来)。
2. **三个布尔位是两个名字**:Kotlin 的 `var isVip: Boolean` 生成的访问器就叫
   `isVip()`,Rhino 的 JavaBean 内省把**属性**认成 `vip`,而 `chapter.isVip`
   拿到的是**方法对象** —— `typeof` 是 `"function"`、**恒真**。书源写
   `if (chapter.isVip)` 会永远走进去:这是真身的样子。`isVolume` / `isPay` 同形。

**顺手修掉的一处工具静默**:`phase3_probe.sh` 的 `--dump` 收尾用 `rg` 判日志里
有没有 `PHASE3_PAGE`。这台机器上没装 `rg`,那一支于是静悄悄走进 else、报
「本轮入口均命中,没有零命中页面需要落盘」——**把该落盘的证据吞掉**,而这条路
正是「别按症状猜」要用的那份证据。换成 `grep`。

**收盘**:十七套 **100204 例 0 FAIL**(豁免 90,一条没多);xpath 套
1732 → 1756 例(属性 trim 那三个岔口 24 条)、js 套 2081 → 2098 例
(`chapter` 在场那一面 17 条)。`cargo test`、`tools/check_generated.sh` 同绿。

**收尾那一遍抽样体检**(macOS 整条 webview 线,**不是判据**,按 M3p 的规矩只当体检):
走到底 **21 / 98**(M3o 那次 19;多出来的两个正是就去看网与塔读)。没走到底里
「引擎侧真因」只剩**三条,且都判过了**:恋听网 `webview:timeout`(M3o 那一族)、
期刊杂志(站点加壳)、奇漫屋(源填错 `bookSourceType`)。剩下的分母里 **29 个站点
这台机器连不上、6 个回 4xx/5xx** —— 这也是「N/98 永远不作数」的原因本身。

**M3s(2026-09-01):发现页分类列表 —— 台账最后一族欠账还清,顺带摁住「整块没判过的面」的第二例**

台账 `--todo` 上最后剩的那 14 行(`ExploreKind::*` 8 行 + `FlexChildStyle::*` 6 行)。
真身是 `help/source/BookSourceExtensions.kt` 的 `exploreKinds()`:把书源的
`exploreUrl` 摊成发现页那一排格子。**动手前以为是「抄一个函数」,拆开发现
判据那一头得先补** —— 与 M3r 的 `chapter` 是同一族毛病:

> **裁判侧整份文件是垫片,这一面就从来没人比过。** jsharness 里
> `BookSourceExtensions.kt` 此前是 `shims/LegadoSourceShims.kt` 的两个桩
> (`getBookType` 逐字复刻 + `exploreKindsJson` 恒空),`exploreKinds()` 根本
> 不在 classpath 上。M3r 那条是「两侧都没绑」,这条是「裁判侧是桩」——
> **两种形状,同一件事:分母压根没建起来。**

**第十八套 `explore`**(入口 `tools/explore_diff.sh`,契约
`fixtures/cases/explore/README.md`):

- 裁判 `:jsharness`,**挂 `BookSourceExtensions.kt` 真身**(落盘那层 `ACache`
  换成内存垫片 `shims/ACacheShim.kt` —— 判据面要的是**冷算**);
- 被测 `pipeline::explore_kinds` + `rubato_core::entities::{ExploreKind, FlexChildStyle}`,
  执行器复用 `js_case_runner`(新 op `exploreKinds`);
- 分母 **1017 例**:手写探针 53 + 语料 960(1704 源里 977 个有 `exploreUrl`,
  按整份裁剪后的书源去重)。**与 js 套分开成一套**是因为分母不同 ——
  那套的分母是「书源里的 JS 片段」,这套是「有发现规则的书源」。

**判据一建起来就照出六处**(逐条都在探针里钉住了):

1. **gson 的无参构造那一位**:`ExploreKind` / `FlexChildStyle` 都没注册
   deserializer,走反射适配器 —— 而 Kotlin 的 data class **全部参数都有默认值**
   时会额外生成无参构造,gson 的 `ConstructorConstructor` 找得到它。于是
   **缺席的字段保留 Kotlin 默认值**(`type = "url"`、`layout_flexShrink = 1F`、
   `layout_flexBasisPercent = -1F`),不是 Unsafe 分配出来的 null / 0。
2. **非空 Kotlin 类型持有 null**:`title` 与 `type` 在 Kotlin 里是非空 `String`,
   但 gson 对**非基元**字段照样写得进 null(`{"title":null}`)—— 真身那里
   `kind.title` 就是 null,界面上任何 `title.xxx` 当场 NPE。被测侧用
   `Option<String>` 表示同一个状态(而不是悄悄落成空串)。
   **裁判侧的观察面自己先踩了这一脚**:第一版 `writeExploreKinds` 直接
   `title.startsWith(...)`,把「真身解出来的一格」误报成「裁判自己炸了」。
3. **`isJsonArray()` 只看首尾字符**:`[这不是 json]` 会进 gson 那一支再抛;
   而 `[{"title":"甲"}` 首尾不成对,反倒走了 `split` 那一支。
4. **`<js>` 那支取的是 `substring(4, lastIndexOf("<"))`** —— **最后一个 `<`**,
   不是 `</js>`。脚本里再出现 `<`(`var a = 1 < 2`)就会被截短;后面没有第二个
   `<` 时真身抛 `StringIndexOutOfBoundsException`,进 `ERROR` 那一格。
   这里**不是** `BaseSource.extractInlineJs`,别顺手抄那一份。
5. **Kotlin 的 `Regex.split` 保留首尾空片**(Java 的 `String.split` 会丢尾部
   空片):规则以换行收尾时真身实实在在多出一格空标题。
6. **`Float.toString` 与 `f32` 加宽**:`style` 那四个位是 `Float`,serde_json
   会先加宽成 f64,`0.29f` 出成 `0.28999999165534973`。这是**报表**的差不是
   语义的差 —— 两侧都出成 `Float.toString` 的形态(新 `java_float_to_string`,
   与 `java_double_to_string` 同一处)。

**顺带修的两处引擎面**(都是这一套照出来的):

- **`org.jsoup.*` 在 source 宿主上够不着**:发现规则里
  `org.jsoup.Jsoup.parse(java.ajax(url))` 是常见写法,而被测侧的 jsoup 元素是
  **句柄**、句柄表挂在 `AnalyzeRule` 上 —— 只给一层 `RuleData` 时元素面报
  `«no-rule-host»`。真身那边 `BaseSource.evalJS` 确实没有 AnalyzeRule,但
  LiveConnect 的类本来就是全局的:补的是**元素表**,不是 `java` 的反调面。
- **`«no-rule-host»` 该归 `js:TypeError`**:`rubato_core::host::NO_RULE_HOST` 的
  文档一直写着「真身在那里是 TypeError:找不到函数」,而 `error_tag` 的表里
  没有它 —— js 套碰不到,这一套一碰就露。

**产品面**(`app/lib/pages/explore_page.dart`,发现页从此有了):

书架页多一个「发现」入口 → 只列**有发现规则**的启用源(`SourceBrief.explorable`,
与真身 `isNullOrBlank()` 同口径)→ 进去是那个源的分类格子 → 点一格走
`explore_one` 出书 → 「加书架」与搜索页同一条路。格子怎么摊按
`layout_flexBasisPercent` 折算(一行几格是书源说了算,常见 0.25 → 四格),
`Wrap` 还原 FlexboxLayout 的意思。`Engine::explore_kinds` 带**进程内缓存**
(键与真身同源:书源地址 + `exploreUrl`,改了规则自动失效)—— `@js:` 那一支
可能真发请求,每开一次发现页重跑一遍既慢又多余。

**没做的三件,写在明处**:

- **表单型分类的界面**(`type` = `text`/`button`/`toggle`/`select`):解析面进了
  差分,界面只渲染 `url` 这一种。语料里 `type` / `action` 各 **1 源**,
  `chars` / `default` / `viewName` **0 源**。
- **落盘那层缓存**(真身的 `ACache`):等书源编辑页有「清缓存」那个按钮再说。
- **`infoMap` 绑定**:真身 JS 那两支多绑一个 `infoMap`(发现页适配器的
  `InfoMap`)。语料 19 条 JS 发现规则里**用到它的 0 条**。

**一条豁免**(`fixtures/cases/explore/exemptions.json`):Rhino 把**整数字面量**
装箱成 `Integer`(`@js:1` 的完成值 `toString()` 给 `"1"`),而 `@js:2.0` /
`@js:4/2` / `@js:2147483648` 都是 Double —— 被测侧的完成值只有一个 f64,
分不出「这个 1 是字面量还是算出来的」;quickjs 自己的 int/float 标记与 Rhino
**不是同一套**(`4/2` 在那边是 int,照它反而更错)。语料里发现规则的 JS
末句返回裸数字的:**0 条**。探针留着 —— 哪天真有,它就是要还的账。

**收盘**:**十八套 101221 例 0 FAIL**(豁免 91:explore 套那一条是新的);
`cargo test` / `cargo fmt --check` / `cargo clippy` / `tools/check_generated.sh` 全绿;
`flutter analyze` 干净,`integration_test/engine_test.dart` 加了一条发现分类的端到端
(不联网:两条纯解析的路 + `explorable` 位 + 空表那一支)。

**台账从此对得上账**:`tools/port_audit.py --check` → `错 0 条、欠账 0 条`,
`--todo` 是空的。**这不等于「移植完了」** —— 台账只管**真身有的面**,
产品面(书源调试页、并发搜索编排、整本缓存、书架分组)一件没动。

**Phase 3 之后该抄的(产品面,真身现成)**:`model/Debug.kt`(书源调试页 ——
诊断口 `searchOne` / `searchOnePage` / `exploreOne` 已经有了)、
`model/webBook/SearchModel.kt`(并发搜索与换源的编排)、`model/CacheBook.kt`
(整本缓存)、`BookCover` / `RuleUpdate` / 书架分组;Phase 4 的 legado 备份导入不变。
**现在 `app/lib` 只有五个页面,而引擎是三万四千行 Rust —— 富矿在产品面那边。**

### 阅读正文排版重构（专项计划）

现有 `TextPainter` 真分页完成了 Phase 1 的产品闭环，但每页重新 layout 剩余全文，
不作为长期排版架构。后续采用“Rust 文档/分页策略 + Flutter Paragraph 测量绘制”的
混合方案；功能契约对齐冻结 Legado，架构不照搬。里程碑、数据协议、性能门槛与
Rust 原生纹理渲染决策门见 **[reader-layout-plan.md](reader-layout-plan.md)**。

**2026-09-01:M0 与 M1 已收工**。产品阅读页换成了新实现(`reader-doc` +
`reader-layout` 两个 Rust crate + `app/lib/reader/`),旧那份留在
`--dart-define=RUBATO_LEGACY_READER=true` 后面。参考机(HITV205N,profile)上
50k 单章:旧实现整章分页 **21 秒**,新实现冷首屏 **28 ms**、整章补齐 191 ms。
下一件是 M2(样式、图片与阅读设置)。

### Phase 4 — 迁移与收尾

legado 备份导入(书架/进度/书源/cookie)、桌面窗口化阅读打磨、「源失败自动生成差分 case」的回流通道。

## 5. 风险清单(按杀伤力排序)

1. **jsoup 序列化/取文本长尾**(高):text() 空白折叠、html() pretty-print——`##` 正则的输入依赖它。对策:自写序列化器 + Phase 1 就建序列化 fuzz 差分。
   **实测(2026-08-29)**:html-compat 落地(选择器方言 + text/html 序列化对照 jsoup 1.16.2 源码逐行移植),3100 例差分 0 FAIL(豁免 36 例同一根因)。踩实的关键行为:未知标签 isBlock=false 但 formatAsBlock=true(Tag.valueOf 怪癖)、`<noscript>` 须按 scripting=false 解析、scraper 要开 `deterministic` 特性保属性序、wholeText 里 `<br>`→`\n`、`:empty` 首个文本节点即定结果。解析层已知缺口(记豁免):html5ever 不保留"无值属性 vs 空值属性"之别与未知标签自闭合语法——影响 html() 字节级一致,待上游或分词器补丁。
2. **java.util.regex vs fancy-regex 方言**(高):lookaround/反引/替换串/编译失败降级。对策:regex-compat 独立差分套件,只兼容语料里真实用到的特性面。
   **实测(2026-08-29)**:regex-compat 落地,8071 例差分 0 FAIL(豁免 26 例带注释)。两项发现:
   ① fancy-regex 0.19 重构后不再把纯正则委托给 regex crate,自带 VM 有量词回溯 bug(`x+z?x+` 匹配单个 x)——**锁定 =0.14.0**,升级前必须重跑 `tools/regex_diff.sh`;
   ② 净化正则家族用**有界变长 lookbehind**(`(?<=X[”）】]?)`、`\h{0,4}`),Java 支持而 fancy 仅支持定长 → 现按编译失败降级并豁免;TODO:fork fancy 实现有界变长 lookbehind 后收敛。
3. **Rhino LiveConnect 方言**(中高):`org.jsoup.*`、`String(javaObj)`、Java List 在 JS 里的行为。对策:prelude shim + `result` 绑定统一转 JS 原生类型;失败样本自动归类进语料。
4. **jayway JSONPath 行为差**(中):必要时 fork crate。
   **实测(2026-08-29)**:未用 jsonpath-rust,自写 jayway 对齐求值器(json-compat)更可控;2307 例差分 0 FAIL 零豁免。
   规则层接入后又抓出三条:① 路径不以 `$`/`@` 开头会被补 `$.`(书源里 `data.books` 这种写法全靠它);
   ② 叶子上的多键选择 `$['a','b']` 是 **merge 成一个 Map** 且算 definite(缺键跳过,全缺给 `{}`),不是列表;
   ③ json-smart 的 PERMISSIVE 解析对非 JSON 文本(HTML 页面)**不抛异常**而是当裸字符串,
   于是「JSON 规则打在 HTML 内容上」走的是 PathNotFound 被吞的路径,不是构造异常。踩实的怪癖:扇出算子前缺失抛 PathNotFound 而其后静默、双端切片是"朴素 from..to 循环+负下标映射"(`[-1:1]`→末尾+开头环绕)、过滤器可作用于对象本身、渲染为"Map 用 `{k=v}` / Array 用 json-smart JSON 文本(`/`→`\/`)"、null 标量在引擎侧因 NPE 吞成空。注意 json-smart 是 PERMISSIVE 解析(接受宽松 JSON),serde_json 严格——真实脏 JSON 的差异留 Phase 2 观察。
5. **QueryTTF 移植**(中):~1000 行可 1:1 移植、可离线差分;工作量风险而非可行性风险,放 Phase 3。
6. **FRB async/取消/生命周期**(中):任务句柄约定 + 「退出页面取消搜索」集成测试。
7. **URL 解析语义差**(低中):getAbsoluteURL 按裁判行为移植并差分。
   **实测(2026-08-29)**:`rubato-core::java_url` 逐行移植 `java.net.URL` + `URLStreamHandler.parseURL`
   + `toExternalForm`,2066 例差分 0 FAIL。踩实的四点:协议表就是 JDK 内置 handler 集合
   (`data:`/`thunder:` 一律 MalformedURLException,被上层吞成原始相对串);**JDK 20+ 收紧了 host 合法性**
   (空格、控制字符与 `" < > [ \ ] ^ ` { | }` 非法——书源里 `{{page}}` 落到 host 上就会撞);
   `parseURL` 的 `indexOf('?')` 从 0 起算;`..` 归一化是 JDK 自己的循环写法,`/../../a` 原样保留。
   未覆盖:`jar:`/`mailto:` 的自有 handler(用例回避)。
8. **裁判漂移**(流程):judge/ 冻结在 LegadoTeam@3046111c;任何改动须在差分报告标注。
   基准升级只允许整体重演切换流程(§4 里程碑 1-4),不允许零星 cherry-pick。

## 6. 砍单

- **继续砍**:RSS/订阅、TTS、替换净化规则、WebDAV、主题系统、段评、书源扩展位(customButton/eventListener 等)、mainJs 纯 JS 书源(新基准新增,显式报「不支持」,Phase 3 再议)、toast/startBrowser(纯浏览)等 UI 型 JS API(空实现记日志)。
- **M3p 新砍(2026-09-01,按台账的分母判的)**:`loginUi` V2 / `jsLib`(mainJs)/ `upLoginData`+`reLoginView` —— 语料里 `loginUi` 个位数源、`jsLib` 1 源,补判据的成本压过收益;`RowUi` 的 `style`/`countdown`/`key`/`value` 同理(表单本身 M3l 已经做了)。要翻案先回答「判据从哪来」。**没砍的是发现页分类列表**(`ExploreKind`/`FlexChildStyle`,台账记 `待做`):那一族有几百个源的分母。
- **延后不承诺**:本地书导入、漫画/听书源(引擎不 panic,UI 不做)、整本离线缓存(Phase 4)。
- **永不做**:与上游 LegadoTeam 主线**持续**同步(基准是锁定快照,升级走一次性重演流程);100% C 层长尾兼容承诺。
