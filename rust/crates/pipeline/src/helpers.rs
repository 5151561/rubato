//! BookHelp / StringUtils / BookChapter 扩展的移植(judge 对应物见各注释)。

use regex_compat::JavaRegex;
use rubato_core::entities::BookChapter;
use rubato_core::net_utils::get_absolute_url_str;
use std::sync::LazyLock;

// AppPattern.nameRegex / authorRegex(Kotlin Regex = java.util.regex)
static NAME_REGEX: LazyLock<JavaRegex> =
    LazyLock::new(|| JavaRegex::compile("\\s+作\\s*者.*|\\s+\\S+\\s+著").expect("nameRegex"));
static AUTHOR_REGEX: LazyLock<JavaRegex> =
    LazyLock::new(|| JavaRegex::compile("^\\s*作\\s*者[:：\\s]+|\\s+著").expect("authorRegex"));
static RN_REGEX: LazyLock<JavaRegex> =
    LazyLock::new(|| JavaRegex::compile("[\\r\\n]").expect("rnRegex"));

/// Kotlin `trim { it <= ' ' }`
fn trim_java(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

/// `BookHelp.formatBookName`
pub fn format_book_name(name: &str) -> String {
    let replaced = NAME_REGEX.replace_all(name, "").unwrap_or_else(|_| name.to_string());
    trim_java(&replaced).to_string()
}

/// `BookHelp.formatBookAuthor`
pub fn format_book_author(author: &str) -> String {
    let replaced = AUTHOR_REGEX.replace_all(author, "").unwrap_or_else(|_| author.to_string());
    trim_java(&replaced).to_string()
}

/// `StringUtils.wordCountFormat(String?)`;Err = Kotlin `toInt()` 溢出抛
/// NumberFormatException(调用方都在 try/catch 里吞掉,错误无 payload 是有意的)
#[allow(clippy::result_unit_err)]
pub fn word_count_format(wc: Option<&str>) -> Result<String, ()> {
    let Some(wc) = wc else { return Ok(String::new()) };
    static NUMERIC: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new("^-?[0-9]+$").expect("numeric"));
    if NUMERIC.is_match(wc) {
        let words = wc.parse::<i32>().map_err(|_| ())?;
        if words > 0 {
            if words > 10000 {
                // DecimalFormat("#.#"):HALF_EVEN 保 1 位小数,去尾零
                let v = (words as f32) as f64 / 10000f64;
                return Ok(format!("{}万字", decimal_format_1(v)));
            }
            return Ok(format!("{words}字"));
        }
        Ok(String::new())
    } else {
        Ok(wc.to_string())
    }
}

/// `DecimalFormat("#.#").format(v)`:HALF_EVEN 到 1 位小数,整数不带小数点
fn decimal_format_1(v: f64) -> String {
    let scaled = v * 10.0;
    let floor = scaled.floor();
    let frac = scaled - floor;
    let rounded = if (frac - 0.5).abs() < 1e-9 {
        // half-even
        if (floor as i64) % 2 == 0 { floor } else { floor + 1.0 }
    } else {
        scaled.round()
    };
    let r = rounded / 10.0;
    if (r - r.trunc()).abs() < 1e-12 { format!("{}", r.trunc() as i64) } else { format!("{r:.1}") }
}

/// `String?.isTrue(nullIsTrue = false)`
pub fn is_true(s: Option<&str>) -> bool {
    let Some(s) = s else { return false };
    if s.chars().all(char::is_whitespace) || s == "null" {
        return false;
    }
    static FALSY: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new("(?i)^(?:false|no|not|0|0.0)$").expect("falsy"));
    !FALSY.is_match(rubato_core::net_utils::kotlin_trim(s).as_str())
}

/// harness 侧 HarnessChapterRuntime.displayTitle:去 \r\n(净化/简繁被砍)
pub fn chapter_display_title(ch: &BookChapter) -> String {
    RN_REGEX.replace_all(&ch.title, "").unwrap_or_else(|_| ch.title.clone())
}

/// harness 侧 HarnessChapterRuntime.absoluteUrl:二级目录「url,option」拼接
pub fn chapter_absolute_url(ch: &BookChapter) -> String {
    let pos = ch.url.find(',');
    let url_before = match pos {
        Some(p) => &ch.url[..p],
        None => &ch.url[..],
    };
    let abs = get_absolute_url_str(Some(&ch.base_url), url_before);
    match pos {
        Some(p) => format!("{abs}{}", &ch.url[p..]),
        None => abs,
    }
}
