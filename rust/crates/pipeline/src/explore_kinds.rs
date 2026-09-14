//! `BookSourceExtensions.exploreKinds()`(judge/engine help/source/BookSourceExtensions.kt L43）
//! —— 书源的 `exploreUrl` 摊成**发现页的分类格子**。
//!
//! ```text
//! if (exploreUrl.isNullOrBlank()) return emptyList()
//! runCatching {
//!     val ruleStr = when {
//!         exploreUrl.startsWith("@js:", true) -> evalJS(exploreUrl.substring(4)).toString().trim()
//!         exploreUrl.startsWith("<js>", true) ->
//!             evalJS(exploreUrl.substring(4, exploreUrl.lastIndexOf("<"))).toString().trim()
//!         else -> exploreUrl
//!     }
//!     if (ruleStr.isJsonArray()) GSON.fromJsonArray<ExploreKind>(ruleStr).getOrThrow()
//!     else ruleStr.split("(&&|\n)+".toRegex()).forEach { ExploreKind(it.split("::")…) }
//! }.onFailure { kinds.add(ExploreKind("ERROR:${it.localizedMessage}", it.stackTraceToString())) }
//! ```
//!
//! **三处容易抄漏的**:
//! 1. 这里**不是** `BaseSource.extractInlineJs` —— 它是 `startsWith` + `substring`,
//!    `<js>` 那支取的是 `substring(4, lastIndexOf("<"))`(**最后一个 `<`**,
//!    不是 `</js>`:脚本里再出现 `<` 就会被截短,这是真身的样子);
//! 2. `isJsonArray()` 只看**首尾字符**(trim 后 `[` 开头 `]` 结尾),不校验内容;
//! 3. Kotlin 的 `Regex.split` **保留首尾空片**(与 Java 的 `String.split` 不同)——
//!    规则以换行收尾时真身会多出一格空标题。
//!
//! **缓存不做**:真身有两层(进程内 `exploreKindsMap` + 落盘 `ACache`),键是
//! `md5(bookSourceUrl + exploreUrl)`。这里给的是**冷算**一次的语义,缓存留给
//! 调用方(`Engine`)—— 差分要的正是冷算。
//!
//! **`infoMap` 绑定不做**:真身 JS 那两支多绑一个 `infoMap`(发现页适配器的
//! `InfoMap`)。语料 1704 源里 19 条 JS 发现规则,**用到它的 0 条**。

use rubato_core::entities::ExploreKind;
use rubato_core::host::{HostEnv, JsBindings, JsHost, JsRuleEnv, JsValue, SourceBinding};
use std::sync::LazyLock;

/// `runCatching` 抓到的两类失败。**差分只比「哪一类错」** —— 消息文本一边是
/// Rhino / Gson、一边是 QuickJS / 我们自己写的,不可能也不该逐字对齐
/// (与 js 套 `errorTag` 同一条口径)。
#[derive(Debug, Clone, PartialEq)]
pub enum ExploreKindsError {
    /// `@js:` / `<js>` 那两支求值抛了
    Js(String),
    /// `GSON.fromJsonArray<ExploreKind>` 那一步抛了(JsonSyntaxException 一族)
    Json(String),
    /// Java 侧抛的(`substring` 越界那一支)—— 标签用真身的异常类名
    Host(String),
}

impl ExploreKindsError {
    /// 消息原文。**归一成标签是差分执行器的事**(`js_case_runner::error_tag`
    /// 那一份是全仓唯一一份口径,别在这里再写一遍)。
    pub fn message(&self) -> &str {
        match self {
            ExploreKindsError::Js(m) | ExploreKindsError::Json(m) | ExploreKindsError::Host(m) => m,
        }
    }
}

/// 真身形态:出错也交回一格(`ERROR:…`),永远不抛
pub fn explore_kinds(
    explore_url: Option<&str>,
    host: &mut dyn HostEnv,
    source: &SourceBinding,
    vars: &mut dyn JsRuleEnv,
) -> Vec<ExploreKind> {
    match explore_kinds_result(explore_url, host, source, vars) {
        Ok(kinds) => kinds,
        // 真身:`ExploreKind("ERROR:${it.localizedMessage}", it.stackTraceToString())`。
        // 栈那一位两侧不可能逐字一致(一边 Java 一边 Rust),差分只比**错的类别**
        // —— 与 js 套 `errorTag` 同一条口径。
        Err(e) => vec![ExploreKind::new(format!("ERROR:{}", e.message()), None)],
    }
}

/// 同一条路,但把失败原样交出去 —— 差分执行器要拿它归一成错误标签
pub fn explore_kinds_result(
    explore_url: Option<&str>,
    host: &mut dyn HostEnv,
    source: &SourceBinding,
    vars: &mut dyn JsRuleEnv,
) -> Result<Vec<ExploreKind>, ExploreKindsError> {
    // `isNullOrBlank()`:Kotlin 的 blank 是「全是空白字符」
    let Some(explore_url) = explore_url.filter(|s| !s.trim().is_empty()) else {
        return Ok(Vec::new());
    };

    let rule_str = if starts_with_ci(explore_url, "@js:") {
        eval_to_string(host, source, vars, &explore_url[4..])?
    } else if starts_with_ci(explore_url, "<js>") {
        // `substring(4, lastIndexOf("<"))` —— 最后一个 `<`,不是 `</js>`。
        // 找不到 `<` 时 lastIndexOf 给 -1,真身在这里抛 StringIndexOutOfBounds
        // (进 ERROR 那一格);而 `<js>` 开头保证至少有一个 `<`,取不到 -1。
        let end = explore_url.rfind('<').unwrap_or(0);
        if end < 4 {
            // `substring(4, lastIndexOf("<"))`:`<js>` 后面再没有 `<` 时
            // `lastIndexOf` 找到的是开头那个(下标 0)→ 真身在 Java 里
            // 抛 StringIndexOutOfBoundsException,进 ERROR 那一格
            return Err(ExploreKindsError::Host("StringIndexOutOfBoundsException".into()));
        }
        eval_to_string(host, source, vars, &explore_url[4..end])?
    } else {
        explore_url.to_string()
    };

    if is_json_array(&rule_str) {
        return ExploreKind::parse_json_array(&rule_str).map_err(|e| ExploreKindsError::Json(e.0));
    }
    Ok(split_kinds(&rule_str))
}

/// `startsWith(prefix, ignoreCase = true)`。**按字节比**:`exploreUrl` 第一个
/// 字符常是汉字,`s[..4]` 那种切法会切在 UTF-8 的字符中间(直接 panic)。
/// 前缀全是 ASCII,逐字节忽略大小写比较即可。
fn starts_with_ci(s: &str, prefix: &str) -> bool {
    let (b, p) = (s.as_bytes(), prefix.as_bytes());
    b.len() >= p.len() && b[..p.len()].eq_ignore_ascii_case(p)
}

/// `String?.isJsonArray()`(StringExtensions.kt L64):**只看首尾**,不校验内容
fn is_json_array(s: &str) -> bool {
    let t = s.trim();
    t.starts_with('[') && t.ends_with(']')
}

/// `(&&|\n)+`:连着的 `&&` 与换行算**一个**分隔符
static SEPS: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(&&|\n)+").expect("分隔符正则"));

/// `ruleStr.split("(&&|\n)+".toRegex())` + `kindStr.split("::")`。
///
/// Kotlin 的 `Regex.split(limit = 0)` **不丢首尾空片**(Java 的 `String.split`
/// 会丢尾部空片)—— 规则末尾带换行时真身实实在在多出一格空标题;Rust 的
/// `Regex::split` 同样保留,形态直接对上。
fn split_kinds(rule_str: &str) -> Vec<ExploreKind> {
    SEPS.split(rule_str)
        .map(|piece| {
            // `kindCfg.first()` / `kindCfg.getOrNull(1)` —— 按 `::` **全切**,
            // 只取前两段:第三段起真身**丢掉**(不是留在 url 里)
            let mut it = piece.split("::");
            let title = it.next().unwrap_or("").to_string();
            ExploreKind::new(title, it.next().map(str::to_string))
        })
        .collect()
}

/// `evalJS(js) { put("infoMap", …) }.toString().trim()` —— **第三个宿主**
/// (`BaseSource.evalJS`:`java` 就是书源实体、`baseUrl` 是 `getKey()`、
/// `java.put/get` 打 CacheManager),与 `net::source_header` 那条同一个绑定面。
fn eval_to_string(
    host: &mut dyn HostEnv,
    source: &SourceBinding,
    vars: &mut dyn JsRuleEnv,
    js: &str,
) -> Result<String, ExploreKindsError> {
    let bindings =
        JsBindings { host: JsHost::Source, source: Some(source.clone()), ..Default::default() };
    let v = host.eval_js(js, &bindings, vars).map_err(ExploreKindsError::Js)?;
    Ok(completion_to_string(&v).trim().to_string())
}

/// Kotlin 的 `Any?.toString()`(**不是** `normalizeJsResult`,那是 header 那条路)
fn completion_to_string(v: &JsValue) -> String {
    match v {
        // JS 的 null 解包成 Java null → Kotlin `.toString()` 给 "null"
        JsValue::Null => "null".into(),
        JsValue::Str(s) => s.clone(),
        JsValue::Bool(b) => b.to_string(),
        // 数字回来的是 java.lang.Double → `Double.toString`
        JsValue::Num(d) => rubato_core::java_double_to_string(*d),
        // Kotlin List 的 toString
        JsValue::List(l) => format!("[{}]", l.join(", ")),
        JsValue::Elements { strings, .. } => format!("[{}]", strings.join(", ")),
        // 对象/对象数组:真身那边完成值还是 Rhino 的 NativeObject/NativeArray,
        // `toString()` 带身份哈希 —— 那种写法**语料里一条都没有**
        // (19 条 JS 发现规则的末句都是 `JSON.stringify(…)` / `join(…)` 之类的串)。
        // 真撞上了差分会当场把它照出来,别在这里猜。
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_keeps_empty_tail() {
        // Kotlin 的 Regex.split 保留尾部空片
        let k = split_kinds("甲::/a\n乙::/b\n");
        assert_eq!(k.len(), 3);
        assert_eq!(k[2].title.as_deref(), Some(""));
        assert_eq!(k[2].url, None);
    }

    #[test]
    fn split_collapses_runs() {
        let k = split_kinds("甲::/a&&\n&&乙::/b");
        assert_eq!(k.len(), 2);
        assert_eq!(k[1].title.as_deref(), Some("乙"));
        assert_eq!(k[1].url.as_deref(), Some("/b"));
    }

    #[test]
    fn third_segment_is_dropped() {
        // `kindCfg.getOrNull(1)` 只取第二段 —— 语料里 `标题::链接::一行个数`
        // 这种三段写法真实存在,第三段在真身那里就是被丢掉的
        let k = split_kinds("甲::/a::多余");
        assert_eq!(k[0].url.as_deref(), Some("/a"));
    }

    #[test]
    fn json_array_is_first_and_last_char_only() {
        assert!(is_json_array("  [不是合法 json]  "));
        assert!(!is_json_array("[1,2"));
    }
}
