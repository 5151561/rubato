//! `AnalyzeUrl.UrlOption` 的移植。
//!
//! gson 是**按字段**反射填充的(不走 setter),所以这里按字段名取值,
//! 再用 getter 的语义(空白归 null、类型转换)读出来。

use rubato_core::gson;
use serde_json::Value;

#[derive(Debug, Default, Clone)]
pub struct UrlOption {
    raw: serde_json::Map<String, Value>,
}

impl UrlOption {
    pub fn from_value(v: Value) -> Option<Self> {
        let Value::Object(raw) = v else {
            // gson 把非对象反序列化到 data class 会抛 → getOrNull() 给 null
            return None;
        };
        // Int?/Long? 字段走 gson 默认适配器:数字或"数字串"可以,
        // 其余(布尔、非数字串、对象)抛 JsonSyntaxException → 整个 option 变 null
        for name in ["retry", "serverID", "webViewDelayTime"] {
            if let Some(v) = raw.get(name).filter(|v| !v.is_null()) {
                long_of(v)?;
            }
        }
        Some(Self { raw })
    }

    fn field(&self, name: &str) -> Option<&Value> {
        self.raw.get(name).filter(|v| !v.is_null())
    }

    /// String 字段:注册了 StringJsonDeserializer。
    /// 注意 **gson 是按字段反射填充的,不走 setter**,所以 setter 里
    /// 「空白归 null」的处理在这条路上不生效——`"charset": ""` 保持空串
    fn string_field(&self, name: &str) -> Option<String> {
        self.field(name).and_then(gson::string_field)
    }

    pub fn method(&self) -> Option<String> {
        self.string_field("method")
    }

    pub fn charset(&self) -> Option<String> {
        self.string_field("charset")
    }

    pub fn type_(&self) -> Option<String> {
        self.string_field("type")
    }

    pub fn web_js(&self) -> Option<String> {
        self.string_field("webJs")
    }

    /// `@SerializedName(value = "dnsIp", alternate = ["resolveIp"])`
    ///
    /// gson 把两个名字绑到**同一个** BoundField,按流顺序依次写入 ——
    /// 两个键都出现时**后出现的那个赢**(哪怕它是 null)。serde_json 开了
    /// `preserve_order`,这里照着文档序取最后一次出现。
    pub fn dns_ip(&self) -> Option<String> {
        let last =
            self.raw.iter().rfind(|(k, _)| k.as_str() == "dnsIp" || k.as_str() == "resolveIp")?;
        gson::string_field(last.1)
    }

    /// `getTimeout()` → `parseRequestTimeoutMillis`(AnalyzeUrlNetworkOptions L12)。
    /// 字段类型是 `Any?`,gson 把 JSON 数字填成 Double、字符串填成 String;
    /// 只有落在 `1..=Int.MAX_VALUE` 的整数值才算数,小数/超界/非数字串 → null。
    pub fn timeout(&self) -> Option<i64> {
        const MAX_OKHTTP_TIMEOUT_MILLIS: i64 = i32::MAX as i64;
        let v = self.field("timeout")?;
        let t = match v {
            // gson 的 Any? 字段:整数也会填成 Double
            Value::Number(n) => {
                let d = n.as_f64()?;
                if d.is_finite() && d % 1.0 == 0.0 { Some(d as i64) } else { None }
            }
            Value::String(s) => s.trim().parse::<i64>().ok(),
            _ => None,
        }?;
        (1..=MAX_OKHTTP_TIMEOUT_MILLIS).contains(&t).then_some(t)
    }

    /// `getFollowRedirects()` → `parseBooleanOption`(同上 L25)。
    /// Boolean 直接用;数字只认 0/1;字符串只认 `true`/`1`/`false`/`0`
    /// (trim + 小写);其余一律 null。
    pub fn follow_redirects(&self) -> Option<bool> {
        match self.field("followRedirects")? {
            Value::Bool(b) => Some(*b),
            Value::Number(n) => match n.as_f64()? {
                0.0 => Some(false),
                1.0 => Some(true),
                _ => None,
            },
            Value::String(s) => match s.trim().to_lowercase().as_str() {
                "true" | "1" => Some(true),
                "false" | "0" => Some(false),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn js(&self) -> Option<String> {
        self.string_field("js")
    }

    pub fn body_js(&self) -> Option<String> {
        self.string_field("bodyJs")
    }

    pub fn retry(&self) -> i32 {
        self.field("retry").and_then(long_of).unwrap_or(0) as i32
    }

    pub fn server_id(&self) -> Option<i64> {
        self.field("serverID").and_then(long_of)
    }

    pub fn web_view_delay_time(&self) -> Option<i64> {
        self.field("webViewDelayTime").and_then(long_of)
    }

    /// `useWebView()`:null / "" / false / "false" 为假,其余(含数字 0)为真
    pub fn use_web_view(&self) -> bool {
        match self.field("webView") {
            None => false,
            Some(Value::Bool(false)) => false,
            Some(Value::String(s)) if s.is_empty() || s == "false" => false,
            Some(_) => true,
        }
    }

    /// `getHeaderMap()`:字段是 Map 就直接用;是 String 再按 JSON 解一层。
    ///
    /// **两支的数字语义相同**,都是 `LONG_OR_DOUBLE`:字段本身是 Map 时
    /// gson 按 `Any?` 走 ObjectTypeAdapter;字符串那支虽然显式声明成
    /// `Map<String, Any>`,但 INITIAL_GSON 注册的
    /// `MapDeserializerDoubleAsIntFix` 在这里**不生效**(见
    /// fixtures/cases/analyze-url/README.md 的实测结论)——`3.0` 两边都是 `"3.0"`。
    pub fn header_map(&self) -> Option<Vec<(String, String)>> {
        let v = self.field("headers")?;
        let m = match v {
            Value::Object(m) => m.clone(),
            Value::String(s) => match gson::parse_lenient(s) {
                Some(Value::Object(m)) => m,
                _ => return None,
            },
            _ => return None,
        };
        Some(m.iter().map(|(k, v)| (k.clone(), gson::object_to_string(v))).collect())
    }

    /// `getBody()`:字段是 String 直接返回,否则 `GSON.toJson`(**带缩进**)
    pub fn body(&self) -> Option<String> {
        let v = self.field("body")?;
        Some(match v {
            Value::String(s) => s.clone(),
            other => gson::to_json_pretty(other),
        })
    }
}

/// gson `JsonReader.nextInt/nextLong`:数字直接取,字符串按数字解析
fn long_of(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}
