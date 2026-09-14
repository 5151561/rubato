//! `EncodingDetect.getHtmlEncode` 的移植(judge/engine 的 utils/EncodingDetect.kt)。
//! meta 判定用 html-compat(即 jsoup 兼容层);meta 判不出时兜底走
//! [`crate::charset_detector`](icu4j CharsetDetector 的移植,charset 套钉住)。

const HEAD_OPEN: &[u8] = b"<head>";
const HEAD_CLOSE: &[u8] = b"</head>";

fn index_of(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

/// Kotlin `String(bytes)`:UTF-8 + U+FFFD 替换
fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub fn get_html_encode(bytes: &[u8]) -> String {
    if let Some(name) = meta_charset(bytes) {
        return name;
    }
    get_encode(bytes)
}

fn meta_charset(bytes: &[u8]) -> Option<String> {
    // 字节窗口找 <head>…</head>(大小写敏感,同真身)
    let head: Option<String> = index_of(bytes, HEAD_OPEN, 0).and_then(|start| {
        index_of(bytes, HEAD_CLOSE, start).map(|end| lossy(&bytes[start..end + HEAD_CLOSE.len()]))
    });
    // 兜底:整串正则 (?i)<head>[\s\S]*?</head>;找不到 → NPE → catch → getEncode
    let head = match head {
        Some(h) => h,
        None => find_head_regex(&lossy(bytes))?,
    };
    let doc = html_compat::parse(&head);
    let charsets = html_compat::select_extract(&doc, "meta", "attr:charset").ok()?;
    let http_equivs = html_compat::select_extract(&doc, "meta", "attr:http-equiv").ok()?;
    let contents = html_compat::select_extract(&doc, "meta", "attr:content").ok()?;
    for (i, charset_str) in charsets.iter().enumerate() {
        if !charset_str.is_empty() {
            return Some(charset_str.clone());
        }
        let http_equiv = http_equivs.get(i).map(String::as_str).unwrap_or("");
        if http_equiv.eq_ignore_ascii_case("content-type") {
            let content = contents.get(i).map(String::as_str).unwrap_or("");
            let idx = content.to_ascii_lowercase().find("charset=");
            let charset_str = match idx {
                Some(i) => &content[i + "charset=".len()..],
                // substringAfter(";"):无 ';' 时返回整串
                None => content.split_once(';').map(|(_, r)| r).unwrap_or(content),
            };
            if !charset_str.is_empty() {
                return Some(charset_str.to_string());
            }
        }
    }
    None
}

/// `(?i)<head>[\s\S]*?</head>` 的 find(手写,免拉 regex 到该路径的 Unicode 语义差)
fn find_head_regex(s: &str) -> Option<String> {
    let low = s.to_ascii_lowercase();
    // ASCII 小写化不改变字节长度,下标可共用
    let start = low.find("<head>")?;
    let end = low[start..].find("</head>").map(|i| i + start)?;
    Some(s[start..end + "</head>".len()].to_string())
}

/// `EncodingDetect.getEncode`:icu4j CharsetDetector(charset_detector.rs),
/// 无匹配 → "UTF-8"
pub fn get_encode(bytes: &[u8]) -> String {
    crate::charset_detector::detect(bytes).unwrap_or("UTF-8").to_string()
}
