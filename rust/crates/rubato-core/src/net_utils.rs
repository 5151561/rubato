//! `NetworkUtils` / `StringExtensions` 里被规则层用到的那几个判定。

use crate::java_url::JavaUrl;

/// `String?.isAbsUrl()`
pub fn is_abs_url(s: &str) -> bool {
    let low = s.to_ascii_lowercase();
    low.starts_with("http://") || low.starts_with("https://")
}

/// `String?.isDataUrl()`——`AppPattern.dataUriRegex` 的 **完全匹配**
/// (`^data:.*?;base64,(.*)`;Java 的 `.` 不匹配行终止符)
pub fn is_data_url(s: &str) -> bool {
    fn no_line_terminator(s: &str) -> bool {
        !s.contains(['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}'])
    }
    let Some(rest) = s.strip_prefix("data:") else {
        return false;
    };
    match rest.find(";base64,") {
        Some(i) => {
            no_line_terminator(&rest[..i]) && no_line_terminator(&rest[i + ";base64,".len()..])
        }
        None => false,
    }
}

/// `String?.isXml()`
pub fn is_xml(s: &str) -> bool {
    let t = kotlin_trim(s);
    t.starts_with('<') && t.ends_with('>')
}

/// `NetworkUtils.getBaseUrl`
pub fn get_base_url(url: &str) -> Option<String> {
    let low = url.to_ascii_lowercase();
    if !(low.starts_with("http://") || low.starts_with("https://")) {
        return None;
    }
    // Java `url.indexOf("/", 9)`:UTF-16 下标、越界返回 -1。
    // http(s):// 前缀保证前 8 字节是 ASCII;第 9 个 char 的字节位置按 char 数
    let byte9 = url.char_indices().nth(9).map(|(i, _)| i);
    Some(match byte9.and_then(|b| url[b..].find('/').map(|i| b + i)) {
        Some(i) => url[..i].to_string(),
        None => url.to_string(),
    })
}

/// `String?.isJson()`
pub fn is_json(s: &str) -> bool {
    let t = kotlin_trim(s);
    (t.starts_with('{') && t.ends_with('}')) || (t.starts_with('[') && t.ends_with(']'))
}

/// Kotlin `String.trim()`:按 `Char.isWhitespace()`(含 NBSP 等 SpaceChar)裁剪,
/// 与 Java `String.trim()`(`<= ' '`)不同
pub fn kotlin_trim(s: &str) -> String {
    s.trim_matches(|c: char| c.is_whitespace()).to_string()
}

/// `NetworkUtils.getAbsoluteURL(baseURL: URL?, relativePath: String)`
pub fn get_absolute_url(base: Option<&JavaUrl>, relative_path: &str) -> String {
    let trimmed = kotlin_trim(relative_path);
    let Some(base) = base else {
        return trimmed;
    };
    if is_abs_url(&trimmed) || is_data_url(&trimmed) {
        return trimmed;
    }
    if trimmed.starts_with("javascript") {
        return String::new();
    }
    // 注意:传给 URL(...) 的是**未 trim** 的原串(裁判如此)
    match base.resolve(relative_path) {
        Ok(u) => u.to_external_form(),
        Err(_) => trimmed,
    }
}

/// `NetworkUtils.getAbsoluteURL(baseURL: String?, relativePath: String)`
pub fn get_absolute_url_str(base_url: Option<&str>, relative_path: &str) -> String {
    match base_url {
        None | Some("") => kotlin_trim(relative_path),
        Some(b) => {
            // URL(baseURL.substringBefore(",")),失败按 null 处理
            let head = b.split(',').next().unwrap_or(b);
            let parsed = JavaUrl::parse(head).ok();
            get_absolute_url(parsed.as_ref(), relative_path)
        }
    }
}

/// `NetworkUtils.isIPAddress`:ipv4 正则完全匹配,或含 ':'(v6 粗判)
pub fn is_ip_address(input: &str) -> bool {
    if input.is_empty() {
        return false;
    }
    if input.contains(':') {
        return true;
    }
    let octets: Vec<&str> = input.split('.').collect();
    if octets.len() != 4 {
        return false;
    }
    octets.iter().all(|o| {
        let b = o.as_bytes();
        match b.len() {
            // `(25[0-5]|2[0-4]\d|[0-1]?\d?\d)`
            1 | 2 => b.iter().all(u8::is_ascii_digit),
            3 => {
                b.iter().all(u8::is_ascii_digit)
                    && (b[0] == b'0'
                        || b[0] == b'1'
                        || (b[0] == b'2' && (b[1] <= b'4' || (b[1] == b'5' && b[2] <= b'5'))))
            }
            _ => false,
        }
    })
}

/// okhttp `PublicSuffixDatabase.getEffectiveTldPlusOne`(经 psl crate;
/// 两侧 PSL 版本可能有微小漂移,罕见域名差异记豁免)
pub fn effective_tld_plus_one(host: &str) -> Option<String> {
    psl::domain_str(host).map(str::to_string)
}

/// `NetworkUtils.getSubDomain`:cookie 存取的键(二级域名;IP 原样;失败回退)
pub fn get_sub_domain(url: &str) -> String {
    let Some(base_url) = get_base_url(url) else {
        return url.to_string();
    };
    // runCatching { URL(baseUrl).host ... }.getOrDefault(baseUrl)
    let Ok(u) = JavaUrl::parse(&base_url) else {
        return base_url;
    };
    let Some(host) = u.host.clone().filter(|h| !h.is_empty()) else {
        return base_url;
    };
    if is_ip_address(&host) {
        return host;
    }
    effective_tld_plus_one(&host).unwrap_or(host)
}
