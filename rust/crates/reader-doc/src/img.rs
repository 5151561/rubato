//! 正文里的 `<img ...>` 标签扫描。
//!
//! # 为什么是手写扫描,不是正则
//!
//! [`crate`] 的依赖表是**空的**(计划书 §4.1:文档模型不依赖网络、数据库、
//! FFI 与 Flutter)。为认一个形状固定的标签把 `regex-compat`(带 java 方言
//! 引擎、公共后缀表那一串)拉进来,代价与收益完全不成比例。
//!
//! # 认哪些标签
//!
//! 口径对齐裁判的 `AppPattern.imgPattern`
//! (`judge/engine/.../constant/AppPattern.kt:14`):
//!
//! ```text
//! <img[^>]*src="([^"]*(?:"[^>]+\})?)"[^>]*>
//! ```
//!
//! 这不是「排版挂裁判」(计划书 §3 说了排版不挂),而是**同一个数据口径问题**:
//! 正文里哪一串字符算图片,决定了 `durChapterPos` 怎么分块 —— 与 §3.1 的
//! 「正文文本约定」同类。而且 Rubato 自己的正文就是
//! `html_format::format_keep_img` 产出的,它规范化之后恒为 `<img src="URL">`
//! (`rust/crates/html-format/src/lib.rs`),两边本来就是一件事的两头。
//!
//! 组 1 里那个 `(?:"[^>]+\})?` 是给 `src="URL,{"width":"50%"}"` 这种
//! **带 option JSON** 的 src 留的(`AnalyzeUrl` 的 `,{…}` 切分)——
//! 这一层只把它**原样**圈出来,option 怎么解是取图那一侧的事。

/// 一个 `<img ...>` 标签在正文里的**字节**区间。
///
/// 四个下标全是 ASCII 字符的位置(`<` `>` `"`),所以都是 UTF-8 字符边界,
/// 拿去切片安全。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ImgTag {
    /// `<` 的位置
    pub start: usize,
    /// `>` 之后一位
    pub end: usize,
    /// 组 1 的起点(`src="` 之后)
    pub src_start: usize,
    /// 组 1 的终点(收尾引号的位置)
    pub src_end: usize,
}

/// 扫出正文里所有图片标签,按位置升序、互不重叠。
///
/// 认不出来的 `<img`(没有 `src="`、或标签没闭合)**原样留在正文里**当文本 ——
/// 与 M1 的行为一致,不为「长得像图片但不是」额外发明一种块。
pub(crate) fn scan(text: &str) -> Vec<ImgTag> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(rel) = find(&b[i..], b"<img") {
        let start = i + rel;
        match parse_at(b, start) {
            Some(tag) => {
                i = tag.end;
                out.push(tag);
            }
            // 这一个不成立,从 `<img` 之后接着找:嵌套的 `<img<img src="a">` 也能认出后一个
            None => i = start + 4,
        }
    }
    out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// 标签到此为止的字符。`>` 是正则里的 `[^>]`;**换行也算**,理由是裁判那边
/// 是**按行**跑 imgPattern 的(`TextChapterLayout.kt:415`,`contents` 已按 `\n` 切),
/// 一个标签跨不了行。切块那一侧也指着这条:片不会横跨行边界。
fn stops(x: u8) -> bool {
    x == b'>' || x == b'\n'
}

/// 从 `<img` 起按上面那条正则往下认。任一步不成立就返回 `None`。
fn parse_at(b: &[u8], start: usize) -> Option<ImgTag> {
    // `[^>]*src="`:第一个 `>` 之前的第一处 `src="`。
    // (真身的 `[^>]*` 是贪婪+回溯,取的是**最后**一处;只有
    // `<img src="a" src="b">` 这种畸形标签才分得出差别,不为它多写一段回溯。)
    let mut p = start + 4;
    let src_start = loop {
        if p >= b.len() || stops(b[p]) {
            return None;
        }
        if b[p..].starts_with(br#"src=""#) {
            break p + 5;
        }
        p += 1;
    };

    // 组 1 = `[^"]*(?:"[^>]+\})?`,收尾是一个 `"`
    let mut q = src_start;
    while q < b.len() && b[q] != b'"' && b[q] != b'\n' {
        q += 1;
    }
    if q >= b.len() || b[q] != b'"' {
        return None;
    }
    let close = option_tail(b, q).unwrap_or(q);

    // `[^>]*>`
    let mut e = close + 1;
    while e < b.len() && !stops(b[e]) {
        e += 1;
    }
    if e >= b.len() || b[e] != b'>' {
        return None;
    }
    Some(ImgTag { start, end: e + 1, src_start, src_end: close })
}

/// `src="URL,{"k":"v"}"` 里那个可选的 `"[^>]+\}`:成立时返回**真正的**收尾引号位置。
///
/// `[^>]+` 贪婪、回溯到「以 `}` 结尾且后面就是 `"`」—— 所以从第一个 `>`
/// (或正文末尾)往回找最近的一对 `}"`。
fn option_tail(b: &[u8], quote: usize) -> Option<usize> {
    let mut stop = quote + 1;
    while stop < b.len() && !stops(b[stop]) {
        stop += 1;
    }
    // `"` + `[^>]+`(至少一位) + `}` + `"` => 收尾引号至少在 quote+3
    let mut r = stop.min(b.len().saturating_sub(1));
    while r >= quote + 3 {
        if b[r] == b'"' && b[r - 1] == b'}' {
            return Some(r);
        }
        r -= 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn srcs(text: &str) -> Vec<&str> {
        scan(text).iter().map(|t| &text[t.src_start..t.src_end]).collect()
    }

    fn tags(text: &str) -> Vec<&str> {
        scan(text).iter().map(|t| &text[t.start..t.end]).collect()
    }

    #[test]
    fn 认出规范化之后的那一种() {
        // format_keep_img 的产物恒为这个形状
        let text = "　　前文<img src=\"http://a/1.jpg\">后文";
        assert_eq!(srcs(text), vec!["http://a/1.jpg"]);
        assert_eq!(tags(text), vec!["<img src=\"http://a/1.jpg\">"]);
    }

    #[test]
    fn 一行里的多张图各自成对() {
        let text = "<img src=\"a\"><img src=\"b\">";
        assert_eq!(srcs(text), vec!["a", "b"]);
        let t = scan(text);
        assert_eq!(t[0].end, t[1].start, "两个标签首尾相接");
    }

    #[test]
    fn 带_option_的_src_原样圈出() {
        let text = "<img src=\"http://a/1.jpg,{\"width\":\"50%\"}\">";
        assert_eq!(srcs(text), vec!["http://a/1.jpg,{\"width\":\"50%\"}"]);
        assert_eq!(tags(text), vec![text]);
    }

    #[test]
    fn option_里的花括号嵌套也吃得下() {
        let text = "<img src=\"u,{\"headers\":{\"Referer\":\"x\"}}\">尾";
        assert_eq!(srcs(text), vec!["u,{\"headers\":{\"Referer\":\"x\"}}"]);
    }

    #[test]
    fn 没有_src_的不算图片() {
        assert!(scan("<img>").is_empty());
        assert!(scan("<img alt=\"x\">").is_empty());
        // src 用单引号:规范化之后不会出现,真身那条正则也不认
        assert!(scan("<img src='a'>").is_empty());
    }

    #[test]
    fn 没闭合的标签不算图片() {
        assert!(scan("<img src=\"a\"").is_empty());
        assert!(scan("<img src=\"a").is_empty());
    }

    #[test]
    fn src_不能跨过尖括号() {
        // `[^>]*src="` 里的 `[^>]*` 挡住了后一个标签的 src
        assert_eq!(srcs("<img alt><img src=\"a\">"), vec!["a"]);
    }

    #[test]
    fn 前面还有别的属性也认() {
        let text = "<img class=\"c\" src=\"a\" width=\"3\">";
        assert_eq!(srcs(text), vec!["a"]);
        assert_eq!(tags(text), vec![text]);
    }

    #[test]
    fn 空_src_是一张空图不是没有图() {
        // 真身的组 1 是 `[^"]*`,空串合法 —— 交给取图那一侧去失败,
        // 不在这里静默当成正文
        assert_eq!(srcs("<img src=\"\">"), vec![""]);
    }

    #[test]
    fn 标签跨不了行() {
        // 裁判是按行跑 imgPattern 的,一个标签跨不了行;切块那一侧也指着这条
        assert!(scan("<img\nsrc=\"a\">").is_empty());
        assert!(scan("<img src=\"a\nb\">").is_empty());
        assert_eq!(srcs("<img src=\"a\"\n<img src=\"b\">"), vec!["b"]);
    }

    #[test]
    fn 下标落在字符边界上() {
        let text = "甲乙<img src=\"图.jpg\">丙😀";
        let t = scan(text);
        assert_eq!(t.len(), 1);
        // 能切片就说明是字符边界(否则 panic)
        assert_eq!(&text[..t[0].start], "甲乙");
        assert_eq!(&text[t[0].end..], "丙😀");
        assert_eq!(&text[t[0].src_start..t[0].src_end], "图.jpg");
    }
}
