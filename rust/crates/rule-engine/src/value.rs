//! 规则链上流转的值。
//!
//! AnalyzeRule 里 `result: Any?` 会依次变成 String / List<String> /
//! jsoup Elements / jayway 值 / JS 返回值……而 `result.toString()` 的形态
//! 各不相同(且被 `##` 正则、`{{}}` 拼接直接消费),所以必须精确区分:
//! - Kotlin `ArrayList<String>.toString()` → `[a, b]`
//! - jsoup `Elements.toString()` → 各 outerHtml 以 `\n` 连接
//! - jayway `JSONArray.toString()` → 严格 JSON 文本
//! - jayway `LinkedHashMap.toString()` → `{k=v}`

use ego_tree::NodeId;
use html_compat::dsl::AnalyzeByJSoup;
use json_compat::{java_double_to_string, java_to_string, json_smart_to_json};
use rubato_core::host::JavaValue;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;

/// 共享文档:`html` 动作会变异 DOM,与裁判一致,故整条规则链共用一份
pub type Doc = Rc<RefCell<AnalyzeByJSoup>>;

#[derive(Clone)]
pub enum RuleValue {
    Str(String),
    /// Kotlin `List<String>`
    StrList(Vec<String>),
    /// `AnalyzeByRegex.getElements` 的 `List<List<String>>`
    StrListList(Vec<Vec<String>>),
    /// jayway 标量 / Map
    Json(Value),
    /// gson `LinkedTreeMap`——**只**由 `setContent` 产生(JS 宿主把
    /// `Map<String, Any?>` 交给 AnalyzeRule 的那条路)。与 `Json` 分开是因为
    /// AnalyzeRule 对 `result is LinkedTreeMap` 有一条**专用短路分支**:
    /// 直取 `ruleList.first().rule` 这个键,不跑 put/makeUpRule/replaceRegex。
    GsonMap(serde_json::Map<String, Value>),
    /// **Java 对象模型**上的 jayway 读取结果。content 是 gson LinkedTreeMap 时,
    /// `JsonPath.parse(Object)` 直接把 Java 集合当模型,读出来的是原样的
    /// `ArrayList` / `LinkedTreeMap`(toString 是 `[a, b]` / `{k=v}`),
    /// 而不是从文本解析时的 json-smart `JSONArray`(toString 是 JSON 文本)。
    Java(Value),
    /// jayway `JSONArray`(toString 是 JSON 文本)
    JsonArray(Vec<Value>),
    /// Kotlin `ArrayList<Any>` 装 jayway 元素(toString 是 `[a, b]`)
    JsonList(Vec<Value>),
    /// Rhino 的 **NativeObject / NativeArray** —— JS 返回的对象与对象数组。
    /// 与 [`RuleValue::Json`](jayway 容器)、[`RuleValue::GsonMap`]
    /// (LinkedTreeMap)分开:AnalyzeRule 对 `result is NativeObject` 有一条
    /// **专用分支**(L321-331 / L215-241)—— 键值直接访问,而且是在
    /// putRule/makeUpRule/replaceRegex 都跑的那一档里。
    Native(Value),
    /// jsoup `Elements`
    Elements(Doc, Vec<NodeId>),
    /// jsoup 单个 `Element`
    Node(Doc, NodeId),
    /// `AnalyzeByXPath.getElements` 的 **`List<JXNode>`**。
    ///
    /// 与 [`RuleValue::Elements`] 分开的两处:
    /// 1. `toString()` 是 **Java 集合口径 `[a, b]`**,不是 jsoup Elements 的
    ///    「各 outerHtml 以换行相连」;
    /// 2. 每一项若是**真实树上的元素**就保住元素身份 —— 真身
    ///    `AnalyzeByJSoup.parse(doc)` 对 `doc is JXNode && isElement` 走
    ///    `doc.asElement()`,**不重新解析**。差别在 `<tr>` 这类片段上是致命的:
    ///    `Jsoup.parse("<tr>…</tr>")` 会被 foster parenting 把内容挪出表外、
    ///    `tag.tr` 一条都选不中(pc03399:`bookList: //*[@id="x"]//tr` +
    ///    `name: tag.tr@tag.p.0@tag.a.0@text`)。
    ///    合成元素(文本节点包出来的)与字符串项仍然降级成串。
    JxList(Vec<RuleValue>),
    Num(f64),
    Bool(bool),
}

/// Java 容器的**出身**:决定**数组**的 `toString()` —— json-smart 的
/// `JSONArray` 是 JSON 文本(`[1,2]`),gson 的 `ArrayList` 是 `[a, b]`。
/// 对象两边都是 `{k=v}`。同一棵 `serde_json::Value` 判不出来,故要带着走。
#[derive(Clone, Copy)]
pub enum JavaFlavor {
    /// `JsonPath.parse(String)` 的模型(net.minidev)
    JsonSmart,
    /// `JsonPath.parse(Object)` 的模型(gson 的 LinkedTreeMap / ArrayList)
    Gson,
}

fn flavor_string(v: &Value, f: JavaFlavor) -> String {
    match f {
        JavaFlavor::JsonSmart => java_to_string(v),
        JavaFlavor::Gson => rubato_core::gson::object_to_string(v),
    }
}

/// JSON 树 → JS 里的 Java 形态(见 [`JavaValue`])。**逐层装箱**:
/// Rhino 的 WrapFactory 对取出来的每个值都包一次,故串是装箱的
/// `java.lang.String`、数是装箱的 `Integer`/`Double`(探针 `js-bind-map-value-typeof`)。
pub fn java_value_of(v: &Value, f: JavaFlavor) -> JavaValue {
    match v {
        Value::Null => JavaValue::Null,
        Value::Bool(b) => JavaValue::Bool(*b),
        Value::Number(n) => JavaValue::Num(n.as_f64().unwrap_or(f64::NAN), flavor_string(v, f)),
        Value::String(s) => JavaValue::Str(s.clone()),
        Value::Array(a) => {
            JavaValue::List(a.iter().map(|x| java_value_of(x, f)).collect(), flavor_string(v, f))
        }
        Value::Object(m) => JavaValue::Map(
            m.iter().map(|(k, x)| (k.clone(), java_value_of(x, f))).collect(),
            flavor_string(v, f),
        ),
    }
}

impl RuleValue {
    /// `Any.toString()`
    pub fn to_java_string(&self) -> String {
        match self {
            RuleValue::Str(s) => s.clone(),
            RuleValue::StrList(v) => format!("[{}]", v.join(", ")),
            RuleValue::StrListList(v) => {
                let inner: Vec<String> = v.iter().map(|x| format!("[{}]", x.join(", "))).collect();
                format!("[{}]", inner.join(", "))
            }
            RuleValue::Json(v) => java_to_string(v),
            // LinkedTreeMap.toString() → `{k=v, k2=v2}`
            RuleValue::GsonMap(m) => rubato_core::gson::object_to_string(&Value::Object(m.clone())),
            // ArrayList/LinkedTreeMap 的 toString(数组也是 `[a, b]`)
            RuleValue::Java(v) => rubato_core::gson::object_to_string(v),
            RuleValue::JsonArray(a) => json_smart_to_json(&Value::Array(a.clone())),
            RuleValue::JsonList(a) => {
                let inner: Vec<String> = a.iter().map(java_to_string).collect();
                format!("[{}]", inner.join(", "))
            }
            // jsoup `Elements.toString()` = `outerHtml()`:各 outerHtml 以换行相连,
            // 分隔符只在缓冲区非空时加(见 jsoup_join —— 元素的 outerHtml 不可能
            // 是空串,故这里与 `join` 等价,口径统一而已)
            RuleValue::Elements(doc, ids) => {
                let d = doc.borrow();
                let parts: Vec<String> = ids.iter().map(|&i| d.outer_html_of(i)).collect();
                rubato_core::host::jsoup_join(&parts, "\n")
            }
            RuleValue::Node(doc, id) => doc.borrow().outer_html_of(*id),
            // Kotlin `List<JXNode>.toString()`,项是 `JXNode.toString()`(= asString())
            RuleValue::JxList(v) => {
                let parts: Vec<String> = v.iter().map(|x| x.to_java_string()).collect();
                format!("[{}]", parts.join(", "))
            }
            // Rhino 的 `NativeObject.toString()` 是 `[object Object]`;
            // NativeArray 那边 Kotlin 的 `Any.toString()` 给的是**身份哈希**
            // (`org.mozilla.…@1f2e`),逐次运行都不同 —— 归一成同一个标记
            // (与 js 套的 `«identity»` 同口径;真落到可观察面上两侧都对不齐,
            // 那种用例本来就不可复现)。
            RuleValue::Native(Value::Array(_)) => "«identity»".into(),
            RuleValue::Native(Value::Object(_)) => "[object Object]".into(),
            // 对象/数组之外的载荷是 NativeObject 的**字段值**(或 NativeArray 的
            // 元素):真身那边它们是 Java 的 String/Double/Boolean,`toString()`
            // 就是值本身 —— 不是 `[object Object]`。口径与 `as_list()` 那支同。
            RuleValue::Native(Value::String(s)) => s.clone(),
            RuleValue::Native(other) => rubato_core::gson::object_to_string(other),
            RuleValue::Num(d) => java_double_to_string(*d),
            RuleValue::Bool(b) => b.to_string(),
        }
    }

    /// **过 GSON 的形态**;`None` = GSON 在它上面抛。
    ///
    /// 与 [`RuleValue::to_java_string`] 是两条独立的观察面(差分实测):
    /// `$.list` 读出来的 jayway 容器 `toString()` 是 JSON 文本、GSON **给得出**;
    /// jsoup 的 `Element`/`Elements` `toString()` 是 outerHtml、GSON **抛**
    /// (`Document.parser` 那圈自引用,裁判侧 `host:JsonIOException`)。
    ///
    /// Kotlin 的 `String`/`Double`/`Boolean` 过 GSON 就是 JSON 的标量;
    /// `List<String>` 是字符串数组。
    pub fn to_gson_json(&self) -> Option<Value> {
        Some(match self {
            RuleValue::Str(s) => Value::String(s.clone()),
            RuleValue::Num(d) => serde_json::Number::from_f64(*d).map(Value::Number)?,
            RuleValue::Bool(b) => Value::Bool(*b),
            RuleValue::StrList(v) => {
                Value::Array(v.iter().map(|s| Value::String(s.clone())).collect())
            }
            RuleValue::StrListList(v) => Value::Array(
                v.iter()
                    .map(|x| Value::Array(x.iter().map(|s| Value::String(s.clone())).collect()))
                    .collect(),
            ),
            RuleValue::Json(v) | RuleValue::Java(v) | RuleValue::Native(v) => v.clone(),
            RuleValue::GsonMap(m) => Value::Object(m.clone()),
            RuleValue::JsonArray(a) | RuleValue::JsonList(a) => Value::Array(a.clone()),
            // 项里只要有一个是 jsoup 元素,整表就过不了 GSON
            RuleValue::JxList(v) => {
                Value::Array(v.iter().map(RuleValue::to_gson_json).collect::<Option<Vec<_>>>()?)
            }
            RuleValue::Elements(..) | RuleValue::Node(..) => return None,
        })
    }

    /// `result is List<*>`——决定 `$N` 取值与 replaceRegex 是否逐元素
    pub fn as_list(&self) -> Option<Vec<Option<String>>> {
        match self {
            RuleValue::StrList(v) => Some(v.iter().map(|s| Some(s.clone())).collect()),
            RuleValue::StrListList(v) => {
                Some(v.iter().map(|x| Some(format!("[{}]", x.join(", ")))).collect())
            }
            RuleValue::JsonArray(a) | RuleValue::JsonList(a) => Some(
                a.iter()
                    .map(|v| if v.is_null() { None } else { Some(java_to_string(v)) })
                    .collect(),
            ),
            RuleValue::Java(Value::Array(a)) => Some(
                a.iter()
                    .map(|v| {
                        if v.is_null() {
                            None
                        } else {
                            Some(rubato_core::gson::object_to_string(v))
                        }
                    })
                    .collect(),
            ),
            RuleValue::Elements(doc, ids) => {
                let d = doc.borrow();
                Some(ids.iter().map(|&i| Some(d.outer_html_of(i))).collect())
            }
            // htmlunit-corejs 的 NativeArray **实现了 java.util.List**,
            // 所以真身那边 `result is List<*>` 对它成立
            RuleValue::Native(Value::Array(a)) => Some(
                a.iter()
                    .map(|v| match v {
                        Value::Null => None,
                        Value::String(s) => Some(s.clone()),
                        Value::Object(_) => Some("[object Object]".to_string()),
                        other => Some(rubato_core::gson::object_to_string(other)),
                    })
                    .collect(),
            ),
            _ => None,
        }
    }

    /// 与 `content` 的 `!=` 判定(jsoup 节点按同一性,字符串按值)
    pub fn same_as(&self, other: &RuleValue) -> bool {
        match (self, other) {
            (RuleValue::Str(a), RuleValue::Str(b)) => a == b,
            (RuleValue::Node(d1, i1), RuleValue::Node(d2, i2)) => Rc::ptr_eq(d1, d2) && i1 == i2,
            (RuleValue::Json(a), RuleValue::Json(b)) => a == b,
            (RuleValue::GsonMap(a), RuleValue::GsonMap(b)) => a == b,
            (RuleValue::Java(a), RuleValue::Java(b)) => a == b,
            _ => false,
        }
    }
}
