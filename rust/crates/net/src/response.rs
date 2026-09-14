//! `ResponseBody.text()`(OkHttpUtils.kt)与 MediaType charset 判定的移植。

use crate::encoding_detect::get_html_encode;
use encoding_rs::Encoding;
use regex::Regex;
use std::sync::LazyLock;

const TOKEN: &str = "[a-zA-Z0-9\\-!#$%&'*+.^_`{|}~]+";

static TYPE_SUBTYPE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("^({TOKEN})/({TOKEN})")).expect("type/subtype"));
static PARAMETER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("^;\\s*(?:({TOKEN})=(?:({TOKEN})|\"([^\"]*)\"))?")).expect("parameter")
});

/// okhttp `MediaType`:只保留差分需要的面(参数表 + 原样串)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaType {
    pub media_type: String,
    pub type_: String,
    pub subtype: String,
    /// (name, value) 平铺,name 查询大小写不敏感
    pub parameters: Vec<(String, String)>,
}

impl MediaType {
    /// `String.toMediaTypeOrNull()`
    pub fn parse(s: &str) -> Option<MediaType> {
        let ts = TYPE_SUBTYPE.captures(s)?;
        let type_ = ts[1].to_ascii_lowercase();
        let subtype = ts[2].to_ascii_lowercase();
        let mut parameters = Vec::new();
        let mut pos = ts.get(0).expect("整段").end();
        while pos < s.len() {
            let m = PARAMETER.captures(&s[pos..])?;
            let whole = m.get(0).expect("整段");
            if whole.end() == 0 {
                // 空匹配意味着语法不合(避免死循环;okhttp require(parameter != null))
                return None;
            }
            if let Some(name) = m.get(1) {
                let value = match (m.get(2), m.get(3)) {
                    (Some(token), _) => {
                        let t = token.as_str();
                        if t.starts_with('\'') && t.ends_with('\'') && t.len() > 2 {
                            t[1..t.len() - 1].to_string()
                        } else {
                            t.to_string()
                        }
                    }
                    (None, Some(quoted)) => quoted.as_str().to_string(),
                    (None, None) => unreachable!("regex 保证"),
                };
                parameters.push((name.as_str().to_string(), value));
            }
            pos += whole.end();
        }
        Some(MediaType { media_type: s.to_string(), type_, subtype, parameters })
    }

    pub fn parameter(&self, name: &str) -> Option<&str> {
        self.parameters.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// `MediaType.charset()`:不识别的 charset → None(与 Java Charset 集合的
    /// 出入按 encoding_rs 标签,差异记豁免)
    pub fn charset(&self) -> Option<&'static Encoding> {
        Encoding::for_label(self.parameter("charset")?.as_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextError {
    /// EncodingDetect 给出的 charset 名 Charset.forName 不识别
    /// (真身此处抛 UnsupportedCharsetException)
    UnsupportedCharset(String),
}

/// `Utf8BomUtils.removeUTF8BOM`(注意真身是 `size > 3` 严格大于)
pub fn remove_utf8_bom(bytes: &[u8]) -> &[u8] {
    if bytes.len() > 3 && bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { &bytes[3..] } else { bytes }
}

/// Java `String(bytes, charset)`:恶字节 → U+FFFD
fn decode(bytes: &[u8], enc: &'static Encoding) -> String {
    enc.decode_without_bom_handling(bytes).0.into_owned()
}

/// `ResponseBody.text(encode = null)`:BOM → content-type charset → EncodingDetect
pub fn response_text(body: &[u8], content_type: Option<&MediaType>) -> Result<String, TextError> {
    let response_bytes = remove_utf8_bom(body);
    if let Some(charset) = content_type.and_then(MediaType::charset) {
        return Ok(decode(response_bytes, charset));
    }
    let charset_name = get_html_encode(response_bytes);
    match Encoding::for_label(charset_name.as_bytes()) {
        Some(enc) => Ok(decode(response_bytes, enc)),
        None => Err(TextError::UnsupportedCharset(charset_name)),
    }
}

/// `AppPattern.xmlContentTypeRegex`:`(application|text)/\w*\+?xml.*` 完全匹配
pub fn is_xml_content_type(media_type_str: &str) -> bool {
    static XML: LazyLock<Regex> = LazyLock::new(|| {
        // Java \w 是 ASCII;Java `.` 不匹配行终止符
        Regex::new(r"^(application|text)/[0-9A-Za-z_]*\+?xml[^\n\r\u{85}\u{2028}\u{2029}]*$")
            .expect("xml ct")
    });
    XML.is_match(media_type_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charset_from_content_type() {
        let mt = MediaType::parse("text/html; charset=gbk").expect("解析");
        let text = response_text(&[0xD6, 0xD0], Some(&mt)).expect("解码");
        assert_eq!(text, "中");
    }

    #[test]
    fn meta_charset_fallback() {
        let html = "<html><head><meta charset=\"gbk\"></head><body>\u{0}</body></html>";
        // 用 gbk 编码字节喂进去
        let src = html.replace('\u{0}', "中");
        let (bytes, _, _) = encoding_rs::GBK.encode(&src);
        let mt = MediaType::parse("text/html").expect("解析");
        let text = response_text(&bytes, Some(&mt)).expect("解码");
        assert!(text.contains('中'));
    }

    #[test]
    fn quoted_charset_param() {
        let mt = MediaType::parse("text/html; charset=\"utf-8\"").expect("解析");
        assert!(mt.charset().is_some());
    }

    #[test]
    fn xml_content_type() {
        assert!(is_xml_content_type("application/xml"));
        assert!(is_xml_content_type("text/atom+xml; charset=utf-8"));
        assert!(!is_xml_content_type("text/html"));
    }
}
