# 移植对照台账

**这份是判据,不是生成物** —— 状态与理由是人写的,`tools/port_audit.py --check`
只负责对账:真身有而这里没有、这里有而真身没了、状态与证据矛盾。
口径、状态那五档的定义、怎么跑,全在 `tools/port_audit.py` 的文件头。

**数字不写在这里**(会跟代码悄悄漂):用量跑 `tools/port_audit.py` 现算。

## `jsext` —— 书源 JS 调得到的宿主方法

真身:`help/JsExtensions.kt`

| 真身 | 出处 | rubato | 状态 | 依据 |
|---|---|---|---|---|
| `ajax` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `ajaxAll` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `ajaxTestAll` | JsExtensions.kt | — | 不做 | 语料 0 源 |
| `androidId` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `base64Decode` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `base64DecodeToByteArray` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `base64Encode` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `bytesToStr` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `cacheFile` | JsExtensions.kt | `js-host::host_env` 未接清单 | 不做 | 语料 0 源 —— 没有分母,移植了也没有一个源能验收(plan §4 M3a)。两侧都在:被测侧抛「未接」 |
| `connect` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `deleteFile` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `downloadFile` | JsExtensions.kt | `js-host::host_env` 未接清单 | 不做 | 语料 0 源 —— 没有分母,移植了也没有一个源能验收(plan §4 M3a)。两侧都在:被测侧抛「未接」 |
| `encodeURI` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `get` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `get7zByteArrayContent` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `get7zStringContent` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `getCookie` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `getFile` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `getRarByteArrayContent` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `getRarStringContent` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `getReadBookConfig` | JsExtensions.kt | — | 不做 | 砍单 §6:主题系统 / 阅读配置;语料 0 源 |
| `getReadBookConfigMap` | JsExtensions.kt | — | 不做 | 砍单 §6:主题系统 / 阅读配置;语料 0 源 |
| `getSource` | JsExtensions.kt | — | 不做 | 语料 0 源 |
| `getTag` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `getThemeConfig` | JsExtensions.kt | — | 不做 | 砍单 §6:主题系统 / 阅读配置;语料 0 源 |
| `getThemeConfigMap` | JsExtensions.kt | — | 不做 | 砍单 §6:主题系统 / 阅读配置;语料 0 源 |
| `getThemeMode` | JsExtensions.kt | — | 不做 | 砍单 §6:主题系统 / 阅读配置;语料 0 源 |
| `getTxtInFolder` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `getVerificationCode` | JsExtensions.kt | `net::verification` | 移植 | M3j:策略层进差分(js 套 verifyOpens)。M3k 的界面那一半随 Flutter 前端于 2026-09-14 移出本仓,端到端判据待新前端重建 |
| `getWebViewUA` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `getZipByteArrayContent` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `getZipStringContent` | JsExtensions.kt | `js-host::host_env` 未接清单 | 不做 | 语料 0 源 —— 没有分母,移植了也没有一个源能验收(plan §4 M3a)。两侧都在:被测侧抛「未接」 |
| `head` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `hexDecodeToByteArray` | JsExtensions.kt | — | 不做 | 语料 0 源 |
| `hexDecodeToString` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `hexEncodeToString` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `htmlFormat` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `importScript` | JsExtensions.kt | `js-host::host_env` 未接清单 | 不做 | 语料 0 源 —— 没有分母,移植了也没有一个源能验收(plan §4 M3a)。两侧都在:被测侧抛「未接」 |
| `lock` | JsExtensions.kt | — | 不做 | 语料 0 源 |
| `log` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `logType` | JsExtensions.kt | — | 不做 | 语料 0 源 |
| `longToast` | JsExtensions.kt | `js-host::host_env`(空实现) | 不做 | 砍单 §6:UI 型 JS API 空实现。真身返回 Unit,两侧同为 undefined,js 套照过 |
| `openUrl` | JsExtensions.kt | — | 不做 | 砍单 §6:UI 型 JS API;语料 0 源 |
| `openVideoPlayer` | JsExtensions.kt | — | 不做 | 砍单 §6:UI 型 JS API;语料 0 源 |
| `post` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `queryBase64TTF` | JsExtensions.kt | `js-host::host_env` 未接清单 | 不做 | 语料 0 源 —— 没有分母,移植了也没有一个源能验收(plan §4 M3a)。两侧都在:被测侧抛「未接」 |
| `queryTTF` | JsExtensions.kt | `js-host::host_env` 未接清单 | 不做 | 语料 0 源 —— 没有分母,移植了也没有一个源能验收(plan §4 M3a)。两侧都在:被测侧抛「未接」 |
| `randomUUID` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `readFile` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `readTxtFile` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `replaceFont` | JsExtensions.kt | `js-host::host_env` 未接清单 | 不做 | 语料 0 源 —— 没有分母,移植了也没有一个源能验收(plan §4 M3a)。两侧都在:被测侧抛「未接」 |
| `s2t` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `showBrowser` | JsExtensions.kt | — | 不做 | 砍单 §6:UI 型 JS API;语料 0 源 |
| `singleFlight` | JsExtensions.kt | — | 不做 | 语料 0 源 |
| `startBrowser` | JsExtensions.kt | `net::verification` | 移植 | M3j:策略层进差分(js 套 verifyOpens)。M3k 的界面那一半随 Flutter 前端于 2026-09-14 移出本仓,端到端判据待新前端重建 |
| `startBrowserAwait` | JsExtensions.kt | `net::verification` | 移植 | M3j:策略层进差分(js 套 verifyOpens)。M3k 的界面那一半随 Flutter 前端于 2026-09-14 移出本仓,端到端判据待新前端重建 |
| `strToBytes` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `t2s` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `tick` | JsExtensions.kt | — | 不做 | 语料 0 源 |
| `timeFormat` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `timeFormatUTC` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `toNumChapter` | JsExtensions.kt | `js-host::host_env` | 移植 | js 套差分(tools/js_diff.sh) |
| `toURL` | JsExtensions.kt | — | 不做 | 语料 0 源 |
| `toast` | JsExtensions.kt | `js-host::host_env`(空实现) | 不做 | 砍单 §6:UI 型 JS API 空实现。真身返回 Unit,两侧同为 undefined,js 套照过 |
| `un7zFile` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `unArchiveFile` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `unrarFile` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `unzipFile` | JsExtensions.kt | — | 不做 | 本地文件 / 压缩包那一支;语料 0 源(砍单:本地书导入延后) |
| `webView` | JsExtensions.kt | `net::webview` + `js-host::net_face` | 移植 | M3c:三个入口的桩两侧一起拆;判据 webview 套 + js 套 |
| `webViewGetOverrideUrl` | JsExtensions.kt | `net::webview` + `js-host::net_face` | 移植 | M3c:三个入口的桩两侧一起拆;判据 webview 套 + js 套 |
| `webViewGetSource` | JsExtensions.kt | `net::webview` + `js-host::net_face` | 移植 | M3c:三个入口的桩两侧一起拆;判据 webview 套 + js 套 |

## `urlopt` —— URL 尾巴 ,{...} 的选项键

真身:`AnalyzeUrl.kt::UrlOption`

| 真身 | 出处 | rubato | 状态 | 依据 |
|---|---|---|---|---|
| `body` | AnalyzeUrl.kt | `net::url_option` | 移植 | analyze-url 套 + fetch 套差分 |
| `bodyJs` | AnalyzeUrl.kt | `net::url_option` | 移植 | analyze-url 套 + fetch 套差分 |
| `charset` | AnalyzeUrl.kt | `net::url_option` | 移植 | analyze-url 套 + fetch 套差分 |
| `dnsIp` | AnalyzeUrl.kt | `net::url_option` | 移植 | analyze-url 套 + fetch 套差分 |
| `followRedirects` | AnalyzeUrl.kt | `net::url_option` | 移植 | analyze-url 套 + fetch 套差分 |
| `headers` | AnalyzeUrl.kt | `net::url_option` | 移植 | analyze-url 套 + fetch 套差分 |
| `js` | AnalyzeUrl.kt | `net::url_option` | 移植 | analyze-url 套 + fetch 套差分 |
| `method` | AnalyzeUrl.kt | `net::url_option` | 移植 | analyze-url 套 + fetch 套差分 |
| `origin` | AnalyzeUrl.kt | — | 不做 | 真身只有界面读它(`AddToBookshelfDialog`/`BookshelfViewModel` 取来源 URL),四步流水线一次都不读;等「粘贴链接加书架」那条路做的时候再取 |
| `retry` | AnalyzeUrl.kt | `net::url_option` | 移植 | analyze-url 套 + fetch 套差分 |
| `serverID` | AnalyzeUrl.kt | `net::url_option` | 移植 | analyze-url 套 + fetch 套差分 |
| `timeout` | AnalyzeUrl.kt | `net::url_option` | 移植 | analyze-url 套 + fetch 套差分 |
| `type` | AnalyzeUrl.kt | `net::url_option` | 移植 | analyze-url 套 + fetch 套差分 |
| `webJs` | AnalyzeUrl.kt | `net::url_option` → `net::webview` | 移植 | M3c:先过 webView 再跑这段;语料 0 源,判据在 webview 套 |
| `webView` | AnalyzeUrl.kt | `net::url_option` → `net::webview` | 移植 | M3b/M3c:webview 套 98 例 + analyze-url 套 |
| `webViewDelayTime` | AnalyzeUrl.kt | `net::url_option` → `net::webview` | 移植 | M3b:重试梯子的起跑点,webview 套逐毫秒比过 |

## `field` —— 书源 JSON 的字段

真身:`data/entities/{BookSource,rule/*}.kt`

| 真身 | 出处 | rubato | 状态 | 依据 |
|---|---|---|---|---|
| `BookInfoRule::author` | BookInfoRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookInfoRule::canReName` | BookInfoRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookInfoRule::coverUrl` | BookInfoRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookInfoRule::downloadUrls` | BookInfoRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookInfoRule::init` | BookInfoRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookInfoRule::intro` | BookInfoRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookInfoRule::kind` | BookInfoRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookInfoRule::lastChapter` | BookInfoRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookInfoRule::name` | BookInfoRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookInfoRule::tocUrl` | BookInfoRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookInfoRule::updateTime` | BookInfoRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookInfoRule::wordCount` | BookInfoRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookListRule::author` | BookListRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookListRule::bookList` | BookListRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookListRule::bookUrl` | BookListRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookListRule::coverUrl` | BookListRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookListRule::intro` | BookListRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookListRule::kind` | BookListRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookListRule::lastChapter` | BookListRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookListRule::name` | BookListRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookListRule::updateTime` | BookListRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookListRule::wordCount` | BookListRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::bookSourceComment` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::bookSourceGroup` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::bookSourceName` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::bookSourceType` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::bookSourceUrl` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::bookUrlPattern` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::coverDecodeJs` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::customButton` | BookSource.kt | — | 不做 | 砍单 §6:书源扩展位;语料 0 源 |
| `BookSource::customOrder` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::enabled` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::enabledExplore` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::eventListener` | BookSource.kt | — | 不做 | 砍单 §6:书源扩展位;语料 0 源 |
| `BookSource::exploreScreen` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::exploreUrl` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::lastUpdateTime` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::loginCheckJs` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::mainJs` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::respondTime` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::ruleBookInfo` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::ruleContent` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::ruleExplore` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::ruleReview` | BookSource.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `BookSource::ruleSearch` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::ruleToc` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::searchUrl` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::variableComment` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `BookSource::weight` | BookSource.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `ContentRule::callBackJs` | ContentRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `ContentRule::content` | ContentRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `ContentRule::imageDecode` | ContentRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `ContentRule::imageStyle` | ContentRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `ContentRule::nextContentUrl` | ContentRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `ContentRule::payAction` | ContentRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `ContentRule::replaceRegex` | ContentRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `ContentRule::sourceRegex` | ContentRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `ContentRule::subContent` | ContentRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `ContentRule::title` | ContentRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `ContentRule::webJs` | ContentRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `ExploreKind::action` | ExploreKind.kt | `entities::ExploreKind` | 移植 | 同 `ExploreKind::type`:解析面进了 explore 套,界面未渲染(语料 1 源) |
| `ExploreKind::chars` | ExploreKind.kt | `entities::ExploreKind` | 移植 | 同 `ExploreKind::type`:解析面进了 explore 套,界面未渲染(语料 0 源) |
| `ExploreKind::default` | ExploreKind.kt | `entities::ExploreKind` | 移植 | 同 `ExploreKind::type`:解析面进了 explore 套,界面未渲染(语料 0 源) |
| `ExploreKind::style` | ExploreKind.kt | `entities::ExploreKind` | 移植 | explore 套差分(tools/explore_diff.sh);`style()` 的兜底与「给没给」分两位比 |
| `ExploreKind::title` | ExploreKind.kt | `entities::ExploreKind` | 移植 | explore 套差分(tools/explore_diff.sh) |
| `ExploreKind::type` | ExploreKind.kt | `entities::ExploreKind` | 移植 | explore 套差分(tools/explore_diff.sh)。**解析面已比**;界面眼下只渲染 `url` 这一种(`text`/`button`/`toggle`/`select` 是表单型分类,语料 1 源) |
| `ExploreKind::url` | ExploreKind.kt | `entities::ExploreKind` | 移植 | explore 套差分(tools/explore_diff.sh) |
| `ExploreKind::viewName` | ExploreKind.kt | `entities::ExploreKind` | 移植 | 同 `ExploreKind::type`:解析面进了 explore 套,界面未渲染(语料 0 源) |
| `FlexChildStyle::layout_alignSelf` | FlexChildStyle.kt | `entities::FlexChildStyle` | 移植 | explore 套差分(tools/explore_diff.sh);`alignSelf()` 那张表六个取值逐条钉(ex-style-align-each) |
| `FlexChildStyle::layout_flexBasisPercent` | FlexChildStyle.kt | `entities::FlexChildStyle` | 移植 | explore 套差分(tools/explore_diff.sh);发现页按它算一行几格 |
| `FlexChildStyle::layout_flexGrow` | FlexChildStyle.kt | `entities::FlexChildStyle` | 移植 | explore 套差分(tools/explore_diff.sh);发现页按它摊格子 |
| `FlexChildStyle::layout_flexShrink` | FlexChildStyle.kt | `entities::FlexChildStyle` | 移植 | explore 套差分(tools/explore_diff.sh);缺省 1F 那一位靠无参构造,探针 ex-style-empty-object |
| `FlexChildStyle::layout_justifySelf` | FlexChildStyle.kt | `entities::FlexChildStyle` | 移植 | explore 套差分(tools/explore_diff.sh);真身自己也不把它交给 FlexboxLayout(注释写着「自定义」),语料 0 源 |
| `FlexChildStyle::layout_wrapBefore` | FlexChildStyle.kt | `entities::FlexChildStyle` | 移植 | explore 套差分(tools/explore_diff.sh) |
| `ReviewRule::avatarRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::contentRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::deleteUrl` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::detailAvatarRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::detailBadgeRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::detailContentRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::detailIdRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::detailListRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::detailNameRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::enabled` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::postQuoteUrl` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::postReviewUrl` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::postTimeRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::replyAvatarRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::replyBadgeRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::replyContentRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::replyIdRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::replyListRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::replyNameRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::reviewDetailNextPageUrl` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::reviewDetailUrl` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::reviewQuoteUrl` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::reviewSummaryUrl` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::reviewUrl` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::summaryCountRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::summaryListRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::summaryParagraphDataRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::summaryParagraphIndexRule` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::voteDownUrl` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `ReviewRule::voteUpUrl` | ReviewRule.kt | — | 不做 | 砍单 §6:段评;语料 0 源 |
| `RowUi::action` | RowUi.kt | — | 待做 | M3l 曾在 Flutter 侧实现(`source_login_page.dart`),2026-09-14 随前端一并移出本仓;引擎这侧仍原样交出 `loginUi` 字符串,缺的是解析与渲染。分母:语料里 `loginUi` 只有个位数源 |
| `RowUi::chars` | RowUi.kt | — | 待做 | M3l 曾在 Flutter 侧实现(`source_login_page.dart`),2026-09-14 随前端一并移出本仓;引擎这侧仍原样交出 `loginUi` 字符串,缺的是解析与渲染。分母:语料里 `loginUi` 只有个位数源 |
| `RowUi::countdown` | RowUi.kt | — | 不做 | loginUi 表单的样式 / 倒计时 / 键值那几位,语料 0 源(`loginUi` 本身也只有个位数源) |
| `RowUi::default` | RowUi.kt | — | 待做 | M3l 曾在 Flutter 侧实现(`source_login_page.dart`),2026-09-14 随前端一并移出本仓;引擎这侧仍原样交出 `loginUi` 字符串,缺的是解析与渲染。分母:语料里 `loginUi` 只有个位数源 |
| `RowUi::hint` | RowUi.kt | — | 待做 | M3l 曾在 Flutter 侧实现(`source_login_page.dart`),2026-09-14 随前端一并移出本仓;引擎这侧仍原样交出 `loginUi` 字符串,缺的是解析与渲染。分母:语料里 `loginUi` 只有个位数源 |
| `RowUi::key` | RowUi.kt | — | 不做 | loginUi 表单的样式 / 倒计时 / 键值那几位,语料 0 源(`loginUi` 本身也只有个位数源) |
| `RowUi::name` | RowUi.kt | — | 待做 | M3l 曾在 Flutter 侧实现(`source_login_page.dart`),2026-09-14 随前端一并移出本仓;引擎这侧仍原样交出 `loginUi` 字符串,缺的是解析与渲染。分母:语料里 `loginUi` 只有个位数源 |
| `RowUi::options` | RowUi.kt | — | 待做 | M3l 曾在 Flutter 侧实现(`source_login_page.dart`),2026-09-14 随前端一并移出本仓;引擎这侧仍原样交出 `loginUi` 字符串,缺的是解析与渲染。分母:语料里 `loginUi` 只有个位数源 |
| `RowUi::style` | RowUi.kt | — | 不做 | loginUi 表单的样式 / 倒计时 / 键值那几位,语料 0 源(`loginUi` 本身也只有个位数源) |
| `RowUi::type` | RowUi.kt | — | 待做 | M3l 曾在 Flutter 侧实现(`source_login_page.dart`),2026-09-14 随前端一并移出本仓;引擎这侧仍原样交出 `loginUi` 字符串,缺的是解析与渲染。分母:语料里 `loginUi` 只有个位数源 |
| `RowUi::value` | RowUi.kt | — | 不做 | loginUi 表单的样式 / 倒计时 / 键值那几位,语料 0 源(`loginUi` 本身也只有个位数源) |
| `RowUi::viewName` | RowUi.kt | — | 待做 | M3l 曾在 Flutter 侧实现(`source_login_page.dart`),2026-09-14 随前端一并移出本仓;引擎这侧仍原样交出 `loginUi` 字符串,缺的是解析与渲染。分母:语料里 `loginUi` 只有个位数源 |
| `SearchRule::checkKeyWord` | SearchRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `TocRule::chapterList` | TocRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `TocRule::chapterName` | TocRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `TocRule::chapterUrl` | TocRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `TocRule::formatJs` | TocRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `TocRule::isPay` | TocRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `TocRule::isVip` | TocRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `TocRule::isVolume` | TocRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `TocRule::nextTocUrl` | TocRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `TocRule::preUpdateJs` | TocRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |
| `TocRule::updateTime` | TocRule.kt | `rubato-core::entities` | 移植 | source 套 + 两套流水线差分 |

## `liveconnect` —— 书源绕开 `java.*` 直接调的 Java 类(**面从语料枚举**,口径见 port_audit.py)

真身:`com/script/rhino/RhinoClassShutter.kt(黑名单)`

| 真身 | 出处 | rubato | 状态 | 依据 |
|---|---|---|---|---|
| `Arrays` | JavaImporter | `js-host::live_connect` | 移植 | `java.util.Arrays.copyOfRange/copyOf/toString`;js 套差分(tools/js_diff.sh)的 `js-lc-arrays-copyofrange` |
| `Base64` | android.util | `js-host::live_connect` | 移植 | **两张表**:`android.util.Base64`(AOSP 的宽松解码表)与 `java.util.Base64`(基本解码器严格、MIME 宽松);js 套差分(tools/js_diff.sh)的 `js-lc-b64-*` 六条 |
| `Cipher` | JavaImporter | `js-host::live_connect` + `js-host::java_api::SymmetricCrypto` | 移植 | getInstance / init / doFinal 分三步,错误落在与真身相同的那一步;js 套差分(tools/js_diff.sh)的 `js-lc-cipher-*`、`js-lc-aes-cbc-enc`、`js-lc-desede-cbc-enc`、`js-lc-err-*`、`js-lc-dec-empty*` |
| `DESKeySpec` | JavaImporter | `js-host::live_connect` | 移植 | 取前 8 字节,不足 8 是 InvalidKeyException;js 套差分(tools/js_diff.sh)的 `js-lc-des-keyfactory` |
| `Date` | JavaImporter | — | 不做 | **被测侧是超集**:导了 `java.util` 之后 `with` 块里的 `Date` 在真身那边被 `java.util.Date` 遮住,`new Date().getFullYear` 是 undefined;被测侧不遮。语料 1 个源(而且它同时是「摸时钟」那一档,四步语料本来就整条剔除)。豁免逐字写在 `fixtures/cases/js-host/exemptions.json` 的 `js-lc-shadow-date-getfullyear` |
| `IvParameterSpec` | JavaImporter | `js-host::live_connect` | 移植 | IV 长度不等于分组长在 `init` 那一步炸;ECB 收到它是 `InvalidAlgorithmParameterException`(不是忽略);js 套差分(tools/js_diff.sh)的 `js-lc-err-ivlen` / `js-lc-err-ecb-iv` |
| `Jsoup` | org.jsoup | `js-host::host_env`(`install_liveconnect`) | 移植 | `org.jsoup.Jsoup.parse` 是 Rhino 的**全局面**(与 `java` 那个宿主对象无关),三个宿主上都要有;js 套差分(tools/js_diff.sh) + pipeline-corpus-b 套 |
| `KeyFactory` | JavaImporter | `js-host::live_connect` + `js-host::java_api::rsa_sign` | 移植 | RSA 一支(语料 2 源:西瓜小说的 `sign` 请求头);js 套差分(tools/js_diff.sh)的 `js-lc-rsa-*` |
| `Mac` | JavaImporter | `js-host::live_connect` + `js-host::java_api::hmac_bytes` | 移植 | `Mac.getInstance(...).init/update/doFinal`,错误类别按 JCE 分;js 套差分(tools/js_diff.sh)的 `js-lc-mac-hmacsha256` |
| `Object` | JavaImporter | — | 不做 | **被测侧是超集**:`with` 块里 `java.lang.Object` 在真身那边把 JS 的 `Object` 遮住,`Object.keys(...)` 因此撞 Rhino 的 InternalError;被测侧的 `java.lang` 只遮 `String`(那条是必须的:`String(byte[])` 是解码)。语料 5 个源 —— 复刻裁判等于让这 5 个源在 Rubato 上也失效。豁免见 `fixtures/cases/js-host/exemptions.json` 的 `js-lc-shadow-object-keys` |
| `OkHttpClient` | okhttp3 | — | 不做 | **一个字节都不许出网**:书源拿 `new Packages.okhttp3.OkHttpClient()` 绕开宿主直接发请求,裁判那条路会真的出网(录放拦不住),差分建不起来。语料 1 个源;被测侧没有这个类 → TypeError,被书源自己的 try/catch 吞掉。口径见 `fixtures/cases/js-host/README.md`「okhttp3 直连」 |
| `PKCS8EncodedKeySpec` | JavaImporter | `js-host::live_connect` | 移植 | 私钥 DER 按 Java 的有符号 byte 传;js 套差分(tools/js_diff.sh)的 `js-lc-rsa-sign-*` / `js-lc-rsa-err-badkey` |
| `Request` | okhttp3 | — | 不做 | 同 `OkHttpClient`(同一个源、同一条路) |
| `SecretKeyFactory` | JavaImporter | `js-host::live_connect` | 移植 | `generateSecret(DESKeySpec)`;js 套差分(tools/js_diff.sh)的 `js-lc-des-keyfactory` |
| `SecretKeySpec` | JavaImporter | `js-host::live_connect` | 移植 | 密钥长度不合法在 `init` 那一步炸(InvalidKeyException);js 套差分(tools/js_diff.sh)的 `js-lc-err-keylen` 与所有加解密条目 |
| `Signature` | JavaImporter | `js-host::live_connect` + `js-host::java_api::rsa_sign` | 移植 | 只签不验(`rsaVerify`/`.verify(` 语料 0 命中,验签那半已下架);js 套差分(tools/js_diff.sh)的 `js-lc-rsa-sign-sha256/sha1/len` |
| `String` | java.lang | `js-host::live_connect` | 移植 | `java.lang.String` 在 `with` 块里**遮住 JS 的 String**:`String(byte[])` 是按字符集解码而不是字符串化,`String('中').getBytes()[0]` 是有符号的 -28;js 套差分(tools/js_diff.sh)的 `js-lc-string-*` 六条 + `js-lc-bytes-*` 四条 |
| `ZipUtil` | cn.hutool.core.util | — | 不做 | `cn.hutool.core.util.ZipUtil.unGzip` —— 本地文件 / 压缩包那一支,与 `un7zFile` 一族同一个砍单(plan §6);语料 1 个源 |
