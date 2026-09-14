//! AnalyzeByJSonPath(judge/engine AnalyzeByJSonPath.kt,172 行)的逐行移植。
//!
//! 口径要点(差分抓出的):
//! - `getString` 先跑 `{$.` 内嵌规则替换,**替换结果非空就不再求值整条 rule**;
//! - jayway 对 null 标量调 `toString()` 抛 NPE,被 `catch (e: Exception)` 吞掉,
//!   于是结果保持内嵌替换后的值(通常是空串)——不是字面量 "null";
//! - `getList` 的 `read<ArrayList<Any>>` 在 Kotlin 侧是带 CHECKCAST 的,
//!   definite 路径拿到非数组标量 → ClassCastException → 返回空表;
//! - 分隔符集合三处不同:getString 是 `&&`/`||`,另两个多一个 `%%`。

use crate::{JsonPath, ReadResult, java_to_string};
use rule_syntax::{RuleAnalyzer, SplitError};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidJson;

impl std::fmt::Display for InvalidJson {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid json")
    }
}
impl std::error::Error for InvalidJson {}

pub struct AnalyzeByJSonPath {
    ctx: Value,
    flavor: ModelFlavor,
}

/// 模型是**谁的对象**——它决定读出来的容器 `toString()` 长什么样。
///
/// jayway 只管按路径取值,取出来的东西是**原模型里的对象**;`getString` 那一步
/// 是 Kotlin 的 `Any.toString()`,于是同一条 `$.text` 在三种模型上是三种串:
///
/// | 模型 | 从哪来 | 对象值的 toString |
/// |---|---|---|
/// | json-smart | `JsonPath.parse(文本)` | `{k=v, k2=v2}`(LinkedHashMap 风格) |
/// | Rhino | `<js>` 交回来的 NativeObject/NativeArray | **`[object Object]`** |
///
/// 2026-08-31 实测(pb02434):书源的 `chapterList` 是 `@js:`,返回
/// `[{text: <页面上的对象>, href: "…"}]`;`chapterName: "$.text"` 读出来的是
/// **内层那个对象**——裁判给 `[object Object]`,而我们照 json-smart 渲染成
/// `{href=c1.html, text=第一章 起, …}`。差的不是路径,是**渲染口径**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFlavor {
    /// 从 JSON 文本解析出来的 json-smart 模型(缺省)
    JsonSmart,
    /// Rhino 的 NativeObject / NativeArray 树(`<js>` 的返回值)
    Rhino,
}

impl AnalyzeByJSonPath {
    /// 对应 `JsonPath.parse(json)`:解析失败在裁判侧抛 InvalidJsonException。
    /// 注意 jayway 3.0.0 默认 provider 是 json-smart(PERMISSIVE),serde_json 严格,
    /// 脏 JSON 的差异记在 docs/plan.md 风险 4。
    pub fn parse(json: &str) -> Result<Self, InvalidJson> {
        serde_json::from_str(json)
            .map(|ctx| Self { ctx, flavor: ModelFlavor::JsonSmart })
            .map_err(|_| InvalidJson)
    }

    pub fn from_value(ctx: Value) -> Self {
        Self { ctx, flavor: ModelFlavor::JsonSmart }
    }

    /// 模型是 Rhino 的对象树(`<js>` 返回的 NativeObject/NativeArray)。
    /// 与 [`Self::from_value`] 只差**容器值的 toString 口径**,见 [`ModelFlavor`]。
    pub fn from_native(ctx: Value) -> Self {
        Self { ctx, flavor: ModelFlavor::Rhino }
    }

    /// 读出来的值 → `Any.toString()`,按模型的口径([`ModelFlavor`])
    fn render(&self, v: &Value) -> String {
        match (self.flavor, v) {
            // Rhino 的 NativeObject.toString();NativeArray 那边 Kotlin 的
            // `Any.toString()` 给的是身份哈希,逐次运行都不同 —— 归一成同一个
            // 标记(与 rule-engine::RuleValue 的 `«identity»` 同口径)
            (ModelFlavor::Rhino, Value::Object(_)) => "[object Object]".into(),
            (ModelFlavor::Rhino, Value::Array(_)) => "«identity»".into(),
            _ => java_to_string(v),
        }
    }

    /// json-smart PERMISSIVE 的近似:非 JSON 文本(HTML 页面之类)不抛异常,
    /// 而是被当成一个「裸字符串」值——AnalyzeRule 走 Json 模式解析 HTML 时
    /// 全靠这条,裁判侧实测同样不抛(随后 read 报 PathNotFound 被吞)。
    /// 以 `{`/`[` 开头却又解析不了的才算真的坏 JSON。
    pub fn parse_permissive(json: &str) -> Result<Self, InvalidJson> {
        match serde_json::from_str(json) {
            Ok(ctx) => Ok(Self { ctx, flavor: ModelFlavor::JsonSmart }),
            Err(_) => {
                let t = json.trim_start();
                if t.starts_with('{') || t.starts_with('[') {
                    Err(InvalidJson)
                } else {
                    Ok(Self {
                        ctx: Value::String(json.to_string()),
                        flavor: ModelFlavor::JsonSmart,
                    })
                }
            }
        }
    }

    /// 裁判 `AnalyzeByJSonPath(json: Any)` 里 **`json is String`** 的那一支:
    /// 走 jayway 的 `JsonPath.parse(String)`,比上面的宽松多两道前置检查。
    ///
    /// **两道检查、两条不同的消息**(拿 jayway 的 jar 实跑出来的,不是照源码推的;
    /// 探针在 json 套的 `DSL_EDGE_DOCS`):
    /// - `ParseContextImpl.parse(String)` 头一行 `notEmpty` —— **空串**抛
    ///   `json string can not be null or empty`;而**全是空白**的串 `"   "`
    ///   **不抛**,照样按裸字符串收(差一个字就是两种行为);
    /// - 解析完 `JsonReader` 再 `notNull` —— 字面量 `null` 解出来是 Java 的 null,
    ///   抛 `json can not be null`。
    ///
    /// 别把这两道搬到 [`Self::parse_permissive`] 上:那条路上的输入多半**不是**
    /// Java String(空的 jsoup Element、`List<String>`…),裁判走的是 `parse(Object)`
    /// 重载,没有这两道 —— 搬过去会让一批本来两侧一起空手而归的 case 变成我们单边
    /// 抛(pipeline-corpus-b 实测 16 例)。
    ///
    /// 少这一支的代价:书源 `<js>` 把正文解成空串再交给 `$.content_html`,
    /// 裁判整步抛而被测侧给 content_empty —— pb02295 / pb02296 就是这么照出来的。
    pub fn parse_string(json: &str) -> Result<Self, InvalidJson> {
        if json.is_empty() {
            return Err(InvalidJson);
        }
        let this = Self::parse_permissive(json)?;
        if this.ctx.is_null() { Err(InvalidJson) } else { Ok(this) }
    }

    pub fn value(&self) -> &Value {
        &self.ctx
    }

    /// 求值单条(无 &&/||)path,异常一律吞掉 → None
    fn read_raw(&self, rule: &str) -> Option<ReadResult> {
        JsonPath::compile(rule).ok()?.read(&self.ctx).ok()
    }

    pub fn get_string(&self, rule: &str) -> Result<Option<String>, SplitError> {
        if rule.is_empty() {
            return Ok(None);
        }
        let mut ra = RuleAnalyzer::new(rule, true); // 平衡组按代码平衡
        let rules = ra.split_rule(&["&&", "||"])?;

        if rules.len() == 1 {
            ra.re_set_pos(); // 复用解析器
            let mut inner_err: Option<SplitError> = None;
            let mut result = ra.inner_rule("{$.", 1, 1, |r| match self.get_string(r) {
                Ok(v) => v,
                Err(e) => {
                    inner_err.get_or_insert(e);
                    None
                }
            })?;
            if let Some(e) = inner_err {
                return Err(e);
            }
            if result.is_empty() {
                // st 为空,表明无成功替换的内嵌规则 → 整条 rule 直接求值
                match self.read_raw(rule) {
                    // definite 路径命中数组时 jayway 给的是 JSONArray(实现了 List)
                    // → 走 `ob is List<*>` 的 joinToString 分支,不是 toString
                    Some(ReadResult::List(l)) | Some(ReadResult::Scalar(Value::Array(l))) => {
                        result = l.iter().map(|o| self.render(o)).collect::<Vec<_>>().join("\n");
                    }
                    // 对 null 调 toString 抛 NPE,被吞 → result 保持空串
                    Some(ReadResult::Scalar(v)) if !v.is_null() => result = self.render(&v),
                    _ => {}
                }
            }
            Ok(Some(result))
        } else {
            let elements_type = ra.elements_type().to_string();
            let mut text_list: Vec<String> = Vec::new();
            for rl in &rules {
                let temp = self.get_string(rl)?;
                if let Some(t) = temp {
                    if !t.is_empty() {
                        text_list.push(t);
                        if elements_type == "||" {
                            break;
                        }
                    }
                }
            }
            Ok(Some(text_list.join("\n")))
        }
    }

    pub fn get_string_list(&self, rule: &str) -> Result<Vec<String>, SplitError> {
        let mut result: Vec<String> = Vec::new();
        if rule.is_empty() {
            return Ok(result);
        }
        let mut ra = RuleAnalyzer::new(rule, true);
        let rules = ra.split_rule(&["&&", "||", "%%"])?;

        if rules.len() == 1 {
            ra.re_set_pos();
            let mut inner_err: Option<SplitError> = None;
            let st = ra.inner_rule("{$.", 1, 1, |r| match self.get_string(r) {
                Ok(v) => v,
                Err(e) => {
                    inner_err.get_or_insert(e);
                    None
                }
            })?;
            if let Some(e) = inner_err {
                return Err(e);
            }
            if st.is_empty() {
                match self.read_raw(rule) {
                    Some(ReadResult::List(l)) | Some(ReadResult::Scalar(Value::Array(l))) => {
                        for o in &l {
                            result.push(self.render(o));
                        }
                    }
                    Some(ReadResult::Scalar(v)) if !v.is_null() => result.push(self.render(&v)),
                    _ => {}
                }
            } else {
                result.push(st);
            }
            return Ok(result);
        }

        let elements_type = ra.elements_type().to_string();
        let mut results: Vec<Vec<String>> = Vec::new();
        for rl in &rules {
            let temp = self.get_string_list(rl)?;
            if !temp.is_empty() {
                results.push(temp);
                if elements_type == "||" {
                    break;
                }
            }
        }
        result.extend(rule_syntax::merge(&results, &elements_type));
        Ok(result)
    }

    /// getObject:`ctx.read(rule)`,异常直接外抛(裁判侧同样不吞)
    pub fn get_object(&self, rule: &str) -> Option<Value> {
        Some(self.get_object_scalar(rule)?.0)
    }

    /// 同 `get_object`,额外告知 jayway 给回来的是 **definite 路径的标量**
    /// (原样的模型对象:Java 模型下就是 ArrayList/LinkedTreeMap 本身)
    /// 还是 **indefinite 路径新建的 JSONArray**(永远是 json-smart 容器)。
    /// 调用方靠这个决定 `toString()` 走哪套口径。
    pub fn get_object_scalar(&self, rule: &str) -> Option<(Value, bool)> {
        match self.read_raw(rule)? {
            ReadResult::Scalar(v) => Some((v, true)),
            ReadResult::List(l) => Some((Value::Array(l), false)),
        }
    }

    pub fn get_list(&self, rule: &str) -> Result<Vec<Value>, SplitError> {
        let mut result: Vec<Value> = Vec::new();
        if rule.is_empty() {
            return Ok(result);
        }
        let mut ra = RuleAnalyzer::new(rule, true);
        let rules = ra.split_rule(&["&&", "||", "%%"])?;
        if rules.len() == 1 {
            // read<ArrayList<Any>>:非数组标量在 Kotlin 侧 CHECKCAST 失败 → 空表
            return Ok(match self.read_raw(&rules[0]) {
                Some(ReadResult::List(l)) => l,
                Some(ReadResult::Scalar(Value::Array(a))) => a,
                _ => Vec::new(),
            });
        }

        let elements_type = ra.elements_type().to_string();
        let mut results: Vec<Vec<Value>> = Vec::new();
        for rl in &rules {
            let temp = self.get_list(rl)?;
            if !temp.is_empty() {
                results.push(temp);
                if elements_type == "||" {
                    break;
                }
            }
        }
        // Kotlin 的 `%%` 交叉支是 temp[i]?.let { result.add(it) } —— null 元素
        // 被丢弃;`&&`/`||` 支的 addAll 不过滤。故只在 `%%` 时链一步过滤
        // (merge 的交叉下标仍按过滤前的位置对齐,与逐字循环一致)。
        let merged = rule_syntax::merge(&results, &elements_type);
        if elements_type == "%%" {
            result.extend(merged.into_iter().filter(|v| !v.is_null()));
        } else {
            result.extend(merged);
        }
        Ok(result)
    }
}
