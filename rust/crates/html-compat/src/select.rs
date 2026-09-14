//! jsoup 选择器方言:QueryParser + Evaluator 的移植
//! (select/{QueryParser,Evaluator,StructuralEvaluator}.java、parser/TokenQueue.java)。
//! `:matches`/`[attr~=]` 用 regex-compat 的 Java 正则语义。

use crate::dom::*;
use crate::text as jtext;
use regex_compat::JavaRegex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorParseError(pub String);

impl std::fmt::Display for SelectorParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "selector parse error: {}", self.0)
    }
}
impl std::error::Error for SelectorParseError {}

pub enum Ev {
    Tag(String),
    TagEndsWith(String),
    Id(String),
    Class(String),
    AllElements,
    Attr(String),
    AttrStarting(String),
    AttrVal(String, String),
    AttrValNot(String, String),
    AttrValStarting(String, String),
    AttrValEnding(String, String),
    AttrValContaining(String, String),
    AttrValMatching(String, JavaRegex),
    IndexLt(i64),
    IndexGt(i64),
    IndexEq(i64),
    IsFirstChild,
    IsLastChild,
    IsNthChild(i64, i64),
    IsNthOfType(i64, i64),
    IsNthLastOfType(i64, i64),
    IsOnlyChild,
    IsEmpty,
    IsRoot,
    ContainsText(String),
    ContainsOwnText(String),
    Matches(JavaRegex),
    MatchesOwn(JavaRegex),
    And(Vec<Ev>),
    Or(Vec<Ev>),
    Root,
    Has(Box<Ev>),
    Not(Box<Ev>),
    Ancestor(Box<Ev>),
    ImmediateParentRun(Vec<Ev>),
    PreviousSibling(Box<Ev>),
    ImmediatePreviousSibling(Box<Ev>),
}

pub fn parse(query: &str) -> Result<Ev, SelectorParseError> {
    QueryParser::new(query).parse()
}

/// Selector.select:root 起(含 root)先序遍历收集匹配元素
pub fn select<'a>(root: NRef<'a>, ev: &Ev) -> Vec<NRef<'a>> {
    let mut out = Vec::new();
    fn walk<'a>(node: NRef<'a>, root: NRef<'a>, ev: &Ev, out: &mut Vec<NRef<'a>>) {
        if is_element_like(node) && ev.matches(root, node) {
            out.push(node);
        }
        for c in node.children() {
            walk(c, root, ev, out);
        }
    }
    walk(root, root, ev, &mut out);
    out
}

impl Ev {
    pub fn matches(&self, root: NRef<'_>, el: NRef<'_>) -> bool {
        use Ev::*;
        match self {
            Tag(t) => normal_name(el) == *t,
            TagEndsWith(t) => normal_name(el).ends_with(t.as_str()),
            Id(id) => attr(el, "id").unwrap_or("") == id,
            Class(c) => has_class(el, c),
            AllElements => true,
            Attr(k) => has_attr(el, k),
            AttrStarting(prefix) => {
                element_attrs(el).any(|(k, _)| k.to_lowercase().starts_with(prefix.as_str()))
            }
            AttrVal(k, v) => attr(el, k).is_some_and(|av| av.trim().eq_ignore_ascii_case(v)),
            AttrValNot(k, v) => !attr(el, k).unwrap_or_default().eq_ignore_ascii_case(v),
            AttrValStarting(k, v) => {
                attr(el, k).is_some_and(|av| av.to_lowercase().starts_with(v.as_str()))
            }
            AttrValEnding(k, v) => {
                attr(el, k).is_some_and(|av| av.to_lowercase().ends_with(v.as_str()))
            }
            AttrValContaining(k, v) => {
                attr(el, k).is_some_and(|av| av.to_lowercase().contains(v.as_str()))
            }
            AttrValMatching(k, re) => {
                attr(el, k).is_some_and(|av| re.find_first(av).is_ok_and(|m| m.is_some()))
            }
            IndexLt(i) => root.id() != el.id() && (element_sibling_index(el) as i64) < *i,
            IndexGt(i) => (element_sibling_index(el) as i64) > *i,
            IndexEq(i) => (element_sibling_index(el) as i64) == *i,
            IsFirstChild => parent_non_doc(el)
                .is_some_and(|p| first_element_child(p).is_some_and(|f| f.id() == el.id())),
            IsLastChild => parent_non_doc(el)
                .is_some_and(|p| last_element_child(p).is_some_and(|f| f.id() == el.id())),
            IsNthChild(a, b) => nth_matches(el, *a, *b, |el| element_sibling_index(el) as i64 + 1),
            IsNthOfType(a, b) => nth_matches(el, *a, *b, |el| {
                let Some(p) = el.parent() else { return 0 };
                let name = normal_name(el);
                let mut pos = 0;
                for c in p.children() {
                    if normal_name(c) == name {
                        pos += 1;
                    }
                    if c.id() == el.id() {
                        break;
                    }
                }
                pos
            }),
            IsNthLastOfType(a, b) => nth_matches(el, *a, *b, |el| {
                let name = normal_name(el);
                let mut pos = 0;
                let mut next = Some(el);
                while let Some(n) = next {
                    if normal_name(n) == name {
                        pos += 1;
                    }
                    next = next_element_sibling(n);
                }
                pos
            }),
            IsOnlyChild => parent_non_doc(el)
                .is_some_and(|p| p.children().filter(|c| c.value().is_element()).count() == 1),
            IsEmpty => {
                for n in el.children() {
                    if is_text(n) {
                        // jsoup 怪癖:首个 TextNode 直接决定结果
                        return is_blank_text(n);
                    }
                    if !matches!(n.value(), scraper::Node::Comment(_) | scraper::Node::Doctype(_)) {
                        return false;
                    }
                }
                true
            }
            IsRoot => {
                let r = if matches!(root.value(), scraper::Node::Document) {
                    first_element_child(root)
                } else {
                    Some(root)
                };
                r.is_some_and(|r| r.id() == el.id())
            }
            ContainsText(t) => jtext::text(el).to_lowercase().contains(t.as_str()),
            ContainsOwnText(t) => jtext::own_text(el).to_lowercase().contains(t.as_str()),
            Matches(re) => re.find_first(&jtext::text(el)).is_ok_and(|m| m.is_some()),
            MatchesOwn(re) => re.find_first(&jtext::own_text(el)).is_ok_and(|m| m.is_some()),
            And(evs) => evs.iter().all(|e| e.matches(root, el)),
            Or(evs) => evs.iter().any(|e| e.matches(root, el)),
            Root => root.id() == el.id(),
            Has(inner) => {
                // 在子孙(不含自身)中找首个匹配
                fn find(n: NRef<'_>, root: NRef<'_>, ev: &Ev) -> bool {
                    if is_element_like(n) && ev.matches(root, n) {
                        return true;
                    }
                    n.children().any(|c| find(c, root, ev))
                }
                el.children().filter(|c| c.value().is_element()).any(|c| find(c, el, inner))
            }
            Not(inner) => !inner.matches(root, el),
            Ancestor(inner) => {
                if root.id() == el.id() {
                    return false;
                }
                let mut p = parent_element(el);
                while let Some(parent) = p {
                    if inner.matches(root, parent) {
                        return true;
                    }
                    if parent.id() == root.id() {
                        break;
                    }
                    p = parent_element(parent);
                }
                false
            }
            ImmediateParentRun(evs) => {
                let mut cur = Some(el);
                for ev in evs.iter().rev() {
                    let Some(e) = cur else { return false };
                    if !ev.matches(root, e) {
                        return false;
                    }
                    cur = parent_element(e);
                }
                true
            }
            PreviousSibling(inner) => {
                if root.id() == el.id() {
                    return false;
                }
                // 从首个元素兄弟到自身之前
                let Some(p) = el.parent() else { return false };
                for sib in p.children().filter(|c| c.value().is_element()) {
                    if sib.id() == el.id() {
                        break;
                    }
                    if inner.matches(root, sib) {
                        return true;
                    }
                }
                false
            }
            ImmediatePreviousSibling(inner) => {
                if root.id() == el.id() {
                    return false;
                }
                prev_element_sibling(el).is_some_and(|p| inner.matches(root, p))
            }
        }
    }
}

fn parent_non_doc(el: NRef<'_>) -> Option<NRef<'_>> {
    el.parent().filter(|p| p.value().is_element())
}

fn nth_matches(el: NRef<'_>, a: i64, b: i64, pos_fn: impl Fn(NRef<'_>) -> i64) -> bool {
    if parent_non_doc(el).is_none() {
        return false;
    }
    let pos = pos_fn(el);
    if a == 0 {
        return pos == b;
    }
    (pos - b) * a >= 0 && (pos - b) % a == 0
}

fn element_attrs<'a>(el: NRef<'a>) -> impl Iterator<Item = (&'a str, &'a str)> {
    el.value().as_element().into_iter().flat_map(|e| e.attrs())
}

/// jsoup Element.hasClass:类名比较忽略大小写,外加一条**等长快路**
/// (`class="a b"` 上 `hasClass("a b")` 为真)。口径只有一份 ——
/// 见 `rubato_core::host::jsoup_has_class` 的注释与它的探针出处。
fn has_class(el: NRef<'_>, class_name: &str) -> bool {
    let Some(class_attr) = attr(el, "class") else { return false };
    rubato_core::host::jsoup_has_class(class_attr, class_name)
}

// ---------- TokenQueue + QueryParser ----------

const ESC: char = '\\';

struct Tq {
    chars: Vec<char>,
    pos: usize,
}

impl Tq {
    fn new(s: &str) -> Self {
        Self { chars: s.chars().collect(), pos: 0 }
    }

    fn is_empty(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn consume(&mut self) -> char {
        let c = self.chars[self.pos];
        self.pos += 1;
        c
    }

    fn matches(&self, seq: &str) -> bool {
        let s: Vec<char> = seq.chars().collect();
        self.pos + s.len() <= self.chars.len()
            && self.chars[self.pos..self.pos + s.len()]
                .iter()
                .zip(&s)
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
    }

    fn matches_any(&self, seqs: &[&str]) -> bool {
        seqs.iter().any(|s| self.matches(s))
    }

    fn matches_any_char(&self, chars: &[char]) -> bool {
        self.peek().is_some_and(|c| chars.contains(&c))
    }

    fn match_chomp(&mut self, seq: &str) -> bool {
        if self.matches(seq) {
            self.pos += seq.chars().count();
            true
        } else {
            false
        }
    }

    fn consume_whitespace(&mut self) -> bool {
        let mut seen = false;
        while self.peek().is_some_and(is_jsoup_whitespace) {
            self.pos += 1;
            seen = true;
        }
        seen
    }

    fn matches_word(&self) -> bool {
        self.peek().is_some_and(|c| c.is_alphanumeric())
    }

    fn remainder(&mut self) -> String {
        let s: String = self.chars[self.pos..].iter().collect();
        self.pos = self.chars.len();
        s
    }

    /// TokenQueue.chompBalanced(含引号与 \Q..\E 处理)
    fn chomp_balanced(&mut self, open: char, close: char) -> Result<String, SelectorParseError> {
        let mut start = -1i64;
        let mut end = -1i64;
        let mut depth = 0i64;
        let mut last = '\0';
        let mut in_single = false;
        let mut in_double = false;
        let mut in_regex_qe = false;

        loop {
            if self.is_empty() {
                break;
            }
            let c = self.consume();
            if last != ESC {
                if c == '\'' && c != open && !in_double {
                    in_single = !in_single;
                } else if c == '"' && c != open && !in_single {
                    in_double = !in_double;
                }
                if in_single || in_double || in_regex_qe {
                    last = c;
                    if depth > 0 {
                        continue;
                    }
                    break;
                }
                if c == open {
                    depth += 1;
                    if start == -1 {
                        start = self.pos as i64;
                    }
                } else if c == close {
                    depth -= 1;
                }
            } else if c == 'Q' {
                in_regex_qe = true;
            } else if c == 'E' {
                in_regex_qe = false;
            }
            if depth > 0 && last != '\0' {
                end = self.pos as i64;
            }
            last = c;
            if depth <= 0 {
                break;
            }
        }
        let out: String = if end >= 0 {
            self.chars[start as usize..end as usize].iter().collect()
        } else {
            String::new()
        };
        if depth > 0 {
            return Err(SelectorParseError(format!("未闭合的 {open}")));
        }
        Ok(out)
    }

    fn consume_escaped_css_identifier(&mut self, extra: &[&str]) -> String {
        let start = self.pos;
        let mut escaped = false;
        while !self.is_empty() {
            if self.peek() == Some(ESC) && self.chars.len() - self.pos > 1 {
                escaped = true;
                self.pos += 2;
            } else if self.matches_word() || self.matches_any(extra) {
                if self.matches_any(extra) {
                    // 逐项吃掉匹配的前缀(如 "*|")
                    let m = extra.iter().find(|s| self.matches(s)).unwrap();
                    self.pos += m.chars().count();
                } else {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
        let consumed: String = self.chars[start..self.pos].iter().collect();
        if escaped { unescape(&consumed) } else { consumed }
    }

    fn consume_css_identifier(&mut self) -> String {
        self.consume_escaped_css_identifier(&["-", "_"])
    }

    fn consume_element_selector(&mut self) -> String {
        self.consume_escaped_css_identifier(&["*|", "|", "_", "-"])
    }

    fn consume_to_any(&mut self, seqs: &[&str]) -> String {
        let start = self.pos;
        while !self.is_empty() && !self.matches_any(seqs) {
            self.pos += 1;
        }
        self.chars[start..self.pos].iter().collect()
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut last = '\0';
    for mut c in s.chars() {
        if c == ESC {
            if last == ESC {
                out.push(c);
                c = '\0';
            }
        } else {
            out.push(c);
        }
        last = c;
    }
    out
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
}

const ATTRIBUTE_EVALS: &[&str] = &["=", "!=", "^=", "$=", "*=", "~="];

struct QueryParser {
    tq: Tq,
    query: String,
    evals: Vec<Ev>,
}

impl QueryParser {
    fn new(query: &str) -> Self {
        Self { tq: Tq::new(query.trim()), query: query.trim().to_string(), evals: Vec::new() }
    }

    fn parse(mut self) -> Result<Ev, SelectorParseError> {
        if self.query.is_empty() {
            return Err(SelectorParseError("空选择器".into()));
        }
        self.tq.consume_whitespace();
        if self.tq.matches_any_char(&[',', '>', '+', '~', ' ']) {
            self.evals.push(Ev::Root);
            let c = self.tq.consume();
            self.combinator(c)?;
        } else {
            let e = self.consume_evaluator()?;
            self.evals.push(e);
        }

        while !self.tq.is_empty() {
            let seen_white = self.tq.consume_whitespace();
            if self.tq.matches_any_char(&[',', '>', '+', '~']) {
                let c = self.tq.consume();
                self.combinator(c)?;
            } else if seen_white {
                self.combinator(' ')?;
            } else {
                let e = self.consume_evaluator()?;
                self.evals.push(e);
            }
        }

        if self.evals.len() == 1 {
            return Ok(self.evals.pop().unwrap());
        }
        Ok(Ev::And(std::mem::take(&mut self.evals)))
    }

    fn combinator(&mut self, combinator: char) -> Result<(), SelectorParseError> {
        self.tq.consume_whitespace();
        let sub_query = self.consume_sub_query();
        let new_eval = parse(&sub_query)?;
        let mut replace_right_most = false;

        let mut current: Ev;
        if self.evals.len() == 1 {
            let root = self.evals.pop().unwrap();
            // 保证 OR(,)的优先级
            if let Ev::Or(mut list) = root {
                if combinator != ',' {
                    current = list.pop().ok_or_else(|| SelectorParseError("空 Or".into()))?;
                    replace_right_most = true;
                    self.evals.push(Ev::Or(list)); // 暂存剩余部分
                } else {
                    current = Ev::Or(list);
                }
            } else {
                current = root;
            }
        } else {
            current = Ev::And(std::mem::take(&mut self.evals));
        }

        current = match combinator {
            '>' => {
                if let Ev::ImmediateParentRun(mut run) = current {
                    run.push(new_eval);
                    Ev::ImmediateParentRun(run)
                } else {
                    Ev::ImmediateParentRun(vec![current, new_eval])
                }
            }
            // **右边那个先判**。`And` 是 `.all()`,而这些匹配器都是纯谓词,
            // 换序不改结果 —— 改的是代价:`select` 对**每个节点**调一次 `matches`,
            // 把 `Ancestor` 放前面等于每个节点都先爬一遍祖先链,而祖先链里那个
            // `inner` 自己可能又是一条 `Ancestor` → 每节点 O(深度^层数)。
            // 裁判(jsoup)是先拿最右那个简单选择器把 99% 的节点当场刷掉。
            // 实测 `.panel-body tbody tr` 在 51 KB 的 wiki 文档上:裁判 27µs,
            // 换序前 3119µs(见 docs/engine-perf.md §3③)。
            ' ' => Ev::And(vec![new_eval, Ev::Ancestor(Box::new(current))]),
            '+' => Ev::And(vec![new_eval, Ev::ImmediatePreviousSibling(Box::new(current))]),
            '~' => Ev::And(vec![new_eval, Ev::PreviousSibling(Box::new(current))]),
            ',' => {
                if let Ev::Or(mut list) = current {
                    list.push(new_eval);
                    Ev::Or(list)
                } else {
                    Ev::Or(vec![current, new_eval])
                }
            }
            other => {
                return Err(SelectorParseError(format!("未知组合符 {other}")));
            }
        };

        if replace_right_most {
            // 取回暂存的 Or,把新的 current 放回其最右
            if let Some(Ev::Or(mut list)) = self.evals.pop() {
                list.push(current);
                self.evals.push(Ev::Or(list));
            } else {
                return Err(SelectorParseError("内部错误:Or 缺失".into()));
            }
        } else {
            self.evals.clear();
            self.evals.push(current);
        }
        Ok(())
    }

    fn consume_sub_query(&mut self) -> String {
        let mut sq = String::new();
        while !self.tq.is_empty() {
            if self.tq.matches("(") {
                sq.push('(');
                sq.push_str(&self.tq.chomp_balanced('(', ')').unwrap_or_default());
                sq.push(')');
            } else if self.tq.matches("[") {
                sq.push('[');
                sq.push_str(&self.tq.chomp_balanced('[', ']').unwrap_or_default());
                sq.push(']');
            } else if self.tq.matches_any_char(&[',', '>', '+', '~', ' ']) {
                if !sq.is_empty() {
                    break;
                }
                self.tq.consume();
            } else {
                sq.push(self.tq.consume());
            }
        }
        sq
    }

    fn consume_evaluator(&mut self) -> Result<Ev, SelectorParseError> {
        if self.tq.match_chomp("#") {
            let id = self.tq.consume_css_identifier();
            if id.is_empty() {
                return Err(SelectorParseError("空 id".into()));
            }
            Ok(Ev::Id(id))
        } else if self.tq.match_chomp(".") {
            let class = self.tq.consume_css_identifier();
            if class.is_empty() {
                return Err(SelectorParseError("空 class".into()));
            }
            Ok(Ev::Class(class.trim().to_string()))
        } else if self.tq.matches_word() || self.tq.matches("*|") {
            let mut tag = normalize(&self.tq.consume_element_selector());
            if tag.is_empty() {
                return Err(SelectorParseError("空标签".into()));
            }
            if let Some(plain) = tag.strip_prefix("*|") {
                Ok(Ev::Or(vec![Ev::Tag(plain.to_string()), Ev::TagEndsWith(format!(":{plain}"))]))
            } else {
                if tag.contains('|') {
                    tag = tag.replace('|', ":");
                }
                Ok(Ev::Tag(tag))
            }
        } else if self.tq.matches("[") {
            self.by_attribute()
        } else if self.tq.match_chomp("*") {
            Ok(Ev::AllElements)
        } else if self.tq.match_chomp(":") {
            self.parse_pseudo_selector()
        } else {
            Err(SelectorParseError(format!(
                "无法解析选择器 '{}'(位于 '{}')",
                self.query,
                self.tq.remainder()
            )))
        }
    }

    fn by_attribute(&mut self) -> Result<Ev, SelectorParseError> {
        // chompBalanced 自己消费起始的 '['
        let content = self.tq.chomp_balanced('[', ']')?;
        let mut cq = Tq::new(&content);
        let key = cq.consume_to_any(ATTRIBUTE_EVALS);
        if key.is_empty() {
            return Err(SelectorParseError("空属性名".into()));
        }
        cq.consume_whitespace();
        if cq.is_empty() {
            if let Some(prefix) = key.strip_prefix('^') {
                Ok(Ev::AttrStarting(normalize(prefix)))
            } else {
                Ok(Ev::Attr(normalize(&key)))
            }
        } else if cq.match_chomp("!=") {
            Ok(Ev::AttrValNot(normalize(&key), self.attr_value_checked(&mut cq)?))
        } else if cq.match_chomp("^=") {
            Ok(Ev::AttrValStarting(normalize(&key), self.attr_value_checked(&mut cq)?))
        } else if cq.match_chomp("$=") {
            Ok(Ev::AttrValEnding(normalize(&key), self.attr_value_checked(&mut cq)?))
        } else if cq.match_chomp("*=") {
            Ok(Ev::AttrValContaining(normalize(&key), self.attr_value_checked(&mut cq)?))
        } else if cq.match_chomp("~=") {
            // `~=` 这一支底下是 `AttributeWithValueMatching`,**不走** AttributeKeyPair
            // 的 notEmpty —— jsoup 里 `[href~=]` 是合法的(空正则匹配一切)
            let re = JavaRegex::compile(&cq.remainder())
                .map_err(|e| SelectorParseError(format!("属性正则: {e}")))?;
            Ok(Ev::AttrValMatching(normalize(&key), re))
        } else if cq.match_chomp("=") {
            Ok(Ev::AttrVal(normalize(&key), self.attr_value_checked(&mut cq)?))
        } else {
            Err(SelectorParseError(format!("无法解析属性选择器 [{content}]")))
        }
    }

    /// 带值的属性选择器(`=` / `!=` / `^=` / `$=` / `*=`)的值。
    ///
    /// **值不许为空**:jsoup 里这五支底下都是 `Evaluator.AttributeKeyPair`,
    /// 构造头两句是 `Validate.notEmpty(key); Validate.notEmpty(value);` ——
    /// `a[href$=]` 抛 ValidationException,而 `QueryParser.parse` 把
    /// IllegalArgumentException 一律重抛成 SelectorParseException。
    /// 书源拼出来的选择器真的会踩到(js-corpus-5175e212a0:`book.tocUrl` 是空串,
    /// `"a[href$="+tocUrl+"]"` 就成了 `a[href$=]`,整段当场中断)。
    /// 注意 notEmpty 看的是**去引号之前**的原串,而且只判长度、不 trim。
    fn attr_value_checked(&self, cq: &mut Tq) -> Result<String, SelectorParseError> {
        let raw = cq.remainder();
        if raw.is_empty() {
            return Err(SelectorParseError("属性值不能为空".into()));
        }
        Ok(attr_value(&raw))
    }

    fn parse_pseudo_selector(&mut self) -> Result<Ev, SelectorParseError> {
        let pseudo = self.tq.consume_css_identifier();
        match pseudo.as_str() {
            "lt" => Ok(Ev::IndexLt(self.consume_index()?)),
            "gt" => Ok(Ev::IndexGt(self.consume_index()?)),
            "eq" => Ok(Ev::IndexEq(self.consume_index()?)),
            "has" => {
                let sub = self.consume_parens()?;
                if sub.is_empty() {
                    return Err(SelectorParseError(":has(selector) 不能为空".into()));
                }
                Ok(Ev::Has(Box::new(parse(&sub)?)))
            }
            "contains" => Ok(Ev::ContainsText(self.consume_search_text(":contains")?)),
            "containsOwn" => Ok(Ev::ContainsOwnText(self.consume_search_text(":containsOwn")?)),
            "matches" => Ok(Ev::Matches(self.consume_regex()?)),
            "matchesOwn" => Ok(Ev::MatchesOwn(self.consume_regex()?)),
            "not" => {
                let sub = self.consume_parens()?;
                if sub.is_empty() {
                    return Err(SelectorParseError(":not(selector) 不能为空".into()));
                }
                Ok(Ev::Not(Box::new(parse(&sub)?)))
            }
            "nth-child" => self.css_nth_child(Ev::IsNthChild),
            "nth-of-type" => self.css_nth_child(Ev::IsNthOfType),
            "nth-last-of-type" => self.css_nth_child(Ev::IsNthLastOfType),
            // :first-child 在 jsoup 是独立 Evaluator(不是 nth-child 的 (0,1) 特例),
            // 语义一致 —— 这里直接给 IsFirstChild
            "first-child" => Ok(Ev::IsFirstChild),
            "last-child" => Ok(Ev::IsLastChild),
            "first-of-type" => Ok(Ev::IsNthOfType(0, 1)),
            "last-of-type" => Ok(Ev::IsNthLastOfType(0, 1)),
            "only-child" => Ok(Ev::IsOnlyChild),
            "empty" => Ok(Ev::IsEmpty),
            "root" => Ok(Ev::IsRoot),
            // jsoup 还有 :containsWholeText/:containsWholeOwnText/:containsData/
            // :matchesWholeText/:matchesWholeOwnText/:only-of-type/:nth-last-child ——
            // 语料零命中,不移植,统一落这条「未知伪类」错误分支
            other => Err(SelectorParseError(format!("未知伪类 :{other}"))),
        }
    }

    fn consume_parens(&mut self) -> Result<String, SelectorParseError> {
        self.tq.chomp_balanced('(', ')')
    }

    fn consume_index(&mut self) -> Result<i64, SelectorParseError> {
        let s = self.consume_parens()?;
        s.trim().parse().map_err(|_| SelectorParseError("索引必须是数字".into()))
    }

    fn consume_search_text(&mut self, name: &str) -> Result<String, SelectorParseError> {
        let t = unescape(&self.consume_parens()?);
        if t.is_empty() {
            return Err(SelectorParseError(format!("{name}(text) 不能为空")));
        }
        // ContainsText/ContainsOwnText 构造:lowerCase(normaliseWhitespace(text))
        let mut norm = String::new();
        crate::dom::append_normalised_whitespace(&mut norm, &t, false);
        Ok(norm.to_lowercase())
    }

    fn consume_regex(&mut self) -> Result<JavaRegex, SelectorParseError> {
        let r = self.consume_parens()?;
        if r.is_empty() {
            return Err(SelectorParseError(":matches(regex) 不能为空".into()));
        }
        JavaRegex::compile(&r).map_err(|e| SelectorParseError(format!("正则: {e}")))
    }

    fn css_nth_child(&mut self, make: fn(i64, i64) -> Ev) -> Result<Ev, SelectorParseError> {
        let arg = normalize(&self.consume_parens()?);
        let (a, b) = parse_nth(&arg)?;
        Ok(make(a, b))
    }
}

fn attr_value(raw: &str) -> String {
    // AttributeKeyPair:剥引号;字面量不 trim,否则 trim;统一小写
    let is_literal = (raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2)
        || (raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2);
    let v = if is_literal { &raw[1..raw.len() - 1] } else { raw };
    if is_literal { v.to_lowercase() } else { v.trim().to_lowercase() }
}

/// jsoup NTH_AB / NTH_B 解析((an+b)、odd、even、纯数字)
fn parse_nth(arg: &str) -> Result<(i64, i64), SelectorParseError> {
    if arg == "odd" {
        return Ok((2, 1));
    }
    if arg == "even" {
        return Ok((2, 0));
    }
    // (([+-])?(\d+)?)n(\s*([+-])?\s*\d+)? 忽略大小写
    let lower = arg.to_lowercase();
    if let Some(n_pos) = lower.find('n') {
        let (a_part, rest) = lower.split_at(n_pos);
        let rest = &rest[1..];
        let a_part = a_part.trim();
        let a = if a_part.is_empty() || a_part == "+" {
            1
        } else if a_part == "-" {
            -1
        } else {
            a_part
                .trim_start_matches('+')
                .parse()
                .map_err(|_| SelectorParseError(format!("坏的 nth 参数 {arg}")))?
        };
        let rest = rest.replace(char::is_whitespace, "");
        let b = if rest.is_empty() {
            0
        } else {
            rest.trim_start_matches('+')
                .parse()
                .map_err(|_| SelectorParseError(format!("坏的 nth 参数 {arg}")))?
        };
        Ok((a, b))
    } else {
        let b = arg
            .trim_start_matches('+')
            .parse()
            .map_err(|_| SelectorParseError(format!("坏的 nth 参数 {arg}")))?;
        Ok((0, b))
    }
}
