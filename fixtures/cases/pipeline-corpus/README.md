# pipeline-corpus 差分用例契约(A 层真实书源 × WebBook 四步)

入口:`tools/pipeline_corpus_diff.sh`(先跑 `tools/gen_pipeline_corpus_cases.py`
重建用例、豁免清单与 `fixtures/http-pipeline-corpus/` 的回落页)。

> **B 层是另一套**:`fixtures/cases/pipeline-corpus-b/`(同一个生成器,
> `--tier=B`)。裁判换成 `:jsharness` —— **真 Rhino** 而不是这一套的确定性桩,
> 被测侧走 `js_case_runner` 的真 QuickJS。本套的 JS 是两侧同契约的桩,
> 钉的是调度与装配;B 层那套才把 JS 打开。
两侧执行器与 `op: "pipeline"` 的用例/输出格式同 `fixtures/cases/pipeline/README.md`,
本套只多一个 `fallback` 字段。

这一套回答的是 plan §4 Phase 1 判据里的**「A 层四步差分」**那一条:
手工套(`fixtures/cases/pipeline/`)钉的是**调度与装配轴**,用的是成对设计的
合成书源;本套钉的是**真实书源规则**在四步上的一致性。

## 为什么用「回落页」而不是真实站点快照

真实站点快照拿不到,也不该进仓库(体量、时效、版权)。但四步的调度、字段装配、
翻页、错误分类在真实规则上的一致性仍然要差分。做法是 docs/http-snapshot.md §6
的**回落页**:case 带 `"fallback": "<页名>"`,回放层对**未命中**的请求返回
`_fallback/<页名>.json` 里那张合成页面。两侧看到的输入完全相同,差异就只可能
来自实现。页名有三种:`html-<内容哈希>` 与 `json-<内容哈希>`(都是
**一个「源 × 步」一张**),以及合不出结构时退回的两张**底板** `html` / `json`。

代价与边界,写清楚免得把这套的 pass 率读大了:

- `requests` 序列不再受快照约束(URL 构造仍逐跳比对,只是不会因为"没录到"而中断);
- 页面与规则不成对,命中率有限 —— 大量 case 落在「空结果 / `toc_empty` /
  `content_empty`」分支上。这些分支本身也是要对齐的语义,但它们**不**覆盖字段装配;
- `pipeline_error` 是**粗粒度归一**(两侧只要求"抛了"),这一档的一致不等于同因。

所以本套的 pass 率要连着下面的「命中面」一起看。

## 回落页怎么来的

**两张页由两份生成器造,机制不同 —— 原因见下面第二段。**

### HTML 页(`tools/fallback_page.py`,**一个「源 × 步」一张**)

按**那个源自己的规则**反向合成,照真身 `AnalyzeByJSoup` 的两条口径:

- **列表规则**(`getElements`):按 `@` 切开后每一段都是元素步,**没有动作段**。
  逐段造一层能被它选中的最小 DOM,最内层重复 3 份(带 `.1` / `!0` / `[2]` 索引的
  规则也就有东西可选;索引本身决定那一层造几份)。
- **字段规则**(`getStringList`):**末段永远是动作** ——
  `text`/`textNodes`/`ownText`/`html`/`all` 取文本,**其余一律 `element.attr(末段)`**
  (所以 `bookUrl: "href"` 是「取当前元素的 href」,而 `name: "h4"` 取的是名叫 h4
  的属性、不是选 h4 元素)。前面那几段造成容器里的一条路径,值按动作落到
  文本或同名属性上。
- 两条路上的元素都补合法父子:`<tr>` 里包 `<td>`、`<ul>` 里包 `<li>` ——
  少这一层,内容会被 HTML 解析器 **foster parenting** 挪到表外,
  那批 case 测的就不是规则而是两个解析器对畸形 HTML 的分歧(见
  `fallback_page.py` 的 `PAYLOAD_WRAPPER` / `CHILD_OK`)。
- 末段不是合法属性名的(书源里塞了半截 JS 的那种)**不落地**:
  造出 `<div result.replace(…)="…">` 只会让两个解析器在畸形属性名上分歧。

只认 jsoup 侧的写法(`id.x` / `class.x` / `tag.x` / `#x` / `.x` / `[k=v]` /
后代空格 / `@` 链),jsonpath / XPath / 正则 / JS 写法跳过。相同形态的源合成出
逐字节相同的页面 → 按内容哈希去重,文件名 `_fallback/html-<hash>.json`。
**造页面的理解只可能造成"选不中"**(退回空结果分支),不可能造出假 PASS ——
两侧看到的是同一张页面。

合不出结构的源退回**底板** `_fallback/html.json`:那是「按全语料的选择器分布
合成的一张大页」(取所有去重后的容器选择器各造一块,拼进固定骨架)。

### JSON 页(`tools/fallback_json.py`,同样**一个「源 × 步」一张**)

按**那个源自己的 JSONPath** 反向合成:入口规则(`bookList` / `chapterList` /
`content` / `ruleBookInfo.init`)决定容器形状,字段规则(`name` / `chapterUrl` / …)
相对列表项合成进 item,`{{$.x}}` / `{$.x}` 这类**内嵌规则**(`SourceRule.makeUpRule`
里 `isRule()` 认的那一档)与 `java.getString('$.x')` 一并收进来。相同形态的源
合成出逐字节相同的文档 → 按内容哈希去重,文件名 `_fallback/json-<hash>.json`。

**两张页都走到「按源一张」的过程**(这一段是教训,别再走回去):

- JSON 那张**从一开始就只能按源一张**:CSS 选择器位置无关,一张 DOM 摆得下
  几十种形态的块;JSONPath 除 `$..` 外**从根锚定**,而语料里最常见的容器名恰恰
  互相打架 —— `$.data[*]`(搜索列表,要 data 是数组)、`$.data.entry`(目录列表,
  要 data 是对象)、`$.data.content`(正文,要对象)三选一。共用一张的年代
  JSON 页命中率只有 **4~13%**,改成按源一张(M2m)之后是 **75~92%**。
- HTML 那张共用了很久,因为「容器」形态确实摆得下 —— 漏掉的是**字段**:
  payload 只能有一份,而一个源的 `name` 是 `h4@text`、另一个是 `.s2@a@text`。
  于是容器选中了、`getSearchItem` 拿不到 name,整条照样丢掉(实测 B 层 search:
  183 例是「容器认得、仍然没命中」)。改成按源一张(M2n)之后 html 页命中
  **47% → 70%**(A 层 53% → 76%、top100 68% → 85%)。
  → **「一张页回答所有请求」是最初的用法,不是机制的限制**:回落页的名字本来
  就是 case 自己带的。改不动的时候先回头问「这个约束是机制要求的,还是第一次
  写的时候顺手定的」。

合成不出任何结构的源(入口规则是 `@js:` 之类)拿到的是**底板**,
逐字节还是改造前那两张。所以这两改**只可能往上抬命中面**。

页面越像真站点越好(「合成的回落页会自己造出分歧」是 M2k 踩过的坑):
键名像时间戳 / id 的给数字(`update_time` → `1700000000`),被塞进
`timeFormat(...)` / JS 算术的路径也给数字,过滤器 `[?(@.k != 1)]` 给让它
**成立**的值。否则测的就不是规则,是 `"玄幻" * 1000` 的 NaN 行为。

## 命中面(pass 率要连着这一段一起读)

`tools/hit_report.py` 每次跟着通过率一起打(打在**最后** —— `all_diff.sh`
每套只留 `tail -40`)。**别手抄这张表**,手算的数字会和代码悄悄漂。
2026-08-31(**M3a 修分层口径**那一轮,A 层 879 → 908 源)收盘:

| 步 | 共 | 命中 | 源没配 | 配了没命中 | (html 页) | (json 页) |
|---|---|---|---|---|---|---|
| search | 908 | 721/740 (97%) | 168 | 19 | 623/810 (76%) | 98/98 (100%) |
| explore | 395 | 326/329 (99%) | 66 | 3 | 305/365 (83%) | 21/30 (70%) |
| info | 908 | 627/636 (98%) | 272 | 9 | 601/875 (68%) | 26/33 (78%) |
| toc | 908 | 744/758 (98%) | 150 | 14 | 718/882 (81%) | 26/26 (100%) |
| content | 908 | 902/908 (99%) | 0 | 6 | 873/879 (99%) | 29/29 (100%) |
| **合计** | **4027** | **3320/3371(98%)** | **656** | **51** | **3120/3811 (81%)** | **200/216 (92%)** |

**「命中」那一列的分母已扣掉「源没配这一步的入口规则」那一档**(M2o 起的口径,
`hit_report.py` 的 `ENTRY_RULE`):`ruleToc.chapterList` 为空、整个 `ruleExplore`
缺席…… 裁判也拿不到东西,留在分母里会把「这个源没有探索页」读成缺口。
html/json 两列是**未扣**的原始数(按回落页口味分),两套口径并排放着。

一路抬上来的三段:M2m 收盘 2163/3894 = 55%(json 页按源反向合成);
M2n「HTML 页也按源一张」抬到 3001 = 77%;2026-08-31 那一轮把生成器认得的
**形态**补齐(索引串/负索引照 `findIndexSet` 数全、`CHILD_OK` 只留 table 一族、
不再用白名单拦未知标签、切掉尾巴上的 `<js>` 后处理块)、口味改成**按哪张页造得出**
判而不是按 `$` 前缀,3088/3247 (95%) → 3162/3246 (97%),未命中 159 → 84。
再之后一轮补了两处形态(**`@put:{…}` 剥掉**、**XPath 翻成选择器再合成**,
逐条理由见 `fixtures/cases/pipeline-corpus-b/README.md` 的「三轮做了什么」),
到 **3185/3246 (98%)**,未命中 84 → 61;再一轮补口味的三处
(`[name=og:x]` 别被当伪类截断、`[*]` 是 JSONPath 独有写法、`info` 步单段裸键
只能发 JSON 页)到 **3198/3246 (98%)**,未命中 → **48**。

命中的定义(只看**裁判侧** —— 它是行为规范):`search`/`explore` → `books` 非空,
`toc` → `chapters` 非空,`content` → `content` 非空,`info` → **`book.name` 非空**。
`info` 为什么不看 tocUrl:`BookInfo.kt L158` 把空 tocUrl 兜底成 baseUrl,
它**永远非空**,拿它当命中信号会把整套读虚。

A 层里 JSON 口味的步只有 202/3894(5%)—— 走 JSON API 的源基本都带 JS,
落在 B 层。所以这一套里**html 那一列才是大头**。

## 语料范围与剔除

- 分层口径**只有一份**:`tools/phase3_caps.py` 的 `classify()` ——
  **A 层 = 无 `<js>` / `@js:` / `jsLib`,且四步路径上没真的用到 `webView` /
  `startBrowser` / `queryTTF` 之类 Phase 3 能力**。1704 源 → A 层 **908**。
  **「真的用到」是按值判定的**(M3a 修的 bug):老口径拿整份书源的 JSON dump
  找裸子串,而 `ContentRule` 默认就带 `"webJs": ""` 与 `"sourceRegex": ""`
  两个空键 —— 92 个源(29 个本该 A、63 个本该 B)被这两个**空键**踢进 C 层、
  整源缺席这两套差分。口径与清单见 `fixtures/phase3/README.md`。
- **XPath 源自 Phase 2 (M2g) 起入册**:此前规则段以 `@XPath:` 或 `/` 开头的源
  整源不入册(Phase 1 两侧都不实现 XPath,混进来就是把"未实现"算进 pass 率),
  共 33 源。现在两侧都实现了(`xpath-compat` / JsoupXpath),不再剔除。
  ~~注意 `tools/fallback_page.py` 反向合成回落页时**仍然跳过 XPath 选择器**~~
  ——2026-08-31 那一轮起 `_xpath_to_css` 把认得的 XPath 形态翻成选择器再合成
  (翻不动的仍退回底板,那只影响命中率,不影响一致性)。这批源退回底板,而底板上照样能选中
  东西:pc03399 就是这么把「XPath 选出来的 JXNode 交给下一条规则时丢了元素身份」
  照出来的(见 `RuleValue::JxList` 的注释)。
- 每个入册的源出 4–5 个 case:`search` / `explore`(有 `exploreUrl` 才出)/
  `info` / `toc` / `content`。`info` / `toc` / `content` 的入口 URL 用
  `<bookSourceUrl>/rb/{b1,t1,c1}.html` 这样的固定路径(页面反正是回落页)。

## 豁免

生成器写 `exemptions.json`,逐条带理由。**M2n 起本套是 0 条** ——
唯一用得上的那一类(IDN)已经不需要了:

1. ~~**bookSourceUrl 的 authority 含非 ASCII**(本套 4 源 17 例)~~:
   **M2n 在 `net::http_url` 接上了 okhttp 的 `idnToAscii`**(UTS-46 + NFC +
   punycode),这 17 例现在照常比、全 PASS。
2. **这一步用到的 `##` 正则在 regex 套里已是已知回避面**(本套 0 例,B 层 1 例):
   有界变长 lookbehind,Java 支持而 fancy-regex 0.14 只支持定长。名单**直接读
   `fixtures/cases/regex-compat/exemptions.json` 的 `pattern` 字段**,
   不在这里另抄一份 —— 同一件事两处记迟早漂。

它们仍然跑、仍然比,只是差异不算 FAIL,数量在 diff 报告里可见;而且只在
**两侧真的不一致时**才生效(比较器先判等、再判豁免)。

## 翻页为什么不会跑飞

回落页里所有 `href` 都是**同目录相对路径**。翻页规则命中时,第二跳把同一个相对
路径按新 base 解析,落回同一个绝对 URL,撞上 `BookChapterList` / `BookContent`
的 `nextUrlList` 去重表即停 —— 页数天然有界。

## 这一套抓出来的东西

都是「旧基准(legado-with-MD3)留下的截断」,LegadoTeam@3046111c 里都没有:

- `BookInfo` 的 `book.kind` 与 `BookList` 的 `searchBook.kind`:被测侧截到 1000 字符,
  真身是 `joinToString(",")`,**不截断**;
- `SearchBook` 的构造 init 块:旧基准对 `kind` / `intro` / `latestChapterTitle`
  分别截到 1000 / 5000 / 200,新基准**没有这个 init 块**。被测侧的
  `apply_ctor_truncation` 已删。

这三处在手工套里照不出来(合成页取不到超长串),是真实规则打在合成页上
(泛选择器一次选中几十个块、拼起来上千字)才炸出来的。
