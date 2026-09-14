//! `java.net.URL` 的解析/拼接语义移植(计划书风险 7)。
//!
//! 被测面是 `NetworkUtils.getAbsoluteURL`,其行为完全由 JDK 的
//! `java.net.URL(URL context, String spec)` + `URLStreamHandler.parseURL`
//! + `toExternalForm` 决定,与 WHATWG URL 差别很大,故逐行移植而非套用 url crate:
//! - 协议表是 JDK 内置 handler 的集合,`data:`/`thunder:` 之类抛
//!   MalformedURLException(上层吞掉后回落到原始相对串);
//! - `parseURL` 里 `spec.indexOf('?')` 是从 **0** 开始找的(JDK 怪癖),
//!   于是 base 带 query 时的行为要照抄;
//! - `..`/`.` 归一化只在「相对路径」分支跑,且是 JDK 自己的循环写法,
//!   与 RFC 3986 的 remove_dot_segments 不等价(`/../../a` 保持不变)。
//!
//! 已知未覆盖(差分用例回避,出现即记豁免):`jar:`(需要 `!/`)、`mailto:`
//! (自有 parseURL)、IPv6 字面量的合法性校验。

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MalformedUrl(pub String);

impl std::fmt::Display for MalformedUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MalformedURLException: {}", self.0)
    }
}
impl std::error::Error for MalformedUrl {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JavaUrl {
    pub protocol: String,
    pub authority: Option<String>,
    pub user_info: Option<String>,
    pub host: Option<String>,
    pub port: i32,
    pub path: Option<String>,
    pub query: Option<String>,
    pub reference: Option<String>,
}

/// `URLStreamHandler.getURLStreamHandler` 能拿到 handler 的协议
/// (JDK 21 实测:jar 有 handler 但要求 spec 含 `!/`)
fn known_protocol(p: &str) -> bool {
    matches!(p, "http" | "https" | "file" | "ftp" | "mailto" | "jrt" | "jmod" | "jar")
}

/// `URL.isValidProtocol`
fn is_valid_protocol(s: &str) -> bool {
    let mut it = s.chars();
    match it.next() {
        Some(c) if c.is_alphabetic() => {}
        _ => return false,
    }
    it.all(|c| c.is_alphanumeric() || c == '.' || c == '+' || c == '-')
}

/// JDK 20+ 的 host 合法性检查(实测 JDK 21):控制字符、空格、DEL 与
/// `" < > [ \ ] ^ ` { | }` 一律非法。注意 userInfo 与 path 不受此限。
fn illegal_host_char(host: &str) -> Option<char> {
    host.chars().find(|&c| {
        c <= '\u{20}'
            || c == '\u{7f}'
            || matches!(c, '"' | '<' | '>' | '[' | '\\' | ']' | '^' | '`' | '{' | '|' | '}')
    })
}

/// `IPAddressUtil.isIPv6LiteralAddress` 的近似:剥掉 `%scope` 后按标准
/// IPv6 文本解析(Java 还接受少数畸形写法,差异记豁免)
fn is_ipv6_literal(s: &str) -> bool {
    let core = s.split('%').next().unwrap_or(s);
    core.parse::<std::net::Ipv6Addr>().is_ok()
}

fn index_of(s: &[char], needle: char, from: usize) -> Option<usize> {
    (from..s.len()).find(|&i| s[i] == needle)
}

/// Java `Integer.parseInt`(仅十进制、允许前导 '-'),失败 → None
fn parse_int(s: &str) -> Option<i32> {
    if s.is_empty() {
        return None;
    }
    let (neg, digits) = match s.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v: i64 = digits.parse().ok()?;
    let v = if neg { -v } else { v };
    i32::try_from(v).ok()
}

impl JavaUrl {
    /// `new URL(spec)`
    pub fn parse(spec: &str) -> Result<Self, MalformedUrl> {
        new_url(None, spec)
    }

    /// `new URL(this, spec)`
    pub fn resolve(&self, spec: &str) -> Result<Self, MalformedUrl> {
        new_url(Some(self), spec)
    }

    /// `URLStreamHandler.toExternalForm` / `URL.toString`
    pub fn to_external_form(&self) -> String {
        let mut r = String::new();
        r.push_str(&self.protocol);
        r.push(':');
        if let Some(a) = &self.authority {
            if !a.is_empty() {
                r.push_str("//");
                r.push_str(a);
            }
        }
        if let Some(p) = &self.path {
            r.push_str(p);
        }
        if let Some(q) = &self.query {
            r.push('?');
            r.push_str(q);
        }
        if let Some(f) = &self.reference {
            r.push('#');
            r.push_str(f);
        }
        r
    }
}

/// `new URL(URL context, String spec)`
pub fn new_url(context: Option<&JavaUrl>, spec: &str) -> Result<JavaUrl, MalformedUrl> {
    let sp: Vec<char> = spec.chars().collect();
    let mut limit = sp.len();
    let mut start = 0usize;
    let mut new_protocol: Option<String> = None;
    let mut a_ref = false;
    let mut is_relative = false;

    while limit > 0 && sp[limit - 1] <= ' ' {
        limit -= 1; // 去掉尾部空白
    }
    while start < limit && sp[start] <= ' ' {
        start += 1; // 去掉头部空白
    }
    if sp.len() >= start + 4 {
        let seg: String = sp[start..start + 4].iter().collect();
        if seg.eq_ignore_ascii_case("url:") {
            start += 4;
        }
    }
    if start < sp.len() && sp[start] == '#' {
        a_ref = true;
    }
    let mut i = start;
    while !a_ref && i < limit && sp[i] != '/' {
        if sp[i] == ':' {
            let s: String = sp[start..i].iter().collect::<String>().to_lowercase();
            if is_valid_protocol(&s) {
                new_protocol = Some(s);
                start = i + 1;
            }
            break;
        }
        i += 1;
    }

    let mut u = JavaUrl { port: -1, ..Default::default() };
    let mut protocol = new_protocol.clone();
    let mut np = new_protocol;
    // handler 从 context 继承时,后面的「unknown protocol」检查被跳过
    let mut handler_from_context = false;

    if let Some(ctx) = context {
        let same = match &np {
            None => true,
            Some(p) => p.eq_ignore_ascii_case(&ctx.protocol),
        };
        if same {
            handler_from_context = true;
            // RFC2396 5.2.3 的向后兼容:context 是层级式 URL 且 spec 带同名 scheme
            // 时,当作 spec 没写 scheme
            if ctx.path.as_deref().is_some_and(|p| p.starts_with('/')) {
                np = None;
            }
            if np.is_none() {
                protocol = Some(ctx.protocol.clone());
                u.authority = ctx.authority.clone();
                u.user_info = ctx.user_info.clone();
                u.host = ctx.host.clone();
                u.port = ctx.port;
                u.path = ctx.path.clone();
                // 注意:query 不在这里继承(只有 start == limit 时才继承)
                is_relative = true;
            }
        }
    }

    let Some(protocol) = protocol else {
        return Err(MalformedUrl(format!("no protocol: {spec}")));
    };
    if !handler_from_context && !known_protocol(&protocol) {
        return Err(MalformedUrl(format!("unknown protocol: {protocol}")));
    }
    u.protocol = protocol;

    if let Some(i) = index_of(&sp, '#', start) {
        if i + 1 > limit {
            return Err(MalformedUrl("String index out of range".into()));
        }
        u.reference = Some(sp[i + 1..limit].iter().collect());
        limit = i;
    }

    // RFC2396 5.2.2:纯 ref 的相对 URL 继承 query 与 ref
    if is_relative && start == limit {
        let ctx = context.expect("isRelative 蕴含 context 非空");
        u.query = ctx.query.clone();
        if u.reference.is_none() {
            u.reference = ctx.reference.clone();
        }
    }

    parse_url(&mut u, &sp, start, limit)?;
    Ok(u)
}

/// `URLStreamHandler.parseURL`
fn parse_url(
    u: &mut JavaUrl,
    spec_in: &[char],
    mut start: usize,
    mut limit: usize,
) -> Result<(), MalformedUrl> {
    let mut spec: Vec<char> = spec_in.to_vec();
    let mut authority = u.authority.clone();
    let mut user_info = u.user_info.clone();
    let mut host = u.host.clone();
    let mut port = u.port;
    let mut path = u.path.clone();
    let mut query = u.query.clone();
    let reference = u.reference.clone();

    let mut is_rel_path = false;
    let mut query_only = false;

    if start < limit {
        // 注意:indexOf('?') 从 0 开始找,不是从 start(JDK 原样如此)
        let query_start = spec.iter().position(|&c| c == '?');
        query_only = query_start == Some(start);
        if let Some(qs) = query_start {
            if qs < limit {
                query = Some(spec[qs + 1..limit].iter().collect());
                if limit > qs {
                    limit = qs;
                }
                spec.truncate(qs);
            }
        }
    }

    let i;
    let is_unc = start + 4 <= limit
        && spec[start] == '/'
        && spec[start + 1] == '/'
        && spec[start + 2] == '/'
        && spec[start + 3] == '/';
    if !is_unc && start + 2 <= limit && spec[start] == '/' && spec[start + 1] == '/' {
        start += 2;
        i = index_of(&spec, '/', start).filter(|&x| x <= limit).unwrap_or_else(|| {
            index_of(&spec, '?', start).filter(|&x| x <= limit).unwrap_or(limit)
        });
        let auth: String = spec[start..i.min(spec.len())].iter().collect();
        authority = Some(auth.clone());
        host = Some(auth.clone());

        match auth.find('@') {
            Some(ind) if auth.rfind('@') == Some(ind) => {
                user_info = Some(auth[..ind].to_string());
                host = Some(auth[ind + '@'.len_utf8()..].to_string());
            }
            Some(_) => {
                // authority 里多于一个 '@' → 非 server-based
                user_info = None;
                host = None;
            }
            None => user_info = None,
        }
        if let Some(h) = host.clone() {
            if h.starts_with('[') {
                match h.find(']') {
                    Some(ind) if ind > 2 && is_ipv6_literal(&h[1..ind]) => {
                        host = Some(h[..=ind].to_string());
                        port = -1;
                        if h.len() > ind + 1 {
                            if h.as_bytes()[ind + 1] == b':' {
                                if h.len() > ind + 2 {
                                    port = parse_int(&h[ind + 2..]).ok_or_else(|| {
                                        MalformedUrl(format!(
                                            "For input string: \"{}\"",
                                            &h[ind + 2..]
                                        ))
                                    })?;
                                }
                            } else {
                                return Err(MalformedUrl(format!(
                                    "Invalid authority field: {auth}"
                                )));
                            }
                        }
                    }
                    _ => return Err(MalformedUrl(format!("Invalid authority field: {auth}"))),
                }
            } else {
                port = -1;
                if let Some(ind) = h.find(':') {
                    if h.len() > ind + 1 {
                        port = parse_int(&h[ind + 1..]).ok_or_else(|| {
                            MalformedUrl(format!("For input string: \"{}\"", &h[ind + 1..]))
                        })?;
                    }
                    host = Some(h[..ind].to_string());
                }
                // JDK 20+ 收紧:host 里出现非法字符直接 MalformedURLException
                if let Some(bad) = host.as_deref().and_then(illegal_host_char) {
                    return Err(MalformedUrl(format!("Illegal character found in host: '{bad}'")));
                }
            }
        } else {
            host = Some(String::new());
        }
        if port < -1 {
            return Err(MalformedUrl(format!("Invalid port number :{port}")));
        }
        start = i;
        // authority 已定,path 只由 spec 决定(RFC 2396 §5.2.4)
        if authority.as_deref().is_some_and(|a| !a.is_empty()) {
            path = Some(String::new());
        }
    }
    if host.is_none() {
        host = Some(String::new());
    }

    if start < limit {
        if spec[start] == '/' {
            path = Some(spec[start..limit].iter().collect());
        } else if path.as_deref().is_some_and(|p| !p.is_empty()) {
            is_rel_path = true;
            let p = path.clone().unwrap();
            let ind = p.rfind('/').map(|x| x as isize).unwrap_or(-1);
            let separator = if ind == -1 && authority.is_some() { "/" } else { "" };
            let head = &p[..(ind + 1) as usize];
            let tail: String = spec[start..limit].iter().collect();
            path = Some(format!("{head}{separator}{tail}"));
        } else {
            let separator = if authority.is_some() { "/" } else { "" };
            let tail: String = spec[start..limit].iter().collect();
            path = Some(format!("{separator}{tail}"));
        }
    } else if query_only && path.is_some() {
        let p = path.clone().unwrap();
        let ind = p.rfind('/').unwrap_or(0);
        path = Some(format!("{}/", &p[..ind]));
    }
    let mut p = path.unwrap_or_default();

    if is_rel_path {
        // 去掉内嵌 /./
        while let Some(i) = p.find("/./") {
            p = format!("{}{}", &p[..i], &p[i + 2..]);
        }
        // 去掉能消解的 /../
        let mut from = 0usize;
        while let Some(rel) = p[from..].find("/../") {
            let i = from + rel;
            let prev = if i > 0 { p[..i].rfind('/') } else { None };
            match prev {
                // Java 是 `path.indexOf("/../", limit) != 0`(绝对下标),
                // 等价于「不是开头那个 /../」——`/../../a` 因此原样保留
                Some(lim) if !(lim == 0 && p.starts_with("/../")) => {
                    p = format!("{}{}", &p[..lim], &p[i + 3..]);
                    from = 0;
                }
                _ => from = i + 3,
            }
        }
        // 去掉能消解的结尾 ..
        while p.ends_with("/..") {
            let i = p.find("/..").unwrap();
            match if i > 0 { p[..i].rfind('/') } else { None } {
                Some(lim) => p = p[..lim + 1].to_string(),
                None => break,
            }
        }
        // 去掉开头的 .
        if p.starts_with("./") && p.len() > 2 {
            p = p[2..].to_string();
        }
        // 去掉结尾的 .
        if p.ends_with("/.") {
            p = p[..p.len() - 1].to_string();
        }
    }

    u.authority = authority;
    u.user_info = user_info;
    u.host = host;
    u.port = port;
    u.path = Some(p);
    u.query = query;
    u.reference = reference;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net_utils::get_absolute_url;

    fn abs(base: &str, path: &str) -> String {
        get_absolute_url(Some(&JavaUrl::parse(base).expect("base 合法")), path)
    }

    #[test]
    fn relative_resolution() {
        let b = "http://www.example.com/a/b/c.html";
        assert_eq!(abs(b, "d.html"), "http://www.example.com/a/b/d.html");
        assert_eq!(abs(b, "../d.html"), "http://www.example.com/a/d.html");
        assert_eq!(abs(b, "/d.html"), "http://www.example.com/d.html");
        assert_eq!(abs(b, "//other.com/x"), "http://other.com/x");
        // JDK 的 queryOnly 分支把 path 截到最后一个 '/':不是 c.html?q=1
        assert_eq!(abs(b, "?q=1"), "http://www.example.com/a/b/?q=1");
        assert_eq!(abs(b, "#top"), "http://www.example.com/a/b/c.html#top");
    }

    #[test]
    fn jdk_quirks() {
        let b = "http://www.example.com/a/b/c.html";
        // JDK 的循环写法消解不掉开头的 /../
        assert_eq!(abs(b, "/../../x"), "http://www.example.com/../../x");
        // 未知协议 → MalformedURLException → 回落到 trim 后的原串
        assert_eq!(abs(b, "thunder://x"), "thunder://x");
        // javascript: 一律空串;data URL 原样
        assert_eq!(abs(b, "javascript:void(0)"), "");
        assert_eq!(abs(b, "data:image/png;base64,AA"), "data:image/png;base64,AA");
        // host 非法字符(JDK 20+ 收紧)
        assert_eq!(abs(b, "//a b.com/x"), "//a b.com/x");
    }

    #[test]
    fn parse_and_external_form() {
        let u = JavaUrl::parse("https://u:p@host.com:8443/x/y?q=1#f").expect("合法");
        assert_eq!(u.protocol, "https");
        assert_eq!(u.host.as_deref(), Some("host.com"));
        assert_eq!(u.port, 8443);
        assert_eq!(u.to_external_form(), "https://u:p@host.com:8443/x/y?q=1#f");
        assert!(JavaUrl::parse("data:text/plain,x").is_err());
    }
}
