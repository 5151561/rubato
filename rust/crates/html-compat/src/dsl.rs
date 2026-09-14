//! AnalyzeByJSoup(judge/engine AnalyzeByJSoup.kt,524 行)的逐行移植。
//!
//! 注意事项:
//! - `html` 动作会**变异文档**(移除选中元素子树里的 script/style),与裁判
//!   一致,因此本类型拥有文档所有权,内部用 NodeId 而非引用;
//! - 索引解析(findIndexSet)是逆向扫描,`!`=排除、`.`=选择、
//!   `[start:end:step]` 支持负数与反向区间;
//! - Kotlin 在 "a." 这类尾分隔符上会因 "".toInt() 抛 NumberFormatException,
//!   这里映射为 DslError,两侧差分同走错误路径。

use crate::dom::{self, NRef};
use crate::select::{self, Ev};
use crate::serialize;
use crate::text as jtext;
use ego_tree::NodeId;
use scraper::Html;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DslError(pub String);

impl std::fmt::Display for DslError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dsl error: {}", self.0)
    }
}
impl std::error::Error for DslError {}

pub struct AnalyzeByJSoup {
    doc: Html,
    root: NodeId,
}

impl AnalyzeByJSoup {
    pub fn from_html(html: &str) -> Self {
        let doc = crate::parse(html);
        let root = doc.tree.root().id();
        Self { doc, root }
    }

    /// 拿**现成的**解析树建(不重新解析)。XPath 那条要用它:
    /// `AnalyzeByXPath` 选出来的元素得连着它自己那棵树往下传,
    /// 重新解析 `<tr>…</tr>` 这类片段会被 foster parenting 丢光。
    pub fn from_tree(doc: Html) -> Self {
        let root = doc.tree.root().id();
        Self { doc, root }
    }

    /// 文档根(AnalyzeRule 复用同一文档、按元素换根时用)
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// 底层解析树。XPath 那条(`AnalyzeByXPath(doc: Element)`)要在**同一棵树**
    /// 上以某个元素为根求值 —— 重新解析会把根上下文从元素变回 `<html>`,
    /// `//` 的范围就错了。
    pub fn html(&self) -> &Html {
        &self.doc
    }

    fn node(&self, id: NodeId) -> NRef<'_> {
        self.doc.tree.get(id).expect("node id 有效")
    }

    /// getString:列表为空 → None;单元素直接返回;多元素 \n 连接
    pub fn get_string(&mut self, rule_str: &str) -> Result<Option<String>, DslError> {
        let root = self.root;
        self.get_string_from(root, rule_str)
    }

    /// 以 `root` 为「当前元素」的 getString(对应 `AnalyzeByJSoup(element)`)
    pub fn get_string_from(
        &mut self,
        root: NodeId,
        rule_str: &str,
    ) -> Result<Option<String>, DslError> {
        if rule_str.is_empty() {
            return Ok(None);
        }
        let list = self.get_string_list_from(root, rule_str)?;
        Ok(match list.len() {
            0 => None,
            1 => Some(list.into_iter().next().unwrap()),
            _ => Some(list.join("\n")),
        })
    }

    /// getString0
    pub fn get_string0(&mut self, rule_str: &str) -> Result<String, DslError> {
        let root = self.root;
        self.get_string0_from(root, rule_str)
    }

    pub fn get_string0_from(&mut self, root: NodeId, rule_str: &str) -> Result<String, DslError> {
        let list = self.get_string_list_from(root, rule_str)?;
        Ok(list.into_iter().next().unwrap_or_default())
    }

    /// getStringList
    pub fn get_string_list(&mut self, rule_str: &str) -> Result<Vec<String>, DslError> {
        let root = self.root;
        self.get_string_list_from(root, rule_str)
    }

    pub fn get_string_list_from(
        &mut self,
        root: NodeId,
        rule_str: &str,
    ) -> Result<Vec<String>, DslError> {
        let mut text_s: Vec<String> = Vec::new();
        if rule_str.is_empty() {
            return Ok(text_s);
        }

        let source_rule = SourceRule::new(rule_str);
        if source_rule.elements_rule.is_empty() {
            text_s.push(jtext::data(self.node(root)));
            return Ok(text_s);
        }

        let mut ra = rule_syntax::RuleAnalyzer::new(&source_rule.elements_rule, false);
        let rule_str_s = ra.split_rule(&["&&", "||", "%%"]).map_err(|e| DslError(e.to_string()))?;
        let elements_type = ra.elements_type().to_string();

        let mut results: Vec<Vec<String>> = Vec::new();
        for rule_str_x in &rule_str_s {
            let temp: Option<Vec<String>> = if source_rule.is_css {
                // Kotlin lastIndexOf('@') 为 -1 时 substring 抛异常,同走错误
                let last_index =
                    rule_str_x.rfind('@').ok_or_else(|| DslError("@CSS 规则缺少 @动作".into()))?;
                let sel = &rule_str_x[..last_index];
                let action = &rule_str_x[last_index + 1..];
                let ev = select::parse(sel).map_err(|e| DslError(e.to_string()))?;
                let els: Vec<NodeId> =
                    select::select(self.node(root), &ev).into_iter().map(|n| n.id()).collect();
                Some(self.get_result_last(&els, action))
            } else {
                self.get_result_list(root, rule_str_x)?
            };

            if let Some(temp) = temp {
                if !temp.is_empty() {
                    results.push(temp);
                    if elements_type == "||" {
                        break;
                    }
                }
            }
        }
        text_s.extend(rule_syntax::merge(&results, &elements_type));
        Ok(text_s)
    }

    /// getElements(对外形态:每个元素的 outerHtml,供差分与上层遍历)
    pub fn get_elements(&self, rule: &str) -> Result<Vec<NodeId>, DslError> {
        self.get_elements_of(self.root, rule)
    }

    pub fn get_elements_of(&self, temp: NodeId, rule: &str) -> Result<Vec<NodeId>, DslError> {
        if rule.is_empty() {
            return Ok(Vec::new());
        }

        let source_rule = SourceRule::new(rule);
        let mut ra = rule_syntax::RuleAnalyzer::new(&source_rule.elements_rule, false);
        let rule_str_s = ra.split_rule(&["&&", "||", "%%"]).map_err(|e| DslError(e.to_string()))?;
        let elements_type = ra.elements_type().to_string();

        let mut elements_list: Vec<Vec<NodeId>> = Vec::new();
        if source_rule.is_css {
            for rule_str in &rule_str_s {
                let ev = select::parse(rule_str).map_err(|e| DslError(e.to_string()))?;
                let temp_s: Vec<NodeId> =
                    select::select(self.node(temp), &ev).into_iter().map(|n| n.id()).collect();
                let found = !temp_s.is_empty();
                elements_list.push(temp_s);
                if found && elements_type == "||" {
                    break;
                }
            }
        } else {
            for rule_str in &rule_str_s {
                let mut rs_rule = rule_syntax::RuleAnalyzer::new(rule_str, false);
                rs_rule.trim().map_err(|e| DslError(e.to_string()))?;
                let rs = rs_rule.split_rule(&["@"]).map_err(|e| DslError(e.to_string()))?;

                let el: Vec<NodeId> = if rs.len() > 1 {
                    let mut el = vec![temp];
                    for rl in &rs {
                        let mut es = Vec::new();
                        for &et in &el {
                            es.extend(self.get_elements_of(et, rl)?);
                        }
                        el = es;
                    }
                    el
                } else {
                    ElementsSingle::default().get_elements_single(self, temp, rule_str)?
                };

                let found = !el.is_empty();
                elements_list.push(el);
                if found && elements_type == "||" {
                    break;
                }
            }
        }

        Ok(rule_syntax::merge(&elements_list, &elements_type))
    }

    fn get_result_list(
        &mut self,
        root: NodeId,
        rule_str: &str,
    ) -> Result<Option<Vec<String>>, DslError> {
        if rule_str.is_empty() {
            return Ok(None);
        }

        let mut elements = vec![root];

        let mut rule = rule_syntax::RuleAnalyzer::new(rule_str, false);
        rule.trim().map_err(|e| DslError(e.to_string()))?;
        let rules = rule.split_rule(&["@"]).map_err(|e| DslError(e.to_string()))?;

        let last = rules.len() - 1;
        for r in &rules[..last] {
            let mut es = Vec::new();
            for &elt in &elements {
                es.extend(ElementsSingle::default().get_elements_single(self, elt, r)?);
            }
            elements = es;
        }
        if elements.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.get_result_last(&elements, &rules[last])))
        }
    }

    /// getResultLast:text/textNodes/ownText/html/all/属性
    fn get_result_last(&mut self, elements: &[NodeId], last_rule: &str) -> Vec<String> {
        let mut text_s: Vec<String> = Vec::new();
        match last_rule {
            "text" => {
                for &id in elements {
                    let text = jtext::text(self.node(id));
                    if !text.is_empty() {
                        text_s.push(text);
                    }
                }
            }
            "textNodes" => {
                for &id in elements {
                    let mut tn: Vec<String> = Vec::new();
                    for raw in jtext::text_nodes(self.node(id)) {
                        // jsoup TextNode.text() = normaliseWhitespace(wholeText),再 Java trim
                        let mut norm = String::new();
                        dom::append_normalised_whitespace(&mut norm, &raw, false);
                        let text = norm.trim_matches(|c: char| c <= ' ').to_string();
                        if !text.is_empty() {
                            tn.push(text);
                        }
                    }
                    if !tn.is_empty() {
                        text_s.push(tn.join("\n"));
                    }
                }
            }
            "ownText" => {
                for &id in elements {
                    let text = jtext::own_text(self.node(id));
                    if !text.is_empty() {
                        text_s.push(text);
                    }
                }
            }
            "html" => {
                // 与裁判一致:先从选中元素子树里移除 script/style(变异文档)
                let mut to_detach: Vec<NodeId> = Vec::new();
                for &id in elements {
                    collect_by_names(self.node(id), &["script", "style"], &mut to_detach);
                }
                for id in to_detach {
                    if let Some(mut m) = self.doc.tree.get_mut(id) {
                        m.detach();
                    }
                }
                let html = self.outer_html_joined(elements);
                if !html.is_empty() {
                    text_s.push(html);
                }
            }
            "all" => text_s.push(self.outer_html_joined(elements)),
            attr_name => {
                for &id in elements {
                    let url = dom::attr(self.node(id), attr_name).unwrap_or("");
                    if is_java_blank(url) || text_s.iter().any(|t| t == url) {
                        continue;
                    }
                    text_s.push(url.to_string());
                }
            }
        }
        text_s
    }

    /// jsoup Elements.outerHtml():逐元素 outerHtml 以 \n 连接
    fn outer_html_joined(&self, elements: &[NodeId]) -> String {
        elements
            .iter()
            .map(|&id| serialize::outer_html(self.node(id)))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn outer_html_of(&self, id: NodeId) -> String {
        serialize::outer_html(self.node(id))
    }

    // ---- 单节点的 jsoup 可观察面(JS 里 `java.getElement(s)` 拿到元素后调的那几个)----

    /// `Element.text()`
    pub fn text_of(&self, id: NodeId) -> String {
        crate::text::text(self.node(id))
    }

    /// `Element.html()`(内层 HTML)
    pub fn inner_html_of(&self, id: NodeId) -> String {
        serialize::inner_html(self.node(id))
    }

    /// `Element.attr(name)`;jsoup 取不到给空串
    pub fn attr_of(&self, id: NodeId, name: &str) -> String {
        dom::attr(self.node(id), name).unwrap_or_default().to_string()
    }

    /// `Element.select(css)`——以该节点为根跑选择器
    pub fn select_of(&self, id: NodeId, css: &str) -> Result<Vec<NodeId>, DslError> {
        self.get_elements_of(id, css)
    }

    /// `Node.parentNode()` / `Element.parent()`。`<html>` 的父是 Document
    /// (解析树的根),Document 再往上是 null —— 与 jsoup 同形。
    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        self.node(id).parent().map(|p| p.id())
    }

    /// 这个节点**自己**匹不匹配 css(`Elements.not(query)` 要的那一问)。
    ///
    /// jsoup 的 `not` 是 `filterOut(this, select(query, this))`:`select` 是
    /// 「后代**含自身**」,而 filterOut 按**同一性**比 —— 结果就等于
    /// 「把自己匹配 query 的那些去掉」(实测 jsoup 1.16.2:
    /// `div>*` 三项 `not("h3,p")` 剩 span,而包着 h3 的 div 留着)。
    pub fn matches_query(&self, id: NodeId, css: &str) -> bool {
        let Ok(ev) = crate::select::parse(css) else { return false };
        let n = self.node(id);
        n.value().is_element() && ev.matches(n, n)
    }

    /// `Node.remove()`:从父节点上摘下来。摘下来的子树还在,
    /// `outerHtml()` 照样取得到(jsoup 同样:`Elements.remove()` 交回的那份
    /// 还打得出内容),只是不再出现在文档里。
    pub fn detach(&mut self, id: NodeId) {
        if let Some(mut n) = self.doc.tree.get_mut(id) {
            n.detach();
        }
    }
}

fn collect_by_names(n: NRef<'_>, names: &[&str], out: &mut Vec<NodeId>) {
    if n.value().is_element() && names.contains(&dom::normal_name(n).as_ref()) {
        out.push(n.id());
    }
    for c in n.children() {
        collect_by_names(c, names, out);
    }
}

/// Java Character.isWhitespace 近似(排除 NBSP/2007/202F 等不间断空白)
fn is_java_blank(s: &str) -> bool {
    s.chars().all(|c| {
        matches!(c, '\t' | '\n' | '\x0B' | '\x0C' | '\r' | '\x1C'..='\x1F')
            || (c.is_whitespace() && !matches!(c, '\u{A0}' | '\u{2007}' | '\u{202F}'))
    })
}

struct SourceRule {
    is_css: bool,
    elements_rule: String,
}

impl SourceRule {
    fn new(rule_str: &str) -> Self {
        if rule_str.get(..5).is_some_and(|h| h.eq_ignore_ascii_case("@CSS:")) {
            Self {
                is_css: true,
                elements_rule: rule_str[5..].trim_matches(|c: char| c <= ' ').to_string(),
            }
        } else {
            Self { is_css: false, elements_rule: rule_str.to_string() }
        }
    }
}

/// ElementsSingle:单段规则(前置规则 + 索引筛选)
#[derive(Default)]
struct ElementsSingle {
    split: char,
    before_rule: String,
    index_default: Vec<i64>,
    indexes: Vec<IndexItem>,
}

enum IndexItem {
    Single(i64),
    /// (start, end, step)(start/end 可省略)
    Range(Option<i64>, Option<i64>, i64),
}

impl ElementsSingle {
    fn get_elements_single(
        mut self,
        host: &AnalyzeByJSoup,
        temp: NodeId,
        rule: &str,
    ) -> Result<Vec<NodeId>, DslError> {
        self.split = '.';
        self.find_index_set(rule)?;

        let temp_ref = host.node(temp);
        let mut elements: Vec<NodeId> = if self.before_rule.is_empty() {
            children_elements(temp_ref)
        } else {
            let rules: Vec<&str> = self.before_rule.split('.').collect();
            let need_arg = |i: usize| -> Result<&str, DslError> {
                rules.get(i).copied().ok_or_else(|| DslError("规则缺少参数".into()))
            };
            match rules[0] {
                "children" => children_elements(temp_ref),
                "class" => collect_eval(temp_ref, &Ev::Class(need_arg(1)?.trim().to_string())),
                "tag" => collect_eval(temp_ref, &Ev::Tag(need_arg(1)?.trim().to_lowercase())),
                "id" => collect_eval(temp_ref, &Ev::Id(need_arg(1)?.to_string())),
                "text" => {
                    // getElementsContainingOwnText:构造时 lowerCase(normaliseWhitespace)
                    let mut norm = String::new();
                    dom::append_normalised_whitespace(&mut norm, need_arg(1)?, false);
                    collect_eval(temp_ref, &Ev::ContainsOwnText(norm.to_lowercase()))
                }
                _ => {
                    let ev =
                        select::parse(&self.before_rule).map_err(|e| DslError(e.to_string()))?;
                    select::select(temp_ref, &ev).into_iter().map(|n| n.id()).collect()
                }
            }
        };

        let len = elements.len() as i64;
        let last_indexes = if !self.index_default.is_empty() {
            self.index_default.len() as i64 - 1
        } else {
            self.indexes.len() as i64 - 1
        };
        // 有序去重集合(Kotlin mutableSetOf = LinkedHashSet)
        let mut index_set: Vec<i64> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut push = |v: i64, index_set: &mut Vec<i64>| {
            if seen.insert(v) {
                index_set.push(v);
            }
        };

        if self.indexes.is_empty() {
            let mut ix = last_indexes;
            while ix >= 0 {
                let it = self.index_default[ix as usize];
                if (0..len).contains(&it) {
                    push(it, &mut index_set);
                } else if it < 0 && len >= -it {
                    push(it + len, &mut index_set);
                }
                ix -= 1;
            }
        } else {
            let mut ix = last_indexes;
            while ix >= 0 {
                match &self.indexes[ix as usize] {
                    IndexItem::Range(start_x, end_x, step_x) => {
                        let mut start = start_x.unwrap_or(0);
                        if start < 0 {
                            start += len;
                        }
                        let mut end = end_x.unwrap_or(len - 1);
                        if end < 0 {
                            end += len;
                        }
                        if (start < 0 && end < 0) || (start >= len && end >= len) {
                            ix -= 1;
                            continue;
                        }
                        // Kotlin coerceIn(0, len-1):len==0 时区间为空,抛
                        // IllegalArgumentException(裁判侧同样失败)
                        if len < 1 {
                            return Err(DslError("coerceIn: empty range".into()));
                        }
                        start = start.clamp(0, len - 1);
                        end = end.clamp(0, len - 1);
                        if start == end || *step_x >= len {
                            push(start, &mut index_set);
                            ix -= 1;
                            continue;
                        }
                        let step = if *step_x > 0 {
                            *step_x
                        } else if -*step_x < len {
                            *step_x + len
                        } else {
                            1
                        };
                        if end > start {
                            let mut i = start;
                            while i <= end {
                                push(i, &mut index_set);
                                i += step;
                            }
                        } else {
                            let mut i = start;
                            while i >= end {
                                push(i, &mut index_set);
                                i -= step;
                            }
                        }
                    }
                    IndexItem::Single(it) => {
                        let it = *it;
                        if (0..len).contains(&it) {
                            push(it, &mut index_set);
                        } else if it < 0 && len >= -it {
                            push(it + len, &mut index_set);
                        }
                    }
                }
                ix -= 1;
            }
        }

        if self.split == '!' {
            let remove: std::collections::HashSet<i64> = index_set.into_iter().collect();
            elements = elements
                .into_iter()
                .enumerate()
                .filter(|(i, _)| !remove.contains(&(*i as i64)))
                .map(|(_, e)| e)
                .collect();
        } else if self.split == '.' {
            let src = elements;
            elements = index_set.into_iter().map(|i| src[i as usize]).collect();
        }
        Ok(elements)
    }

    /// findIndexSet:逆向扫描规则尾部的索引表达式。
    /// Kotlin 的 while (len-- >= 0) 扫过串头必然越界抛异常(空串则 last() 抛),
    /// 此处以 Err 忠实复刻;循环只能经 break/return 正常退出。
    fn find_index_set(&mut self, rule: &str) -> Result<(), DslError> {
        let rus: Vec<char> = rule.trim_matches(|c: char| c <= ' ').chars().collect();

        let mut len = rus.len() as i64;
        let mut cur_minus = false;
        let mut cur_list: Vec<Option<i64>> = Vec::new();
        let mut l = String::new();

        let parse_int = |l: &str, minus: bool| -> Result<i64, DslError> {
            let v: i64 = l.parse().map_err(|_| DslError(format!("坏索引数字 {l:?}")))?;
            Ok(if minus { -v } else { v })
        };

        let head = *rus.last().ok_or_else(|| DslError("空规则".into()))? == ']';

        if head {
            len -= 1; // 跳过尾部 ']'
            'outer: loop {
                len -= 1;
                if len < 0 {
                    return Err(DslError("索引扫描越界(对应 Kotlin 越界异常)".into()));
                }
                let mut rl = rus[len as usize];
                if rl == ' ' {
                    continue;
                }
                if rl.is_ascii_digit() {
                    l.insert(0, rl);
                } else if rl == '-' {
                    cur_minus = true;
                } else {
                    let cur_int: Option<i64> =
                        if l.is_empty() { None } else { Some(parse_int(&l, cur_minus)?) };
                    match rl {
                        ':' => cur_list.push(cur_int),
                        _ => {
                            if cur_list.is_empty() {
                                let Some(ci) = cur_int else { break 'outer };
                                self.indexes.push(IndexItem::Single(ci));
                            } else {
                                let end = *cur_list.last().unwrap();
                                let step = if cur_list.len() == 2 {
                                    // Kotlin 此处把 null 塞进 Int 槽位,解构时抛异常
                                    cur_list[0].ok_or_else(|| {
                                        DslError("区间步长为空(对应 Kotlin 解构异常)".into())
                                    })?
                                } else {
                                    1
                                };
                                self.indexes.push(IndexItem::Range(cur_int, end, step));
                                cur_list.clear();
                            }
                            if rl == '!' {
                                self.split = '!';
                                loop {
                                    len -= 1;
                                    if len < 0 {
                                        return Err(DslError(
                                            "索引扫描越界(对应 Kotlin 越界异常)".into(),
                                        ));
                                    }
                                    rl = rus[len as usize];
                                    if !(len > 0 && rl == ' ') {
                                        break;
                                    }
                                }
                            }
                            if rl == '[' {
                                self.before_rule = rus[..len as usize].iter().collect();
                                return Ok(());
                            }
                            if rl != ',' {
                                break 'outer;
                            }
                        }
                    }
                    l.clear();
                    cur_minus = false;
                }
            }
        } else {
            loop {
                len -= 1;
                if len < 0 {
                    return Err(DslError("索引扫描越界(对应 Kotlin 越界异常)".into()));
                }
                let rl = rus[len as usize];
                if rl == ' ' {
                    continue;
                }
                if rl.is_ascii_digit() {
                    l.insert(0, rl);
                } else if rl == '-' {
                    cur_minus = true;
                } else {
                    if rl == '!' || rl == '.' || rl == ':' {
                        // Kotlin 在 l 为空时 "".toInt() 抛异常,忠实复刻
                        self.index_default.push(parse_int(&l, cur_minus)?);
                        if rl != ':' {
                            self.split = rl;
                            self.before_rule = rus[..len as usize].iter().collect();
                            return Ok(());
                        }
                    } else {
                        break;
                    }
                    l.clear();
                    cur_minus = false;
                }
            }
        }

        self.split = ' ';
        self.before_rule = rus.iter().collect();
        Ok(())
    }
}

fn children_elements(n: NRef<'_>) -> Vec<NodeId> {
    n.children().filter(|c| c.value().is_element()).map(|c| c.id()).collect()
}

fn collect_eval(root: NRef<'_>, ev: &Ev) -> Vec<NodeId> {
    select::select(root, ev).into_iter().map(|n| n.id()).collect()
}
