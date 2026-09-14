//! okhttp 5.4.0 `Cookie.parse`/`parseAll` 的移植——差分只关心三件事:
//! 该条 Set-Cookie 是否被采纳(域匹配/公共后缀/格式检查)、name/value、
//! 以及 persistent(有合法 expires 或 max-age)。expiresAt 具体数值不进差分
//! (录放无「当前时间」),但 expires 日期**能否解析**必须逐行对齐,
//! 因为它决定 persistent → cookie 落库还是进 session。

use crate::http_url::HttpUrl;
use regex::Regex;
use rubato_core::net_utils::effective_tld_plus_one;
use std::sync::LazyLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCookie {
    pub name: String,
    pub value: String,
    pub persistent: bool,
}

static TIME_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{1,2}):(\d{1,2}):(\d{1,2})[^\d]*$").expect("time"));
static DAY_OF_MONTH_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{1,2})[^\d]*$").expect("day"));
static MONTH_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec).*$").expect("month")
});
static YEAR_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{2,4})[^\d]*$").expect("year"));
static VERIFY_AS_IP_ADDRESS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(([0-9a-fA-F]*:[0-9a-fA-F:.]*)|([\d.]+))$").expect("ip"));

fn can_parse_as_ip_address(s: &str) -> bool {
    VERIFY_AS_IP_ADDRESS.is_match(s)
}

fn delimiter_offset(s: &[char], delimiter: char, pos: usize, limit: usize) -> usize {
    (pos..limit).find(|&i| s[i] == delimiter).unwrap_or(limit)
}

/// Kotlin `trimSubstring`(ASCII 空白语义,同 okhttp indexOf*NonAsciiWhitespace)
fn trim_substring(s: &[char], start: usize, end: usize) -> String {
    let is_ws = |c: char| matches!(c, '\t' | '\n' | '\u{000c}' | '\r' | ' ');
    let a = (start..end).find(|&i| !is_ws(s[i])).unwrap_or(end);
    let b = (a..end).rev().find(|&i| !is_ws(s[i])).map(|i| i + 1).unwrap_or(a);
    s[a..b].iter().collect()
}

fn index_of_control_or_non_ascii(s: &str) -> bool {
    s.chars().any(|c| c <= '\u{001f}' || c >= '\u{007f}')
}

/// `Cookie.parseAll(url, headers)`:headers 里全部 `set-cookie`(大小写不敏感)按序解析,
/// 非法条目丢弃。
pub fn parse_all(url: &HttpUrl, headers: &[(String, String)]) -> Vec<ParsedCookie> {
    headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie"))
        .filter_map(|(_, v)| parse(url, v))
        .collect()
}

pub fn parse(url: &HttpUrl, set_cookie: &str) -> Option<ParsedCookie> {
    let s: Vec<char> = set_cookie.chars().collect();
    let limit0 = s.len();
    let cookie_pair_end = delimiter_offset(&s, ';', 0, limit0);

    let pair_equals_sign = delimiter_offset(&s, '=', 0, cookie_pair_end);
    if pair_equals_sign == cookie_pair_end {
        return None;
    }

    let cookie_name = trim_substring(&s, 0, pair_equals_sign);
    if cookie_name.is_empty() || index_of_control_or_non_ascii(&cookie_name) {
        return None;
    }
    let cookie_value = trim_substring(&s, pair_equals_sign + 1, cookie_pair_end);
    if index_of_control_or_non_ascii(&cookie_value) {
        return None;
    }

    let mut persistent = false;
    let mut expires_ok = false; // 是否出现过合法 expires / max-age
    let mut domain: Option<String> = None;

    let mut pos = cookie_pair_end + 1;
    let limit = s.len();
    while pos < limit {
        let attribute_pair_end = delimiter_offset(&s, ';', pos, limit);
        let attribute_equals_sign = delimiter_offset(&s, '=', pos, attribute_pair_end);
        let attribute_name = trim_substring(&s, pos, attribute_equals_sign);
        let attribute_value = if attribute_equals_sign < attribute_pair_end {
            trim_substring(&s, attribute_equals_sign + 1, attribute_pair_end)
        } else {
            String::new()
        };

        if attribute_name.eq_ignore_ascii_case("expires") {
            if parse_expires(&attribute_value) {
                expires_ok = true;
            }
        } else if attribute_name.eq_ignore_ascii_case("max-age") {
            if parse_max_age(&attribute_value) {
                expires_ok = true;
            }
        } else if attribute_name.eq_ignore_ascii_case("domain") {
            if let Some(d) = parse_domain(&attribute_value) {
                domain = Some(d);
            }
        }
        // path/secure/httponly/samesite:不影响差分观察面

        pos = attribute_pair_end + 1;
    }
    persistent |= expires_ok;

    // 域匹配:显式 domain 必须匹配 url host,否则整条丢弃
    let url_host = &url.host;
    if let Some(d) = &domain {
        if !domain_match(url_host, d) {
            return None;
        }
        // domain 是 host 的真后缀时不得是公共后缀
        if url_host.len() != d.len() && effective_tld_plus_one(d).is_none() {
            return None;
        }
    }

    Some(ParsedCookie { name: cookie_name, value: cookie_value, persistent })
}

fn domain_match(url_host: &str, domain: &str) -> bool {
    if url_host == domain {
        return true;
    }
    url_host.ends_with(domain)
        && url_host.as_bytes()[url_host.len() - domain.len() - 1] == b'.'
        && !can_parse_as_ip_address(url_host)
}

/// `parseDomain`:末尾 '.' 非法;去前导 '.';host 规范化失败非法 → None(属性被忽略)
fn parse_domain(s: &str) -> Option<String> {
    if s.ends_with('.') {
        return None;
    }
    let s = s.strip_prefix('.').unwrap_or(s);
    // toCanonicalHost 的 ASCII 面与 http_url 一致
    crate::http_url::canonical_host_for_cookie(s)
}

/// `parseMaxAge`:整数即合法(数值本身不进差分)
fn parse_max_age(s: &str) -> bool {
    if s.parse::<i64>().is_ok() {
        return true;
    }
    // 超长整数(正负皆可)也算合法
    static INT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-?\d+$").expect("int"));
    INT.is_match(s)
}

/// `parseExpires`:RFC 6265 5.1.1 的 token 扫描;返回「是否解析成功」。
/// 对齐 GregorianCalendar(isLenient=false) 的日历有效性检查。
fn parse_expires(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let limit = chars.len();
    let mut pos = date_character_offset(&chars, 0, limit, false);

    let mut hour: i32 = -1;
    let mut minute: i32 = -1;
    let mut second: i32 = -1;
    let mut day_of_month: i32 = -1;
    let mut month: i32 = -1;
    let mut year: i32 = -1;

    while pos < limit {
        let end = date_character_offset(&chars, pos + 1, limit, true);
        let token: String = chars[pos..end].iter().collect();

        if hour == -1
            && let Some(c) = TIME_PATTERN.captures(&token)
        {
            hour = c[1].parse().unwrap_or(-1);
            minute = c[2].parse().unwrap_or(-1);
            second = c[3].parse().unwrap_or(-1);
        } else if day_of_month == -1
            && let Some(c) = DAY_OF_MONTH_PATTERN.captures(&token)
        {
            day_of_month = c[1].parse().unwrap_or(-1);
        } else if month == -1
            && let Some(c) = MONTH_PATTERN.captures(&token)
        {
            #[rustfmt::skip] // 手排的分列表:rustfmt 会拆成一行一个
            const MONTHS: [&str; 12] = [
                "jan", "feb", "mar", "apr", "may", "jun",
                "jul", "aug", "sep", "oct", "nov", "dec",
            ];
            let m = c[1].to_ascii_lowercase();
            month = MONTHS.iter().position(|&x| x == m).map(|i| i as i32 + 1).unwrap_or(-1);
        } else if year == -1
            && let Some(c) = YEAR_PATTERN.captures(&token)
        {
            year = c[1].parse().unwrap_or(-1);
        }

        pos = date_character_offset(&chars, end + 1, limit, false);
    }

    if (70..=99).contains(&year) {
        year += 1900;
    }
    if (0..=69).contains(&year) {
        year += 2000;
    }

    if year < 1601 || month == -1 || !(1..=31).contains(&day_of_month) {
        return false;
    }
    if !(0..=23).contains(&hour) || !(0..=59).contains(&minute) || !(0..=59).contains(&second) {
        return false;
    }
    // GregorianCalendar 严格模式:日必须在该月天数内
    day_of_month <= days_in_month(year, month)
}

fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            if leap { 29 } else { 28 }
        }
        _ => 0,
    }
}

// okhttp `dateCharacterOffset` 的逐下标移植:保持与原文同形,不改写成迭代器
#[allow(clippy::needless_range_loop)]
fn date_character_offset(input: &[char], pos: usize, limit: usize, invert: bool) -> usize {
    for i in pos..limit {
        let c = input[i] as u32;
        let date_character = (c < 0x20 && c != 0x09)
            || c >= 0x7f
            || (0x30..=0x39).contains(&c)
            || (0x61..=0x7a).contains(&c)
            || (0x41..=0x5a).contains(&c)
            || c == 0x3a;
        if date_character == !invert {
            return i;
        }
    }
    limit
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> HttpUrl {
        HttpUrl::parse(s).expect("url")
    }

    #[test]
    fn simple_session_cookie() {
        let c = parse(&url("http://example.com/"), "sid=abc; Path=/").expect("采纳");
        assert_eq!((c.name.as_str(), c.value.as_str(), c.persistent), ("sid", "abc", false));
    }

    #[test]
    fn expires_makes_persistent() {
        let c = parse(&url("http://example.com/"), "a=1; expires=Wed, 21 Oct 2065 07:28:00 GMT")
            .expect("采纳");
        assert!(c.persistent);
    }

    #[test]
    fn invalid_calendar_date_not_persistent() {
        let c =
            parse(&url("http://example.com/"), "a=1; expires=31 Feb 2065 07:28:00").expect("采纳");
        assert!(!c.persistent);
    }

    #[test]
    fn foreign_domain_dropped() {
        assert!(parse(&url("http://example.com/"), "a=1; domain=other.com").is_none());
    }

    #[test]
    fn public_suffix_domain_dropped() {
        assert!(parse(&url("http://foo.example.com/"), "a=1; domain=com").is_none());
    }

    #[test]
    fn parent_domain_ok() {
        assert!(parse(&url("http://foo.example.com/"), "a=1; domain=example.com").is_some());
    }
}
