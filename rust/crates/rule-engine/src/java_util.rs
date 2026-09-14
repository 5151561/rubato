//! 裁判侧 JVM 语义里会影响差分结果的两件小事:
//! `HashMap` 的遍历序、以及 gson 解 `@put:{...}` 的宽严两档。

use serde_json::Value;

/// Java `String.hashCode`(按 UTF-16 码元)
pub fn java_string_hash(s: &str) -> i32 {
    let mut h: i32 = 0;
    for u in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(u as i32);
    }
    h
}

/// `HashMap` 的遍历序:按 (桶下标, 插入序) 排序。
/// 桶下标 = `(h ^ (h >>> 16)) & (n-1)`,n 是 2 的幂且满足 size <= 0.75n。
/// Java 8 的 resize 会保持链内相对顺序,故只需按最终容量算一次。
pub fn hash_map_order<T>(entries: Vec<(String, T)>) -> Vec<(String, T)> {
    let mut n: usize = 16;
    while entries.len() > n * 3 / 4 {
        n *= 2;
    }
    let mut idx: Vec<(usize, usize)> = entries
        .iter()
        .enumerate()
        .map(|(i, (k, _))| {
            let h = java_string_hash(k);
            let spread = h ^ ((h as u32) >> 16) as i32;
            ((spread as usize) & (n - 1), i)
        })
        .collect();
    idx.sort();
    let mut slots: Vec<Option<(String, T)>> = entries.into_iter().map(Some).collect();
    idx.into_iter().map(|(_, i)| slots[i].take().expect("每个下标只取一次")).collect()
}

/// gson `JsonPrimitive.getAsString()` 等价:字符串裸串、数字/布尔按字面、
/// 结构体按 gson `JsonElement.toString()`(默认开 HTML 转义)
fn gson_as_string(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        other => Some(gson_json_text(other)),
    }
}

/// gson `JsonElement.toString()`:紧凑 JSON + HTML 转义(`< > & = '` 走 \\u)
fn gson_json_text(v: &Value) -> String {
    fn esc(s: &str, out: &mut String) {
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '<' | '>' | '&' | '=' | '\'' => {
                    out.push_str(&format!("\\u{:04x}", c as u32));
                }
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
    }
    fn ser(v: &Value, out: &mut String) {
        match v {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&n.to_string()),
            Value::String(s) => esc(s, out),
            Value::Array(a) => {
                out.push('[');
                for (i, x) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    ser(x, out);
                }
                out.push(']');
            }
            Value::Object(m) => {
                out.push('{');
                for (i, (k, x)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    esc(k, out);
                    out.push(':');
                    ser(x, out);
                }
                out.push('}');
            }
        }
    }
    let mut out = String::new();
    ser(v, &mut out);
    out
}

/// `GSONStrict.fromJsonObject<Map<String, String>>`:严格 JSON,顶层必须是对象
pub fn parse_put_json_strict(s: &str) -> Option<Vec<(String, Option<String>)>> {
    let v: Value = serde_json::from_str(s).ok()?;
    let Value::Object(m) = v else { return None };
    Some(m.into_iter().map(|(k, v)| (k, gson_as_string(&v))).collect())
}

/// `GSON.fromJsonObject<Map<String, String>>` 的宽松档。
/// `putPattern` 保证串形如 `{...}` 且内部无 `}`,故只需处理扁平对象:
/// 键/值可裸写、可单引号、可双引号(gson lenient 的常见面)。
pub fn parse_put_json_lenient(s: &str) -> Option<Vec<(String, Option<String>)>> {
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    let skip_ws = |i: &mut usize, cs: &[char]| {
        while *i < cs.len() && cs[*i].is_whitespace() {
            *i += 1
        }
    };

    skip_ws(&mut i, &cs);
    if i >= cs.len() || cs[i] != '{' {
        return None;
    }
    i += 1;
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    loop {
        skip_ws(&mut i, &cs);
        if i < cs.len() && cs[i] == '}' {
            i += 1;
            break;
        }
        let key = read_token(&cs, &mut i, &[':'])?.0;
        skip_ws(&mut i, &cs);
        if i >= cs.len() || cs[i] != ':' {
            return None;
        }
        i += 1;
        skip_ws(&mut i, &cs);
        let (raw, quoted) = read_token(&cs, &mut i, &[',', '}'])?;
        // 裸写的 null 才是 JSON null;带引号的是字符串 "null"
        let val = if raw == "null" && !quoted { None } else { Some(raw) };
        out.push((key, val));
        skip_ws(&mut i, &cs);
        if i < cs.len() && cs[i] == ',' {
            i += 1;
            continue;
        }
        if i < cs.len() && cs[i] == '}' {
            i += 1;
            break;
        }
        return None;
    }
    skip_ws(&mut i, &cs);
    if i != cs.len() {
        return None;
    }
    Some(out)
}

/// 读一个 lenient 记号:带引号则读到配对引号(支持 \\ 转义),
/// 否则读到 stop 集合或空白为止(gson 的 unquoted 值口径)
fn read_token(cs: &[char], i: &mut usize, stop: &[char]) -> Option<(String, bool)> {
    if *i >= cs.len() {
        return None;
    }
    let q = cs[*i];
    if q == '"' || q == '\'' {
        *i += 1;
        let mut s = String::new();
        while *i < cs.len() && cs[*i] != q {
            if cs[*i] == '\\' && *i + 1 < cs.len() {
                *i += 1;
                s.push(match cs[*i] {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    'b' => '\u{8}',
                    'f' => '\u{c}',
                    c => c,
                });
            } else {
                s.push(cs[*i]);
            }
            *i += 1;
        }
        if *i >= cs.len() {
            return None;
        }
        *i += 1;
        return Some((s, true));
    }
    let start = *i;
    while *i < cs.len() && !stop.contains(&cs[*i]) && !cs[*i].is_whitespace() {
        *i += 1;
    }
    if *i == start {
        return None;
    }
    Some((cs[start..*i].iter().collect(), false))
}
