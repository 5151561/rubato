//! okhttp 5.4.0 `HttpUrl` 的移植(解析 / 规范化 / 相对解析 / toString)。
//!
//! 这是「实际发出的 URL」的唯一权威:AnalyzeUrl 的 urlNoQuery + encodedQuery
//! 都要经它规范化,快照 key 与重定向 Location 解析也依赖它
//! (契约见 docs/http-snapshot.md §7.2)。
//!
//! 逐行对照 okhttp 源码移植:HttpUrl.kt(Builder.parse/resolvePath/push/pop)、
//! internal/url/-Url.kt(canonicalize/percentDecode)、
//! internal/-HostnamesCommon.kt(toCanonicalHost)。
//! 已知缩水(Phase 1 用例回避,README 记录):
//! - IPv6 字面量 host:一律判非法。
//!
//! IDN host 已接(M2n):okhttp 5 的 `idnToAscii` 是「UTS-46 映射表 → NFC →
//! 逐标签 punycode 解码/校验/再编码」,这里交给 `idna` crate 的
//! `domain_to_ascii`(同一份 Unicode 数据、同样是 non-transitional、
//! UseSTD3ASCIIRules=false)。**全 ASCII 的 host 仍走原来的快路**:
//! 映射表在那一档只做大小写折叠,而那条路上压着五千多条已经绿了的用例,
//! 没有理由换一份实现去重跑它。

pub const USERNAME_ENCODE_SET: &str = " \"':;<=>@[]^`{}|/\\?#";
pub const PASSWORD_ENCODE_SET: &str = " \"':;<=>@[]^`{}|/\\?#";
pub const PATH_SEGMENT_ENCODE_SET: &str = " \"<>^`{}|/\\?#";
pub const QUERY_ENCODE_SET: &str = " \"'<>#";
pub const FRAGMENT_ENCODE_SET: &str = "";

const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";

fn parse_hex_digit(c: char) -> i32 {
    match c {
        '0'..='9' => c as i32 - '0' as i32,
        'a'..='f' => c as i32 - 'a' as i32 + 10,
        'A'..='F' => c as i32 - 'A' as i32 + 10,
        _ => -1,
    }
}

fn is_percent_encoded(s: &[char], pos: usize, limit: usize) -> bool {
    pos + 2 < limit
        && s[pos] == '%'
        && parse_hex_digit(s[pos + 1]) != -1
        && parse_hex_digit(s[pos + 2]) != -1
}

/// okhttp `String.canonicalize`(charset 恒 UTF-8)。输入/输出都是 char 视角,
/// 与 Java 的 codePointAt 循环等价(char 层面无需代理对拆分——Rust char 即码点)。
pub fn canonicalize(
    input: &str,
    encode_set: &str,
    already_encoded: bool,
    strict: bool,
    plus_is_space: bool,
    unicode_allowed: bool,
) -> String {
    let chars: Vec<char> = input.chars().collect();
    canonicalize_range(
        &chars,
        0,
        chars.len(),
        encode_set,
        already_encoded,
        strict,
        plus_is_space,
        unicode_allowed,
    )
}

fn canonicalize_range(
    chars: &[char],
    pos: usize,
    limit: usize,
    encode_set: &str,
    already_encoded: bool,
    strict: bool,
    plus_is_space: bool,
    unicode_allowed: bool,
) -> String {
    let mut out = String::new();
    let mut i = pos;
    while i < limit {
        let c = chars[i];
        let cp = c as u32;
        if already_encoded && (c == '\t' || c == '\n' || c == '\u{000c}' || c == '\r') {
            // 跳过
        } else if c == '+' && plus_is_space {
            out.push_str(if already_encoded { "+" } else { "%2B" });
        } else if cp < 0x20
            || cp == 0x7f
            || (cp >= 0x80 && !unicode_allowed)
            || encode_set.contains(c)
            || (c == '%' && (!already_encoded || (strict && !is_percent_encoded(chars, i, limit))))
        {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).as_bytes() {
                out.push('%');
                out.push(HEX_DIGITS[(b >> 4) as usize] as char);
                out.push(HEX_DIGITS[(b & 0xf) as usize] as char);
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// okhttp `String.percentDecode`:无效 %XX 原样保留;解出的字节按 UTF-8
/// 组串(无效序列 → U+FFFD,对齐 okio readUtf8 的替换行为)。
pub fn percent_decode(input: &str, plus_is_space: bool) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut bytes: Vec<u8> = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '%' && i + 2 < chars.len() {
            let d1 = parse_hex_digit(chars[i + 1]);
            let d2 = parse_hex_digit(chars[i + 2]);
            if d1 != -1 && d2 != -1 {
                bytes.push(((d1 << 4) + d2) as u8);
                i += 3;
                continue;
            }
        } else if c == '+' && plus_is_space {
            bytes.push(b' ');
            i += 1;
            continue;
        }
        let mut buf = [0u8; 4];
        bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// cookie 的 `parseDomain` 复用同一套 host 规范化
pub(crate) fn canonical_host_for_cookie(host: &str) -> Option<String> {
    to_canonical_host(host)
}

/// okhttp `toCanonicalHost` 的 ASCII 面。
fn to_canonical_host(host: &str) -> Option<String> {
    if host.contains(':') {
        // IPv6 字面量:Phase 1 不做(README 记录)
        return None;
    }
    let result = idn_to_ascii(host)?;
    if result.is_empty() {
        return None;
    }
    if contains_invalid_hostname_ascii_codes(&result) {
        return None;
    }
    if contains_invalid_label_lengths(&result) {
        return None;
    }
    Some(result)
}

/// okhttp `String.idnToAscii()`。
///
/// 全 ASCII 的一档自己走:UTS-46 的映射表对 ASCII 只做 `A-Z → a-z`,
/// 其余原样(`_`、`,`、`!` 这些 STD3 不允许的字符,okhttp 的
/// UseSTD3ASCIIRules 是关的,照样留着)。`xn--` 标签在这一档会被 idna
/// 解码后再编码,结果与原串一致 —— 但也可能因为解不开而**判非法**,
/// 那正是 okhttp 的行为(Punycode.decode 失败 → null)。
fn idn_to_ascii(host: &str) -> Option<String> {
    if host.is_ascii()
        && !host.split('.').any(|l| l.len() >= 4 && l[..4].eq_ignore_ascii_case("xn--"))
    {
        return Some(host.to_ascii_lowercase());
    }
    idna::domain_to_ascii(host).ok()
}

fn contains_invalid_hostname_ascii_codes(s: &str) -> bool {
    s.chars().any(|c| c <= '\u{001f}' || c >= '\u{007f}' || " #%/:?@[\\]".contains(c))
}

fn contains_invalid_label_lengths(s: &str) -> bool {
    let len = s.len();
    if !(1..=253).contains(&len) {
        return true;
    }
    let mut label_start = 0;
    loop {
        let dot = s[label_start..].find('.').map(|d| d + label_start);
        let label_length = match dot {
            None => len - label_start,
            Some(d) => d - label_start,
        };
        if !(1..=63).contains(&label_length) {
            return true;
        }
        match dot {
            None => break,
            Some(d) if d == len - 1 => break, // 末尾 '.' 允许
            Some(d) => label_start = d + 1,
        }
    }
    false
}

pub fn default_port(scheme: &str) -> u16 {
    match scheme {
        "http" => 80,
        "https" => 443,
        _ => 0,
    }
}

/// 已规范化的 URL。字段全部保存**编码后**形态(okhttp Builder 同构)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpUrl {
    pub scheme: String,
    pub encoded_username: String,
    pub encoded_password: String,
    pub host: String,
    /// 生效端口(缺省端口也填充;toString 时按默认端口省略)
    pub port: u16,
    pub encoded_path_segments: Vec<String>,
    /// 展平的 name/value 交替表(value 可缺失),同 okhttp
    pub encoded_query_names_and_values: Option<Vec<Option<String>>>,
    pub encoded_fragment: Option<String>,
}

impl HttpUrl {
    pub fn parse(input: &str) -> Result<HttpUrl, String> {
        parse_with_base(None, input)
    }

    /// okhttp `HttpUrl.resolve(link)`:失败 → None
    pub fn resolve(&self, link: &str) -> Option<HttpUrl> {
        parse_with_base(Some(self), link).ok()
    }

    pub fn encoded_path(&self) -> String {
        let mut out = String::new();
        for seg in &self.encoded_path_segments {
            out.push('/');
            out.push_str(seg);
        }
        out
    }

    pub fn encoded_query(&self) -> Option<String> {
        self.encoded_query_names_and_values.as_ref().map(|q| query_string(q))
    }

    /// okhttp `Builder.encodedQuery(v)` setter
    pub fn set_encoded_query(&mut self, encoded_query: Option<&str>) {
        self.encoded_query_names_and_values = encoded_query.map(|q| {
            to_query_names_and_values(&canonicalize(q, QUERY_ENCODE_SET, true, false, true, false))
        });
    }
}

fn query_string(q: &[Option<String>]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < q.len() {
        if i > 0 {
            out.push('&');
        }
        if let Some(name) = &q[i] {
            out.push_str(name);
        }
        if let Some(value) = q.get(i + 1).and_then(|v| v.as_ref()) {
            out.push('=');
            out.push_str(value);
        }
        i += 2;
    }
    out
}

fn to_query_names_and_values(s: &str) -> Vec<Option<String>> {
    let chars: Vec<char> = s.chars().collect();
    let mut result = Vec::new();
    let mut pos = 0usize;
    while pos <= chars.len() {
        let ampersand_offset =
            chars[pos..].iter().position(|&c| c == '&').map(|d| d + pos).unwrap_or(chars.len());
        let equals_offset = chars[pos..].iter().position(|&c| c == '=').map(|d| d + pos);
        match equals_offset {
            Some(eq) if eq <= ampersand_offset => {
                result.push(Some(chars[pos..eq].iter().collect()));
                result.push(Some(chars[eq + 1..ampersand_offset].iter().collect()));
            }
            _ => {
                result.push(Some(chars[pos..ampersand_offset].iter().collect()));
                result.push(None);
            }
        }
        pos = ampersand_offset + 1;
    }
    result
}

impl std::fmt::Display for HttpUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}://", self.scheme)?;
        if !self.encoded_username.is_empty() || !self.encoded_password.is_empty() {
            write!(f, "{}", self.encoded_username)?;
            if !self.encoded_password.is_empty() {
                write!(f, ":{}", self.encoded_password)?;
            }
            write!(f, "@")?;
        }
        write!(f, "{}", self.host)?;
        if self.port != default_port(&self.scheme) {
            write!(f, ":{}", self.port)?;
        }
        write!(f, "{}", self.encoded_path())?;
        if let Some(q) = &self.encoded_query_names_and_values {
            write!(f, "?{}", query_string(q))?;
        }
        if let Some(frag) = &self.encoded_fragment {
            write!(f, "#{frag}")?;
        }
        Ok(())
    }
}

fn is_ascii_ws(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{000c}' | '\r' | ' ')
}

fn delimiter_offset(chars: &[char], delimiters: &str, pos: usize, limit: usize) -> usize {
    (pos..limit).find(|&i| delimiters.contains(chars[i])).unwrap_or(limit)
}

/// okhttp `Builder.parse` + `build`。错误信息不进差分(两侧只比「成败」)。
fn parse_with_base(base: Option<&HttpUrl>, input: &str) -> Result<HttpUrl, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = (0..chars.len()).find(|&i| !is_ascii_ws(chars[i])).unwrap_or(chars.len());
    let limit =
        (pos..chars.len()).rev().find(|&i| !is_ascii_ws(chars[i])).map(|i| i + 1).unwrap_or(pos);

    // Scheme
    let scheme: String;
    let scheme_delimiter = scheme_delimiter_offset(&chars, pos, limit);
    let lower: String =
        chars[pos..limit.min(pos + 6)].iter().collect::<String>().to_ascii_lowercase();
    if scheme_delimiter != -1 {
        if lower.starts_with("https:") {
            scheme = "https".into();
            pos += 6;
        } else if lower.starts_with("http:") {
            scheme = "http".into();
            pos += 5;
        } else {
            return Err(format!(
                "Expected URL scheme 'http' or 'https' but was '{}'",
                chars[..scheme_delimiter as usize].iter().collect::<String>()
            ));
        }
    } else if let Some(b) = base {
        scheme = b.scheme.clone();
    } else {
        return Err("Expected URL scheme 'http' or 'https' but no scheme was found".into());
    }

    let mut encoded_username = String::new();
    let mut encoded_password = String::new();
    let host: Option<String>;
    let port: i32;
    let mut encoded_path_segments: Vec<String> = vec![String::new()];
    let mut encoded_query_names_and_values: Option<Vec<Option<String>>> = None;

    // Authority
    let mut has_username = false;
    let mut has_password = false;
    let slash_count = (pos..limit).take_while(|&i| chars[i] == '/' || chars[i] == '\\').count();
    if slash_count >= 2 || base.is_none() || base.is_some_and(|b| b.scheme != scheme) {
        pos += slash_count;
        'authority: loop {
            let component_delimiter = delimiter_offset(&chars, "@/\\?#", pos, limit);
            let c: i32 =
                if component_delimiter != limit { chars[component_delimiter] as i32 } else { -1 };
            if c == '@' as i32 {
                if !has_password {
                    let password_colon = delimiter_offset(&chars, ":", pos, component_delimiter);
                    let canonical_username = canonicalize_range(
                        &chars,
                        pos,
                        password_colon,
                        USERNAME_ENCODE_SET,
                        true,
                        false,
                        false,
                        false,
                    );
                    encoded_username = if has_username {
                        format!("{encoded_username}%40{canonical_username}")
                    } else {
                        canonical_username
                    };
                    if password_colon != component_delimiter {
                        has_password = true;
                        encoded_password = canonicalize_range(
                            &chars,
                            password_colon + 1,
                            component_delimiter,
                            PASSWORD_ENCODE_SET,
                            true,
                            false,
                            false,
                            false,
                        );
                    }
                    has_username = true;
                } else {
                    let extra = canonicalize_range(
                        &chars,
                        pos,
                        component_delimiter,
                        PASSWORD_ENCODE_SET,
                        true,
                        false,
                        false,
                        false,
                    );
                    encoded_password = format!("{encoded_password}%40{extra}");
                }
                pos = component_delimiter + 1;
            } else {
                // -1 / '/' / '\\' / '?' / '#':host 段
                let port_colon = port_colon_offset(&chars, pos, component_delimiter);
                let raw_host: String = chars[pos..port_colon].iter().collect();
                host = to_canonical_host(&percent_decode(&raw_host, false));
                if port_colon + 1 < component_delimiter {
                    port = parse_port(&chars, port_colon + 1, component_delimiter);
                    if port == -1 {
                        return Err(format!(
                            "Invalid URL port: \"{}\"",
                            chars[port_colon + 1..component_delimiter].iter().collect::<String>()
                        ));
                    }
                } else {
                    port = default_port(&scheme) as i32;
                }
                if host.is_none() {
                    return Err(format!("Invalid URL host: \"{raw_host}\""));
                }
                pos = component_delimiter;
                break 'authority;
            }
        }
    } else {
        // 相对链接:authority 全抄 base(进这支说明上面的 `base.is_none()` 为假)
        #[allow(clippy::unnecessary_unwrap)]
        let b = base.expect("有 base");
        encoded_username = b.encoded_username.clone();
        encoded_password = b.encoded_password.clone();
        host = Some(b.host.clone());
        port = b.port as i32;
        encoded_path_segments = b.encoded_path_segments.clone();
        if pos == limit || chars[pos] == '#' {
            encoded_query_names_and_values = b.encoded_query().map(|q| {
                to_query_names_and_values(&canonicalize(
                    &q,
                    QUERY_ENCODE_SET,
                    true,
                    false,
                    true,
                    false,
                ))
            });
        }
    }

    // Path
    let path_delimiter = delimiter_offset(&chars, "?#", pos, limit);
    resolve_path(&mut encoded_path_segments, &chars, pos, path_delimiter);
    pos = path_delimiter;

    // Query
    if pos < limit && chars[pos] == '?' {
        let query_delimiter = delimiter_offset(&chars, "#", pos, limit);
        encoded_query_names_and_values = Some(to_query_names_and_values(&canonicalize_range(
            &chars,
            pos + 1,
            query_delimiter,
            QUERY_ENCODE_SET,
            true,
            false,
            true,
            false,
        )));
        pos = query_delimiter;
    }

    // Fragment
    let mut encoded_fragment: Option<String> = None;
    if pos < limit && chars[pos] == '#' {
        encoded_fragment = Some(canonicalize_range(
            &chars,
            pos + 1,
            limit,
            FRAGMENT_ENCODE_SET,
            true,
            false,
            false,
            true,
        ));
    }

    Ok(HttpUrl {
        scheme,
        encoded_username,
        encoded_password,
        host: host.expect("authority 已定"),
        port: port as u16,
        encoded_path_segments,
        encoded_query_names_and_values,
        encoded_fragment,
    })
}

fn resolve_path(segments: &mut Vec<String>, chars: &[char], start_pos: usize, limit: usize) {
    let mut pos = start_pos;
    if pos == limit {
        return; // 空路径:保持 base 路径
    }
    let c = chars[pos];
    if c == '/' || c == '\\' {
        segments.clear();
        segments.push(String::new());
        pos += 1;
    } else {
        let last = segments.len() - 1;
        segments[last] = String::new();
    }
    let mut i = pos;
    while i < limit {
        let seg_delimiter = delimiter_offset(chars, "/\\", i, limit);
        let has_trailing_slash = seg_delimiter < limit;
        push_segment(segments, chars, i, seg_delimiter, has_trailing_slash);
        i = seg_delimiter;
        if has_trailing_slash {
            i += 1;
        }
    }
}

fn push_segment(
    segments: &mut Vec<String>,
    chars: &[char],
    pos: usize,
    limit: usize,
    add_trailing_slash: bool,
) {
    let segment =
        canonicalize_range(chars, pos, limit, PATH_SEGMENT_ENCODE_SET, true, false, false, false);
    if is_dot(&segment) {
        return;
    }
    if is_dot_dot(&segment) {
        pop_segment(segments);
        return;
    }
    let last = segments.len() - 1;
    if segments[last].is_empty() {
        segments[last] = segment;
    } else {
        segments.push(segment);
    }
    if add_trailing_slash {
        segments.push(String::new());
    }
}

fn pop_segment(segments: &mut Vec<String>) {
    let removed = segments.pop().expect("segments 非空");
    if removed.is_empty() && !segments.is_empty() {
        let last = segments.len() - 1;
        segments[last] = String::new();
    } else {
        segments.push(String::new());
    }
}

fn is_dot(s: &str) -> bool {
    s == "." || s.eq_ignore_ascii_case("%2e")
}

fn is_dot_dot(s: &str) -> bool {
    s == ".."
        || s.eq_ignore_ascii_case("%2e.")
        || s.eq_ignore_ascii_case(".%2e")
        || s.eq_ignore_ascii_case("%2e%2e")
}

/// 返回 scheme 后的 ':' 下标;没有合法 scheme 前缀 → -1
// okhttp `schemeDelimiterOffset` 的逐下标移植:保持与原文同形,不改写成迭代器
#[allow(clippy::needless_range_loop)]
fn scheme_delimiter_offset(chars: &[char], pos: usize, limit: usize) -> i32 {
    if limit.saturating_sub(pos) < 2 {
        return -1;
    }
    if !chars[pos].is_ascii_alphabetic() {
        return -1;
    }
    for i in (pos + 1)..limit {
        match chars[i] {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '+' | '-' | '.' => continue,
            ':' => return i as i32,
            _ => return -1,
        }
    }
    -1
}

fn port_colon_offset(chars: &[char], pos: usize, limit: usize) -> usize {
    let mut i = pos;
    while i < limit {
        match chars[i] {
            '[' => {
                while i + 1 < limit {
                    i += 1;
                    if chars[i] == ']' {
                        break;
                    }
                }
            }
            ':' => return i,
            _ => {}
        }
        i += 1;
    }
    limit
}

fn parse_port(chars: &[char], pos: usize, limit: usize) -> i32 {
    let port_string = canonicalize_range(chars, pos, limit, "", false, false, false, false);
    match port_string.parse::<i64>() {
        Ok(i) if (1..=65535).contains(&i) => i as i32,
        _ => -1,
    }
}
