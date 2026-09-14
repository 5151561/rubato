# Phase 3 的分母:C 层长尾验收清单

Phase 3 的判据是「**指定清单源逐个验收,不设总 pass 率**」(docs/plan.md §4)。
判据得有分母 —— 这一份就是那个分母:`checklist.json`,由
`tools/gen_phase3_checklist.py` 从 `fixtures/sources/*.json` 重算,**别手改**
(要改改探测器 `tools/phase3_caps.py`,再重跑)。

```bash
tools/gen_phase3_checklist.py           # 重算 + 打表
tools/gen_phase3_checklist.py --check   # 只核对:与语料对不上就非零退出
```

**`checklist.json` 本身不入库**(生成物一律不入库,见 `.gitignore` 与
`tools/check_generated.sh` —— 那条判据会把这个生成器一起跑一遍)。
留在库里的是**输入**(`fixtures/sources/`)、**口径**(本文件 +
`tools/phase3_caps.py`)与**生成器**;清单跑一条命令就有。

表里的数字**别手抄进任何文档**(手算的数字会和代码悄悄漂,这仓库栽过两次)。
下面写死的是**口径**,不是数字。

## 口径:什么叫「这个源用到了 Phase 3 能力」

**按值判定,键在不算数。** 探测器读的是书源 **dict**、逐字段看**值**:

- `ruleContent.sourceRegex` / `ruleContent.webJs` **值非空**才算;
- URL 选项里 `webView` 出现且**值不是显式假**才算(真身 `useWebView()` 的假值面
  是 null / `""` / `false` / `"false"`;语料里实测**一例假值都没有**);
- `startBrowser` / `getVerificationCode` / `queryTTF` / `importScript` 之类
  要**看到调用**(`名字(`)才算,光提一嘴不算。

**这一条不是洁癖,是修 bug。** 老口径(`extract_js.py` 与
`gen_pipeline_corpus_cases.py` 各抄一份的 `classify`)拿**整份书源的 JSON dump**
找裸子串:

```python
has_c = any(w in src_text for w in ("webView", "startBrowser", "webJs", …, "sourceRegex"))
```

`src_text` 里带着**键名**,而 `ContentRule` 默认就带 `"webJs": ""` 与
`"sourceRegex": ""` 两个空键 —— 于是**92 个源**(29 个本该是 A、63 个本该是 B)
被这两个空键踢进 C 层,**整源缺席 A/B 两套流水线差分**。改完的分层见
`checklist.json` 的 `corpus.tiers`(A/B 各涨一截,C 从 203 落到实数)。

口径现在**只有一份**:`tools/phase3_caps.py::classify`,那两个生成器都 import 它。

**登录三件套(`loginUrl` / `loginUi` / `loginCheckJs`)不进分层。** 它们挂在源上,
搜索/详情/目录/正文四步一次都不会碰;算进 C 层等于把 803 个「只是带了个空
`loginUrl` 键」的源踢出 A/B 套。它们仍进本清单,只是走**另一条验收线**。

## 六条验收线

清单里每个源带 `lines`(可能同时踩两条)、`caps`(具体能力)、`steps`
(四步里哪一步会踩到)、`hits`(触发字段与写法片段,给人 review 用)。

| 线 | 真身 | 验收方式 |
|---|---|---|
| `webview` | `AnalyzeUrl.useWebView` → `BackstageWebView` | 见下「webView 的两半」 |
| `browser` | `JsExtensions.startBrowser/startBrowserAwait` | **已接**(M3j 策略进差分 + M3k 界面):手工「可见窗口 → 用户操作完 → 回四步」这条线现在走得通,端到端判据 `app/integration_test/verify_test.dart` |
| `verify` | `JsExtensions.getVerificationCode` | **已接**(同上):弹图 → 用户输入 → 回四步 |
| `login` | `SourceLogin*`(url / ui / checkJs) | **已接**(M3l):`loginUrl` 可见 WebView + cookie 回抄；`loginUi` RowUi + AES + action/login JS；端到端判据 `app/integration_test/login_test.dart` |
| `font` | `QueryTTF` / `replaceFont` | **本语料 0 例** —— 见下 |
| `fs` | `importScript` / `cacheFile` / `downloadFile` / `getZipStringContent` | **本语料 0 例** —— 见下 |

### webView 的两半:能差分的和不能差分的

plan §3 写的是「BackstageWebView 不进自动差分(Phase 3 手工清单)」。那句话对
**WebView 本身**成立,对**它外面那层策略**不成立 —— 两半要分开算账:

- **不能差分的**:真的去加载一个页面、执行页面自己的脚本、过盾。裁判那侧是
  Android 的 `WebView`,快照录不下来,只能上真机逐个源看。
- **能差分的**:`BackstageWebView.kt` 那 394 行里,**WebView 之外全是策略** ——
  默认 JS 是 `document.documentElement.outerHTML`;`javaScript == null && delayTime == 0`
  时 delay 补成 900ms;`onPageFinished` 之后 `100L + delayTime` 起跑;取不到结果按
  `200/400/600/800/1000` 的梯子重试、`retry > 30` 报「js执行超时」;结果过
  `unescapeJson` 再剥掉首尾引号;跟过重定向就把 `StrResponse` 包成带
  `priorResponse(302)` 的那种(`res.url` 与 `isRedirect` 是书源读得到的);
  `sourceRegex` / `overrideUrlRegex` 非空时换 `SnifferWebClient`,回的是**命中的
  资源 URL 本身**而不是页面;页面加载完把 `CookieManager` 的 cookie 抄进
  `CookieStore`。这一半**该进差分**:把 WebView 换成两侧同一份确定性桩
  (脚本化的 `onPageFinished` / `evaluateJavascript` 序列),策略就全都可比。

**策略那一半已经做了**(M3b):`fixtures/cases/webview/` 那一套 ——
第三个 harness `:wvharness` 挂真身那 394 行,两侧的 WebView 换成同一份剧本 +
虚拟时钟,89 例 0 FAIL。**M3c 又把它接进了引擎**:`{"webView": true}` 的书源、
`@webjs:`、`java.webView*` 三处两侧都走这条策略(裁判那边 `:harness` /
`:jsharness` 也挂上了真身),于是「AnalyzeUrl 怎么进出 webView」也有判据了 ——
fetch 套那十例、js-host 套那十一例。所以:

> 清单里 `webview` 这条线的源数是**手工验收的分母**,而且**只剩「真的去加载
> 一个页面」这一件要人看** —— 延时、重试、超时、重定向、嗅探、cookie 回抄
> 都已经有自动判据了。策略那一半不占这个分母。

### `font` / `fs` 两条线在本语料是 0 例

`queryTTF` / `queryBase64TTF` / `replaceFont` / `importScript` / `cacheFile` /
`downloadFile` / `getZipStringContent` —— 1704 源里**一次调用都没有**
(`ruleContent.webJs` 与 `@webjs:` 同样是 0)。

这**不等于**「真身没有这些能力」,只等于「**本语料给不出判据**」。plan §5 把
QueryTTF 排进 Phase 3 是按「工作量风险而非可行性风险」定的;按本清单,它现在
**没有分母**:移植了也没有任何一个源能验收它,只能靠自造用例自证。故:

> `font` / `fs` 两条线**排在最后**,做之前先回答「判据从哪来」——
> 要么换一批带字体混淆的源进语料(那是换分母,得先说清为什么),
> 要么明确按自造用例 + 与 `QueryTTF.java` 逐行对照验收。

在那之前,`java.queryTTF` 等仍按 `NET_UNSUPPORTED` 抛 —— 语料里没人调用它,
抛与不抛都照不到,**不许**因为「反正没人用」就悄悄改成返回空串:那会把一个
显式缺口变成静默错误。

## 怎么验收(M3e:逐源验收探针)

清单是分母,**验收是拿真站点跑**。三条命令:

```bash
tools/gen_phase3_checklist.py                    # ① 分母(生成物,不入库)
tools/phase3_probe.sh --from=0 --limit=10        # ② 逐源跑(默认 macOS)
tools/phase3_report.py tools/out/phase3-*.log    # ③ 按真因分堆(可多份日志一起喂)
```

探针跑三段:**入口**(有搜索走 `searchOne`,只有发现走 `exploreOne`)→
**0 命中的再抓一次同入口页面**(`searchOnePage` / `exploreOnePage`)→
**该往下走的走目录/正文**。

`phase3_probe.sh` 干三件:`tools/phase3_plan.py` 把「清单 ∩ 语料」摊成
`tools/out/phase3-plan.json`;起一个**本地 http 服务**把它递给 app
(**沙箱里读不到仓库的文件** —— macOS 上 stat 得到、open 不得,真机更是两台机器,
所以 `--host=<本机局域网 IP>` 就能让真机跑同一份);再把探针
`app/integration_test/phase3_probe_test.dart` 跑起来。

探针照 plan 给每个源摊好的 `entry` 进入,照它 `steps` 里最深的那一步跑真实抓取
—— 搜索/发现 → 加书架(详情 + 目录)→ 正文 —— 每源打一行
`PHASE3 {json}`,带每一步的结果、错误原串与耗时。

**它不打分**。判据是「逐个验收,不设总 pass 率」,而失败有三种完全不同的因:

- **过盾没过**:`webview:js_timeout`(重试梯子跑满)/ `webview:timeout`
  (页面没加载完、嗅探没命中)—— 这一族才是 Phase 3 的正题;
- **站点没了 / 拒了**:DNS 解不出、连不上、4xx/5xx —— 与我们无关,但要**看见**
  它是这一族,别读成过盾失败;
- **规则不匹配**:`toc_empty` / `content_empty` —— 站点改版或书源本身过期。

报表按这三族分堆并把原串一起打出来,`fixtures/phase3/acceptance.json`
(生成物,不入库)留全量。

**跑完照出来的四处会骗人的地方,记在这里**:

1. **`--step-timeout` 必须大于 webView 自己的预算(60 s)**,否则探针的超时会
   **盖住**真因 —— webView 那条路本该报 `webview:timeout`,却被探针先一步
   `TimeoutException` 掉,于是「过盾没过」与「站点慢」混成一堆。默认 90 s
   就是为这个;图快调到 45 s 那一遍,15 个源全落进了这个坑。
2. **「search 0 命中」不等于「站点没书」**:`Engine::search` 是**逐源吞异常**的
   (单源失败不影响其他源),所以书源当场炸了和站点真的没结果在这一侧同形。
   头一遍 98 个源里 65 个落在这一堆 —— 那不是结论,是**还没查明**。
   **已补**:引擎多了一个「单源搜索、错误原样交回」的诊断口
   (`Engine::search_source` → FRB `searchOne`,将来书源调试页要的也是它),
   探针的搜索那一步改走它 —— 同一批源,那一堆从 65 落到 40,
   剩下的分成了「网络发不出去」「webView 超时」「书源 JS 出错」
   「源根本没有搜索规则」几族。
3. **`webview:timeout` 那一堆里,一个过盾失败都没有**(M3f)。头一遍 15 个源
   报「页面没加载完」,查下来是**平台那一侧把加载失败吞了**:真身跑在 Android 的
   WebView 上,连不上 / 解不出的页面照样 `onReceivedError` → **`onPageFinished`
   (错误页)**,策略层当场求值、当场有结论;而 WKWebView(macOS/iOS)只发
   `didFailProvisionalNavigation`、**没有 didFinish** —— `net::webview` 于是等满
   60 s 的 `withTimeout`,**站点连不上被读成过盾超时**。补在
   `app/lib/platform/web_view.dart::_Session._onError`(等一个宽限期,真的
   `onLoadStop` 没来才补发 pageFinished —— Android 上仍然是真身那一下先到),
   判据是 `webview_test.dart` 里那条「页面加载失败也算加载完」(打一个**关着的
   本机端口**,不联外网)。补完之后 15 → 3,剩下三个是**连 SYN 都没人回**的站点,
   真身在那儿同样只能等到超时。
4. **修完之后那 12 个源落进了「0 命中」**——这不是遮掩,是真身的行为:加载失败
   给的是错误页,规则当然什么都选不中。所以**报表这一侧另探一次站点**
   (`tools/phase3_report.py`,`--no-net` 关掉):没走到底的源逐个 curl 一遍
   书源自己的地址,连不上的打 `✗`、回 4xx/5xx 的打 `⚠`。
   **别拿 python 的 `urllib` 探** —— macOS 上它没有系统根证书,几乎每个 https
   站点都报 `CERTIFICATE_VERIFY_FAILED`,头一版就这么把 40 个活着的站点判成
   「连不上」;curl 用的是系统信任库,与浏览器同一套。

**「0 命中」那一堆靠第二个诊断口才判得动**(M3g):`Engine::search_source_page`
→ FRB `searchOnePage` —— **搜索那一步抓回来的页面原样交出**,与搜索**共用
同一条抓取路径**(`pipeline::search_fetch`;分成两份实现就会出现「诊断说抓到了、
搜索却没有」那种最糟的报告)。探针对每个 0 命中的源再抓一次,只留一句话摘要
(多长、`<title>`、正文头 120 字、最终 url),报表打在那一行下面。
一句话就把这一堆拆开了:

| 页面长这样 | 判 |
|---|---|
| **39 字节**、没有标题 | webView 的**空文档**(`<html><head></head><body></body></html>`)—— 页面根本没加载上,与站点连不上那一族是同一件事 |
| `Redirecting…` / `请稍候…正在进行安全验证` / `Just a moment…` | **盾**:webView 加载完的是过渡页。真身在同样的排期下也只等 `100+900 ms` 就取 outerHTML —— 要过它得靠书源自己写 `delayTime` / `sourceRegex`,或者换个网络环境再判 |
| `域名可以转让` / `This domain is for sale` / `mamma` / `404` / `400` / `The region has been denied` | **站点没了或拒了**,书源过期 |
| 真的搜索结果页 | 这才轮到问「规则为什么没选中」——**先看容器在不在**(`基友书屋` 那条:`.SHsectionThree-middle` 在,里面是空的,站点真没这本书) |

**逐条判下来是什么(2026-08-31 那一遍,98 个源)**:webView 那条线上
**我们这一侧的过盾失败一个都没有** —— 失败要么是站点从这台机器连不上 / 回 4xx、
要么是书源自己过期,余下十来个是**站点的盾**(Cloudflare、安全验证、广告过渡页),
那一族**在这台机器上判不出结论**:真身的排期我们逐字节照搬(第十七套 89 例钉着),
过不过取决于网络环境与书源自己的 `delayTime`/`sourceRegex` —— **要判它得上真机**。

**要两台机器都跑**(M3h):真机(`--device=<id>`,走 `adb reverse` 从 USB 回连,
不必与手机同一个局域网)与 macOS 各跑一遍,**98 个源里 47 个结论不同** ——
一半是「macOS 连不上而手机连得上」(出口不同),另一半是 Android WebView 与
WKWebView 的差别。**一台机器的网络不是判据**;日志名带设备就是为了两遍都留着。
盯一个源想看那张页就 `--only=<源> --dump`(整页落 `tools/out/pages/`)。

> 真机那一遍**照出了一处真的移植缺口**:`m.bqg225.com` 在 macOS 上出书、
> 在真机上只拿回 538 字节的 `<head>` —— 顺着查到真身**每次 `onPageFinished`
> 都把求值重排**,而移植版只认第一次(M3h,见 plan §4 与
> `fixtures/cases/webview/README.md`)。逐源验收的价值就在这里:
> 差分只能证明「比过的面一致」,**没被剧本表达出来的面它照不到**。

**跑得多快**:三步都在池子里并发(`--pool`,缺省 12),不是「一次亮一个源」串着
跑 —— 实测 100 个源**从 45 分钟降到约 3 分钟**。**它仍然不是开发回路**:
改代码请跑离线判据(`tools/all_diff.sh` + `app/integration_test/webview_test.dart`),
盯一个源就 `--only=`。

**仅发现源已经可达(M3m)**:计划生成器按源选择搜索或第一条非空发现分类;
当前 webView 线摊成 98 个搜索入口 + 2 个发现入口,后两条都已从真站点走到目录。
动态 `@js:` / `<js>` 发现若依赖 `infoMap`,仍明确列作 `unreachable`,不猜 URL。
**真机才是 webView 的真环境**:桌面 headless 与
Android WebView 在 UA、指纹、Cookie 面上都不同 —— macOS 那一遍先用来把
「站点死没死」这一族筛掉,值得细看的再上真机。

`browser` 线可用 `--line=browser --device=<id>` 定向跑。真机测试必须挂
`VerifyPlatform` 并持续 pump;若平台视图盖住 Flutter 的坐标点击,显式加
`--auto-verify` 会在延迟后直接调用“完成”按钮的同一个回调,但盾页里的真人操作
仍要在设备上完成。M3m 已在 Android 11 真机触发“八一中文”的真实 Turnstile:
链路能弹、能点、能回传,但该设备被 Cloudflare 再挑战,所以诚实归为“盾未通过”。

## 清单条目长什么样

```json
{
  "url": "http://api.17k.com/",
  "name": "17k小说",
  "pack": "aoaostar_2a1f129b.json",
  "tier": "C",
  "lines": ["webview"],
  "caps": ["webview.url_option"],
  "steps": ["toc"],
  "hits": [{"cap": "webview.url_option", "field": "ruleToc.chapterUrl",
            "step": "toc", "snippet": "…,{'webView': true}"}],
  "in_top100": false
}
```

`in_top100` 是给「这个源已经在 top100 流水线套里跑着」打的标 —— 它意味着
webView 之外的那几步已经有自动判据,手工验收只需要盯 webView 那一步。
