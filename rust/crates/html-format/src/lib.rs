//! `utils/HtmlFormatter.kt` 的移植(format / formatKeepImg;display/intro/summary
//! 是 UI 面,Phase 1 不进差分)。正则全部走 java 方言(regex-compat)。
//!
//! **为什么是独立 crate**:这一份从前长在 `pipeline` 里(只有正文/简介那条路用),
//! 而 `java.htmlFormat` 是书源 JS 直接调的宿主 API —— js-host 也要同一份口径,
//! 又不能反过来依赖 pipeline(pipeline 依赖 rule-engine,js-host 在它下面)。
//! 一件事只记一处,故下沉成两边共用的底座。

use net::param_split;
use regex_compat::JavaRegex;
use rubato_core::java_url::JavaUrl;
use rubato_core::net_utils::get_absolute_url;
use std::sync::LazyLock;

macro_rules! jre {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<JavaRegex> =
            LazyLock::new(|| JavaRegex::compile($pat).expect(stringify!($name)));
    };
}

jre!(NBSP, "(&nbsp;)+");
jre!(ESP, "(&ensp;|&emsp;)");
jre!(NO_PRINT, "(&thinsp;|&zwnj;|&zwj;|\u{2009}|\u{200C}|\u{200D})");
jre!(WRAP_HTML, "</?(?:div|p|br|hr|h\\d|article|dd|dl)[^>]*>");
jre!(COMMENT, "<!--[^>]*-->");
jre!(NOT_IMG_HTML, "</?(?!img)[a-zA-Z]+(?=[ >])[^<>]*>");
jre!(OTHER_HTML, "</?[a-zA-Z]+(?=[ >])[^<>]*>");
jre!(INDENT1, "\\s*\\n+\\s*");
jre!(INDENT2, "^[\\n\\s]+");
jre!(LAST, "[\\n\\s]+$");
// LegadoTeam 基准的 formatImagePattern 是四选一(旧基准三选一):
// 1) src 里带 `{...}` 模板的(后面还要切 `,{option}`)
// 2) data-src / data-original / data-srcset
// 3) src="..."(只认双引号)
// 4) 任意 data-* 或 src(单双引号皆可,允许空值)
static FORMAT_IMAGE: LazyLock<JavaRegex> = LazyLock::new(|| {
    JavaRegex::compile(
        "(?i)<img[^>]*\\ssrc\\s*=\\s*['\"]([^'\"{>]*\\{(?:[^{}]|\\{[^}>]+\\})+\\})['\"][^>]*>|<img[^>]*\\sdata-(?:src|original|srcset)\\s*=\\s*['\"]([^'\">]+)['\"][^>]*>|<img[^>]*\\ssrc\\s*=\\s*\"([^\">]+)\"[^>]*>|<img[^>]*\\s(?:data-[^=>]*|src)=\\s*['\"]([^'\">]*)['\"][^>]*>",
    )
    .expect("formatImagePattern")
});

const PARAGRAPH_INDENT: &str = "　　";

fn rep(re: &JavaRegex, s: &str, replacement: &str) -> String {
    re.replace_all(s, replacement).unwrap_or_else(|_| s.to_string())
}

/// `HtmlFormatter.format(html)`(otherRegex = otherHtmlRegex,段首缩进两个全角空格)
pub fn format(html: Option<&str>) -> String {
    format_with(html, &OTHER_HTML, PARAGRAPH_INDENT)
}

/// `HtmlFormatter.formatIntro(html)`:**段首不缩进**(paragraphIndent = "")。
/// 简介与正文走的是同一个 formatText,只有缩进串不同。
pub fn format_intro(html: Option<&str>) -> String {
    format_with(html, &OTHER_HTML, "")
}

fn format_with(html: Option<&str>, other: &JavaRegex, paragraph_indent: &str) -> String {
    let Some(html) = html else { return String::new() };
    let s = rep(&NBSP, html, " ");
    let s = rep(&ESP, &s, " ");
    let s = rep(&NO_PRINT, &s, "");
    let s = rep(&WRAP_HTML, &s, "\n");
    let s = rep(&COMMENT, &s, "");
    let s = rep(other, &s, "");
    let s = rep(&INDENT1, &s, &format!("\n{paragraph_indent}"));
    let s = rep(&INDENT2, &s, paragraph_indent);
    rep(&LAST, &s, "")
}

/// `HtmlFormatter.formatKeepImg(html, redirectUrl)`
pub fn format_keep_img(html: Option<&str>, redirect_url: Option<&JavaUrl>) -> String {
    let Some(html) = html else { return String::new() };
    let keep_img_html = format_with(Some(html), &NOT_IMG_HTML, PARAGRAPH_INDENT);

    let mut append_pos = 0usize;
    let mut sb = String::new();
    let matches = FORMAT_IMAGE.find_all_with_ranges(&keep_img_html).unwrap_or_default();
    for (m_start, m_end, groups) in matches {
        let mut param = String::new();
        let src = match groups.get(1).and_then(|g| g.clone()) {
            Some(g1) => {
                // AnalyzeUrl.paramPattern 切「,{option}」
                match param_split(&g1) {
                    Some((start, end)) => {
                        param = format!(",{}", &g1[end..]);
                        g1[..start].to_string()
                    }
                    None => g1,
                }
            }
            None => groups
                .get(2)
                .and_then(|g| g.clone())
                .or_else(|| groups.get(3).and_then(|g| g.clone()))
                .or_else(|| groups.get(4).and_then(|g| g.clone()))
                .unwrap_or_default(),
        };
        sb.push_str(&keep_img_html[append_pos..m_start]);
        sb.push_str(&format!("<img src=\"{}{}\">", get_absolute_url(redirect_url, &src), param));
        append_pos = m_end;
    }
    if append_pos < keep_img_html.len() {
        sb.push_str(&keep_img_html[append_pos..]);
    }
    sb
}
