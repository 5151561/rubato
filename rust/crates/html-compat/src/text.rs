//! jsoup Element.text()/ownText()/wholeText()/data() 的逐行移植。

use crate::dom::*;

/// Element.text():规范化空白;DataNode(script/style)不参与;
/// 块级/br 边界补空格
pub fn text(el: NRef<'_>) -> String {
    let mut accum = String::new();
    fn walk(n: NRef<'_>, accum: &mut String) {
        head(n, accum);
        for c in n.children() {
            walk(c, accum);
        }
        tail(n, accum);
    }
    walk(el, &mut accum);
    accum.trim_matches(|c: char| c <= ' ').to_string() // Java String.trim 语义
}

fn head(node: NRef<'_>, accum: &mut String) {
    if is_text_node(node) {
        append_normalised_text(accum, node);
    } else if is_element_like(node) {
        let f = flags(node);
        if !accum.is_empty()
            && (f.is_block || normal_name(node) == "br")
            && !last_char_is_whitespace(accum)
        {
            accum.push(' ');
        }
    }
}

fn tail(node: NRef<'_>, accum: &mut String) {
    // 块级标签与紧随其后的文本/行内元素之间保证有空格:<div>One</div>Two → "One Two"
    if is_element_like(node) && is_block(node) {
        if let Some(next) = node.next_sibling() {
            let next_ok =
                is_text(next) || (next.value().is_element() && !flags(next).format_as_block);
            if next_ok && !last_char_is_whitespace(accum) {
                accum.push(' ');
            }
        }
    }
}

/// Element.appendNormalisedText
fn append_normalised_text(accum: &mut String, text_node: NRef<'_>) {
    let t = text_content(text_node);
    if preserve_whitespace(text_node.parent()) {
        accum.push_str(t);
    } else {
        let strip = last_char_is_whitespace(accum);
        append_normalised_whitespace(accum, t, strip);
    }
}

/// Element.ownText()
pub fn own_text(el: NRef<'_>) -> String {
    let mut accum = String::new();
    for child in el.children() {
        if is_text_node(child) {
            append_normalised_text(&mut accum, child);
        } else if normal_name(child) == "br" && !last_char_is_whitespace(&accum) {
            accum.push(' ');
        }
    }
    accum.trim_matches(|c: char| c <= ' ').to_string() // Java String.trim 语义
}

/// Element.wholeText():原始文本(TextNode 与 CDATA;DataNode 不含)
pub fn whole_text(el: NRef<'_>) -> String {
    let mut accum = String::new();
    fn walk(n: NRef<'_>, accum: &mut String) {
        if is_text_node(n) {
            accum.push_str(text_content(n));
        } else if normal_name(n) == "br" {
            accum.push('\n'); // jsoup wholeText:<br> 计为换行
        }
        for c in n.children() {
            walk(c, accum);
        }
    }
    walk(el, &mut accum);
    accum
}

/// Element.data():script/style 等 DataNode 内容(+注释外壳内数据不含)
pub fn data(el: NRef<'_>) -> String {
    let mut accum = String::new();
    fn walk(n: NRef<'_>, accum: &mut String) {
        if is_text(n) && parent_is_data_tag(n) {
            accum.push_str(text_content(n));
        }
        for c in n.children() {
            walk(c, accum);
        }
    }
    walk(el, &mut accum);
    accum
}

/// Element.textNodes() 的文本值(仅直接子 TextNode)
pub fn text_nodes(el: NRef<'_>) -> Vec<String> {
    el.children().filter(|c| is_text_node(*c)).map(|c| text_content(c).to_string()).collect()
}
