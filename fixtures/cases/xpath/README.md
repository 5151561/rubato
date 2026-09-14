# xpath 差分用例契约

两侧执行器与比较口径同 `fixtures/cases/rule-syntax/README.md`。入口:`tools/xpath_diff.sh`。

- 裁判:`judge/harness` 里**原样挂 `judge/engine/.../AnalyzeByXPath.kt`**,
  底下是**真 JsoupXpath 2.5.5**(`cn.wanghaomiao:JsoupXpath`,`org.seimicrawler.xpath`)。
- 被测:`rust/crates/xpath-compat` + `difftest` 的 `case_runner`(op `xpathDsl`)。

## 为什么不是"序列化成 XML 交 libxml2"

plan §1 的选型表里 XPath 那格写的是「Phase 2 将 DOM 序列化为 XML 交 libxml2 求值」。
真读了裁判之后这条路**不成立**:`AnalyzeByXPath` 底下不是标准 XPath 引擎,是
JsoupXpath —— 一个直接跑在 jsoup 树上的方言实现。差别不是边角:

- 多出 `allText()` / `html()` / `outerHtml()` / `num()` / `node()` 五个节点测试,
  以及 `^=` `$=` `*=` `~=` `!~` 五个运算符、`following-sibling-one` /
  `preceding-sibling-one` 两个轴、`substring-ex` / `substring-after-last` 等函数;
- **`[n]` 不是文档位置**,是"在**当前上下文集合**里、同一个父下、同标签兄弟中的
  第几个",而且支持 `[-1]`(倒数)、越界钳到 1;
- 文本节点被包成**合成元素**(`JX_TEXT`)继续参与后续步骤与谓词;
- `html` / `text` / `node` / `num` / `comment` / `allText` / `outerHtml` 是**词法保留字**,
  于是 `/html` 是语法错误而不是"选不中";
- 文法的 `main` **没有 EOF**,尾巴上的记号一律无视 —— 语料里大把
  "其实是相对 URL"的规则(`/api/book-info?id=…`)因此静悄悄地返回空串。

判据是与裁判逐字节相同,所以走的是**照着 JsoupXpath 移植**这条路;解析树仍然
只有 html5ever 一份(plan §2 的约束没破)。

## 用例格式

```jsonc
{
  "id": "xp00001", "op": "xpathDsl",
  "doc": "book",              // defineDoc 注册过的页面名(与 html 二选一)
  "html": "<td>甲</td>",       // 直接内联的内容
  "contentType": "string",    // string(默认)| document | element
  "at": "#main",              // contentType=element 时:取该选择器的第一个元素
  "rule": "//dd[last()]",
  "mode": "string" | "stringList" | "elements",
  "note": "人读的说明,只给 FAIL 定位用,不参与比较"
}
```

`contentType` 对应 `AnalyzeByXPath(doc: Any)` 的三条分支:

- `string` → `strToJXDocument(s)`:以 `</td>` 结尾先包 `<tr>`,再(此时已以
  `</tr>` 结尾)包 `<table>`——**一段 `</td>` 会被连包两层**;trim 后以 `<?xml`
  开头的走 `Parser.xmlParser()`;
- `document` → `JXDocument.create(doc)`,根上下文是 `doc.children()`,
  也就是 `<html>` **这一个元素**(不是 Document,`/` 从 `<html>` 的子节点起算);
- `element` → `JXDocument.create(Elements(el))`,根就是那个元素。

**文件名带序号**:`case_diff.sh` 按 glob 序合并 `*.json`,`defineDoc` 必须排在
用到它的 case 之前,所以是 `01-docs` / `02-probe` / `03-corpus`。

## 输出

```jsonc
{"id":"…","result": <按 mode 而定>, "kinds": ["el","str",…]}
{"id":"…","error":"xpath_error"}
```

- `string` → 字符串或 null;`stringList` → 字符串数组;
  `elements` → 各 `JXNode.asString()` 的数组(null 时输出 null),
  外加 `kinds`:每项 `isElement()` 为 `el` / `str`。
  `asString()` 的分支:JX_TEXT 合成元素给 `ownText()`,其余元素给 `toString()`
  (jsoup prettyPrint 的 outerHtml),非元素给自身字符串。
- `xpath_error`:任何 Throwable。`JXDocument.selN` 把**所有**失败都裹成
  `XpathSyntaxErrorException`,ANTLR 的报错文案不该逐字比,故归一成一个标签;
  真因打到 **stderr**(`xpath_error <id> <rule> :: <异常>`),定位 FAIL 时看它。

## 语料规则怎么裁

喂给 `AnalyzeByXPath` 的**不是**书源里那条整规则 —— 上游的 `AnalyzeRule` 先切过:

- `##正则` 归 `replaceRegex`,不进 XPath;
- `<js>` / `@js:` 由 `splitSourceRule` 切成独立 SourceRule,不进 XPath;
- `{{…}}` 由 `makeUpRule` **先求值再拼回**,XPath 看到的是**替换后**的串;
- `@XPath:` 前缀由 `SourceRule.init` 剥掉。

`tools/gen_xpath_cases.py` 按同样口径裁,`{{…}}` 换成占位串 `X1` / `X2` ——
**形状保真、取值不保真**。取值那一面由 rule-engine 套(整条规则跑 AnalyzeRule)
覆盖,两套分工。

1611 源里 107 个用了 XPath,去重后 131 条表达式,每条 × 3 个页面 × 3 个 mode。

## 判据边界:裁判自己不可复现的那一处

`descendant::` / `descendant-or-self::` 轴与 `..` 都用 **`HashSet<Element>`** 归并,
而 jsoup 的 `Element` 没覆写 `hashCode`(identity hash)。于是**命中多于一个元素时,
迭代序跨进程随机**:实测同一条 `//dl/descendant::a`,`string` 与 `stringList` 两个
case 给出的顺序就不一样(两次跑整套也不一样)。

这类用例**不进套**:探针里 `descendant::` 只探单命中形态,`..` 的用例都落在
"上下文元素共一个父"(HashSet 里只有一个元素)上。被测侧实现成文档序去重 ——
是个确定的选择,但它与裁判的一致**只在单命中时成立**,别指望多命中。

判据仍然是**两侧各连跑两次、输出逐字节相同**;上面那条就是这么照出来的。

## 已知近似(进 exemptions.json)

- **`<?xml` 开头的内容**:`strToJXDocument` 走 `Parser.xmlParser()`,而
  html-compat 只有 HTML 解析器(plan §2 明写解析树只留 html5ever 一份)。
  XML 内容会被套上 `html/head/body`、`<link>` 被当空元素。**这条分支由内容触发
  而不是规则触发**,语料里没有规则依赖它,故留豁免而不是再引一个 XML 解析器。
- **wiki 页的无值属性**:与 `fixtures/cases/html-compat/exemptions.json` 同一条
  已知近似(html5ever 分词器把 `<a data-x>` 与 `data-x=""` 都归一为空串值)。

## 本套抓到的方言事实(实测,不是读代码猜的)

1. **`/html` 是语法错误**。`html` 与 `text` / `node` / `num` / `comment` /
   `allText` / `outerHtml` 一样是 NodeType 保留字,而文法的 `nCName` 只收
   `NCName | AxisName` —— 名字里没有 NodeType。`//htmls` 反而没事(更长,落回 NCName)。
2. **`main` 没有 EOF**。`//a xyz` 给全部 `a`;`concat('a','b') junk` 给 `"ab"`。
   语料里 20 多条"相对 URL 被当成 XPath"的规则全靠这条不报错。
3. **带冒号的 qName 不是名字测试**。`visitQName` 的 `size > 1` 分支返回
   `XValue.create(join(":"))` 而**没调 `.exprStr()`**,于是 `visitStep` 既不当它是
   名字也不当它是节点集 —— 整个 step 把这个**字符串当结果抬走**。
   `//p:q` 的结果就是字面量 `"p:q"`;语料里 `//javascript:gotochapter('2332','577419')`
   靠的就是它(顺带,`('2332','577419')` 那截被第 2 条无视掉)。
   但 `@get:{bid}` 那种写法里它照样能当**属性名**用(`nodeTest.asString()` 不看这个标志)。
4. **ANTLR 的 `sync()` 会静默删记号**。`DoFailOnErrorHandler` 只覆写了
   `recover` / `recoverInline`,没覆写 `sync` —— 可选块 / 循环入口上前瞻不匹配时,
   `consumeUntil(follow set)` 只报 unwanted token 不抛。语料里 `[@href!~='…']`
   多出来的那个 `=` 就是这么被吞掉的。
5. **`node()` 遇到非空 ownText 必炸**。真身是 `new Element("")`,jsoup 的
   `Tag.valueOf` 对空标签名抛 `IllegalArgumentException`。ownText 全空白时反而没事
   (那段 `if (isNotBlank(txt))` 不进)。
6. **合成元素的标签决定它怎么序列化**。`Element("V")` / `Element("JX_TEXT")` 是
   **未注册标签**(jsoup 的 `formatAsBlock` 字段默认 true)→ `<V>\n x\n</V>`;
   而 `following-sibling` 包文本用的 `Element("text")` —— 这个也**不在** jsoup
   1.16.2 的 Tag 表里,同样按未注册标签缩进。
   **这一条顺手修好了 html-compat 的一个老账**:它的 `INLINE_TAGS` 多列了
   `text` / `mi` / `mo` / `msup` / `mn` / `mtext` 六个,jsoup 1.16.2 里根本没有它们
   (逐个探针实测)。html 套 23077 例从没碰到这些标签,所以一直没现形。
7. **合成元素也有 `textNodes()`**。`Element.text(s)` 塞的那一个 TextNode 是真的,
   于是 `//dt/following-sibling::*/text()` 能从 `<text>` 合成元素里再抠出文本
   —— 裁判给的是 11 个 JX_TEXT(10 个空 + 1 个 "正文卷")。
8. **递归 `text()` 会把合成元素的父指回原树**(反射 `setParentNode`),
   非递归那支不设。这条线不是摆设:`ownText()` 沿父链找 `preserveWhitespace`,
   于是 `<pre>` 里的文本在 `//pre//text()` 下**不归一空白**、在 `//pre/text()` 下归一。
9. **`last()` 与谓词里的 `[n]` 数的不是同一个东西**。`[n]` 走
   `preComputePredicateIndices`(只数**在上下文集合里的直接子元素**),
   而 `last()` 走 `CommonUtil.sameTagElNums` —— `parent.getElementsByTag()`,
   数的是父元素**整棵子树**里的同名标签。
10. **`num()` 抽不到数字时给 `XValue(null)`**,`selN` 一路走到
    `JXNode.create(calRes.asString())` = 字符串 `"null"`;而 `sum()` 遇到非数字是
    **`return null`(Java null)**,走的是 `calRes == null` 那条 → 空串。两条别混。
