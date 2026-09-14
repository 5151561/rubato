//! gson 行为的等价面(书源里 `@put:{}` 与链接 option JSON 都经它)。
//!
//! 关键口径:
//! - `GSON` = 宽松档 + `LONG_OR_DOUBLE` 数字策略 + `disableHtmlEscaping` +
//!   **`setPrettyPrinting`**(所以 `GSON.toJson(map)` 是带缩进的多行文本);
//! - `GSONStrict` 只是把 Strictness 调成 STRICT;
//! - 注册了 `StringJsonDeserializer`:String 字段拿基元的 `asString`,
//!   拿到结构体则是 `JsonElement.toString()`(那条路**开着** HTML 转义);
//! - Object 字段走默认 ObjectTypeAdapter:对象→LinkedTreeMap、数组→ArrayList、
//!   数字→Long 或 Double,于是 `toString()` 是 `{k=v}` / `[a, b]` / `1` / `1.0`。

use serde_json::Value;

/// 严格档:等价 `GSONStrict.fromJson`(serde_json 本身就是严格的)
pub fn parse_strict(s: &str) -> Option<Value> {
    serde_json::from_str(s).ok()
}

/// 宽松档:gson `JsonReader` 的 lenient 面里书源真实用到的部分——
/// 裸键/裸值、单引号、`//` `#` `/* */` 注释、`=`/`=>` 当 `:`、`;` 当 `,`。
pub fn parse_lenient(s: &str) -> Option<Value> {
    let cs: Vec<char> = s.chars().collect();
    let mut p = Lenient { cs, i: 0 };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i != p.cs.len() {
        return None; // fromJson 会检查尾部必须是 END_DOCUMENT
    }
    Some(v)
}

struct Lenient {
    cs: Vec<char>,
    i: usize,
}

impl Lenient {
    fn peek(&self) -> Option<char> {
        self.cs.get(self.i).copied()
    }

    fn ws(&mut self) {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.i += 1;
            }
            match (self.peek(), self.cs.get(self.i + 1)) {
                (Some('/'), Some('/')) | (Some('#'), _) => {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.i += 1;
                    }
                }
                (Some('/'), Some('*')) => {
                    self.i += 2;
                    while self.i < self.cs.len()
                        && !(self.cs[self.i] == '*' && self.cs.get(self.i + 1) == Some(&'/'))
                    {
                        self.i += 1;
                    }
                    self.i = (self.i + 2).min(self.cs.len());
                }
                _ => return,
            }
        }
    }

    fn value(&mut self) -> Option<Value> {
        match self.peek()? {
            '{' => self.object(),
            '[' => self.array(),
            '"' | '\'' => Some(Value::String(self.quoted()?)),
            _ => {
                let raw = self.bare()?;
                Some(match raw.as_str() {
                    "true" => Value::Bool(true),
                    "false" => Value::Bool(false),
                    "null" => Value::Null,
                    _ => match serde_json::from_str::<Value>(&raw) {
                        Ok(v @ Value::Number(_)) => v,
                        _ => Value::String(raw),
                    },
                })
            }
        }
    }

    fn object(&mut self) -> Option<Value> {
        self.i += 1; // {
        let mut m = serde_json::Map::new();
        self.ws();
        if self.peek()? == '}' {
            self.i += 1;
            return Some(Value::Object(m));
        }
        loop {
            self.ws();
            // 逗号后必须是键:gson 即使 lenient 也不接受尾逗号
            let key = match self.peek()? {
                '"' | '\'' => self.quoted()?,
                _ => self.bare()?,
            };
            self.ws();
            match self.peek()? {
                ':' => self.i += 1,
                '=' => {
                    self.i += 1;
                    if self.peek() == Some('>') {
                        self.i += 1;
                    }
                }
                _ => return None,
            }
            self.ws();
            m.insert(key, self.value()?);
            self.ws();
            match self.peek()? {
                ',' | ';' => self.i += 1,
                '}' => {
                    self.i += 1;
                    return Some(Value::Object(m));
                }
                _ => return None,
            }
        }
    }

    fn array(&mut self) -> Option<Value> {
        self.i += 1; // [
        let mut a = Vec::new();
        self.ws();
        if self.peek()? == ']' {
            self.i += 1;
            return Some(Value::Array(a));
        }
        loop {
            self.ws();
            a.push(self.value()?);
            self.ws();
            match self.peek()? {
                ',' | ';' => self.i += 1,
                ']' => {
                    self.i += 1;
                    return Some(Value::Array(a));
                }
                _ => return None,
            }
        }
    }

    fn quoted(&mut self) -> Option<String> {
        let q = self.cs[self.i];
        self.i += 1;
        let mut s = String::new();
        while self.i < self.cs.len() && self.cs[self.i] != q {
            if self.cs[self.i] == '\\' && self.i + 1 < self.cs.len() {
                self.i += 1;
                match self.cs[self.i] {
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    'r' => s.push('\r'),
                    'b' => s.push('\u{8}'),
                    'f' => s.push('\u{c}'),
                    'u' => {
                        let hex: String = self.cs.get(self.i + 1..self.i + 5)?.iter().collect();
                        let cp = u32::from_str_radix(&hex, 16).ok()?;
                        s.push(char::from_u32(cp)?);
                        self.i += 4;
                    }
                    c => s.push(c),
                }
            } else {
                s.push(self.cs[self.i]);
            }
            self.i += 1;
        }
        if self.i >= self.cs.len() {
            return None;
        }
        self.i += 1;
        Some(s)
    }

    /// gson lenient 的裸记号:到结构字符或空白为止
    fn bare(&mut self) -> Option<String> {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_whitespace() || "{}[]:,;=/\\\"'#".contains(c) {
                break;
            }
            self.i += 1;
        }
        if self.i == start {
            return None;
        }
        Some(self.cs[start..self.i].iter().collect())
    }
}

/// `StringJsonDeserializer.deserialize`:基元取 asString,null 给 null,
/// 结构体给 `JsonElement.toString()`(**开** HTML 转义)
pub fn string_field(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        // JsonElement.toString():JsonWriter 默认 serializeNulls = true
        other => Some(to_json(other, true, false, true)),
    }
}

/// Java 侧 `Object` 字段的 `toString()`:LinkedTreeMap → `{k=v}`、
/// ArrayList → `[a, b]`、Long → `1`、Double → Java Double.toString
pub fn object_to_string(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) => number_to_string(n),
        Value::Array(a) => {
            format!("[{}]", a.iter().map(object_to_string).collect::<Vec<_>>().join(", "))
        }
        Value::Object(m) => {
            let items: Vec<String> =
                m.iter().map(|(k, v)| format!("{k}={}", object_to_string(v))).collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}

pub fn number_to_string(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        i.to_string()
    } else if let Some(u) = n.as_u64() {
        u.to_string()
    } else {
        java_double(n.as_f64().unwrap_or(f64::NAN))
    }
}

/// `GSON.toJson(any)`——GSON 开了 setPrettyPrinting;
/// 且 gson **默认 serializeNulls = false**,对象里值为 null 的键**整条丢掉**
/// (数组里的 null 照写:JsonWriter.nullValue 只在有 deferredName 时才跳过)
pub fn to_json_pretty(v: &Value) -> String {
    to_json(v, false, true, false)
}

fn to_json(v: &Value, html_escape: bool, pretty: bool, serialize_nulls: bool) -> String {
    fn esc(s: &str, html: bool, out: &mut String) {
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '<' | '>' | '&' | '=' | '\'' if html => {
                    out.push_str(&format!("\\u{:04x}", c as u32));
                }
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    fn ser(v: &Value, html: bool, pretty: bool, nulls: bool, depth: usize, out: &mut String) {
        let (nl, pad, pad_in) = if pretty {
            ("\n".to_string(), "  ".repeat(depth), "  ".repeat(depth + 1))
        } else {
            (String::new(), String::new(), String::new())
        };
        let colon = if pretty { ": " } else { ":" };
        match v {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&number_to_string(n)),
            Value::String(s) => esc(s, html, out),
            Value::Array(a) if a.is_empty() => out.push_str("[]"),
            Value::Array(a) => {
                out.push('[');
                for (i, x) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&nl);
                    out.push_str(&pad_in);
                    ser(x, html, pretty, nulls, depth + 1, out);
                }
                out.push_str(&nl);
                out.push_str(&pad);
                out.push(']');
            }
            Value::Object(m) => {
                // serializeNulls = false:值为 null 的键在写 key 之前就被丢掉
                let entries: Vec<(&String, &Value)> =
                    m.iter().filter(|(_, x)| nulls || !x.is_null()).collect();
                if entries.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                for (i, (k, x)) in entries.into_iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&nl);
                    out.push_str(&pad_in);
                    esc(k, html, out);
                    out.push_str(colon);
                    ser(x, html, pretty, nulls, depth + 1, out);
                }
                out.push_str(&nl);
                out.push_str(&pad);
                out.push('}');
            }
        }
    }
    let mut out = String::new();
    ser(v, html_escape, pretty, serialize_nulls, 0, &mut out);
    out
}

/// Java `Double.toString`。本体在 [`crate::java_num`] —— 全仓唯一一份。
pub use crate::java_num::java_double_to_string as java_double;
