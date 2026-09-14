//! scraper(html5ever)树上的 jsoup 视角辅助函数。
//!
//! 映射关系:
//! - jsoup TextNode ↔ scraper Text 且父元素不是 script/style;
//! - jsoup DataNode ↔ scraper Text 且父元素是 script/style;
//! - jsoup Document(#root 元素)↔ scraper 的 Document 根节点
//!   (选择器与文本提取把它当 normalName == "#root" 的伪元素)。

use crate::tags::{DATA_TAGS, TagFlags, tag_flags};
use ego_tree::NodeRef;
use scraper::Node;

pub type NRef<'a> = NodeRef<'a, Node>;

pub fn is_element_like(n: NRef<'_>) -> bool {
    matches!(n.value(), Node::Element(_) | Node::Document)
}

/// jsoup normalName:小写标签名;Document 是 "#root"
///
/// 回 `Cow` 而不是 `String`:html5ever 解析 HTML 时标签名已经是小写的
/// (只有 SVG/MathML 的调整名会带大写),所以**绝大多数调用一个字节都不用抄**。
/// 这个函数在选择器匹配与文本提取的最内层 —— `select` 对每个节点调一次、
/// 文本走查每个节点调一次 —— 原来每次都 `to_ascii_lowercase()` 分配一个 String,
/// 是 html 套那个「普遍略慢」的一半(见 docs/engine-perf.md §3③)。
pub fn normal_name(n: NRef<'_>) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    match n.value() {
        Node::Element(e) => {
            let name = e.name();
            if name.bytes().any(|b| b.is_ascii_uppercase()) {
                Cow::Owned(name.to_ascii_lowercase())
            } else {
                Cow::Borrowed(name)
            }
        }
        Node::Document => Cow::Borrowed("#root"),
        _ => Cow::Borrowed(""),
    }
}

/// 序列化用原始标签名(html5ever 已按 HTML 规则小写,SVG 保留大小写)
pub fn tag_name(n: NRef<'_>) -> &str {
    match n.value() {
        Node::Element(e) => e.name(),
        _ => "",
    }
}

pub fn flags(n: NRef<'_>) -> TagFlags {
    match n.value() {
        Node::Element(_) => tag_flags(&normal_name(n)),
        // Document(#root):jsoup 未注册标签 → 全 false
        _ => tag_flags("#root"),
    }
}

pub fn is_block(n: NRef<'_>) -> bool {
    flags(n).is_block
}

pub fn is_text(n: NRef<'_>) -> bool {
    matches!(n.value(), Node::Text(_))
}

/// 该文本节点是否 jsoup 意义上的 TextNode(而非 script/style 的 DataNode)
pub fn is_text_node(n: NRef<'_>) -> bool {
    if !is_text(n) {
        return false;
    }
    !parent_is_data_tag(n)
}

pub fn parent_is_data_tag(n: NRef<'_>) -> bool {
    n.parent().is_some_and(|p| {
        p.value()
            .as_element()
            .is_some_and(|e| DATA_TAGS.contains(&e.name().to_ascii_lowercase().as_str()))
    })
}

pub fn text_content(n: NRef<'_>) -> &str {
    match n.value() {
        Node::Text(t) => &t.text,
        _ => "",
    }
}

/// jsoup TextNode.isBlank:全部字符是 isWhitespace(不含 nbsp)
pub fn is_blank_text(n: NRef<'_>) -> bool {
    text_content(n).chars().all(is_jsoup_whitespace)
}

pub fn is_jsoup_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\x0C' | '\r')
}

pub fn is_actually_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\x0C' | '\r' | '\u{A0}')
}

pub fn is_invisible_char(c: char) -> bool {
    c == '\u{200B}' || c == '\u{AD}' // zero width sp, soft hyphen
}

/// StringUtil.appendNormalisedWhitespace
pub fn append_normalised_whitespace(accum: &mut String, s: &str, strip_leading: bool) {
    let mut last_was_white = false;
    let mut reached_non_white = false;
    for c in s.chars() {
        if is_actually_whitespace(c) {
            if (strip_leading && !reached_non_white) || last_was_white {
                continue;
            }
            accum.push(' ');
            last_was_white = true;
        } else if !is_invisible_char(c) {
            accum.push(c);
            last_was_white = false;
            reached_non_white = true;
        }
    }
}

pub fn last_char_is_whitespace(s: &str) -> bool {
    s.ends_with(' ')
}

/// 属性读取:名字大小写不敏感(jsoup getIgnoreCase 语义;html5ever 已小写
/// HTML 属性名,这里再做一层保险)
pub fn attr<'a>(n: NRef<'a>, key: &str) -> Option<&'a str> {
    let e = n.value().as_element()?;
    e.attrs().find_map(|(k, v)| if k.eq_ignore_ascii_case(key) { Some(v) } else { None })
}

pub fn has_attr(n: NRef<'_>, key: &str) -> bool {
    attr(n, key).is_some()
}

/// 元素在其父的元素子节点中的序号(jsoup elementSiblingIndex)
pub fn element_sibling_index(n: NRef<'_>) -> usize {
    let Some(parent) = n.parent() else { return 0 };
    parent.children().filter(|c| c.value().is_element()).position(|c| c.id() == n.id()).unwrap_or(0)
}

/// 所有节点(含文本)的兄弟序号(jsoup siblingIndex)
pub fn sibling_index(n: NRef<'_>) -> usize {
    let Some(parent) = n.parent() else { return 0 };
    parent.children().position(|c| c.id() == n.id()).unwrap_or(0)
}

pub fn parent_element(n: NRef<'_>) -> Option<NRef<'_>> {
    n.parent().filter(|p| is_element_like(*p))
}

pub fn first_element_child(n: NRef<'_>) -> Option<NRef<'_>> {
    n.children().find(|c| c.value().is_element())
}

pub fn last_element_child(n: NRef<'_>) -> Option<NRef<'_>> {
    n.children().filter(|c| c.value().is_element()).last()
}

pub fn next_element_sibling(n: NRef<'_>) -> Option<NRef<'_>> {
    let mut cur = n.next_sibling();
    while let Some(s) = cur {
        if s.value().is_element() {
            return Some(s);
        }
        cur = s.next_sibling();
    }
    None
}

pub fn prev_element_sibling(n: NRef<'_>) -> Option<NRef<'_>> {
    let mut cur = n.prev_sibling();
    while let Some(s) = cur {
        if s.value().is_element() {
            return Some(s);
        }
        cur = s.prev_sibling();
    }
    None
}

/// Node.isEffectivelyFirst
pub fn is_effectively_first(n: NRef<'_>) -> bool {
    let idx = sibling_index(n);
    if idx == 0 {
        return true;
    }
    if idx == 1 {
        if let Some(prev) = n.prev_sibling() {
            return is_text(prev) && is_blank_text(prev);
        }
    }
    false
}

/// Element.preserveWhitespace:自身与上溯至多 5 层
pub fn preserve_whitespace(node: Option<NRef<'_>>) -> bool {
    let mut cur = node;
    let mut i = 0;
    while let Some(n) = cur {
        if n.value().is_element() {
            if flags(n).preserve_whitespace {
                return true;
            }
        } else {
            return false;
        }
        cur = n.parent();
        i += 1;
        if i >= 6 {
            break;
        }
    }
    false
}
