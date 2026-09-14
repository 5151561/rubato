//! 打破 `rule-engine ↔ js-host / net` 环依赖的接口层。
//!
//! rule-engine 只认这里的 trait;js-host(QuickJS)与 net(AnalyzeUrl)在
//! engine 组装时注入实现,差分里注入的是确定性桩(契约见
//! fixtures/cases/rule-engine/README.md)。

use serde_json::Value;

/// `evalJS` 的返回形态——AnalyzeRule 对 String / Double / List / null 的
/// 分支处理不同,故必须区分
#[derive(Debug, Clone, PartialEq)]
pub enum JsValue {
    Null,
    Str(String),
    Num(f64),
    Bool(bool),
    /// Kotlin List<String>(toString → `[a, b]`)
    List(Vec<String>),
    /// gson/JSON 对象
    Json(Value),
    /// **jsoup 的 Element / Elements 从 JS 里回来**(`result.toArray().map(…)`
    /// 那一类:元素进 JS、被挑一遍、再原样交回规则层)。
    ///
    /// `handles` 给 rule-engine —— 它能查回实体,元素因此**穿过 JS 而不被拍平**;
    /// `strings` 是同一批值的 `toString()` 形态,给不认识句柄的消费方
    /// (AnalyzeUrl / 差分执行器 / 桩),与本变体存在之前的 [`JsValue::List`]
    /// 逐字一致 —— 加这一支不改它们任何一处的输出。
    ///
    /// `list` = 完成值**自己**是不是一个列表(JS 数组,或 jsoup 的 `Elements`)。
    /// 这一位不能省:`[el]` 与 `el` 在真身那边是两种东西 —— 前者
    /// `getElements` 给一个元素,后者给**空表**(单个 Element 不是 List)。
    Elements {
        handles: Vec<ElementHandle>,
        strings: Vec<String>,
        list: bool,
    },
    /// Rhino 的 **NativeArray,且元素不是标量** —— JS 返回的对象数组
    /// (`@js:….map(x=>({n:x.text(),u:x.attr('href')}))`)。
    ///
    /// 与 [`JsValue::List`] 分开是因为后者把元素**拍成串**,而真身那边元素还是
    /// NativeObject:下游 `chapterName: "n"` 走的是 AnalyzeRule 的
    /// 「NativeObject 键值直接访问」分支(AnalyzeRule.kt L321-331),
    /// 拍平之后那条规则打在 `"[object Object]"` 上,只能是 InvalidJson。
    ///
    /// 元素**全是标量**的数组仍走 `List` —— 那条路十四套差分都绿着,不动它。
    /// 已知口径缺口:JS 的数字经 `JSON.stringify` 过来丢了「Rhino 那边一律是
    /// Double」这一位(`{n:1}` 真身 `toString()` 给 `1.0`,这里给 `1`)。
    /// 语料里对象字段全是串,还没有用例踩到。
    Native(Vec<Value>),
}

/// 真身有**两个** JS 宿主,`java` 指向的对象与绑定面都不同。挑错宿主会把语义
/// 钉歪(searchUrl 片段里的 `key` 在 AnalyzeRule 上是 ReferenceError,而它必须有值)。
/// 由 js-host 差分套的 `host` 维度钉,见 fixtures/cases/js-host/README.md「两个宿主」。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum JsHost {
    /// `AnalyzeRule.evalJS`:`java` **就是 AnalyzeRule** —— JS 能反过来调规则求值
    /// (`java.getString` / `getElements` / `setContent`)。绑 result/baseUrl/src/
    /// title/nextChapterUrl/fromBookInfo/page,**不绑 key**。
    #[default]
    Rule,
    /// `AnalyzeUrl.evalJS`(searchUrl / exploreUrl):`java` 是 AnalyzeUrl ——
    /// 只有 `JsExtensions` 那一面,**没有** getString/getElement/setContent。
    /// 绑 key/page/baseUrl/result,**不绑** src/title/nextChapterUrl/fromBookInfo。
    Url,
    /// `BaseSource.evalJS`(BaseSource.kt L395):source 的 `header` / `loginUrl` /
    /// `loginCheckJs` 那条路径。`java` **就是书源实体本身**(`source` 与 `sourceApi`
    /// 是同一个对象),绑定面最窄 —— 只有 `java`/`source`/`sourceApi`/`baseUrl`/
    /// `cookie`/`cache`,`baseUrl` 是 `getKey()`(bookSourceUrl)而不是页面地址。
    /// `result`/`book`/`chapter`/`page`/`key` 在这里**未声明**(ReferenceError)。
    ///
    /// 另一处要紧的差别:`java.put/get` 打的是 **CacheManager**
    /// (`v_<sourceKey>_<key>`),不是 AnalyzeRule 那套四层变量。
    Source,
}

/// `book` 绑定的可观察面(`Book` 实体上书源 JS 真正读得到的字段)。
/// 变量层(`putVariable`/`getVariable`)不在这里——那是 [`VarStore`]。
#[derive(Debug, Default, Clone)]
pub struct BookBinding {
    pub name: String,
    pub author: String,
    pub book_url: String,
    pub origin: String,
    /// `Book.kind` / `SearchBook.kind` 在 Kotlin 里是 **`String? = null`**
    /// (name/author/bookUrl/tocUrl 都是非空 `""`,只有它可空)。
    /// 绑成 `""` 会在 `JSON.stringify({BookID: book.kind})` 这类写法上错:
    /// 真身给 `null`,绑成串就成了 `""`。
    pub kind: Option<String>,
    pub toc_url: String,
}

/// `chapter` 绑定的可观察面(`BookChapter`)。真身在 `AnalyzeRule.setChapter`
/// 之后把实体**本身**绑进去(AnalyzeRule.kt L902),书源在正文/目录规则里读
/// `chapter.title` / `chapter.url` / `chapter.index` 很常见。
///
/// 它是**活的**:真身那边规则跑到哪一步、实体就已经被写到哪一步
/// (目录循环里 `chapterUrl` 的 JS 读得到刚算出来的 `chapter.title`)。
/// 组装方要在每次改动之后刷新这份绑定。
#[derive(Debug, Default, Clone)]
pub struct ChapterBinding {
    pub url: String,
    pub title: String,
    pub base_url: String,
    pub book_url: String,
    pub index: i32,
    pub is_volume: bool,
    pub is_vip: bool,
    pub is_pay: bool,
    pub tag: Option<String>,
}

/// `source` 绑定的可观察面(`BookSource`)。`raw` 是原始 JSON,
/// 书源 JS 会读任意字段(`source.ruleSearch` 之类),故整份带着。
#[derive(Debug, Default, Clone)]
pub struct SourceBinding {
    /// `BaseSource.getKey()` —— BookSource 是 `bookSourceUrl`
    pub key: String,
    /// `BaseSource.getTag()` —— BookSource 是 `bookSourceName`
    pub tag: String,
    /// 整份 BookSource JSON(字段按原名暴露给 JS)
    pub raw: Option<Value>,
}

/// `result` / `src` 两个绑定的形态。
///
/// 真身 `bindings["result"] = result` 绑的是**对象本身**(`Any?`),不是它的
/// 字符串形态:jsoup 的 `Elements` 是 `java.util.List`,Rhino 包成
/// NativeJavaList —— `result.toArray()` / `result.parentNode()` / `result[0]`
/// 在真身里都能调。拍平成串之后这些一律 `TypeError: not a function`,
/// 而 `result[0]` 还会从「元素」悄悄变成「首字符」。
///
/// 只有这两个绑定要分形态:别的(`title`/`baseUrl`/`nextChapterUrl`…)在真身
/// 那边本来就是 `String?`。契约见 `fixtures/cases/pipeline-corpus-b/README.md`。
#[derive(Debug, Clone, PartialEq)]
pub enum BoundValue {
    /// Kotlin `String` —— `Context.javaToJS` 之后是 **JS 原生 string**
    /// (js-host 探针 `bind-result-typeof` 实测:`typeof result === "string"`)
    Str(String),
    /// `java.util.List<String>`:NativeJavaList —— 有下标 / `size()` / `get()`,
    /// **没有** `map`/`join`(那两个在真身里是 TypeError,见 java_proxy 的 `javaList`)
    StrList(Vec<String>),
    /// Kotlin `Double` —— `Context.javaToJS` 之后是 **JS 原生 number**
    /// (Rhino 的 javaToJS 对 String/Number/Boolean 原样交回)。
    /// 绑成串会在 `makeUpRule` 的 `{{result}}` 那一档上错:真身那里
    /// 整数 Double 走 `String.format("%.0f")` → `1100000001`,
    /// 绑成串就成了 Java 的 `Double.toString` → `1.100000001E9`。
    Num(f64),
    /// Kotlin `Boolean` —— 同上,JS 原生 boolean
    Bool(bool),
    /// jsoup `Element` / `Elements` —— 只过**号**,见 [`ElementHandle`]
    Element(ElementHandle),
    /// **Java 对象**(jayway 读出来的 Map/List、gson 的 LinkedTreeMap、
    /// `List<JXNode>`…)—— Rhino 包成 NativeJavaMap / NativeJavaList,见 [`JavaValue`]。
    /// `init: $.xxx` 之后那一步的 `result` 就是这一支。
    Java(JavaValue),
    /// Rhino 的 **NativeObject / NativeArray** —— `<js>` 自己造出来的对象。
    /// 与 [`BoundValue::Java`] 相反:它是**真 JS 对象**,不是 Java 对象的包装
    /// (`String(result)` 是 `[object Object]`、`hasOwnProperty` 在、
    ///  `instanceof Object` 为真),探针 `js-bind-native-*` 实测。
    Native {
        json: serde_json::Value,
        /// Kotlin 侧 `Any.toString()`(`[object Object]` / 身份哈希)
        str: String,
    },
    /// `StrResponse` —— 只有 `loginCheckJs` 那条路绑它(真身
    /// `analyzeUrl.evalJS(checkJs, res)`,`res` 就是刚拿到的响应)
    Response(NetResponse),
}

impl BoundValue {
    /// `Any.toString()` 形态。给不认识句柄的消费方(差分桩等)——
    /// 元素要查表,故要 [`RuleHost`]。
    pub fn to_java_string(&self, env: &mut dyn RuleHost) -> String {
        match self {
            BoundValue::Str(s) => s.clone(),
            // Kotlin List<String>.toString()
            BoundValue::StrList(v) => format!("[{}]", v.join(", ")),
            BoundValue::Num(d) => crate::java_double_to_string(*d),
            BoundValue::Bool(b) => b.to_string(),
            BoundValue::Element(h) => env.el_to_string(*h),
            BoundValue::Java(v) => match v {
                JavaValue::Bool(b) => b.to_string(),
                JavaValue::Null => "null".into(),
                JavaValue::Element(h) => env.el_to_string(*h),
                _ => v.java_string().unwrap_or_default().to_string(),
            },
            BoundValue::Native { str, .. } => str.clone(),
            // okhttp `Response.toString()`
            BoundValue::Response(r) => {
                format!("Response{{protocol=http/1.1, code={}, message=, url={}}}", r.code, r.url)
            }
        }
    }
}

/// 传给 JS 的绑定(裁判侧 `buildScriptBindings` 的可差分子集)
#[derive(Debug, Default, Clone)]
pub struct JsBindings {
    /// 哪个宿主(决定绑定面与 `java` 的方法面),见 [`JsHost`]
    pub host: JsHost,
    /// `SourceLoginDialog` 在 `BaseSource.evalJS` 的窄绑定面上额外塞入
    /// `result/book/chapter/isLongClick`。普通 source header/loginCheckJs 求值恒假，
    /// 防止为了登录界面把第三宿主原本的 ReferenceError 语义改掉。
    pub source_login: bool,
    /// 规则链上的当前值。**不是串** —— 见 [`BoundValue`]
    pub result: Option<BoundValue>,
    pub base_url: Option<String>,
    /// `AnalyzeRule.content`(JS 里的 `src`)。与 `result` 同理是 `Any?`
    pub src: Option<BoundValue>,
    pub title: Option<String>,
    pub next_chapter_url: Option<String>,
    pub from_book_info: bool,
    /// AnalyzeUrl 独有:搜索关键字与页码
    pub key: Option<String>,
    pub page: Option<i32>,
    /// `book` 绑定(两个宿主都绑;缺席时 JS 里是 null)
    pub book: Option<BookBinding>,
    /// `source` 绑定(两个宿主都绑)
    pub source: Option<SourceBinding>,
    /// `chapter` 绑定。**只有 [`JsHost::Rule`] 有** —— AnalyzeUrl.evalJS
    /// 的 bindings 里根本没有这个名字(AnalyzeUrl.kt L378-393)。
    pub chapter: Option<ChapterBinding>,
}

/// JS 里 `java.put/get` 打到的变量存储(由 rule-engine 提供)
pub trait VarStore {
    fn put(&mut self, key: &str, value: &str) -> String;
    fn get(&self, key: &str) -> String;

    /// `Book.putVariable` / `BookChapter.putVariable` —— 打的是**那一个实体自己
    /// 那一层**,不是 `AnalyzeRule.put` 的优先级链。两者在章节在场时不是一回事:
    /// `java.put` 写章节层,而 `book.putVariable` 写书层(真身
    /// `BaseBook.putVariable` / `BookChapter.putVariable` 各写各的 variableMap)。
    /// 真身两个方法都**无条件 `return true`**。
    ///
    /// 缺省空实现:变量层不分实体的宿主(`AnalyzeUrl`、`CacheManager` 那两条)
    /// 根本没有这两个绑定。
    fn put_entity(&mut self, _entity: VarEntity, _key: &str, _value: &str) {}

    /// `Book.getVariable` / `BookChapter.getVariable` —— 同样只看自己那一层
    /// (`variableMap[key] ?: getBigVariable(key) ?: ""`),取不到给**空串**。
    fn get_entity(&self, _entity: VarEntity, _key: &str) -> String {
        String::new()
    }
}

/// [`VarStore::put_entity`] / [`VarStore::get_entity`] 指的是哪一层
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarEntity {
    Book,
    Chapter,
}

/// 元素句柄。jsoup 的 `Element`/`Elements` 是 html-compat 的类型,不能出现在
/// core 的接口上(依赖方向不允许),故只过**号**——实体留在规则引擎里。
pub type ElementHandle = u32;

/// `AnalyzeRule.getElements` 交回来的那个 `List<Any>` 的**形态**。
///
/// 容器是 jsoup 的 `Elements` 还是普通 `ArrayList`,书源看得见的差别有两处 ——
/// 差分实测(fixtures/cases/js-host 的 `e-json-*` / `e-html-*` 一组探针):
/// - `toString()`:`Elements` 是**各 outerHtml 以换行相连**,`ArrayList` 是 `[a, b]`;
/// - **过不过得了 GSON**:`Elements` 抛 JsonIOException,`ArrayList` 给得出。
///
/// 哪一档由**规则模式**决定,不由内容:`Mode::Default` 走
/// `AnalyzeByJSoup.getElements` → `Elements`;`Json`(jayway `getList`)、
/// `XPath`(`List<JXNode>`)、`Regex`、`Js` 那几条都是普通 `ArrayList`。
/// 此前被测侧一律按 `Elements` 建箱 —— JSON 页上 `java.getElements` 的
/// 完成值形态整片是错的。
pub struct ElementList {
    /// 各项的句柄(下标 / `get(i)` / `size()` 用)
    pub items: Vec<ElementHandle>,
    /// 容器是 jsoup 的 `Elements` 吗
    pub jsoup: bool,
}

/// **Java 对象在 JS 里的形态**(Rhino 的 `NativeJavaMap` / `NativeJavaList` /
/// 装箱的 `java.lang.String` / `Integer`)。
///
/// jayway 从 JSON 页读出来的 Map/List 交给 JS 时,Rhino 的 `WrapFactory`
/// **逐层**包装:容器按键(下标)取属性、`for..in` 只给键、**没有**
/// `Object.prototype`(`hasOwnProperty` 是 undefined、`instanceof Object` 为假),
/// 而**每个取出来的值又各自装一次箱** —— jsharness 探针实测(`js-bind-*`):
///
/// | 表达式 | 真身 |
/// |---|---|
/// | `typeof result` | `"object"` |
/// | `result.anchor` | 键值(**不是** `String.prototype.anchor`) |
/// | `typeof result.n` | `"object"`(装箱的 `java.lang.String`) |
/// | `result.n === '书名A'` | `false`(`==` 为真) |
/// | `typeof result.hasOwnProperty` | `"undefined"` |
/// | `String(result)` | Java 的 `toString()` |
/// | `JSON.stringify(result)` | 真 JSON |
///
/// **每个节点自带 `toString()`**:它按容器的**出身**走不同口径 ——
/// json-smart 的 `JSONArray` 是 JSON 文本、gson 的 `ArrayList` 是 `[a, b]`,
/// 同一棵 `serde_json::Value` 判不出来。故由建树的那一侧(rule-engine,
/// 它手上是 [`RuleValue`](../../rule_engine/value/enum.RuleValue.html))算好,
/// js-host 只照着建,不在 JS 侧重算一份。
#[derive(Debug, Clone, PartialEq)]
pub enum JavaValue {
    /// 装箱的 `java.lang.String`:`typeof` 是 `"object"`、`.length` 是**方法对象**
    Str(String),
    /// 装箱的 `Integer` / `Double`:`typeof` 是 `"object"`,`toString` 是 Java 的
    Num(f64, String),
    /// 装箱的 `Boolean`
    Bool(bool),
    /// Java 的 null(JS 里就是 `null`)
    Null,
    /// `java.util.Map` → NativeJavaMap。插入序要留着(`for..in` 的次序是面)
    Map(Vec<(String, JavaValue)>, String),
    /// `java.util.List` → NativeJavaList
    List(Vec<JavaValue>, String),
    /// jsoup 的元素(`List<JXNode>` 里那些):只过**号**,形态与
    /// [`ElementList`] 里的项同一份
    Element(ElementHandle),
}

impl JavaValue {
    /// 这个节点的 `toString()`
    pub fn java_string(&self) -> Option<&str> {
        match self {
            JavaValue::Str(s) => Some(s),
            JavaValue::Num(_, s) => Some(s),
            JavaValue::Map(_, s) | JavaValue::List(_, s) => Some(s),
            // 元素要查表(句柄不认识自己),布尔/null 由调用方直接给
            JavaValue::Bool(_) | JavaValue::Null | JavaValue::Element(_) => None,
        }
    }

    /// **过 GSON 的形态**;`None` = 树里有 jsoup 元素 → GSON 在它上面抛
    /// (裁判侧 `host:JsonIOException`)。
    pub fn to_json(&self) -> Option<serde_json::Value> {
        Some(match self {
            JavaValue::Str(s) => serde_json::Value::String(s.clone()),
            JavaValue::Num(d, _) => {
                serde_json::Number::from_f64(*d).map(serde_json::Value::Number)?
            }
            JavaValue::Bool(b) => serde_json::Value::Bool(*b),
            JavaValue::Null => serde_json::Value::Null,
            JavaValue::Map(entries, _) => serde_json::Value::Object(
                entries
                    .iter()
                    .map(|(k, v)| v.to_json().map(|j| (k.clone(), j)))
                    .collect::<Option<serde_json::Map<_, _>>>()?,
            ),
            JavaValue::List(items, _) => serde_json::Value::Array(
                items.iter().map(JavaValue::to_json).collect::<Option<Vec<_>>>()?,
            ),
            JavaValue::Element(_) => return None,
        })
    }
}

/// jsoup 的 `Elements.text()` / `html()` / `outerHtml()` 拼接口径:
/// **分隔符只在缓冲区非空时才加**(反编译 jsoup 1.16.2 的 `Elements.text()`:
/// `if (sb.length() != 0) sb.append(sep)`)。
///
/// 与 `join(sep)` 只差在**空串项**上,而那正是常见形态:`select('h2')` 命中
/// 三个空 `<h2>` 时 jsoup 给 `""`,`join(" ")` 给 `"  "` —— 差分里就是
/// `title + '：' + …` 前面多两个空格(pb00024)。
pub fn jsoup_join<S: AsRef<str>>(parts: &[S], sep: &str) -> String {
    let mut out = String::new();
    for p in parts {
        if !out.is_empty() {
            out.push_str(sep);
        }
        out.push_str(p.as_ref());
    }
    out
}

/// jsoup 的 `Element.hasClass(name)`:class 属性按空白切成 token,
/// **大小写不敏感**地比一个整 token(实测 jsoup 1.16.2:`class="Vip  x"`
/// 上 `hasClass("vip")` / `hasClass("VIP")` 都为真);空名字恒为假。
/// `Elements.hasClass` 是「其中任意一个」——那一层在调用方。
///
/// **外加一条等长快路**:jsoup 1.16.2 在切 token 之前先比整串 ——
/// ```java
/// if (len == 0 || len < wantLen) return false;
/// if (len == wantLen) return className.equalsIgnoreCase(classAttr);
/// ```
/// 于是**名字里带空格也能命中**:`class="txt-list txt-list-row5"` 上
/// `hasClass("txt-list txt-list-row5")` 为真。这不是纸上推的,是拿 jsoup 的 jar
/// 探出来的(scratch 的 Q.java:整串相等真、`class=" a b "` 假、`class="a  b"`
/// 对 `"a b"` 假 —— 空白一个字节都不许差)。
/// 语料里 **146 处** `class.<带空格的名字>`(`class.txt-list txt-list-row5@tag.li!0`)
/// 走的就是这条路,少了它这些书源的列表规则一条都选不中。
pub fn jsoup_has_class(class_attr: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // 等长快路(整串比)在前:`class` 属性里的空白按原样参与比较
    class_attr.eq_ignore_ascii_case(name)
        || class_attr.split_whitespace().any(|t| t.eq_ignore_ascii_case(name))
}

/// 没有规则面的宿主(AnalyzeUrl / 差分桩)调到反调面时的错误。
/// 真身在那里是 `TypeError: 找不到函数 getString`,被测侧照同一形态报。
pub const NO_RULE_HOST: &str = "«no-rule-host»";

/// **`java` 反过来调规则求值的那一面**。
///
/// 真身的 `java` 绑定**就是 AnalyzeRule 本身**(AnalyzeRule.kt L895
/// `bindings["java"] = this`),所以书源 JS 里能写 `java.getString('.t@text')`
/// 反过来跑规则、`java.setContent(html)` 换掉待解析的内容。
///
/// 只有 [`JsHost::Rule`] 有这一面:`AnalyzeUrl` 虽然也叫 `java`,但它没有这些
/// 方法(探针实测是 TypeError),故这里的缺省实现一律报 [`NO_RULE_HOST`]。
pub trait RuleHost {
    /// `AnalyzeRule.getString(ruleStr, mContent)`。
    ///
    /// **第二个实参是「拿哪份内容跑这条规则」**,不是 isUrl:真身的重载是
    /// `getString(ruleStr: String?, mContent: Any? = null, isUrl: Boolean = false)`,
    /// 书源写 `java.getString("@@html", java.ajax(u))` 就是「在刚拿到的这页上求值」
    /// (js-corpus-8dc9258a1e:漏掉它就跑在**当前** content 上,两侧结果完全不同)。
    /// `content = None` 时用宿主自己的 content。
    fn rule_get_string(&mut self, rule: &str, content: Option<&str>) -> Result<String, String> {
        let _ = (rule, content);
        Err(NO_RULE_HOST.into())
    }

    /// `AnalyzeRule.getStringList(rule, mContent)`;`None` = 规则为空
    fn rule_get_string_list(
        &mut self,
        rule: &str,
        content: Option<&str>,
    ) -> Result<Option<Vec<String>>, String> {
        let _ = (rule, content);
        Err(NO_RULE_HOST.into())
    }

    /// `AnalyzeRule.getElement(rule)`
    fn rule_get_element(&mut self, rule: &str) -> Result<Option<ElementHandle>, String> {
        let _ = rule;
        Err(NO_RULE_HOST.into())
    }

    /// `AnalyzeRule.getElements(rule)`
    fn rule_get_elements(&mut self, rule: &str) -> Result<ElementList, String> {
        let _ = rule;
        Err(NO_RULE_HOST.into())
    }

    /// 这个句柄背后那个值**过 GSON** 是什么形态;`None` = GSON 在它上面抛
    /// (jsoup 的 `Element`/`Elements`:`Document.parser` 那圈自引用,
    ///  裁判侧记 `host:JsonIOException`)。
    ///
    /// 与 [`RuleHost::el_to_string`] 是两条独立的观察面:`getElement('$.list')`
    /// 的 `toString()` 是 json-smart 的 JSON 文本,而它**过得了** GSON;
    /// `getElements('class.t')` 的 `toString()` 是各 outerHtml,而它过不了。
    fn el_json(&mut self, h: ElementHandle) -> Option<serde_json::Value> {
        let _ = h;
        None
    }

    /// 这个句柄背后那个值在 **JS 里的 Java 形态**(见 [`JavaValue`])。
    /// `None` = 它是 jsoup 的元素,该走元素代理那条路。
    ///
    /// **不由这一层现搭**:容器的 `toString()` 判不出出身(json-smart 的
    /// `JSONArray` 是 JSON 文本、gson 的 `ArrayList` 是 `[a, b]`,同一棵
    /// `serde_json::Value` 两者无从分辨),故只有手上有 `RuleValue` 的
    /// 规则引擎给得出。缺省 `None` —— 落回元素代理,与接这一面之前同形。
    fn el_java(&mut self, h: ElementHandle) -> Option<JavaValue> {
        let _ = h;
        None
    }

    /// `org.jsoup.Jsoup.parse(html)`(Rhino LiveConnect:书源真的会直接调 jsoup)。
    /// 走这条反调而不是让 js-host 自己依赖 html-compat,是为了让解析出来的文档
    /// 与 `getElement(s)` 拿到的元素**共用同一套句柄与操作**。
    fn rule_parse_html(&mut self, html: &str) -> Result<ElementHandle, String> {
        let _ = html;
        Err(NO_RULE_HOST.into())
    }

    /// `AnalyzeRule.setContent(content, baseUrl)`
    fn rule_set_content(&mut self, content: &str, base_url: Option<&str>) -> Result<(), String> {
        let _ = (content, base_url);
        Err(NO_RULE_HOST.into())
    }

    // ---- 句柄上的操作(jsoup Element/Elements 的可观察面)----

    /// `Any.toString()`:Element → outerHtml;Elements → 各 outerHtml 以 `\n` 相连
    fn el_to_string(&mut self, h: ElementHandle) -> String {
        let _ = h;
        String::new()
    }
    /// `Element.text()` / `Elements.text()`(后者以空格相连)
    fn el_text(&mut self, h: ElementHandle) -> String {
        let _ = h;
        String::new()
    }
    /// `Element.html()`(内层 HTML)
    fn el_html(&mut self, h: ElementHandle) -> String {
        let _ = h;
        String::new()
    }
    /// `Element.attr(name)`;取不到给空串(jsoup 语义)
    fn el_attr(&mut self, h: ElementHandle, name: &str) -> String {
        let _ = (h, name);
        String::new()
    }
    /// `Element.select(css)`。
    ///
    /// **有错误通道**:jsoup 的 `select` 在选择器不合法时**抛**,不是返回空 ——
    /// `Validate.notEmpty(query)` 给 `ValidationException`,`QueryParser.parse`
    /// 给 `SelectorParseException`。书源真的踩得到(js-corpus-5175e212a0 拼出
    /// `a[href$=]`,整段当场中断)。此前这里没有 Err、调用方一路
    /// `if let Ok(..)` 吞掉,于是被测侧默默返回空、继续往下跑 —— 两侧从这一步
    /// 起分岔。Err 里带 [`host_tag`] 那种书名号标签,交给 js-host 抛成 JS 异常。
    fn el_select(&mut self, h: ElementHandle, css: &str) -> Result<Vec<ElementHandle>, String> {
        let _ = (h, css);
        Ok(Vec::new())
    }
    /// `Elements.size()`;单个 Element 是 1
    fn el_size(&mut self, h: ElementHandle) -> usize {
        let _ = h;
        0
    }
    /// `Elements.get(i)`
    fn el_get(&mut self, h: ElementHandle, i: usize) -> Option<ElementHandle> {
        let _ = (h, i);
        None
    }
    /// `Node.parentNode()` / `Element.parent()`。到根(Document)为止,
    /// 再往上给 `None`(jsoup 那里是 null)
    fn el_parent(&mut self, h: ElementHandle) -> Option<ElementHandle> {
        let _ = h;
        None
    }
    /// `Element.remove()` / `Elements.remove()`:把这些节点从文档里摘掉。
    /// 书源用它净化正文(`doc.select("div.text>*").not("h3,p").remove()`),
    /// 摘完再 `doc.html()` 取整页 —— 所以**改写要落在同一棵树上**。
    fn el_remove(&mut self, h: ElementHandle) {
        let _ = h;
    }
    /// `Elements.not(css)`:把自己匹配 css 的那些去掉(只读,不改树)。
    /// 错误通道同 [`RuleHost::el_select`](底下是同一个 `QueryParser`)。
    fn el_not(&mut self, h: ElementHandle, css: &str) -> Result<Vec<ElementHandle>, String> {
        let _ = (h, css);
        Ok(Vec::new())
    }
}

/// 传给 [`HostEnv::eval_js`] 的环境:变量层 + `java` 的反调面。
/// 两者必须是**同一个对象**——真身里 `java.put` 与 `java.getString` 打的都是
/// 那一个 AnalyzeRule 实例。
pub trait JsRuleEnv: VarStore + RuleHost {}
impl<T: VarStore + RuleHost + ?Sized> JsRuleEnv for T {}

/// JS 宿主 + 网络能力
pub trait HostEnv {
    /// `AnalyzeRule.evalJS`;抛异常对应 Err
    fn eval_js(
        &mut self,
        code: &str,
        bindings: &JsBindings,
        env: &mut dyn JsRuleEnv,
    ) -> Result<JsValue, String>;

    /// `@webjs:` → BackstageWebView 取回的字符串(`AnalyzeRule.getWebJsResult`)
    fn web_js(&mut self, req: &WebJsRequest<'_>) -> Result<String, String>;

    /// `Debug.log` / `AnalyzeRule.log`
    fn log(&mut self, _msg: &str) {}
}

/// `JsExtensions` 那三个 webView 入口的入参(JsExtensions.kt L245-328)。
///
/// `headerMap = getSource()?.getHeaderMap(true)` 与 `tag = getSource()?.getKey()`
/// **不在这里** —— 那两位是书源的,由实现方(宿主的网络面)自己补。
pub struct WebViewReq<'a> {
    pub html: Option<&'a str>,
    pub url: Option<&'a str>,
    /// 取返回值的那句 js;没有就是整页源码(`BackstageWebView` 的缺省 JS)
    pub js: Option<&'a str>,
    /// `webViewGetSource` 那一路
    pub source_regex: Option<&'a str>,
    /// `webViewGetOverrideUrl` 那一路
    pub override_url_regex: Option<&'a str>,
    pub cache_first: bool,
    pub delay_time: i64,
}

/// `AnalyzeRule.getWebJsResult`(AnalyzeRule.kt L179-196)交给 webView 的那一份。
///
/// 为什么要带 `base_url` / `content`:那两位是 **AnalyzeRule 的状态**,而
/// `headerMap` / `tag` 是**书源的**(在宿主那一侧)——真身把两边凑成一个
/// `BackstageWebView(url = baseUrl, html = content.toString(), javaScript = jsStr,
/// headerMap = getSource()?.getHeaderMap(true), tag = getSource()?.getKey(),
/// cacheFirst = true, timeout = 10000, result = GSON.toJson(result), isRule = true)`。
/// 于是这个结构体只装 AnalyzeRule 那半边,余下的由实现方自己补。
pub struct WebJsRequest<'a> {
    /// `@webjs:` 后面那段(`jsStr`)
    pub js: &'a str,
    /// `GSON.toJson(result)`——当前值,注进页面给 JS 读
    pub result_json: &'a str,
    /// `AnalyzeRule.baseUrl`(可空 —— 真身那里就是 `String?`)
    pub base_url: Option<&'a str>,
    /// `content.toString()`(**AnalyzeRule 的 content 字段**,不是当前值;
    /// 字段为 null 时 Kotlin 的 `toString()` 给的是 `"null"`)
    pub content: &'a str,
}

/// 一条被采纳的 Set-Cookie(net 侧按 okhttp `Cookie.parseAll` 解析后交给 store)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetCookie {
    pub name: String,
    pub value: String,
    /// 有合法 expires/max-age → 落库;否则进 session 内存
    pub persistent: bool,
}

/// cookie 存取(store 提供;逐行对齐 judge/engine 的 CookieStore/CookieManager)。
/// url 参数一律传**完整 URL 或域名串**,二级域名归一在实现侧做。
pub trait CookieEnv {
    /// `CookieStore.getCookie(url)`:库 + session 合并
    fn get_cookie(&mut self, url: &str) -> String;

    /// `CookieManager.loadRequest`:返回新的 `Cookie` 头值;None = 保持原头。
    /// 合并结果含非法头字符时按真身清空该域 cookie 并返回 None。
    fn load_request_cookie(&mut self, url: &str, request_cookie: Option<&str>) -> Option<String>;

    /// `CookieManager.saveResponse`(cookies 按响应头解析序)
    fn save_response_cookies(&mut self, url: &str, cookies: &[SetCookie]);

    // ---- 书源 JS 里的 `cookie` 绑定(CookieStore 那一面)----
    //
    // 真身的 `cookie` 绑定**就是 CookieStore 这个单例**,与网络层写的是同一张表:
    // JS 里 `cookie.setCookie(...)` 存下的,下一跳请求就带得上。
    // **故意不给缺省实现**:漏接一处就是「JS 写了 cookie 但谁也没收到」的静默
    // 单边差异(pb01192 就是这么来的),编译器逼着每个实现方表态。

    /// `CookieStore.setCookie(url, cookie)`
    fn set_cookie(&mut self, url: &str, cookie: &str);
    /// `CookieStore.replaceCookie(url, cookie)`
    fn replace_cookie(&mut self, url: &str, cookie: &str);
    /// `CookieStore.removeCookie(url)`
    fn remove_cookie(&mut self, url: &str);
    /// `CookieStore.getKey(url, key)`
    fn get_cookie_key(&mut self, url: &str, key: &str) -> String;
}

/// `Box<dyn CookieEnv>` 也直接是 CookieEnv(泛型消费方不必区分装没装箱)
impl<C: CookieEnv + ?Sized> CookieEnv for Box<C> {
    fn get_cookie(&mut self, url: &str) -> String {
        (**self).get_cookie(url)
    }
    fn load_request_cookie(&mut self, url: &str, request_cookie: Option<&str>) -> Option<String> {
        (**self).load_request_cookie(url, request_cookie)
    }
    fn save_response_cookies(&mut self, url: &str, cookies: &[SetCookie]) {
        (**self).save_response_cookies(url, cookies)
    }
    fn set_cookie(&mut self, url: &str, cookie: &str) {
        (**self).set_cookie(url, cookie)
    }
    fn replace_cookie(&mut self, url: &str, cookie: &str) {
        (**self).replace_cookie(url, cookie)
    }
    fn remove_cookie(&mut self, url: &str) {
        (**self).remove_cookie(url)
    }
    fn get_cookie_key(&mut self, url: &str, key: &str) -> String {
        (**self).get_cookie_key(url, key)
    }
}

/// 一次网络调用的可观察面。真身里有两种响应类型,书源看到的成员不同:
/// - `java.connect(url)` → `StrResponse`(okhttp 的包装):`body()` / `code()` /
///   `headers()` / `url()`,`toString()` 是 okhttp `Response.toString()`;
/// - `java.get/head/post(url, headers)` → jsoup 的 `Connection.Response`:
///   `body()` / `statusCode()` / `headers()` / `cookies()`。
///
/// 两者的并集放在这里,由 [`NetProvider`] 的实现填;js-host 按调用点挑成员装。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NetResponse {
    /// 最终 URL(跟完重定向之后)
    pub url: String,
    /// 响应体(文本)。`None` = 真身那里是 null body
    pub body: Option<String>,
    pub code: i32,
    /// 响应头,保持出现序;同名多值按出现次数各占一行
    pub headers: Vec<(String, String)>,
    /// `Set-Cookie` 解析出来的键值(jsoup `Connection.Response.cookies()`)
    pub cookies: Vec<(String, String)>,
}

/// 网络调用失败的**两类**,真身对它们的处理完全不同(踩出来的):
///
/// ```kotlin
/// val analyzeUrl = AnalyzeUrl(urlStr, …)              // ← 在 runCatching **之外**
/// return kotlin.runCatching { analyzeUrl.getStrResponse().body }
///     .onFailure { AppLog.put(…) }.getOrElse { it.stackTraceStr }
/// ```
///
/// 构造 `AnalyzeUrl` 会跑 `initUrl()`(内含 `analyzeJs` / `replaceKeyPageJs`),
/// url 里写 `{{$.id}}` 这类东西时会抛 —— 那一步**不在 runCatching 里**,异常
/// 直接穿到 JS。只有请求本身的失败才被吞成栈字符串。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetError {
    /// `AnalyzeUrl` 构造(`initUrl`)就抛了 —— **不吞**,穿到 JS。
    /// 串里带归一标签(差分侧 `«host:ScriptException»`),由 js-host 抛给 JS。
    Construct(String),
    /// 请求本身失败 —— **吞**,返回异常的栈字符串。
    Request(String),
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetError::Construct(m) | NetError::Request(m) => write!(f, "{m}"),
        }
    }
}

/// **`JsExtensions` 的网络面**:`java.ajax` / `connect` / `get` / `head` / `post`。
///
/// 为什么单开一个 trait 而不塞进 [`HostEnv`]:网络能力的提供方与 JS 宿主不是
/// 同一个东西 —— 产品侧由 engine 注入真实 okhttp,差分侧由 difftest 注入
/// **HTTP 录放**(契约 docs/http-snapshot.md)。js-host 只认这个接口。
///
/// 缺省实现一律返回 [`NET_UNSUPPORTED`]:没注入网络的宿主(单元测试、
/// rule-engine 的确定性桩)行为与从前一致。
pub trait NetProvider {
    /// `ajax(url[, callTimeout])`。
    ///
    /// **两个宿主的 `ajax` 不是同一个方法**(踩出来的):
    /// - `JsHost::Url` 走 `JsExtensions.ajax`(JsExtensions.kt L134):
    ///   `AnalyzeUrl(url, source=…, callTimeout=…)`,**不带 ruleData**;
    ///   失败走 `AppLog.put` —— 不进 Debug 日志。
    /// - `JsHost::Rule` 走 **`AnalyzeRule.ajax` 的覆写**(AnalyzeRule.kt L951):
    ///   `AnalyzeUrl(url, source=…, **ruleData = book**)`,没有 callTimeout;
    ///   失败走 **`Debug.log("ajax(url) error\n<栈>")`** —— **进** Debug 日志。
    ///
    /// `with_rule_data` 就是这一位。日志那一位由调用方(js-host)按宿主落。
    ///
    /// **真身吞异常**:失败时返回异常的栈字符串而不是抛。`Err` 在这里表示
    /// 「拿不到响应」,怎么呈现由调用方定 —— 两个引擎的栈文本不可能逐字相同。
    fn ajax(
        &mut self,
        url: &str,
        call_timeout: Option<i64>,
        with_rule_data: bool,
    ) -> Result<String, NetError> {
        let _ = (url, call_timeout, with_rule_data);
        Err(NetError::Request(NET_UNSUPPORTED.into()))
    }

    /// **嵌套 `AnalyzeUrl` 落下的 Debug 日志**。
    ///
    /// `ajax`/`connect` 会拿 url 串再建一个 `AnalyzeUrl`,而那一次构造自己也会
    /// 记日志(典型:option JSON 只有宽松档解得开 → 「链接参数 JSON 格式不规范」)。
    /// 真身那条日志落的是全局 `Debug`,所以外层看得见;被测侧的嵌套宿主是另一个
    /// 实例,不接出来就会少一条。调用方(js-host)在每次网络调用**之后**取走,
    /// 顺序与真身一致。
    fn take_logs(&mut self) -> Vec<String> {
        Vec::new()
    }

    /// `JsExtensions.webView` / `webViewGetSource` / `webViewGetOverrideUrl`
    /// (JsExtensions.kt L245-328)——三个入口只差**哪一个正则非空**。
    ///
    /// 返回的是 `getStrResponse().body`(可空);失败**不吞**,异常穿到 JS
    /// (真身那三处都没有 runCatching)。
    fn web_view(&mut self, req: &WebViewReq<'_>) -> Result<Option<String>, NetError> {
        let _ = req;
        Err(NetError::Request(NET_UNSUPPORTED.into()))
    }

    /// `AnalyzeRule.getWebJsResult` 的网络那半边:宿主把 AnalyzeRule 给的
    /// [`WebJsRequest`] 与**书源那半边**(headerMap / tag)凑齐,交给
    /// `BackstageWebView` 的策略层。缺省 = 这一侧没接平台原语。
    fn web_js(&mut self, req: &WebJsRequest<'_>) -> Result<String, NetError> {
        let _ = req;
        Err(NetError::Request(NET_UNSUPPORTED.into()))
    }

    /// `JsExtensions.connect(url[, header][, callTimeout])`
    fn connect(
        &mut self,
        url: &str,
        header_json: Option<&str>,
        call_timeout: Option<i64>,
    ) -> Result<NetResponse, NetError> {
        let _ = (url, header_json, call_timeout);
        Err(NetError::Request(NET_UNSUPPORTED.into()))
    }

    /// `JsExtensions.get/head/post` —— 这三个走的是 **jsoup 的 `Connection`**,
    /// 不是 okhttp:`followRedirects(false)`、`ignoreContentType(true)`,
    /// 且**不经 okhttp 的拦截器链**(不补 UA / Keep-Alive)。
    fn jsoup(
        &mut self,
        method: &str,
        url: &str,
        headers_json: Option<&str>,
        body: Option<&str>,
    ) -> Result<NetResponse, NetError> {
        let _ = (method, url, headers_json, body);
        Err(NetError::Request(NET_UNSUPPORTED.into()))
    }

    /// `JsExtensions.ajaxAll(urlList[, skipRateLimit])`(JsExtensions.kt L154)。
    ///
    /// 与 [`NetProvider::ajax`] 的三处不同,一处都不能省:
    /// - **不吞异常**:那边整段包在 `runCatching` 里、失败返回栈字符串,
    ///   这边没有 —— 一跳失败整个调用抛出去。
    /// - **不带 ruleData**:建的是 `AnalyzeUrl(url, source = getSource())`,
    ///   url 里的 `{{...}}` 解不到书的变量层。
    /// - 返回的是 **`StrResponse` 的数组**,不是正文串:书源拿 `.body()` 取。
    ///
    /// 真身 `mapAsync(threadCount)` 并发发,但 `toList()` 回来是**入参序**;
    /// 这里顺序发,观察面(hops 的次序)与它一致。
    fn ajax_all(&mut self, urls: &[String]) -> Result<Vec<NetResponse>, NetError> {
        let _ = urls;
        Err(NetError::Request(NET_UNSUPPORTED.into()))
    }

    /// `JsExtensions.startBrowser(url, title[, html])`(JsExtensions.kt L352-359):
    /// **弹出内置浏览器就走,不等结果**。语料 2 处(播放页跳浏览器那种)。
    fn start_browser(
        &mut self,
        url: &str,
        title: &str,
        html: Option<&str>,
    ) -> Result<(), NetError> {
        let _ = (url, title, html);
        Err(NetError::Request(NET_UNSUPPORTED.into()))
    }

    /// `JsExtensions.startBrowserAwait(url, title[, refetchAfterSuccess[, html]])`
    /// (JsExtensions.kt L364-388):**弹浏览器并等用户过盾**,回来给一个响应。
    ///
    /// 语料 16 处,写法几乎只有一种 ——「`result.match(/Just a moment/)` 就弹一下、
    /// 过完再 `java.ajax` 抓一遍」。也就是说这是**「盾」那一族的正解**。
    ///
    /// 两种结局(真身 `VerificationResult`):用户那边把页面源码带回来了
    /// (`StrResponse(url2.ifEmpty { url }, body)`),或者**别人刚过完同一个盾**
    /// —— 那时不用它的结果,自己 `AnalyzeUrl(url).getStrResponse(useWebView=false)`
    /// 重抓一次。
    fn start_browser_await(
        &mut self,
        url: &str,
        title: &str,
        refetch_after_success: bool,
        html: Option<&str>,
    ) -> Result<NetResponse, NetError> {
        let _ = (url, title, refetch_after_success, html);
        Err(NetError::Request(NET_UNSUPPORTED.into()))
    }

    /// `JsExtensions.getVerificationCode(imageUrl)`(JsExtensions.kt L393-400):
    /// 弹图片验证码对话框,等用户把字打进去。语料 1 处。
    fn get_verification_code(&mut self, image_url: &str) -> Result<String, NetError> {
        let _ = image_url;
        Err(NetError::Request(NET_UNSUPPORTED.into()))
    }

    /// 本次求值发出的请求序列 `(method, 规范化 URL)`。差分的观察面之一 ——
    /// URL 拼装与跳数是 `ajax` 类书源最容易两侧走岔的地方。
    fn hops(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}

/// 宿主侧异常 → 差分侧认得的类别标签。书名号是 js_case_runner 切分用的,
/// 写成裸的 `host:X` 会静默落到 `js:throw` 那一档(M2o 踩过)。
pub fn host_tag(class_name: &str) -> String {
    format!("«host:{class_name}»")
}

/// 仍未接的能力(webView / 文件系统 / 字体)的统一标记。两侧同串。
pub const NET_UNSUPPORTED: &str = "«net-unsupported»";

// ---------------------------------------------------------------- PlatformHooks:webView
//
// Rust **不持有 webview**(plan §1):Android 那边是 flutter_inappwebview 的
// headless 实例、桌面降档走可见窗口,都在 Dart 侧。这里只放接口 ——
// 策略(默认 JS、补 900ms、重试梯子、超时、重定向包装、cookie 回抄)全在
// `net::webview`,它是**可差分**的那一半;平台只提供「加载」「求值」两件原语。
//
// 契约与判据:fixtures/cases/webview/README.md。

/// `createWebView()` 写进 `WebSettings` 的那几位。
///
/// **与加载分成两步**是有意的:真身 `load()` 先 `createWebView(requestConfig)`
/// 再走 `when` 分支发起加载,而那个 `when` 会为 `url == null` 抛 NPE ——
/// 也就是说**「设置写过了、页没加载」这一档是真实存在的**,合成一步就看不见它。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WebViewSettings {
    /// `WebSettings.userAgentString`(`toWebViewRequestConfig` 选出来的)
    pub user_agent: String,
    /// `WebSettings.blockNetworkImage`(真身恒为 true)
    pub block_network_image: bool,
    /// `cacheFirst` → LOAD_CACHE_ELSE_NETWORK(1),否则 LOAD_DEFAULT(-1)
    pub cache_mode: i32,
}

/// 一次加载(`loadUrl` / `loadDataWithBaseURL`)
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WebViewLoad {
    /// `loadUrl` 的地址;`html` 非空时它是 `loadDataWithBaseURL` 的 **baseUrl**
    /// —— 那一支的 baseUrl 是可空的(真 WebView 会拿 `about:blank` 当请求地址),
    /// 而 `loadUrl` 那一支永远非空(`url!!`)
    pub url: Option<String>,
    /// 非空 → `loadDataWithBaseURL(url, html, "text/html", encoding, url)`
    pub html: Option<String>,
    /// `getEncoding()`:`encode ?: "utf-8"`
    pub encoding: String,
    /// 除 UA / CookieJar / proxy 之外的头,原序。**空表时真身走的是不带头的
    /// `loadUrl(url)` 重载** —— 这一位有语义
    pub headers: Vec<(String, String)>,
}

/// 平台回上来的事件,顺序照真 WebView:
/// `shouldOverrideUrlLoading`* → `onLoadResource`* → `onPageFinished`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebViewEvent {
    /// `shouldOverrideUrlLoading`;`is_redirect` 是 `WebResourceRequest.isRedirect`
    OverrideUrl { url: String, is_redirect: bool },
    /// `onLoadResource`
    LoadResource { url: String },
    /// `onPageFinished`(url 可能与请求地址不同 —— 跟过转向)
    PageFinished { url: String },
}

/// 平台侧的 webview 原语。**只有两件事是平台的**:加载、求值。
/// 「用户点停了吗」。调用方(engine)把取消令牌包成这个交给平台侧 ——
/// `rubato-core` 不认识 engine 的 `CancelToken`,而 webView 那条路要跨线程
/// (等事件的 worker 与点「停」的 UI 不是同一条),所以是 `Arc + Send + Sync`。
pub type CancelFn = std::sync::Arc<dyn Fn() -> bool + Send + Sync>;

pub trait WebViewHost {
    /// `createWebView()`:把设置写进去(此时还没加载)
    fn create(&mut self, settings: &WebViewSettings);

    /// 开始加载。真身那边它是异步的:事件随后由 [`Self::next_event`] 取。
    fn load(&mut self, load: &WebViewLoad);

    /// 等到「**加载开始**之后的第 `until_ms` 毫秒」;途中来了事件就把它连同
    /// **发生时刻**交回来(那时还没到 `until_ms`)。`None` = 到点了也没有新事件。
    ///
    /// **为什么是「等到某刻,顺便收事件」而不是「先收完事件再等」**:真身那边
    /// `onPageFinished` 每来一次都 `mHandler.removeCallbacks(runnable)` +
    /// `postDelayed(runnable, 100 + delayTime)` —— **求值排期会被后来的加载完成
    /// 重置**。跳转站(`Redirecting…` 那种)因此有两次 `onPageFinished`,真身取的是
    /// **最后一次**之后的页面;只认第一次就会把**加载到一半**的 DOM 交上去
    /// (真机验收里 `m.bqg225.com` 就是这样只拿回 538 字节的 `<head>`)。
    ///
    /// 排期算在策略层(补 900 ms、`100 + delayTime`、重试梯子),平台只负责
    /// 「等到那一刻、期间有事件就喊一声」:产品侧真的等,差分侧照剧本的时间轴走。
    fn next_event_until(&mut self, until_ms: i64) -> Option<(i64, WebViewEvent)>;

    /// `evaluateJavascript`:返回**原样串** —— Android 给的就是 JSON 编码后的值
    /// (`"null"` 表示 JS 返回 null,`"\"abc\""` 表示字符串 `abc`)
    fn eval(&mut self, js: &str) -> String;

    /// `loadUrl("javascript:…")`:执行但**不收结果**(嗅探那条路用它)
    fn eval_void(&mut self, js: &str);

    /// `android.webkit.CookieManager.getInstance().getCookie(url)`
    fn page_cookie(&mut self, url: &str) -> Option<String>;

    /// `WebViewPool.release`:还回去。真身在**出结果 / 报错 / 超时**三条路上都要还
    fn destroy(&mut self);

    /// 用户点停了吗。**缺省是「永远没有」** —— 差分侧的剧本 host 用的就是它
    /// (那边没有用户、也没有线程),于是这一位不进判据面。
    ///
    /// 真身那边取消是协程的事:`withTimeout` / `job.cancel()` 让
    /// `suspendCancellableCoroutine` 带着 CancellationException 恢复,
    /// `invokeOnCancellation` 把 WebView 还回池子。移植版没有协程,
    /// 所以策略层每一轮问一次 —— webView 那一步可以长达 60 秒,不问就等于
    /// 「点了停还得再等一分钟」。
    fn cancelled(&self) -> bool {
        false
    }
}

/// 平台那一侧的**供给方**(真身 `WebViewPool`:acquire 一个、用完 release)。
///
/// 产品侧的实现在 `ffi::platform`(过 FFI 到 Dart 的 headless webview);
/// 差分侧不用它 —— 那边的 [`WebViewHost`] 由 case 的剧本直接造。
///
/// [`available`](Self::available) 与 [`acquire`](Self::acquire) 分开是因为
/// **「有没有平台」这件事是会变的**:Dart 侧登记之前(以及桌面上根本没接的
/// 那些平台)一个都要不到,调用方据此决定给不给 `web_view`——给不出就走
/// `FetchError::WebViewUnsupported`,而不是给一个必然失败的壳。
pub trait WebViewProvider: Send + Sync {
    /// 现在要得到吗(Dart 侧登记了平台就要得到)
    fn available(&self) -> bool;

    /// 要一个新的。**调用方保证先问过 [`available`](Self::available)** ——
    /// 中途掉线时这里仍给得出壳,它的加载会以超时收场。
    ///
    /// `cancel` 是这一次调用的取消令牌(见 [`CancelFn`]);`None` = 这条路上
    /// 没有取消(引擎里那几个不领任务号的入口)。
    fn acquire(&self, cancel: Option<CancelFn>) -> Box<dyn WebViewHost>;
}
