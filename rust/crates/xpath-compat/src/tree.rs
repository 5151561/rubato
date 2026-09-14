//! JsoupXpath 眼里的"元素":真实 jsoup 节点 **加上**它自己造出来的合成元素。
//!
//! 合成元素有三处来源,都在真身里明写:
//! - `Text` 节点测试:每个 TextNode 包成 `Element("JX_TEXT")`,`text(wholeText)`,
//!   挂上 `EL_SAME_TAG_INDEX` / `EL_SAME_TAG_ALL_NUM` 属性;**递归那支**还用反射
//!   把它的 parentNode 指回原 TextNode 的父元素 —— 这条线不是摆设:
//!   `ownText()` 要沿父链找 `preserveWhitespace`,于是 `<pre>` 里的文本
//!   在 `//pre//text()` 下**不归一空白**,在 `//pre/text()` 下(不设父)**归一**。
//! - `following-sibling` / `preceding-sibling` / `CommonUtil.*Sibling`:
//!   路过的 TextNode 包成 `Element("text")`。
//! - `visitUnionExprNoRoot` 把标量并进节点集时包成 `Element("V")`。
//!
//! 合成元素不进真实树(没有子节点、拿不到兄弟),但要能 `toString()` ——
//! 序列化按 jsoup 的 prettyPrint 手写,与 html-compat 那份共用空白归一。

use ego_tree::NodeId;
use html_compat::dom::{self, NRef};
use html_compat::serialize::{escape_html, indent, inner_html, outer_html};
use html_compat::tags::tag_flags;
use html_compat::{Ev, select, text};
use scraper::{Html, Node};
use std::collections::HashMap;

pub const JX_TEXT: &str = "JX_TEXT";
pub const EL_SAME_TAG_INDEX_KEY: &str = "EL_SAME_TAG_INDEX";
pub const EL_SAME_TAG_ALL_NUM_KEY: &str = "EL_SAME_TAG_ALL_NUM";

/// 一个 TextNode:真实树上的,或合成元素肚子里的那一个
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextRef {
    Real(NodeId),
    Synth(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XEl {
    Real(NodeId),
    Synth(usize),
}

#[derive(Debug, Clone)]
pub struct Synth {
    pub tag: String,
    /// `Element.text(s)` / `appendText(s)` 塞进去的那一个 TextNode
    pub text: String,
    pub attrs: Vec<(String, String)>,
    /// 反射设的 parentNode(只有递归 text() 那支有)
    pub parent: Option<NodeId>,
}

pub struct Tree<'a> {
    pub doc: &'a Html,
    synth: Vec<Synth>,
    /// 选择器编译缓存(`select("[attr]")` / `select(tag)` 在递归步里逐元素调)
    sel_cache: HashMap<String, Option<Ev>>,
}

impl<'a> Tree<'a> {
    pub fn new(doc: &'a Html) -> Self {
        Tree { doc, synth: Vec::new(), sel_cache: HashMap::new() }
    }

    pub fn nref(&self, id: NodeId) -> NRef<'a> {
        self.doc.tree.get(id).expect("节点 id 属于本树")
    }

    pub fn new_synth(&mut self, tag: &str, text: &str) -> XEl {
        self.synth.push(Synth {
            tag: tag.to_string(),
            text: text.to_string(),
            attrs: Vec::new(),
            parent: None,
        });
        XEl::Synth(self.synth.len() - 1)
    }

    pub fn set_synth_attr(&mut self, e: XEl, k: &str, v: &str) {
        if let XEl::Synth(i) = e {
            let s = &mut self.synth[i];
            match s.attrs.iter_mut().find(|(ak, _)| ak == k) {
                Some(slot) => slot.1 = v.to_string(),
                None => s.attrs.push((k.to_string(), v.to_string())),
            }
        }
    }

    pub fn set_synth_parent(&mut self, e: XEl, p: NodeId) {
        if let XEl::Synth(i) = e {
            self.synth[i].parent = Some(p);
        }
    }

    // ------------------------------------------------------------ 元素面

    /// jsoup `nodeName()` / `tagName()`(Document 是 `#root`)
    pub fn tag_name(&self, e: XEl) -> String {
        match e {
            XEl::Real(id) => {
                let n = self.nref(id);
                match n.value() {
                    Node::Element(el) => el.name().to_string(),
                    Node::Document => "#root".to_string(),
                    _ => String::new(),
                }
            }
            XEl::Synth(i) => self.synth[i].tag.clone(),
        }
    }

    /// jsoup `attr(k)`:大小写不敏感,缺失给空串
    pub fn attr(&self, e: XEl, k: &str) -> String {
        match e {
            XEl::Real(id) => dom::attr(self.nref(id), k).unwrap_or("").to_string(),
            XEl::Synth(i) => self.synth[i]
                .attrs
                .iter()
                .find(|(ak, _)| ak.eq_ignore_ascii_case(k))
                .map(|(_, v)| v.clone())
                .unwrap_or_default(),
        }
    }

    pub fn own_text(&self, e: XEl) -> String {
        match e {
            XEl::Real(id) => text::own_text(self.nref(id)),
            XEl::Synth(i) => {
                let s = &self.synth[i];
                let mut accum = String::new();
                if self.synth_preserves_whitespace(i) {
                    accum.push_str(&s.text);
                } else {
                    dom::append_normalised_whitespace(&mut accum, &s.text, false);
                }
                accum.trim_matches(|c: char| c <= ' ').to_string()
            }
        }
    }

    pub fn text(&self, e: XEl) -> String {
        match e {
            XEl::Real(id) => text::text(self.nref(id)),
            XEl::Synth(_) => self.own_text(e),
        }
    }

    pub fn data(&self, e: XEl) -> String {
        match e {
            XEl::Real(id) => text::data(self.nref(id)),
            XEl::Synth(_) => String::new(),
        }
    }

    pub fn inner_html(&self, e: XEl) -> String {
        match e {
            XEl::Real(id) => inner_html(self.nref(id)),
            XEl::Synth(i) => {
                let mut accum = String::new();
                self.synth_text_html(i, &mut accum);
                accum.trim_matches(|c: char| c <= ' ').to_string()
            }
        }
    }

    /// jsoup `outerHtml()` / `toString()`。
    ///
    /// 合成元素恒是"一个标签 + 一个 TextNode 子节点",按 jsoup 的
    /// prettyPrint 两分支走:`Element("V")` / `Element("JX_TEXT")` 是**未注册
    /// 标签**(formatAsBlock 默认 true)→ `<V>\n x\n</V>`;
    /// 而 `Element("text")` 在 jsoup 的 Tag 表里是**行内**(math 那组)→
    /// `<text>x</text>` 不缩进。这一格差别是差分照出来的。
    pub fn outer_html(&self, e: XEl) -> String {
        match e {
            XEl::Real(id) => outer_html(self.nref(id)),
            XEl::Synth(i) => {
                let tag = self.synth[i].tag.clone();
                let f = tag_flags(&tag.to_ascii_lowercase());
                let preserve = self.synth_preserves_whitespace(i);
                let mut accum = String::new();
                accum.push('<');
                accum.push_str(&tag);
                for (k, v) in &self.synth[i].attrs {
                    accum.push(' ');
                    accum.push_str(k);
                    accum.push_str("=\"");
                    escape_html(&mut accum, v, true, false, false, false);
                    accum.push('"');
                }
                accum.push('>');
                // TextNode.outerHtmlHead(siblingIndex = 0,前后无兄弟)
                let normalise = !preserve;
                let trim_like_block = f.is_block || f.format_as_block;
                let blank = self.synth[i].text.chars().all(dom::is_jsoup_whitespace);
                if normalise && f.format_as_block && !blank {
                    indent(&mut accum, 1); // 那一个 TextNode 恒在 depth=1
                }
                escape_html(
                    &mut accum,
                    &self.synth[i].text,
                    false,
                    normalise,
                    normalise && trim_like_block,
                    normalise && trim_like_block,
                );
                // Element.outerHtmlTail
                if f.format_as_block && !preserve {
                    accum.push('\n');
                }
                accum.push_str("</");
                accum.push_str(&tag);
                accum.push('>');
                accum
            }
        }
    }

    /// 合成元素的 `html()`:只有那一个 TextNode,再 trim
    fn synth_text_html(&self, i: usize, accum: &mut String) {
        let normalise = !self.synth_preserves_whitespace(i);
        escape_html(accum, &self.synth[i].text, false, normalise, false, false);
    }

    /// `Element.preserveWhitespace`:自身 + 上溯至多 5 层。
    /// 合成元素的标签(JX_TEXT / text / V)都没注册 → 只看反射设上去的父链。
    fn synth_preserves_whitespace(&self, i: usize) -> bool {
        match self.synth[i].parent {
            // 合成元素自己算一层,故父链只剩 5 层 —— dom::preserve_whitespace
            // 从传入节点起算 6 层,这里传父节点正好差一层,与真身一致
            Some(p) => {
                let n = self.nref(p);
                let mut cur = Some(n);
                let mut k = 1; // 合成元素自身已经数过
                while let Some(x) = cur {
                    if x.value().is_element() {
                        if dom::flags(x).preserve_whitespace {
                            return true;
                        }
                    } else {
                        return false;
                    }
                    cur = x.parent();
                    k += 1;
                    if k >= 6 {
                        break;
                    }
                }
                false
            }
            None => false,
        }
    }

    /// jsoup `Element.children()`(仅元素子节点)
    pub fn children(&self, e: XEl) -> Vec<XEl> {
        match e {
            XEl::Real(id) => self
                .nref(id)
                .children()
                .filter(|c| c.value().is_element())
                .map(|c| XEl::Real(c.id()))
                .collect(),
            XEl::Synth(_) => Vec::new(),
        }
    }

    /// jsoup `Node.parent()`:可能是 Document
    pub fn parent(&self, e: XEl) -> Option<XEl> {
        match e {
            XEl::Real(id) => self
                .nref(id)
                .parent()
                .filter(|p| dom::is_element_like(*p))
                .map(|p| XEl::Real(p.id())),
            XEl::Synth(i) => self.synth[i].parent.map(XEl::Real),
        }
    }

    /// jsoup `Element.parents()`:上溯到 `#root` 之前为止(不含 Document)
    pub fn parents(&self, e: XEl) -> Vec<XEl> {
        let mut out = Vec::new();
        let mut cur = self.parent(e);
        while let Some(p) = cur {
            if self.tag_name(p) == "#root" {
                break;
            }
            out.push(p);
            cur = self.parent(p);
        }
        out
    }

    /// jsoup `Element.getAllElements()`:自身 + 全部后代,文档序
    pub fn all_elements(&self, e: XEl) -> Vec<XEl> {
        let XEl::Real(id) = e else { return vec![e] };
        let mut out = Vec::new();
        fn walk(n: NRef<'_>, out: &mut Vec<XEl>) {
            if dom::is_element_like(n) {
                out.push(XEl::Real(n.id()));
            }
            for c in n.children() {
                walk(c, out);
            }
        }
        walk(self.nref(id), &mut out);
        out
    }

    /// jsoup `Element.getElementsByTag(tag)`:自身 + 后代,按 normalName 比
    pub fn elements_by_tag(&self, e: XEl, tag: &str) -> Vec<XEl> {
        let t = tag.trim().to_lowercase();
        self.all_elements(e).into_iter().filter(|&x| self.normal_name(x) == t).collect()
    }

    pub fn normal_name(&self, e: XEl) -> String {
        match e {
            XEl::Real(id) => dom::normal_name(self.nref(id)).into_owned(),
            XEl::Synth(i) => self.synth[i].tag.to_lowercase(),
        }
    }

    /// jsoup `Selector.select(query, roots)`:逐 root 收集,按 identity 去重保序
    pub fn select(&mut self, roots: &[XEl], query: &str) -> Option<Vec<XEl>> {
        if !self.sel_cache.contains_key(query) {
            self.sel_cache.insert(query.to_string(), select::parse(query).ok());
        }
        let ev = self.sel_cache.get(query)?.as_ref()?;
        let mut out: Vec<XEl> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for &r in roots {
            let XEl::Real(id) = r else { continue };
            for n in select::select(self.nref(id), ev) {
                if seen.insert(n.id()) {
                    out.push(XEl::Real(n.id()));
                }
            }
        }
        Some(out)
    }

    /// jsoup `Element.textNodes()`:直接子 TextNode(script/style 里的是
    /// DataNode,不算)。**合成元素也有一个** —— `Element.text(s)` 塞进去的
    /// 那一个,于是 `following-sibling::*/text()` 这种写法能从 `<text>`
    /// 合成元素里再抠出文本(差分实测如此)。
    pub fn text_nodes(&self, e: XEl) -> Vec<TextRef> {
        match e {
            XEl::Real(id) => self
                .nref(id)
                .children()
                .filter(|c| dom::is_text_node(*c))
                .map(|c| TextRef::Real(c.id()))
                .collect(),
            XEl::Synth(i) => vec![TextRef::Synth(i)],
        }
    }

    /// `TextNode.getWholeText()`
    pub fn whole_text_of(&self, t: TextRef) -> String {
        match t {
            TextRef::Real(id) => dom::text_content(self.nref(id)).to_string(),
            TextRef::Synth(i) => self.synth[i].text.clone(),
        }
    }

    /// `NodeTraversor.traverse` 只看 TextNode:回调 (TextNode, depth, 父元素)。
    /// 合成元素也参与 —— 它自己是根(depth 0),那一个 TextNode 在 depth 1。
    pub fn traverse_text_nodes(&self, e: XEl, f: &mut dyn FnMut(TextRef, usize, Option<NodeId>)) {
        match e {
            XEl::Synth(i) => f(TextRef::Synth(i), 1, self.synth[i].parent),
            XEl::Real(id) => {
                fn walk(
                    n: NRef<'_>,
                    depth: usize,
                    f: &mut dyn FnMut(TextRef, usize, Option<NodeId>),
                ) {
                    if dom::is_text_node(n) {
                        f(TextRef::Real(n.id()), depth, n.parent().map(|p| p.id()));
                    }
                    for c in n.children() {
                        walk(c, depth + 1, f);
                    }
                }
                walk(self.nref(id), 0, f);
            }
        }
    }

    /// 全节点意义上的后继兄弟(元素给 Real,文本节点包成 `<text>`,其余跳过)
    pub fn following_siblings(&mut self, e: XEl) -> Vec<XEl> {
        self.siblings(e, true)
    }

    pub fn preceding_siblings(&mut self, e: XEl) -> Vec<XEl> {
        self.siblings(e, false)
    }

    fn siblings(&mut self, e: XEl, forward: bool) -> Vec<XEl> {
        let XEl::Real(id) = e else { return Vec::new() };
        let mut out = Vec::new();
        let mut cur =
            if forward { self.nref(id).next_sibling() } else { self.nref(id).prev_sibling() };
        let mut texts = Vec::new();
        while let Some(n) = cur {
            if n.value().is_element() {
                out.push(XEl::Real(n.id()));
            } else if dom::is_text_node(n) {
                // Element("text").text(textNode.text()) —— 注意是 text() 不是
                // getWholeText():TextNode.text() 已归一空白并 trim
                texts.push((out.len(), n.id()));
                out.push(XEl::Real(n.id())); // 占位,下面替换
            }
            cur = if forward { n.next_sibling() } else { n.prev_sibling() };
        }
        for (pos, tid) in texts {
            let t = self.text_node_text(TextRef::Real(tid));
            let s = self.new_synth("text", &t);
            out[pos] = s;
        }
        out
    }

    /// jsoup `TextNode.text()` = `StringUtil.normaliseWhitespace(wholeText)`
    /// —— 只归一空白,**不 trim**
    pub fn text_node_text(&self, t: TextRef) -> String {
        let raw = self.whole_text_of(t);
        let raw = raw.as_str();
        let mut accum = String::new();
        dom::append_normalised_whitespace(&mut accum, raw, false);
        accum
    }

    pub fn next_element_sibling(&self, e: XEl) -> Option<XEl> {
        let XEl::Real(id) = e else { return None };
        dom::next_element_sibling(self.nref(id)).map(|n| XEl::Real(n.id()))
    }

    pub fn prev_element_sibling(&self, e: XEl) -> Option<XEl> {
        let XEl::Real(id) = e else { return None };
        dom::prev_element_sibling(self.nref(id)).map(|n| XEl::Real(n.id()))
    }
}
