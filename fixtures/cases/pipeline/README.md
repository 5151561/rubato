# pipeline 差分用例契约(WebBook 四步流水线)

入口:`tools/pipeline_diff.sh`(先跑 `tools/gen_pipeline_cases.py` 重建用例与
`fixtures/http-pipeline/` 快照)。快照 key 算法契约:**docs/http-snapshot.md**。

- 裁判:原样挂载的 `WebBook.kt` / `BookList.kt` / `BookInfo.kt` /
  `BookChapterList.kt` / `BookContent.kt` / `ContentProcessor.kt` /
  `HtmlFormatter.kt` / `ContentHelp.kt` + 实体真身(Book / BookChapter /
  SearchBook / BookSource / rule 实体);appDb、BookHelp 的磁盘缓存、Debug 日志
  由 shims 顶掉,HTTP 走 `harness/ReplayHttp`。
- 被测:`rust/crates/pipeline`(web_book / helpers / html_formatter)+
  `net` + `rule-engine` + `store::CookieStore` + `difftest::replay`。

这是**流水线级**差分(docs/plan.md §3 的第 2 级):规则级差分保证单条规则一致,
本套保证「四步的调度、字段装配、翻页、变量流转、错误分类」一致。

## 用例格式

```jsonc
{
  "id": "pl0001", "op": "pipeline",
  "source": "<书源 JSON 字符串>",          // 两侧各自反序列化(source 套已钉住语义)
  "step": "search" | "explore" | "info" | "toc" | "content",
  // search
  "key": "abc", "page": 1,
  // explore
  "url": "/e/1.html", "page": 1,
  // info / toc / content:入口 Book(只带流水线要用的字段)
  "book": {"bookUrl": "...", "name": "...", "author": "...", "tocUrl": "...",
           "variable": "...", "type": 8, "durChapterIndex": 0, "totalChapterNum": 0},
  // content
  "chapter": {"url": "...", "title": "...", "baseUrl": "...", "index": 0,
              "isVolume": false, "tag": "..."},
  "nextChapterUrl": "..."          // 正文翻页的「撞到下一章就停」判定
}
```

每个 case 执行前重置 CacheManager 内存、cookie 库与请求序列。

## 输出

- `search` / `explore` → `books`:每本投影 name / author / kind / coverUrl /
  intro / wordCount / latestChapterTitle / bookUrl / origin / type / vars / infoHtml;
- `info` → `book`:name / author / kind / wordCount / latestChapterTitle / intro /
  coverUrl / tocUrl / bookUrl / type / durChapterTitle / totalChapterNum / vars /
  tocHtml / infoHtml;
- `toc` → `chapters`(title / url / tag / **wordCount** / isVolume / isVip / isPay /
  index / vars)+ 回写后的 `book`;
- `content` → `content`(ContentProcessor 之后的正文文本)+ 回写后的 `chapter`;
- 所有步都附 `requests`(每跳 `[method, url]`,跨重试与翻页累计)与
  `cookiesDb`。

`vars` 按键排序输出(裁判用 TreeMap);变量字段解析不成 Map 时原样给字符串。

错误(两侧字符串必须逐字一致):
- `source_error`——书源 JSON 反序列化失败;
- `snapshot_miss:<key>:<规范化URL>` / `too_many_redirects`——同 fetch 套;
- `toc_empty` / `content_empty`——`TocEmptyException` / `ContentEmptyException`;
- `pipeline_error`——其余一切异常的归一化(搜索 url 为空、规则求值抛错等)。
  **注意这是粗粒度归一**:两侧都只要求"抛了",不比对异常消息。

## 语料

全部**手工合成**:书源规则与页面成对设计,一条轴一条轴地铺
(`tools/gen_pipeline_cases.py` 里按「列表步 / 详情步 / 目录步 / 正文步」分段)。
本套只钉**调度与装配**。

**真实书源的四步覆盖在 `fixtures/cases/pipeline-corpus/`**(A 层 846 源 ×
四步,用 docs/http-snapshot.md §6 的回落页供页),那一套回答 plan §4 的
「A 层四步差分」判据;规则面的真实语料覆盖另由 rule-engine / analyze-url /
source 三套承担。

已铺的轴:

- 列表步:jsoup / jsonpath / 正则三种 bookList;`-`/`+` 前缀;列表为空落回详情页
  解析;`bookUrlPattern` 命中;`checkKeyWord` 命中与不命中;name 取空跳过;
  书名/作者净化;字数格式化;简介 `formatIntro`;`@put` 变量与写回;302 到详情页;
  searchUrl 为空;explore 的 JSON 形态 exploreUrl 与「ruleExplore.bookList 为空回落
  ruleSearch」。
- 详情步:`init` 规则(命中/不命中);`canReName` 规则空与非空 × canReName 参数;
  tocUrl 取空落回 baseUrl(并存 tocHtml);intro 的 `<usehtml>` 前缀直通;kind 多值。
- 目录步:单页 / `nextTocUrl` 链式翻页 / 一次给多页(并发分支,threadCount=1 串行)/
  自指防环;列表为空 → `toc_empty`;isVolume / isVip / isPay / updateTime 与
  `tocCountWords` 的字数抽取;chapterUrl 取空的两条回退;`formatJs`;`-` 前缀。
- 正文步:单页 / `nextContentUrl` 链式 / 一次给多页;撞到 `nextChapterUrl` 停;
  `replaceRegex`;正文取空 → `content_empty`;卷章节不解析;图片正文 + `imageStyle`;
  `title` 规则。

## 桩与契约

- JS:两侧同一指令表桩(`fixtures/cases/rule-engine/README.md`)。`formatJs` /
  `preUpdateJs` / `webJs` / header 内联 JS 都走它,真 JS 留给 Phase 2。
- `AppConfig`:`threadCount = 1`(并发页退化成顺序,请求序列才确定)、
  `tocCountWords = true`、`chineseConverterType = 0`;`ReadBookConfig.paragraphIndent`
  取真身默认的两个全角空格。
- 净化替换规则被砍(docs/plan.md §6):`ReplaceRuleDao` 恒空,`ContentProcessor`
  的替换分支不进差分;`removeSameTitleCache` 因 `BookHelp.getChapterFiles` 恒空
  而永不命中。
- `source.header` 走 `BaseSource.getHeaderMap()`(默认 UA 注入),契约见
  `fixtures/cases/fetch/README.md`。
- mainJs(JS 单文件书源)砍单:两侧都报「不支持」,本套不生成这类用例。

## Phase 1 未建模

- `subContent` 副内容、`payAction`、`sourceRegex` 的图片二次抓取(要多轮真实请求);
- webView 过盾与验证码(Phase 3);
- `preUpdateJs` 里 `reGetBook` / `refreshTocUrl` 的自调用面;
- 章节缓存(`BookHelp.saveContent`)与 `upChapterInfo` 的库回填(appDb 恒空)。

## `bookSourceType` → `BookType` 位

`BookSource.getBookType()`(help/source/BookSourceExtensions L130)把书源类型映到
`Book.type` 的位:`file(3)` → `text|webFile`、`image(2)` → `image`、
`audio(1)` → `audio`、**`video(4)` → `BookType.video`(`0b100`)**、其余 → `text`。
`video` 那支与 `BookType.allBookType` 里的 video 位是 LegadoTeam 基准新加的,
后者决定 `resetType` 清位时清不清 `0b100`。

用例按 `bookSourceType ∈ {0,1,2,3,4,5,-1}` × 四步 × 书上已有的 type 前值
(`0 / 4 / 8 / 0b1111111111`)铺开。被测侧漏掉 `4 => TYPE_VIDEO` 或
`ALL_BOOK_TYPE` 漏掉 video 位,会让其中 16 例 FAIL。
