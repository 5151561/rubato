//! `BaseSource.getHeaderMap()`(judge/engine data/entities/BaseSource.kt L182)。
//!
//! **基准切换的语义漂移点**:旧基准(MD3 fork)的差分契约把 `source.header`
//! 恒当空处理;LegadoTeam 基准下这条规则进入契约——header 规则 JSON 解析、
//! 默认 UA 注入、登录头合并都影响真实请求头,必须两侧一致。
//!
//! ```text
//! header?.let {
//!     val json = extractInlineJs(it)?.let { js -> normalizeJsResult(evalJS(js)) } ?: it
//!     GSON.fromJsonObject<Map<String, String>>(json).getOrNull()?.let { putAll(it) }
//! }                                   // 解析失败/形状不符 → 整块跳过
//! if (!has(UA_NAME, true)) put(UA_NAME, AppConfig.userAgent)
//! if (hasLoginHeader) getLoginHeaderMap()?.let { putAll(it) }
//! ```

use rubato_core::gson;
use rubato_core::host::{HostEnv, JsBindings, JsHost, JsValue, SourceBinding};
use serde_json::Value;

use crate::{JS_PATTERN, header_get_ci, header_put};

/// `BaseSource.extractInlineJs`:整串**完全匹配** `<js>..</js>` 或 `@js:..`
/// 才算内联 JS,取捕获组并 trim;否则原样当 JSON 用。
fn extract_inline_js(rule: &str) -> Option<String> {
    let text = rule.trim();
    if text.is_empty() {
        return None;
    }
    let m = JS_PATTERN.captures(text)?;
    // Java 的 Matcher.matches():必须整串匹配
    if m.get(0)?.as_str().len() != text.len() {
        return None;
    }
    let g = m.get(1).or_else(|| m.get(2))?;
    Some(g.as_str().trim().to_string())
}

/// `GSON.fromJsonObject<Map<String, String>>`:顶层必须是对象;值走 legado
/// **自己注册的** `StringJsonDeserializer`(GsonExtensions.kt L135)——
/// 基元取 `asString`,JSON null 给 Java null,**对象/数组给 `toString()`**
/// (不是抛异常:差分抓到一条 header 写成 `{"headers":{"User-Agent":…}}`,
/// 真身给的是键 `headers` 配一整段 JSON 串)。
///
/// 值为 JSON `null` 时 Gson 往 Map 里放 Java null;语料(1704 源 387 条 header)
/// 里不存在这种写法,这里按「丢掉该项」处理并留记号。
fn parse_header_json(json: &str) -> Option<Vec<(String, String)>> {
    let Some(Value::Object(m)) = gson::parse_lenient(json) else { return None };
    let mut out = Vec::with_capacity(m.len());
    for (k, v) in &m {
        if let Some(s) = gson::string_field(v) {
            out.push((k.clone(), s));
        }
    }
    Some(out)
}

/// 建出一次请求的 source 级请求头表。
///
/// - `header`:书源的 `header` 字段;
/// - `default_ua`:`AppConfig.userAgent`;
/// - `login_header`:`hasLoginHeader` 为真时的 `getLoginHeaderMap()`(Phase 1
///   恒 None——登录头属 Phase 2 面)。
pub fn source_header_map(
    header: Option<&str>,
    default_ua: &str,
    login_header: Option<&[(String, String)]>,
    host: &mut dyn HostEnv,
    source: &SourceBinding,
    vars: &mut dyn rubato_core::host::JsRuleEnv,
) -> Vec<(String, String)> {
    // header 的 JS 跑在**第三个宿主** `BaseSource.evalJS` 上(BaseSource.kt L186):
    // `java` 就是书源实体、`baseUrl` 是 getKey()、没有 result/book/chapter/page/key,
    // 而 `java.put/get` 打 CacheManager。按 AnalyzeRule 的绑定面跑会把语义钉歪。
    let bindings =
        &JsBindings { host: JsHost::Source, source: Some(source.clone()), ..Default::default() };
    let mut map: Vec<(String, String)> = Vec::new();
    if let Some(h) = header {
        // 真身把整个 header 块包在 try/catch 里,任何异常都只记日志
        let json = match extract_inline_js(h) {
            Some(js) => match host.eval_js(&js, bindings, vars) {
                Ok(v) => js_result_string(&v),
                Err(_) => String::new(),
            },
            None => h.to_string(),
        };
        if let Some(entries) = parse_header_json(&json) {
            for (k, v) in entries {
                header_put(&mut map, &k, &v);
            }
        }
    }
    // `if (!has(UA_NAME, true)) put(UA_NAME, AppConfig.userAgent)`
    if header_get_ci(&map, UA_NAME).is_none() {
        header_put(&mut map, UA_NAME, default_ua);
    }
    if let Some(login) = login_header {
        for (k, v) in login {
            header_put(&mut map, k, v);
        }
    }
    map
}

/// `JsSourceEngine.normalizeJsResult(result).orEmpty()`:
/// null/undefined → `""`;String/CharSequence → 原串;其余 → `GSON.toJson`
/// (GSON 开了 setPrettyPrinting)。Phase 1 的 JS 是确定性桩,真实 JS header
/// (语料 1704 源里 14 条)留给 Phase 2 的 js-host 差分。
fn js_result_string(v: &JsValue) -> String {
    match v {
        JsValue::Null => String::new(),
        JsValue::Str(s) => s.clone(),
        JsValue::Json(j) => gson::to_json_pretty(j),
        other => {
            gson::to_json_pretty(&serde_json::to_value(js_plain(other)).unwrap_or(Value::Null))
        }
    }
}

/// 桩 JS 的非字符串返回值(Num/Bool/List)交给 GSON 序列化前先转成 serde 值
fn js_plain(v: &JsValue) -> Value {
    match v {
        JsValue::Num(d) => serde_json::Number::from_f64(*d).map_or(Value::Null, Value::Number),
        JsValue::Bool(b) => Value::Bool(*b),
        JsValue::List(l) => Value::Array(l.iter().map(|s| Value::String(s.clone())).collect()),
        JsValue::Elements { strings, .. } => {
            Value::Array(strings.iter().map(|s| Value::String(s.clone())).collect())
        }
        // 对象数组:GSON 序列化的就是这份结构
        JsValue::Native(a) => Value::Array(a.clone()),
        JsValue::Str(s) => Value::String(s.clone()),
        JsValue::Json(j) => j.clone(),
        JsValue::Null => Value::Null,
    }
}

/// `AppConst.UA_NAME`
const UA_NAME: &str = "User-Agent";
