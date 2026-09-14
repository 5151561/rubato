//! `io.legado.app.model.analyzeRule.AnalyzeByXPath` + 它底下的
//! **JsoupXpath 2.5.5**(`org.seimicrawler.xpath`)的移植。
//!
//! 路线说明(与 plan §1 表里"序列化成 XML 交 libxml2"那条不同,见
//! `docs/plan.md` Phase 2 的 M2g 段):裁判用的不是标准 XPath 引擎,而是
//! JsoupXpath —— 它在 jsoup 树上自己实现了一套方言:
//! - 多出 `allText()` / `html()` / `outerHtml()` / `num()` 节点测试,
//!   以及 `^=` `$=` `*=` `~=` `!~` 五个运算符、`following-sibling-one` 之类的轴;
//! - `[n]` **不是文档位置**,是"当前上下文集合里同标签兄弟的第几个",还支持 `[-1]`;
//! - 文本节点被包成合成元素参与后续步骤。
//!
//! 把 DOM 转成 XML 交给一个标准引擎,这些一条都对不上;而判据是与裁判逐字节
//! 相同,所以走的是照着 JsoupXpath 移植这条路,解析树仍然只有 html5ever 一份。

pub mod eval;
pub mod lex;
pub mod parse;
pub mod tree;

use ego_tree::NodeId;
use eval::{Proc, XValue};
use scraper::Html;
use tree::{JX_TEXT, Tree, XEl};

/// `JXNode` 的对外形态。合成元素(JX_TEXT / text / V)在真实树上没有位置,
/// 后续规则只能拿它的字符串继续走 —— 与真身把 JXNode 再喂回
/// `AnalyzeByXPath(doc)` 时 `strToJXDocument(doc.toString())` 的降级一致。
#[derive(Debug, Clone, PartialEq)]
pub enum JxNode {
    El(NodeId),
    SynthEl(String),
    Str(String),
}

impl JxNode {
    pub fn is_element(&self) -> bool {
        matches!(self, JxNode::El(_) | JxNode::SynthEl(_))
    }
}

#[derive(Debug)]
pub struct XPathError(pub String);

/// 一次 `AnalyzeByXPath(content)`:拿着解析好的树 + 根上下文。
pub struct AnalyzeByXPath {
    doc: Html,
    /// `JXDocument` 的 `elements` 字段
    root: Vec<NodeId>,
}

impl AnalyzeByXPath {
    /// `AnalyzeByXPath(doc: String)` → `strToJXDocument`。
    ///
    /// 三个包裹分支逐条照抄:以 `</td>` 结尾先包 `<tr>`,以 `</tr>` /
    /// `</tbody>` 结尾再包 `<table>`(注意**先 td 后 tr**,一段 `</td>`
    /// 会被连包两层),trim 后以 `<?xml` 开头的走 XML 解析器。
    pub fn from_content_string(html: &str) -> Self {
        let mut s = html.to_string();
        if s.ends_with("</td>") {
            s = format!("<tr>{s}</tr>");
        }
        if s.ends_with("</tr>") || s.ends_with("</tbody>") {
            s = format!("<table>{s}</table>");
        }
        // `<?xml` 开头本该走 Parser.xmlParser();html-compat 只有 HTML 解析器,
        // 这是本层唯一的已知近似(见 fixtures/cases/xpath/README.md)
        Self::from_document(&s)
    }

    /// `AnalyzeByXPath(doc: Document)` → `JXDocument.create(doc)`,
    /// 根上下文是 `doc.children()`,也就是 `<html>` 这一个元素(不是 Document)
    pub fn from_document(html: &str) -> Self {
        let doc = html_compat::parse(html);
        let root: Vec<NodeId> =
            doc.tree.root().children().filter(|c| c.value().is_element()).map(|c| c.id()).collect();
        AnalyzeByXPath { doc, root }
    }

    /// `AnalyzeByXPath(doc: Element)` → `JXDocument.create(Elements(doc))`
    pub fn from_elements(doc: Html, root: Vec<NodeId>) -> Self {
        AnalyzeByXPath { doc, root }
    }

    pub fn document(&self) -> &Html {
        &self.doc
    }

    /// 拆出(树, 根上下文)。AnalyzeRule 要把 XPath 选出来的元素**当元素**
    /// 往下传(真身 `AnalyzeByJSoup.parse(doc)` 对 `doc is JXNode && isElement`
    /// 走 `doc.asElement()`),而元素只有连着它那棵树才有意义 ——
    /// 于是包裹与根上下文的口径仍只在这里写一份。
    pub fn into_parts(self) -> (Html, Vec<NodeId>) {
        (self.doc, self.root)
    }

    /// `JXDocument.elements` —— 根上下文
    pub fn root(&self) -> &[NodeId] {
        &self.root
    }

    /// `AnalyzeByXPath.getElements`
    pub fn get_elements(&self, xpath: &str) -> Result<Option<Vec<JxNode>>, XPathError> {
        get_elements_on(&self.doc, &self.root, xpath)
    }

    /// `AnalyzeByXPath.getStringList`
    pub fn get_string_list(&self, xpath: &str) -> Result<Vec<String>, XPathError> {
        get_string_list_on(&self.doc, &self.root, xpath)
    }

    /// `AnalyzeByXPath.getString`
    pub fn get_string(&self, rule: &str) -> Result<Option<String>, XPathError> {
        get_string_on(&self.doc, &self.root, rule)
    }
}

// ---------------------------------------------------------------- 借树的入口
// AnalyzeRule 里 content 可能已经是**别人那棵树上**的 jsoup 元素
// (`AnalyzeByXPath(doc: Element)` → `JXDocument.create(Elements(doc))`)。
// 那种情况下不能重新解析(根上下文会从元素变成 <html>,`//` 的范围就错了),
// 所以核心面按 `(&Html, &[NodeId])` 开放,壳子只是持有者。

/// [`PARSE_TREE_CACHE`] 的上限。真身那张表无界(JsoupXpath 自己的老问题),
/// 但我们没有义务连这个一起复刻:走到这里的 `rule` 是 `makeUpRule` **插值之后**
/// 的串,`//div[@id='{{key}}']` 这类写法每换一次关键词就是一个新键,
/// 而搜索 worker 是长命线程。256 条足够放下任何一个源的全部规则。
const PARSE_TREE_CACHE_CAP: usize = 256;

thread_local! {
    /// `JXDocument.PARSE_TREE_CACHE`。真身是**静态** `ConcurrentHashMap` +
    /// `computeIfAbsent(xpath, ::parse)` —— 同一条 XPath 只解析一次。
    /// 这里按线程各一份(引擎里每个搜索 worker 一条线程,不必跨线程共享),
    /// 满了整表清空重来 —— 这张表只为防重复解析,不承诺命中率,
    /// 有界(见 [`PARSE_TREE_CACHE_CAP`])就够了。
    /// 解析失败不进表(真身的 computeIfAbsent 抛出时同样不落缓存)。
    static PARSE_TREE_CACHE: std::cell::RefCell<
        std::collections::HashMap<String, std::rc::Rc<parse::OrExpr>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

fn parse_cached(xpath: &str) -> Result<std::rc::Rc<parse::OrExpr>, XPathError> {
    let hit = PARSE_TREE_CACHE.with(|c| c.borrow().get(xpath).cloned());
    if let Some(a) = hit {
        return Ok(a);
    }
    let ast = std::rc::Rc::new(parse::parse(xpath).map_err(|_| XPathError("parse".into()))?);
    PARSE_TREE_CACHE.with(|c| {
        let mut m = c.borrow_mut();
        if m.len() >= PARSE_TREE_CACHE_CAP {
            m.clear();
        }
        m.insert(xpath.to_string(), ast.clone());
    });
    Ok(ast)
}

/// `JXDocument.selN(xpath)`
fn sel_n(doc: &Html, root: &[NodeId], xpath: &str) -> Result<Vec<JxNode>, XPathError> {
    let ast = parse_cached(xpath)?;
    let tree = Tree::new(doc);
    let root: Vec<XEl> = root.iter().map(|&id| XEl::Real(id)).collect();
    let mut p = Proc::new(tree, root);
    let v = p.visit_or(&ast).map_err(|e| XPathError(e.0))?;
    Ok(render(&p, v))
}

/// `AnalyzeByXPath.getElements`
pub fn get_elements_on(
    doc: &Html,
    root: &[NodeId],
    xpath: &str,
) -> Result<Option<Vec<JxNode>>, XPathError> {
    if xpath.is_empty() {
        return Ok(None);
    }
    let mut ra = rule_syntax::RuleAnalyzer::new(xpath, false);
    let rules = ra.split_rule(&["&&", "||", "%%"]).map_err(|_| XPathError("split".into()))?;
    if rules.len() == 1 {
        return Ok(Some(sel_n(doc, root, &rules[0])?));
    }
    let et = ra.elements_type().to_string();
    let mut results: Vec<Vec<JxNode>> = Vec::new();
    for rl in &rules {
        if let Some(t) = get_elements_on(doc, root, rl)? {
            if !t.is_empty() {
                let stop = et == "||";
                results.push(t);
                if stop {
                    break;
                }
            }
        }
    }
    Ok(Some(rule_syntax::merge(&results, &et)))
}

/// `AnalyzeByXPath.getStringList`
pub fn get_string_list_on(
    doc: &Html,
    root: &[NodeId],
    xpath: &str,
) -> Result<Vec<String>, XPathError> {
    let mut ra = rule_syntax::RuleAnalyzer::new(xpath, false);
    let rules = ra.split_rule(&["&&", "||", "%%"]).map_err(|_| XPathError("split".into()))?;
    if rules.len() == 1 {
        // 真身这里传的是 **xPath 原串**而不是 rules[0]
        return Ok(sel_n(doc, root, xpath)?.iter().map(|n| node_as_string(doc, n)).collect());
    }
    let et = ra.elements_type().to_string();
    let mut results: Vec<Vec<String>> = Vec::new();
    for rl in &rules {
        let temp = get_string_list_on(doc, root, rl)?;
        if !temp.is_empty() {
            let stop = et == "||";
            results.push(temp);
            if stop {
                break;
            }
        }
    }
    Ok(rule_syntax::merge(&results, &et))
}

/// `AnalyzeByXPath.getString`(注意只切 `&&` / `||`,`%%` 留在串里)
pub fn get_string_on(
    doc: &Html,
    root: &[NodeId],
    rule: &str,
) -> Result<Option<String>, XPathError> {
    let mut ra = rule_syntax::RuleAnalyzer::new(rule, false);
    let rules = ra.split_rule(&["&&", "||"]).map_err(|_| XPathError("split".into()))?;
    if rules.len() == 1 {
        let nodes = sel_n(doc, root, rule)?;
        let joined = nodes.iter().map(|n| node_as_string(doc, n)).collect::<Vec<_>>().join("\n");
        return Ok(Some(joined));
    }
    let et = ra.elements_type().to_string();
    let mut texts: Vec<String> = Vec::new();
    for rl in &rules {
        if let Some(t) = get_string_on(doc, root, rl)? {
            if !t.is_empty() {
                texts.push(t);
                if et == "||" {
                    break;
                }
            }
        }
    }
    Ok(Some(texts.join("\n")))
}

// `%%` 交叉、其余顺次拼的收尾用 rule_syntax::merge(三个 compat crate 共用)

/// `JXDocument.selN` 的收尾:把 XValue 摊成 JXNode 列表
fn render(p: &Proc<'_>, v: XValue) -> Vec<JxNode> {
    let t = p.tree();
    match v {
        // 真身:processor.visit 给 null(只有 Sum 那条)→ [JXNode("")]
        XValue::NullReturn => vec![JxNode::Str(String::new())],
        XValue::Elements(els) => els
            .into_iter()
            .map(|e| match e {
                XEl::Real(id) => JxNode::El(id),
                XEl::Synth(_) => {
                    // JXNode.asString():JX_TEXT 给 ownText,其余给 toString()
                    if t.tag_name(e) == JX_TEXT {
                        JxNode::SynthEl(t.own_text(e))
                    } else {
                        JxNode::SynthEl(t.outer_html(e))
                    }
                }
            })
            .collect(),
        // `isList()` 那支是 `JXNode.create(item)` —— 逐项**不过 asString()**,不 trim
        XValue::List(l) => l.into_iter().map(JxNode::Str).collect(),
        // `isString()` 那支是 `JXNode.create(calRes.asString())`,而
        // `XValue.asString()` 的兜底是 `String.valueOf(value).trim()` ——
        // 于是**单个元素上取到的属性值两头会被削掉**(Java trim:`c <= ' '`,
        // 不含 NBSP / 全角空格)。语料里 `href="  /book/…/  "` 这种写法不少见,
        // 漏了这一下,url 拼接与去重都会跟真身错开。
        XValue::Str(s) => vec![JxNode::Str(s.trim_matches(|c: char| c <= ' ').to_string())],
        XValue::Num(n) => vec![JxNode::Str(match n {
            eval::Num::Long(x) => x.to_string(),
            eval::Num::Int(x) => x.to_string(),
            eval::Num::Double(d) => eval::java_double_to_string(d),
        })],
        XValue::Bool(b) => vec![JxNode::Str(b.to_string())],
        // XValue(null):isXxx 全 false → JXNode.create(calRes.asString()) = "null"
        XValue::Null | XValue::Attr => vec![JxNode::Str("null".to_string())],
    }
}

/// 真实元素的 `JXNode.asString()`:JX_TEXT 之外都是 `toString()`(outerHtml)
pub fn node_as_string(doc: &Html, n: &JxNode) -> String {
    match n {
        JxNode::El(id) => {
            let t = Tree::new(doc);
            t.outer_html(XEl::Real(*id))
        }
        JxNode::SynthEl(s) | JxNode::Str(s) => s.clone(),
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    /// 插值后的规则会把表撑爆(`//div[@id='<关键词>']` 每搜一次一个新键),
    /// 而 worker 线程是长命的 —— 表要有上限。
    #[test]
    fn parse_cache_is_bounded() {
        for i in 0..PARSE_TREE_CACHE_CAP * 2 {
            assert!(parse_cached(&format!("//div[@id='k{i}']")).is_ok());
        }
        PARSE_TREE_CACHE.with(|c| {
            let m = c.borrow();
            assert!(m.len() <= PARSE_TREE_CACHE_CAP, "表撑到了 {}", m.len());
        });
    }

    /// 解析失败不进表(真身 computeIfAbsent 抛出时同样不落缓存)
    #[test]
    fn parse_failure_not_cached() {
        let bad = "//div[";
        assert!(parse_cached(bad).is_err());
        PARSE_TREE_CACHE.with(|c| assert!(!c.borrow().contains_key(bad)));
    }
}
