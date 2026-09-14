//! jsoup 1.16.2 兼容层:html5ever(经 scraper)解析 + 自写的
//! jsoup 方言选择器(select)、text()/html() 序列化(serialize/text)。
//!
//! 行为口径:tools/html_diff.sh 与 jsoup 裁判逐 case 对齐。
//! 已知近似(解析层,序列化无从恢复):
//! - html5ever 不保留未知标签的自闭合语法(`<foo />`→`<foo>` 开标签);
//! - html5ever 分词器把无值属性(`<a data-x>`)与空值属性(`data-x=""`)
//!   都归一为空串,jsoup 序列化会区分两者(豁免清单有注释);
//! - **表格里的格式化元素(foster parenting)两边分歧**,见下。
//!
//! # `<tr>` 里的 `<a>`:jsoup 留在行内,html5ever 挪出表外
//!
//! 最小复现(M2j 用 jsoup 1.16.2 与本 crate 各跑一遍):
//!
//! ```text
//! <table><tbody><tr><a><a href="c1.html">X</a><span>Y</span></a></tr></tbody></table>
//!
//! jsoup:      <a></a><span>Y</span>            ← 外层 a 与 span 被 foster 出去
//!             <table><tbody><tr>
//!               <a href="c1.html">X</a>        ← **内层 a 留在 tr 里**
//!             </tr></tbody></table>
//!
//! html5ever:  <a></a><a href="c1.html">X</a><span>Y</span>   ← 三个都出去了
//!             <table><tbody><tr></tr></tbody></table>        ← tr 是空的
//! ```
//!
//! html5ever 是**按规范**做的(「in row」遇到非 td/th/tr 走 in-table 的
//! anything-else → 开 foster parenting → 按 in-body 插入 → 插到 table 前面)。
//! jsoup 是它自己的 bug:内层 `<a>` 命中「已有活动 a」那条,先
//! `tb.processEndTag("a")`,那次嵌套处理又走了一遍 InTable.anythingElse,
//! 收尾时把 `fosterInserts` **置回 false** —— 于是紧接着的 `tb.insert()`
//! 就插进了当前节点(`<tr>`)。
//!
//! 后果:`bookList: tbody tr` + `bookUrl: a.0@href` 这类书源在这种畸形表格上
//! 两侧取到的元素不同(pipeline-corpus-b 有 15 例,见那套 README)。要复刻
//! 得改 html5ever 的树构造,不是本层能收的;**先记在这里,别重新推一遍**。

pub mod dom;
pub mod dsl;
pub mod entities;
pub mod select;
pub mod serialize;
pub mod tags;
pub mod text;

use dom::NRef;
use html5ever::driver;
use scraper::{Html, HtmlTreeSink};
use tendril::TendrilSink;

pub use select::{Ev, SelectorParseError};

pub fn parse(html: &str) -> Html {
    // scripting_enabled=false:jsoup 无脚本,<noscript> 内容按标记解析
    let opts = driver::ParseOpts {
        tree_builder: html5ever::tree_builder::TreeBuilderOpts {
            scripting_enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    driver::parse_document(HtmlTreeSink::new(Html::new_document()), opts).one(html)
}

/// 文档级 select + 动作提取(差分与 DSL 的公共入口)。
/// action: text | ownText | wholeText | html | outerHtml | data | attr:<名>
pub fn select_extract(
    doc: &Html,
    selector: &str,
    action: &str,
) -> Result<Vec<String>, SelectorParseError> {
    let ev = select::parse(selector)?;
    let root = doc.tree.root();
    Ok(select::select(root, &ev).into_iter().map(|el| extract(el, action)).collect())
}

pub fn extract(el: NRef<'_>, action: &str) -> String {
    match action {
        "text" => text::text(el),
        "ownText" => text::own_text(el),
        "wholeText" => text::whole_text(el),
        "data" => text::data(el),
        "html" => serialize::inner_html(el),
        "outerHtml" => serialize::outer_html(el),
        attr => attr
            .strip_prefix("attr:")
            .and_then(|a| dom::attr(el, a))
            .unwrap_or_default()
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract1(html: &str, sel: &str, action: &str) -> Vec<String> {
        select_extract(&parse(html), sel, action).unwrap()
    }

    #[test]
    fn text_normalization() {
        let out = extract1("<div><p>  Hello   \n world </p></div>", "div", "text");
        assert_eq!(out, vec!["Hello world"]);
    }

    #[test]
    fn text_block_boundaries() {
        let out = extract1("<div><div>One</div>Two</div>", "body > div", "text");
        assert_eq!(out, vec!["One Two"]);
    }

    #[test]
    fn text_excludes_script() {
        let out = extract1("<div>a<script>var x=1;</script>b</div>", "div", "text");
        assert_eq!(out, vec!["a b"]); // script 是块级 → 边界空格
    }

    #[test]
    fn own_text_and_br() {
        let out = extract1("<p>Hello <b>bold</b> world<br>next</p>", "p", "ownText");
        assert_eq!(out, vec!["Hello world next"]);
    }

    #[test]
    fn nbsp_collapses() {
        let out = extract1("<p>a\u{a0}\u{a0}b</p>", "p", "text");
        assert_eq!(out, vec!["a b"]);
    }

    #[test]
    fn pre_preserves() {
        // pre 内部空白保留,但 text() 末尾的 Java trim 会削掉首尾
        let out = extract1("<pre>  a\n  b</pre>", "pre", "text");
        assert_eq!(out, vec!["a\n  b"]);
    }

    #[test]
    fn pseudo_selectors() {
        let html = "<ul><li>一</li><li>二</li><li>三</li></ul>";
        assert_eq!(extract1(html, "li:eq(1)", "text"), vec!["二"]);
        assert_eq!(extract1(html, "li:lt(2)", "text"), vec!["一", "二"]);
        assert_eq!(extract1(html, "li:gt(1)", "text"), vec!["三"]);
        assert_eq!(extract1(html, "li:contains(二)", "text"), vec!["二"]);
        assert_eq!(extract1(html, "li:matches(^[一三]$)", "text"), vec!["一", "三"]);
        assert_eq!(extract1(html, "li:not(:contains(二))", "text"), vec!["一", "三"]);
        assert_eq!(extract1(html, "ul:has(li)", "text").len(), 1);
        assert_eq!(extract1(html, "li:nth-child(2n+1)", "text"), vec!["一", "三"]);
        assert_eq!(extract1(html, "li:first-child", "text"), vec!["一"]);
        assert_eq!(extract1(html, "li:last-of-type", "text"), vec!["三"]);
    }

    #[test]
    fn attribute_selectors() {
        let html = r#"<a href="/a" data-id="X1">a</a><a class="Big">b</a>"#;
        assert_eq!(extract1(html, "a[href]", "text"), vec!["a"]);
        assert_eq!(extract1(html, "[data-id=x1]", "text"), vec!["a"]); // 值不区分大小写
        assert_eq!(extract1(html, "a[^data-]", "text"), vec!["a"]);
        assert_eq!(extract1(html, ".big", "text"), vec!["b"]); // 类名不区分大小写
        assert_eq!(extract1(html, "a[href^=/]", "text"), vec!["a"]);
        assert_eq!(extract1(html, "a[data-id~=\\d]", "text"), vec!["a"]);
    }

    #[test]
    fn combinators() {
        let html = "<div><p>1</p><span>2</span><p>3</p></div><p>4</p>";
        assert_eq!(extract1(html, "div > p", "text"), vec!["1", "3"]);
        assert_eq!(extract1(html, "span + p", "text"), vec!["3"]);
        assert_eq!(extract1(html, "p ~ p", "text"), vec!["3"]);
        assert_eq!(extract1(html, "div p, body > p", "text"), vec!["1", "3", "4"]);
    }

    #[test]
    fn pretty_print_html() {
        let html = "<div><p>One</p><p>Two <b>bold</b></p></div>";
        let out = extract1(html, "div", "html");
        assert_eq!(out, vec!["<p>One</p>\n<p>Two <b>bold</b></p>"]);
    }

    #[test]
    fn outer_html_nesting() {
        let html = "<div><ul><li>a</li><li>b</li></ul></div>";
        let out = extract1(html, "div", "outerHtml");
        assert_eq!(out, vec!["<div>\n <ul>\n  <li>a</li>\n  <li>b</li>\n </ul>\n</div>"]);
    }

    #[test]
    fn attr_extraction() {
        let out = extract1(r#"<a href="/x">l</a>"#, "a", "attr:href");
        assert_eq!(out, vec!["/x"]);
        let out = extract1(r#"<a href="/x">l</a>"#, "a", "attr:missing");
        assert_eq!(out, vec![""]);
    }
}
