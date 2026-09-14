//! jsoup 1.16.2 outerHtml/html 序列化(默认 OutputSettings:prettyPrint=true,
//! indentAmount=1,maxPaddingWidth=30,escapeMode=base,charset=UTF-8,syntax=html)。
//! 对照源:nodes/{Element,TextNode,Comment,DataNode,DocumentType}.java、Entities.escape。

use crate::dom::*;
use crate::tags::BOOLEAN_ATTRIBUTES;
use scraper::Node;

/// 默认 OutputSettings 全仓只有这一种取值(prettyPrint=true,outline=false),
/// 结构体收成两个常量;outline/非 prettyPrint 的分支随之删去
const INDENT_AMOUNT: usize = 1;
const MAX_PADDING_WIDTH: usize = 30;

/// Element.html():子节点序列化;prettyPrint 时 trim
pub fn inner_html(el: NRef<'_>) -> String {
    let mut accum = String::new();
    for child in el.children() {
        serialize(child, 0, &mut accum);
    }
    accum.trim_matches(|c: char| c <= ' ').to_string()
}

/// Node.outerHtml()
pub fn outer_html(node: NRef<'_>) -> String {
    let mut accum = String::new();
    // jsoup Document.outerHtml == html():子节点序列化 + trim
    if matches!(node.value(), Node::Document) {
        return inner_html(node);
    }
    serialize(node, 0, &mut accum);
    accum
}

fn serialize(node: NRef<'_>, depth: usize, accum: &mut String) {
    outer_html_head(node, depth, accum);
    for child in node.children() {
        serialize(child, depth + 1, accum);
    }
    outer_html_tail(node, depth, accum);
}

/// jsoup Node.indent(pub:xpath-compat 序列化合成元素时同款缩进)
pub fn indent(accum: &mut String, depth: usize) {
    accum.push('\n');
    accum.push_str(&" ".repeat((depth * INDENT_AMOUNT).min(MAX_PADDING_WIDTH)));
}

// ---------- Element ----------

fn is_format_as_block(el: NRef<'_>) -> bool {
    flags(el).is_block || parent_element(el).is_some_and(|p| flags(p).format_as_block)
}

fn is_inlineable(el: NRef<'_>) -> bool {
    let f = flags(el);
    if f.is_block {
        return false; // tag.isInline() == !isBlock
    }
    (parent_element(el).is_none_or(|p| is_block(p)))
        && !is_effectively_first(el)
        && normal_name(el) != "br"
}

pub(crate) fn should_indent(el: NRef<'_>) -> bool {
    is_format_as_block(el) && !is_inlineable(el) && !preserve_whitespace(el.parent())
}

fn outer_html_head(node: NRef<'_>, depth: usize, accum: &mut String) {
    match node.value() {
        Node::Element(e) => {
            if should_indent(node) && !accum.is_empty() {
                indent(accum, depth);
            }
            accum.push('<');
            accum.push_str(tag_name(node));
            for (k, v) in e.attrs() {
                accum.push(' ');
                attribute_html(k, v, accum);
            }
            // jsoup 在这里按 selfClosing/xml 语法分支;本移植只做 html 语法
            // (html5ever 不保留未知标签的 /> 写法),空元素与普通元素同样输出 '>'
            accum.push('>');
        }
        Node::Text(_) => text_outer_html_head(node, depth, accum),
        Node::Comment(c) => {
            if is_effectively_first(node)
                && node.parent().is_some_and(|p| p.value().is_element() && flags(p).format_as_block)
            {
                indent(accum, depth);
            }
            accum.push_str("<!--");
            accum.push_str(&c.comment);
            accum.push_str("-->");
        }
        Node::Doctype(d) => {
            // DocumentType.outerHtmlHead(html 语法,无 public/system id 时小写)
            let name = d.name();
            let public_id = d.public_id();
            let system_id = d.system_id();
            if public_id.is_empty() && system_id.is_empty() {
                accum.push_str("<!doctype");
            } else {
                accum.push_str("<!DOCTYPE");
            }
            if !name.is_empty() {
                accum.push(' ');
                accum.push_str(name);
            }
            if !public_id.is_empty() {
                accum.push_str(" PUBLIC \"");
                accum.push_str(public_id);
                accum.push('"');
            }
            if !system_id.is_empty() {
                accum.push_str(" \"");
                accum.push_str(system_id);
                accum.push('"');
            }
            accum.push('>');
        }
        _ => {}
    }
}

fn outer_html_tail(node: NRef<'_>, depth: usize, accum: &mut String) {
    if let Node::Element(_) = node.value() {
        let f = flags(node);
        let no_children = node.children().next().is_none();
        if no_children && f.empty {
            return; // 空元素无闭合
        }
        if node.children().next().is_some()
            && f.format_as_block
            && !preserve_whitespace(node.parent())
        {
            indent(accum, depth);
        }
        accum.push_str("</");
        accum.push_str(tag_name(node));
        accum.push('>');
    }
}

// ---------- TextNode ----------

fn text_outer_html_head(node: NRef<'_>, depth: usize, accum: &mut String) {
    // DataNode(script/style 内容):原样输出,不转义不美化
    if parent_is_data_tag(node) {
        accum.push_str(text_content(node));
        return;
    }
    let parent = node.parent().filter(|p| p.value().is_element());
    let normalise_white = !preserve_whitespace(node.parent());
    let trim_like_block = parent.is_some_and(|p| flags(p).is_block || flags(p).format_as_block);
    let mut trim_leading = false;
    let mut trim_trailing = false;
    let sib_index = sibling_index(node);

    if normalise_white {
        trim_leading = (trim_like_block && sib_index == 0)
            || node.parent().is_some_and(|p| matches!(p.value(), Node::Document));
        trim_trailing = trim_like_block && node.next_sibling().is_none();

        // 若此文本全空白,且下一节点将换行缩进,则跳过
        let next = node.next_sibling();
        let prev = node.prev_sibling();
        let blank = is_blank_text(node);
        let could_skip = next.is_some_and(|n| n.value().is_element() && should_indent(n))
            || next.is_some_and(|n| is_text(n) && is_blank_text(n))
            || prev
                .is_some_and(|p| p.value().is_element() && (is_block(p) || normal_name(p) == "br"));
        if could_skip && blank {
            return;
        }

        if (sib_index == 0 && parent.is_some_and(|p| flags(p).format_as_block) && !blank)
            || (sib_index > 0 && prev.is_some_and(|p| normal_name(p) == "br"))
        {
            indent(accum, depth);
        }
    }

    escape_html(accum, text_content(node), false, normalise_white, trim_leading, trim_trailing);
}

// ---------- 属性与实体 ----------

fn attribute_html(key: &str, val: &str, accum: &mut String) {
    // html 语法:布尔属性且值为空/等于键名 → 折叠为裸键
    accum.push_str(key);
    let collapse = (val.is_empty() || val.eq_ignore_ascii_case(key))
        && BOOLEAN_ATTRIBUTES.binary_search(&key.to_ascii_lowercase().as_str()).is_ok();
    if !collapse {
        accum.push_str("=\"");
        escape_html(accum, val, true, false, false, false);
        accum.push('"');
    }
}

/// Entities.escape(base 模式,UTF-8:除必需实体外原样输出;
/// <0x20 的控制字符转 &#x..;)
pub fn escape_html(
    accum: &mut String,
    s: &str,
    in_attribute: bool,
    normalise_white: bool,
    strip_leading_white: bool,
    trim_trailing: bool,
) {
    let mut last_was_white = false;
    let mut reached_non_white = false;
    let mut skipped = false;
    for c in s.chars() {
        if normalise_white {
            if is_jsoup_whitespace(c) {
                if strip_leading_white && !reached_non_white {
                    continue;
                }
                if last_was_white {
                    continue;
                }
                if trim_trailing {
                    skipped = true;
                    continue;
                }
                accum.push(' ');
                last_was_white = true;
                continue;
            } else {
                last_was_white = false;
                reached_non_white = true;
                if skipped {
                    accum.push(' ');
                    skipped = false;
                }
            }
        }
        match c {
            '&' => accum.push_str("&amp;"),
            '\u{A0}' => accum.push_str("&nbsp;"),
            '<' => {
                if !in_attribute {
                    accum.push_str("&lt;");
                } else {
                    accum.push(c);
                }
            }
            '>' => {
                if !in_attribute {
                    accum.push_str("&gt;");
                } else {
                    accum.push(c);
                }
            }
            '"' => {
                if in_attribute {
                    accum.push_str("&quot;");
                } else {
                    accum.push(c);
                }
            }
            '\t' | '\n' | '\r' => accum.push(c),
            c if (c as u32) < 0x20 => {
                accum.push_str(&format!("&#x{:x};", c as u32));
            }
            c => accum.push(c),
        }
    }
}
