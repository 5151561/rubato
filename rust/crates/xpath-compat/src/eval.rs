//! `org.seimicrawler.xpath.core.XpathProcessor` + `Scanner` 注册的
//! 14 个轴 / 6 个节点测试 / 17 个函数 + `CommonUtil` 的逐方法移植。
//!
//! 求值器保留了真身的**可变 scope 栈**模型(而不是改写成纯函数):
//! `visitStep` 会就地改当前 scope 的 context,`visitPredicate` 每个元素压一层,
//! `last()` / `position()` 读的是 `scope.getParent()` 的 context —— 这些都靠
//! 栈的形状说话,换成别的结构就对不上了。

use crate::parse::*;
use crate::tree::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct XError(pub String);

pub type XResult<T> = Result<T, XError>;

fn err<T>(m: &str) -> XResult<T> {
    Err(XError(m.to_string()))
}

// ---------------------------------------------------------------- XValue

#[derive(Debug, Clone, PartialEq)]
pub enum Num {
    Long(i64),
    Int(i32),
    Double(f64),
}

#[derive(Debug, Clone)]
pub enum XValue {
    Elements(Vec<XEl>),
    Str(String),
    List(Vec<String>),
    Num(Num),
    Bool(bool),
    /// `XValue.create(null)`(num() 抽不到数字那条)
    Null,
    /// `Sum` 那条 `return null`:processor.visit 直接给 Java null,
    /// `JXDocument.selN` 走 "calRes == null" 那支
    NullReturn,
    /// `@` / `attribute::` 给的 `XValue.create(null).attr()`
    Attr,
}

/// `valType()` 的等价物:visitEqualityExpr 拿它比"是不是同一个 Java 类"
#[derive(PartialEq, Debug)]
enum VType {
    Elements,
    Str,
    List,
    Long,
    Int,
    Double,
    Bool,
    Object,
}

impl XValue {
    fn vtype(&self) -> VType {
        match self {
            XValue::Elements(_) => VType::Elements,
            XValue::Str(_) => VType::Str,
            XValue::List(_) => VType::List,
            XValue::Num(Num::Long(_)) => VType::Long,
            XValue::Num(Num::Int(_)) => VType::Int,
            XValue::Num(Num::Double(_)) => VType::Double,
            XValue::Bool(_) => VType::Bool,
            XValue::Null | XValue::Attr | XValue::NullReturn => VType::Object,
        }
    }

    fn is_number(&self) -> bool {
        matches!(self, XValue::Num(_))
    }

    /// `Objects.equals(left, right)` —— XValue.equals 比的是内层 value
    fn same_value(&self, o: &XValue) -> bool {
        match (self, o) {
            (XValue::Elements(a), XValue::Elements(b)) => a == b,
            (XValue::Str(a), XValue::Str(b)) => a == b,
            (XValue::List(a), XValue::List(b)) => a == b,
            (XValue::Num(a), XValue::Num(b)) => a == b,
            (XValue::Bool(a), XValue::Bool(b)) => a == b,
            (
                XValue::Null | XValue::Attr | XValue::NullReturn,
                XValue::Null | XValue::Attr | XValue::NullReturn,
            ) => true,
            _ => false,
        }
    }
}

/// Java `Double.toString`。本体在 `rubato_core::java_num` —— 全仓唯一一份。
///
/// 这里曾自己写过一份,漏了**科学计数**那一支(`1e7` 给 `"10000000.0"`,
/// Java 给 `"1.0E7"`);`num()` 抽到 8 位以上的整数、或 `//td/num() + 0`
/// 这类走 `visit_add` 的写法都会分岔,而探针恰好只抽到 `1` / `12.3`。
pub use rubato_core::java_double_to_string;

impl<'a> Proc<'a> {
    /// `XValue.asString()`
    fn as_string(&self, v: &XValue) -> String {
        match v {
            XValue::Elements(els) => {
                let mut s = String::new();
                for &e in els {
                    s.push_str(&self.t.own_text(e));
                }
                s
            }
            XValue::List(l) => l.join(","),
            // 其余走 String.valueOf(value).trim()
            XValue::Str(s) => s.trim_matches(|c: char| c <= ' ').to_string(),
            XValue::Num(Num::Long(n)) => n.to_string(),
            XValue::Num(Num::Int(n)) => n.to_string(),
            XValue::Num(Num::Double(d)) => java_double_to_string(*d),
            XValue::Bool(b) => b.to_string(),
            XValue::Null | XValue::Attr | XValue::NullReturn => "null".to_string(),
        }
    }

    /// `XValue.asBoolean()`
    fn as_bool(&self, v: &XValue) -> bool {
        match v {
            XValue::Bool(b) => *b,
            // value == null → false;否则看 asString() 是不是空白
            XValue::Null | XValue::Attr | XValue::NullReturn => false,
            _ => !self.as_string(v).trim().is_empty(),
        }
    }

    /// `XValue.asDouble()`:字符串走 BigDecimal(解析失败抛)
    fn as_double(&self, v: &XValue) -> XResult<f64> {
        match v {
            XValue::Num(Num::Long(n)) => Ok(*n as f64),
            XValue::Num(Num::Int(n)) => Ok(*n as f64),
            XValue::Num(Num::Double(d)) => Ok(*d),
            XValue::Str(s) => parse_big_decimal(s).ok_or_else(|| XError("NumberFormat".into())),
            _ => err("cast to number fail"),
        }
    }

    /// `XValue.asLong()`:BigDecimal.setScale(0, HALF_UP).longValue()
    fn as_long(&self, v: &XValue) -> XResult<i64> {
        match v {
            XValue::Num(Num::Long(n)) => Ok(*n),
            XValue::Num(Num::Int(n)) => Ok(*n as i64),
            XValue::Num(Num::Double(d)) => Ok(*d as i64),
            XValue::Str(s) => {
                let d = parse_big_decimal(s).ok_or_else(|| XError("NumberFormat".into()))?;
                Ok(half_up(d) as i64)
            }
            _ => err("cast to number fail"),
        }
    }

    /// `XValue.compareTo`
    fn compare(&self, a: &XValue, b: &XValue) -> XResult<std::cmp::Ordering> {
        use std::cmp::Ordering;
        if a.same_value(b) {
            return Ok(Ordering::Equal);
        }
        if matches!(b, XValue::Null | XValue::Attr | XValue::NullReturn) {
            return Ok(Ordering::Greater);
        }
        if matches!(a, XValue::Null | XValue::Attr | XValue::NullReturn) {
            return Ok(Ordering::Less);
        }
        if matches!(a, XValue::Str(_)) {
            // Java String.compareTo:按 UTF-16 码元
            let (x, y) = (self.as_string(a), self.as_string(b));
            return Ok(java_str_cmp(&x, &y));
        }
        if a.is_number() {
            let (x, y) = (self.as_double(a)?, self.as_double(b)?);
            return Ok(x.partial_cmp(&y).unwrap_or(Ordering::Equal));
        }
        err("Unsupported comparable XValue")
    }
}

fn java_str_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `new BigDecimal(str)` 能吃下的形状(比 Rust 的 f64::from_str 严:
/// 不接受 `Infinity` / `NaN` / 前后空白 / 下划线)
fn parse_big_decimal(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    let b = s.as_bytes();
    let mut i = 0;
    if b[i] == b'+' || b[i] == b'-' {
        i += 1;
    }
    let mut digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let mut ed = 0;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            ed += 1;
        }
        if ed == 0 {
            return None;
        }
    }
    if i != b.len() {
        return None;
    }
    s.parse::<f64>().ok()
}

fn half_up(d: f64) -> f64 {
    if d >= 0.0 { (d + 0.5).floor() } else { -((-d + 0.5).floor()) }
}

// ---------------------------------------------------------------- Scope

struct Scope {
    context: Vec<XEl>,
    recursion: bool,
    parent: Option<usize>,
}

pub struct Proc<'a> {
    t: Tree<'a>,
    scopes: Vec<Scope>,
    stack: Vec<usize>,
    root: usize,
}

impl<'a> Proc<'a> {
    pub fn new(tree: Tree<'a>, root_ctx: Vec<XEl>) -> Self {
        let scopes = vec![
            Scope { context: root_ctx.clone(), recursion: false, parent: None },
            Scope { context: root_ctx, recursion: false, parent: Some(0) },
        ];
        Proc { t: tree, scopes, stack: vec![1], root: 0 }
    }

    pub fn tree(&self) -> &Tree<'a> {
        &self.t
    }

    fn cur(&self) -> usize {
        *self.stack.last().expect("scope 栈非空")
    }
    fn ctx(&self) -> Vec<XEl> {
        self.scopes[self.cur()].context.clone()
    }
    fn set_ctx(&mut self, v: Vec<XEl>) {
        let c = self.cur();
        self.scopes[c].context = v;
    }
    fn is_recursion(&self) -> bool {
        self.scopes[self.cur()].recursion
    }
    fn set_recursion(&mut self, r: bool) {
        let c = self.cur();
        self.scopes[c].recursion = r;
    }
    fn single_el(&self) -> XResult<XEl> {
        let c = &self.scopes[self.cur()].context;
        if c.len() == 1 { Ok(c[0]) } else { err("current context is more than one el") }
    }
    /// `Scope.create(scope)`:抄 context,parent 指向它
    fn push_from(&mut self, from: usize) {
        let ctx = self.scopes[from].context.clone();
        self.scopes.push(Scope { context: ctx, recursion: false, parent: Some(from) });
        let i = self.scopes.len() - 1;
        self.stack.push(i);
    }
    fn push_ctx(&mut self, ctx: Vec<XEl>, parent: usize) {
        self.scopes.push(Scope { context: ctx, recursion: false, parent: Some(parent) });
        let i = self.scopes.len() - 1;
        self.stack.push(i);
    }
    fn pop(&mut self) {
        self.stack.pop();
    }

    // ------------------------------------------------------------ 表达式

    pub fn visit_or(&mut self, e: &OrExpr) -> XResult<XValue> {
        self.visit_bool_chain(&e.0, true, Self::visit_and)
    }

    fn visit_and(&mut self, e: &AndExpr) -> XResult<XValue> {
        self.visit_bool_chain(&e.0, false, Self::visit_eq)
    }

    /// or/and 共用的骨架:单支透传,多支逐支求值后按位并——
    /// 真身用的是 `|`/`&` 而不是 `||`/`&&`,每一支都求值,没有短路
    fn visit_bool_chain<T>(
        &mut self,
        items: &[T],
        is_or: bool,
        visit: fn(&mut Self, &T) -> XResult<XValue>,
    ) -> XResult<XValue> {
        if items.len() == 1 {
            return visit(self, &items[0]);
        }
        let v = visit(self, &items[0])?;
        let mut res = self.as_bool(&v);
        for a in &items[1..] {
            let v = visit(self, a)?;
            let b = self.as_bool(&v);
            if is_or {
                res |= b;
            } else {
                res &= b;
            }
        }
        Ok(XValue::Bool(res))
    }

    fn visit_eq(&mut self, e: &EqualityExpr) -> XResult<XValue> {
        if e.items.len() == 1 {
            return self.visit_rel(&e.items[0]);
        }
        if e.items.len() == 2 {
            let l = self.visit_rel(&e.items[0])?;
            let r = self.visit_rel(&e.items[1])?;
            let same = if l.vtype() == r.vtype() {
                l.same_value(&r)
            } else {
                self.as_string(&l) == self.as_string(&r)
            };
            return Ok(XValue::Bool(if e.op == Some(EqOp::Eq) { same } else { !same }));
        }
        err("error equalityExpr")
    }

    fn visit_rel(&mut self, e: &RelationalExpr) -> XResult<XValue> {
        if e.items.len() == 1 {
            return self.visit_add(&e.items[0]);
        }
        if e.items.len() == 2 {
            let l = self.visit_add(&e.items[0])?;
            let r = self.visit_add(&e.items[1])?;
            use std::cmp::Ordering::*;
            let b = match e.op.expect("两项必有运算符") {
                RelOp::Lt => self.compare(&l, &r)? == Less,
                RelOp::Gt => self.compare(&l, &r)? == Greater,
                RelOp::Le => self.compare(&l, &r)? != Greater,
                RelOp::Ge => self.compare(&l, &r)? != Less,
                RelOp::StartWith => self.as_string(&l).starts_with(&self.as_string(&r)),
                RelOp::EndWith => self.as_string(&l).ends_with(&self.as_string(&r)),
                RelOp::ContainWith => self.as_string(&l).contains(&self.as_string(&r)),
                RelOp::RegexpWith => java_matches(&self.as_string(&r), &self.as_string(&l))?,
                RelOp::RegexpNotWith => !java_matches(&self.as_string(&r), &self.as_string(&l))?,
            };
            return Ok(XValue::Bool(b));
        }
        err("error relationalExpr")
    }

    fn visit_add(&mut self, e: &AdditiveExpr) -> XResult<XValue> {
        if e.rest.is_empty() {
            return self.visit_mul(&e.first);
        }
        let f = self.visit_mul(&e.first)?;
        let mut res = self.as_double(&f)?;
        for (op, m) in &e.rest {
            let v = self.visit_mul(m)?;
            let d = self.as_double(&v)?;
            match op {
                '+' => res += d,
                '-' => res -= d,
                _ => return err("syntax error"),
            }
        }
        Ok(XValue::Num(Num::Double(res)))
    }

    fn visit_mul(&mut self, e: &MultiplicativeExpr) -> XResult<XValue> {
        let Some((op, right)) = &e.rest else {
            return self.visit_unary(&e.left);
        };
        let l = self.visit_unary(&e.left)?;
        let r = self.visit_mul(right)?;
        let (a, b) = (self.as_double(&l)?, self.as_double(&r)?);
        Ok(XValue::Num(Num::Double(match op {
            MulOp::Mul => a * b,
            MulOp::Div => a / b,
            MulOp::Mod => a % b,
        })))
    }

    fn visit_unary(&mut self, e: &UnaryExpr) -> XResult<XValue> {
        let v = self.visit_union(&e.inner)?;
        if !e.neg {
            return Ok(v);
        }
        Ok(XValue::Num(Num::Double(-self.as_double(&v)?)))
    }

    fn visit_union(&mut self, e: &UnionExpr) -> XResult<XValue> {
        let left = self.visit_path(&e.path)?;
        let Some(rest) = &e.rest else { return Ok(left) };
        // 真身:push(Scope.create(currentScope().getParent())) —— parent 为空就 NPE
        let Some(p) = self.scopes[self.cur()].parent else {
            return err("union: parent scope is null");
        };
        self.push_from(p);
        let right = self.visit_union(rest)?;
        self.pop();
        self.merge_union(left, right)
    }

    /// visitUnionExprNoRoot 的合并分支(标量并进节点集时包一个 `<V>`)
    fn merge_union(&mut self, left: XValue, right: XValue) -> XResult<XValue> {
        match &left {
            XValue::Elements(l) => {
                let mut l = l.clone();
                if let XValue::Elements(r) = &right {
                    l.extend(r.iter().copied());
                } else {
                    let s = self.as_string(&right);
                    let v = self.t.new_synth("V", &s);
                    l.push(v);
                }
                Ok(XValue::Elements(l))
            }
            XValue::Str(_) => {
                if let XValue::Elements(r) = &right {
                    let s = self.as_string(&left);
                    let v = self.t.new_synth("V", &s);
                    let mut r = r.clone();
                    r.push(v);
                    return Ok(XValue::Elements(r));
                }
                Ok(XValue::Str(self.as_string(&left) + &self.as_string(&right)))
            }
            XValue::Bool(b) => match &right {
                XValue::Bool(rb) => Ok(XValue::Bool(*b | *rb)),
                XValue::Elements(r) => {
                    let s = self.as_string(&left);
                    let v = self.t.new_synth("V", &s);
                    let mut r = r.clone();
                    r.push(v);
                    Ok(XValue::Elements(r))
                }
                XValue::Str(_) => Ok(XValue::Str(b.to_string() + &self.as_string(&right))),
                _ => err("can not merge"),
            },
            XValue::Num(_) => match &right {
                XValue::Str(_) => {
                    let d = self.as_double(&left)?;
                    Ok(XValue::Str(java_double_to_string(d) + &self.as_string(&right)))
                }
                XValue::Elements(r) => {
                    let s = self.as_string(&left);
                    let v = self.t.new_synth("V", &s);
                    let mut r = r.clone();
                    r.push(v);
                    Ok(XValue::Elements(r))
                }
                _ => err("can not merge"),
            },
            _ => {
                let mut tmp = Vec::new();
                let ls = self.as_string(&left);
                if !ls.trim().is_empty() {
                    tmp.push(ls);
                }
                let rs = self.as_string(&right);
                if !rs.trim().is_empty() {
                    tmp.push(rs);
                }
                Ok(XValue::Str(tmp.join(",")))
            }
        }
    }

    fn visit_path(&mut self, e: &PathExprNoRoot) -> XResult<XValue> {
        match e {
            PathExprNoRoot::Location(lp) => self.visit_location(lp),
            // 真身的 visitPathExprNoRoot:带尾巴时**根本不 visit filterExpr**,
            // 只把 recursion 打开然后跑 relativeLocationPath —— `foo()/bar`
            // 里的 foo() 是白写的
            PathExprNoRoot::Filter { primary, tail, .. } => match tail {
                None => self.visit_primary(primary),
                Some((abr, rel)) => {
                    if *abr {
                        self.set_recursion(true);
                    }
                    self.visit_relative(rel)
                }
            },
        }
    }

    fn visit_primary(&mut self, e: &PrimaryExpr) -> XResult<XValue> {
        match e {
            PrimaryExpr::VarRef => err("not support variableReference"),
            PrimaryExpr::Paren(x) => self.visit_or(x),
            PrimaryExpr::Literal(s) => Ok(XValue::Str(s.clone())),
            PrimaryExpr::Number(n) => Ok(XValue::Num(Num::Double(*n))),
            PrimaryExpr::Call { name, args } => {
                let mut params = Vec::new();
                for a in args {
                    let c = self.cur();
                    self.push_from(c);
                    let v = self.visit_or(a);
                    self.pop();
                    params.push(v?);
                }
                self.call_function(name, params)
            }
        }
    }

    fn visit_location(&mut self, e: &LocationPath) -> XResult<XValue> {
        match e {
            LocationPath::Relative(r) => self.visit_relative(r),
            LocationPath::Absolute { abr, rel } => {
                let root_ctx = self.scopes[self.root].context.clone();
                let c = self.cur();
                self.push_ctx(root_ctx, c);
                if *abr {
                    self.set_recursion(true);
                }
                let v = self.visit_relative(rel);
                self.pop();
                v
            }
        }
    }

    fn visit_relative(&mut self, e: &RelativeLocationPath) -> XResult<XValue> {
        let mut final_val = self.visit_step(&e.first)?;
        if let XValue::Elements(els) = &final_val {
            let els = els.clone();
            self.set_ctx(els);
        }
        for (abr, step) in &e.rest {
            self.set_recursion(*abr);
            final_val = self.visit_step(step)?;
            if let XValue::Elements(els) = &final_val {
                let els = els.clone();
                self.set_ctx(els);
            }
        }
        Ok(final_val)
    }

    fn visit_step(&mut self, e: &Step) -> XResult<XValue> {
        match e {
            // visitAbbreviatedStep:`..` 走 HashSet<Element> 归并 ——
            // Java 那边的迭代序由 identity hashCode 决定,**跨进程不可复现**;
            // 这里用文档序去重(契约见 fixtures/cases/xpath/README.md)
            Step::DotDot => {
                let mut out = Vec::new();
                let mut seen = HashSet::new();
                for el in self.ctx() {
                    let p = self.t.parent(el);
                    if let Some(p) = p {
                        if seen.insert(p) {
                            out.push(p);
                        }
                    }
                }
                Ok(XValue::Elements(out))
            }
            Step::Dot => Ok(XValue::Elements(self.ctx())),
            Step::Full { axis, test, preds } => {
                let mut filter_by_attr = false;
                let mut is_axis_ok = false;
                match axis {
                    Axis::None => {}
                    Axis::At => {
                        is_axis_ok = true;
                        filter_by_attr = true;
                    }
                    Axis::Named(a) => {
                        is_axis_ok = true;
                        match self.apply_axis(a)? {
                            XValue::Attr => filter_by_attr = true,
                            XValue::Elements(els) => self.set_ctx(els),
                            _ => {}
                        }
                    }
                }

                let node_test = self.visit_node_test(test)?;

                if filter_by_attr {
                    // 真身是 `nodeTest.asString()` —— 带冒号的 qName(非 exprStr)
                    // 照样能当属性名用,语料里 `@get:{bid}` 走的就是这条
                    let attr_name = match &node_test {
                        NodeTestVal::Name(s) => s.trim().to_string(),
                        NodeTestVal::Value(v) => self.as_string(v),
                    };
                    let ctx = self.ctx();
                    let q = format!("[{attr_name}]");
                    if self.is_recursion() {
                        let found = if ctx.len() == 1 {
                            self.t.select(&[self.single_el()?], &q)
                        } else {
                            // 真身这支用 Elements.addAll 逐个拼,**不去重**
                            let mut acc = Vec::new();
                            for el in &ctx {
                                match self.t.select(&[*el], &q) {
                                    Some(v) => acc.extend(v),
                                    None => return err("selector"),
                                }
                            }
                            Some(acc)
                        };
                        let Some(found) = found else { return err("selector") };
                        let attrs =
                            found.iter().map(|&e| self.t.attr(e, &attr_name)).collect::<Vec<_>>();
                        return Ok(XValue::List(attrs));
                    }
                    if ctx.len() == 1 {
                        return Ok(XValue::Str(self.t.attr(ctx[0], &attr_name)));
                    }
                    let attrs = ctx.iter().map(|&e| self.t.attr(e, &attr_name)).collect::<Vec<_>>();
                    return Ok(XValue::List(attrs));
                }

                match node_test {
                    NodeTestVal::Name(tag_name) => {
                        let tag_name = tag_name.trim().to_string();
                        let ctx = self.ctx();
                        if self.is_recursion() {
                            let Some(sel) = self.t.select(&ctx, &tag_name) else {
                                return err("selector parse");
                            };
                            self.set_ctx(sel);
                        } else {
                            let mut new_ctx = Vec::new();
                            for el in ctx {
                                if is_axis_ok {
                                    let n = self.t.tag_name(el);
                                    if n == tag_name || tag_name == "*" {
                                        new_ctx.push(el);
                                    }
                                } else {
                                    for c in self.t.children(el) {
                                        let n = self.t.tag_name(c);
                                        if n == tag_name || tag_name == "*" {
                                            new_ctx.push(c);
                                        }
                                    }
                                }
                            }
                            self.set_ctx(new_ctx);
                        }
                    }
                    NodeTestVal::Value(XValue::Elements(els)) => self.set_ctx(els),
                    // allText() / html() / outerHtml() / num() 直接把值抬走,
                    // 后面的谓词**不跑**
                    NodeTestVal::Value(v) => return Ok(v),
                }

                for p in preds {
                    let v = self.visit_predicate(p)?;
                    if let XValue::Elements(els) = v {
                        self.set_ctx(els);
                    } else {
                        return err("predicate");
                    }
                }
                Ok(XValue::Elements(self.ctx()))
            }
        }
    }

    fn visit_predicate(&mut self, e: &OrExpr) -> XResult<XValue> {
        let ctx = self.ctx();
        let ctx_set: HashSet<XEl> = ctx.iter().copied().collect();
        let (index_map, count_map) = self.pre_compute_predicate_indices(&ctx, &ctx_set);
        let mut new_ctx = Vec::new();
        for e_el in &ctx {
            let c = self.cur();
            self.push_ctx(vec![*e_el], c);
            let v = self.visit_or(e);
            self.pop();
            let v = v?;
            match &v {
                XValue::Num(_) => {
                    let mut index = self.as_long(&v)?;
                    let is_jx = self.t.tag_name(*e_el) == JX_TEXT;
                    if index < 0 {
                        let count = if is_jx {
                            self.jx_same_tag_nums(*e_el)
                        } else {
                            match count_map.get(e_el) {
                                Some(c) => *c,
                                None => self.same_tag_el_nums(*e_el, &|c| ctx_set.contains(&c))?,
                            }
                        };
                        index = count as i64 + index + 1;
                        if index < 0 {
                            index = 1;
                        }
                    }
                    if !is_jx {
                        let el_index = match index_map.get(e_el) {
                            Some(i) => *i,
                            None => self.el_index_in_same_tags(*e_el, &|c| ctx_set.contains(&c))?,
                        };
                        if index == el_index as i64 {
                            new_ctx.push(*e_el);
                        }
                    } else if index == self.jx_same_tag_index(*e_el) as i64 {
                        new_ctx.push(*e_el);
                    }
                }
                XValue::Bool(b) => {
                    if *b {
                        new_ctx.push(*e_el);
                    }
                }
                XValue::Str(_) => {
                    if !self.as_string(&v).trim().is_empty() {
                        new_ctx.push(*e_el);
                    }
                }
                XValue::Elements(els) => {
                    if !els.is_empty() {
                        new_ctx.push(*e_el);
                    }
                }
                XValue::List(l) => {
                    if !l.is_empty() {
                        new_ctx.push(*e_el);
                    }
                }
                _ => return err("unknown expr val"),
            }
        }
        Ok(XValue::Elements(new_ctx))
    }

    // ------------------------------------------------------------ 轴

    fn apply_axis(&mut self, name: &str) -> XResult<XValue> {
        let ctx = self.ctx();
        let v = match name {
            "attribute" => XValue::Attr,
            "self" => XValue::Elements(ctx),
            "child" => {
                let mut out = Vec::new();
                for e in ctx {
                    out.extend(self.t.children(e));
                }
                XValue::Elements(out)
            }
            "parent" => {
                // 真身是 total.add(el.parent()) —— 不去重、不过滤 null
                let mut out = Vec::new();
                for e in ctx {
                    match self.t.parent(e) {
                        Some(p) => out.push(p),
                        None => return err("parent axis: null parent"),
                    }
                }
                XValue::Elements(out)
            }
            "ancestor" => {
                let mut out = Vec::new();
                for e in ctx {
                    out.extend(self.t.parents(e));
                }
                XValue::Elements(out)
            }
            "ancestor-or-self" => {
                let mut out = Vec::new();
                for e in ctx {
                    out.extend(self.t.parents(e));
                    out.push(e);
                }
                XValue::Elements(out)
            }
            // descendant / descendant-or-self:真身用 HashSet<Element> 归并,
            // 迭代序由 identity hashCode 决定 —— 不可复现,见 README
            "descendant" => {
                let mut out = Vec::new();
                let mut seen = HashSet::new();
                for e in ctx {
                    for d in self.t.all_elements(e) {
                        if d != e && seen.insert(d) {
                            out.push(d);
                        }
                    }
                }
                XValue::Elements(out)
            }
            "descendant-or-self" => {
                let mut out = Vec::new();
                let mut seen = HashSet::new();
                for e in ctx {
                    for d in self.t.all_elements(e) {
                        if seen.insert(d) {
                            out.push(d);
                        }
                    }
                }
                XValue::Elements(out)
            }
            "following-sibling" => {
                let mut out = Vec::new();
                for e in ctx {
                    out.extend(self.t.following_siblings(e));
                }
                XValue::Elements(out)
            }
            "preceding-sibling" => {
                let mut out = Vec::new();
                for e in ctx {
                    out.extend(self.t.preceding_siblings(e));
                }
                XValue::Elements(out)
            }
            "following-sibling-one" => {
                let mut out = Vec::new();
                for e in ctx {
                    if let Some(n) = self.t.next_element_sibling(e) {
                        out.push(n);
                    }
                }
                XValue::Elements(out)
            }
            "preceding-sibling-one" => {
                let mut out = Vec::new();
                for e in ctx {
                    if let Some(n) = self.t.prev_element_sibling(e) {
                        out.push(n);
                    }
                }
                XValue::Elements(out)
            }
            "following" => {
                let mut out = Vec::new();
                for e in ctx {
                    for pe in self.t.parents(e) {
                        for pse in self.t.following_siblings(pe) {
                            out.extend(self.t.all_elements(pse));
                        }
                    }
                    for se in self.t.following_siblings(e) {
                        out.extend(self.t.all_elements(se));
                    }
                }
                XValue::Elements(out)
            }
            "preceding" => {
                let mut out = Vec::new();
                for e in ctx {
                    for pe in self.t.parents(e) {
                        out.extend(self.t.preceding_siblings(pe));
                    }
                    out.extend(self.t.preceding_siblings(e));
                }
                XValue::Elements(out)
            }
            // Scanner 没注册 `sibling` —— NoSuchAxisException
            _ => return err(&format!("not support axis: {name}")),
        };
        Ok(v)
    }

    // ------------------------------------------------------------ 节点测试

    fn visit_node_test(&mut self, t: &NodeTest) -> XResult<NodeTestVal> {
        match t {
            NodeTest::Name { name, expr_str: true } => Ok(NodeTestVal::Name(name.clone())),
            // 带冒号的 qName:isExprStr 为 false,visitStep 把它当结果抬走
            NodeTest::Name { name, expr_str: false } => {
                Ok(NodeTestVal::Value(XValue::Str(name.clone())))
            }
            // 'processing-instruction' 在文法里是独立字面量,visitNodeTest 两个
            // 分支都不命中 → 返回 null → visitStep 上 NPE
            NodeTest::ProcInstr => err("nodeTest is null"),
            NodeTest::Type(name) => {
                let v = match name.as_str() {
                    "text" => self.node_test_text()?,
                    "allText" => self.node_test_all_text(),
                    "html" => {
                        let ctx = self.ctx();
                        XValue::List(ctx.iter().map(|&e| self.t.inner_html(e)).collect())
                    }
                    "outerHtml" => {
                        let ctx = self.ctx();
                        XValue::List(ctx.iter().map(|&e| self.t.outer_html(e)).collect())
                    }
                    "node" => self.node_test_node()?,
                    "num" => self.node_test_num(),
                    // Scanner 只注册了 allText/html/node/num/outerHtml/text
                    _ => return err(&format!("not support nodeTest: {name}")),
                };
                Ok(NodeTestVal::Value(v))
            }
        }
    }

    fn node_test_all_text(&self) -> XValue {
        let ctx = self.ctx();
        XValue::List(
            ctx.iter()
                .map(
                    |&e| {
                        if self.t.tag_name(e) == "script" { self.t.data(e) } else { self.t.text(e) }
                    },
                )
                .collect(),
        )
    }

    fn node_test_num(&self) -> XValue {
        let XValue::List(all) = self.node_test_all_text() else { unreachable!() };
        let whole: String = all.concat();
        match find_num(&whole) {
            Some(s) => num_of_decimal_literal(&s),
            None => XValue::Null,
        }
    }

    fn node_test_node(&mut self) -> XResult<XValue> {
        let ctx = self.ctx();
        let mut out = Vec::new();
        for e in ctx {
            out.extend(self.t.children(e));
            let txt = self.t.own_text(e);
            if !txt.trim().is_empty() {
                // 真身是 `new Element("")` —— jsoup 的 Tag.valueOf 对空名
                // 抛 IllegalArgumentException,于是 ownText 非空时 node() 必炸
                return err("String must not be empty");
            }
        }
        Ok(XValue::Elements(out))
    }

    fn node_test_text(&mut self) -> XResult<XValue> {
        let ctx = self.ctx();
        let mut res = Vec::new();
        if ctx.is_empty() {
            return Ok(XValue::Elements(res));
        }
        if self.is_recursion() {
            // key 是 (depth, 父节点),真身用 depth + "_" + parent.hashCode()
            let mut index_map: HashMap<(usize, Option<ego_tree::NodeId>), i32> = HashMap::new();
            let mut keys: Vec<(usize, Option<ego_tree::NodeId>)> = Vec::new();
            let mut pending: Vec<(String, Option<ego_tree::NodeId>, i32)> = Vec::new();
            for el in &ctx {
                let mut hits: Vec<(TextRef, usize, Option<ego_tree::NodeId>)> = Vec::new();
                self.t.traverse_text_nodes(*el, &mut |tr, depth, parent| {
                    hits.push((tr, depth, parent))
                });
                for (tr, depth, parent) in hits {
                    let key = (depth, parent);
                    let idx = index_map.entry(key).or_insert(0);
                    *idx += 1;
                    let idx = *idx;
                    keys.push(key);
                    pending.push((self.t.whole_text_of(tr), parent, idx));
                }
            }
            for (i, (text, parent, idx)) in pending.into_iter().enumerate() {
                let s = self.t.new_synth(JX_TEXT, &text);
                if let Some(p) = parent {
                    self.t.set_synth_parent(s, p);
                }
                self.t.set_synth_attr(s, EL_SAME_TAG_INDEX_KEY, &idx.to_string());
                let total = index_map[&keys[i]];
                self.t.set_synth_attr(s, EL_SAME_TAG_ALL_NUM_KEY, &total.to_string());
                res.push(s);
            }
        } else {
            for el in ctx {
                if self.t.tag_name(el) == "script" {
                    let d = self.t.data(el);
                    let s = self.t.new_synth(JX_TEXT, &d);
                    self.t.set_synth_attr(s, EL_SAME_TAG_INDEX_KEY, "1");
                    self.t.set_synth_attr(s, EL_SAME_TAG_ALL_NUM_KEY, "1");
                    res.push(s);
                } else {
                    let tns = self.t.text_nodes(el);
                    let n = tns.len();
                    for (i, tr) in tns.into_iter().enumerate() {
                        let text = self.t.whole_text_of(tr);
                        let s = self.t.new_synth(JX_TEXT, &text);
                        self.t.set_synth_attr(s, EL_SAME_TAG_INDEX_KEY, &(i + 1).to_string());
                        self.t.set_synth_attr(s, EL_SAME_TAG_ALL_NUM_KEY, &n.to_string());
                        res.push(s);
                    }
                }
            }
        }
        Ok(XValue::Elements(res))
    }

    // ------------------------------------------------------------ CommonUtil

    fn jx_same_tag_index(&self, e: XEl) -> i32 {
        let v = self.t.attr(e, EL_SAME_TAG_INDEX_KEY);
        if v.trim().is_empty() { -1 } else { v.parse().unwrap_or(-1) }
    }

    fn jx_same_tag_nums(&self, e: XEl) -> i32 {
        let v = self.t.attr(e, EL_SAME_TAG_ALL_NUM_KEY);
        if v.trim().is_empty() { -1 } else { v.parse().unwrap_or(-1) }
    }

    /// `CommonUtil.preComputePredicateIndices`:按父元素分组,
    /// 只数**在 contextSet 里的直接子元素**,同标签内 1 起编号
    fn pre_compute_predicate_indices(
        &self,
        ctx: &[XEl],
        ctx_set: &HashSet<XEl>,
    ) -> (HashMap<XEl, i32>, HashMap<XEl, i32>) {
        let mut index_map = HashMap::new();
        let mut count_map = HashMap::new();
        let mut parents: Vec<XEl> = Vec::new();
        for &e in ctx {
            if let Some(p) = self.t.parent(e) {
                if !parents.contains(&p) {
                    parents.push(p);
                }
            }
        }
        for p in parents {
            let children = self.t.children(p);
            let mut totals: HashMap<String, i32> = HashMap::new();
            for &c in &children {
                if ctx_set.contains(&c) {
                    *totals.entry(self.t.tag_name(c)).or_insert(0) += 1;
                }
            }
            let mut counters: HashMap<String, i32> = HashMap::new();
            for &c in &children {
                if ctx_set.contains(&c) {
                    let tag = self.t.tag_name(c);
                    let idx = counters.entry(tag.clone()).or_insert(0);
                    *idx += 1;
                    index_map.insert(c, *idx);
                    count_map.insert(c, totals[&tag]);
                }
            }
        }
        (index_map, count_map)
    }

    /// `CommonUtil.getElIndexInSameTags`。`inside` 判元素在不在当前上下文
    /// (谓词那条给 ctx 集合,position() 给**父 scope 的 context**)
    fn el_index_in_same_tags(&self, e: XEl, inside: &dyn Fn(XEl) -> bool) -> XResult<i32> {
        let Some(parent) = self.t.parent(e) else {
            return err("getElIndexInSameTags: null parent");
        };
        let tag = self.t.tag_name(e);
        let mut index = 1;
        for c in self.t.children(parent) {
            if self.t.tag_name(c) != tag || !inside(c) {
                continue;
            }
            if c == e {
                break;
            }
            index += 1;
        }
        Ok(index)
    }

    /// `CommonUtil.sameTagElNums`:注意是 `parent.getElementsByTag()` ——
    /// 数的是父元素**整棵子树**里的同名标签,不是直接子元素
    fn same_tag_el_nums(&self, e: XEl, inside: &dyn Fn(XEl) -> bool) -> XResult<i32> {
        let Some(parent) = self.t.parent(e) else {
            return err("sameTagElNums: null parent");
        };
        let tag = self.t.tag_name(e);
        let mut count = 0;
        for el in self.t.elements_by_tag(parent, &tag) {
            if inside(el) {
                count += 1;
            }
        }
        Ok(count)
    }

    // ------------------------------------------------------------ 函数

    fn call_function(&mut self, name: &str, p: Vec<XValue>) -> XResult<XValue> {
        let s = |me: &Self, i: usize| -> String { me.as_string(&p[i]) };
        match name {
            "concat" => {
                let mut acc = String::new();
                for v in &p {
                    acc.push_str(&self.as_string(v));
                }
                Ok(XValue::Str(acc))
            }
            "contains" => {
                if p.len() < 2 {
                    return err("contains: params");
                }
                Ok(XValue::Bool(s(self, 0).contains(&s(self, 1))))
            }
            "starts-with" => {
                if p.len() < 2 {
                    return err("starts-with: params");
                }
                Ok(XValue::Bool(s(self, 0).starts_with(&s(self, 1))))
            }
            "count" => {
                if p.is_empty() {
                    return Ok(XValue::Num(Num::Int(0)));
                }
                match &p[0] {
                    XValue::Elements(e) => Ok(XValue::Num(Num::Int(e.len() as i32))),
                    _ => err("count: not elements"),
                }
            }
            "first" => Ok(XValue::Num(Num::Int(1))),
            "last" => {
                let e = self.single_el()?;
                let Some(parent_scope) = self.scopes[self.cur()].parent else {
                    return err("last(): null parent scope");
                };
                let pctx = self.scopes[parent_scope].context.clone();
                Ok(XValue::Num(Num::Int(self.same_tag_el_nums(e, &|c| pctx.contains(&c))?)))
            }
            "position" => {
                let e = self.single_el()?;
                let Some(parent_scope) = self.scopes[self.cur()].parent else {
                    return err("position(): null parent scope");
                };
                let pctx = self.scopes[parent_scope].context.clone();
                Ok(XValue::Num(Num::Int(self.el_index_in_same_tags(e, &|c| pctx.contains(&c))?)))
            }
            "not" => {
                if p.len() != 1 {
                    return err("error param in not(bool) function");
                }
                Ok(XValue::Bool(!self.as_bool(&p[0])))
            }
            "string-length" => {
                if p.is_empty() {
                    return Ok(XValue::Num(Num::Int(0)));
                }
                Ok(XValue::Num(Num::Int(s(self, 0).encode_utf16().count() as i32)))
            }
            "substring" => {
                if p.len() < 2 {
                    return err("substring: params");
                }
                let target = s(self, 0);
                let start = ((self.as_long(&p[1])? as i32) - 1).max(0);
                if p.len() < 3 {
                    // 真身写死了 params.get(2) —— 只给两个参就 IndexOutOfBounds
                    return err("Index 2 out of bounds");
                }
                let end = self.as_long(&p[2])? as i32;
                let len = target.encode_utf16().count() as i32;
                Ok(XValue::Str(java_substring(&target, start, (start + end).min(len).max(0))))
            }
            "substring-ex" => {
                if p.len() < 2 {
                    return err("substring-ex: params");
                }
                let target = s(self, 0);
                let start = self.as_long(&p[1])? as i32;
                if p.len() < 3 {
                    return err("Index 2 out of bounds");
                }
                let end = self.as_long(&p[2])? as i32;
                Ok(XValue::Str(commons_substring(&target, start, Some(end))))
            }
            "substring-after" => {
                if p.len() < 2 {
                    return err("substring-after: params");
                }
                Ok(XValue::Str(substring_after(&s(self, 0), &s(self, 1))))
            }
            "substring-after-last" => {
                if p.len() < 2 {
                    return err("substring-after-last: params");
                }
                Ok(XValue::Str(substring_after_last(&s(self, 0), &s(self, 1))))
            }
            "substring-before" => {
                if p.len() < 2 {
                    return err("substring-before: params");
                }
                Ok(XValue::Str(substring_before(&s(self, 0), &s(self, 1))))
            }
            "substring-before-last" => {
                if p.len() < 2 {
                    return err("substring-before-last: params");
                }
                Ok(XValue::Str(substring_before_last(&s(self, 0), &s(self, 1))))
            }
            "sum" => {
                if p.is_empty() {
                    return Ok(XValue::Num(Num::Int(0)));
                }
                let mut vals: Vec<f64> = Vec::new();
                for v in &p {
                    match v {
                        XValue::Num(_) => vals.push(self.as_double(v)?),
                        XValue::Str(_) => match num_from_str(&self.as_string(v)) {
                            Some(d) => vals.push(d),
                            // 真身在这里 return null —— XValue 是 null,
                            // 上层 selN 会走 "calRes == null" 那条给空串
                            None => return Ok(XValue::NullReturn),
                        },
                        XValue::Elements(els) => {
                            for &e in els {
                                match num_from_str(&self.t.own_text(e)) {
                                    Some(d) => vals.push(d),
                                    None => return Ok(XValue::NullReturn),
                                }
                            }
                        }
                        _ => {}
                    }
                }
                let total: f64 = vals.iter().sum();
                if total == total.trunc() {
                    Ok(XValue::Num(Num::Long(total as i64)))
                } else {
                    Ok(XValue::Num(Num::Double(total)))
                }
            }
            // 语料与探针都没有,真身走 SimpleDateFormat;留成显式错误
            // 而不是静默算错(差分上会现形)
            "format-date" => err("format-date not ported"),
            _ => err(&format!("not support function: {name}")),
        }
    }
}

enum NodeTestVal {
    Name(String),
    Value(XValue),
}

fn find_num(s: &str) -> Option<String> {
    // Constants.NUM_PATTERN = \d*\.?\d+
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() || (b[i] == '.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i < b.len() && b[i] == '.' && i + 1 < b.len() && b[i + 1].is_ascii_digit() {
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            return Some(b[start..i].iter().collect());
        }
        i += 1;
    }
    None
}

/// `Num.call` 的取值一支。真身(JsoupXpath `core/node/Num`)是:
///
/// ```java
/// BigDecimal b = new BigDecimal(numStr);
/// return b.compareTo(new BigDecimal(b.longValue())) == 0
///     ? XValue.create(b.longValue())      // Long
///     : XValue.create(b.doubleValue());   // Double
/// ```
///
/// 关键是 **BigDecimal 是精确十进制**,不能先 `parse::<f64>()` 再 `as i64`:
/// - `1234567890123456789`(19 位,装得下 long)—— 走 f64 会变 `…768`;
/// - 超出 long 范围时 `BigDecimal.longValue()` 是**截低 64 位**、Rust 的
///   `as i64` 是**饱和**,于是 `compareTo` 必不为 0 → 真身给 Double,
///   被测侧却给 `i64::MAX`。
///
/// `numStr` 由 `NUM_PATTERN = \d*\.?\d+` 抓出来:无符号、无指数,
/// 所以「小数部分全是 0 且整数部分装得下 i64」就等价于两个 BigDecimal 相等。
fn num_of_decimal_literal(lit: &str) -> XValue {
    let (int_part, frac_part) = lit.split_once('.').unwrap_or((lit, ""));
    if frac_part.bytes().all(|b| b == b'0') {
        let digits = int_part.trim_start_matches('0');
        if digits.is_empty() {
            return XValue::Num(Num::Long(0));
        }
        if let Ok(v) = digits.parse::<i64>() {
            return XValue::Num(Num::Long(v));
        }
        // 装不下 long:longValue() 截低位 ⇒ compareTo != 0 ⇒ 落 Double
    }
    XValue::Num(Num::Double(lit.parse().unwrap_or(0.0)))
}

/// `Sum.getNumFromStr`:整串匹配 `\d*\.?\d+` 才算数。
/// 对齐原手写游标的**不回溯**口径:整数段贪吃之后 `\d+` 只能由小数点后的
/// 数字来凑 —— 必须带 `.` 且点后至少一位数字,纯整数串不算
fn num_from_str(s: &str) -> Option<f64> {
    let (int_part, frac_part) = s.split_once('.')?;
    let ok = int_part.bytes().all(|b| b.is_ascii_digit())
        && !frac_part.is_empty()
        && frac_part.bytes().all(|b| b.is_ascii_digit());
    if ok { s.parse().ok() } else { None }
}

/// java.util.regex `matches()`:整串匹配
fn java_matches(pattern: &str, input: &str) -> XResult<bool> {
    // Matcher.matches() 是整串匹配 —— regex-compat 只给 find,
    // 故按 `\A(?:P)\z` 包一层(不改捕获组语义,这里只要布尔)
    let re = regex_compat::JavaRegex::compile(&format!("\\A(?:{pattern})\\z"))
        .map_err(|_| XError("PatternSyntaxException".into()))?;
    Ok(re.find_first(input).map_err(|_| XError("regex".into()))?.is_some())
}

fn utf16_slice(s: &str, start: i32, end: i32) -> String {
    let u: Vec<u16> = s.encode_utf16().collect();
    let (a, b) = (start.max(0) as usize, (end.max(0) as usize).min(u.len()));
    if a >= b {
        return String::new();
    }
    String::from_utf16_lossy(&u[a..b])
}

/// `String.substring(start, end)`(越界即抛,这里由调用方先钳)
fn java_substring(s: &str, start: i32, end: i32) -> String {
    utf16_slice(s, start, end)
}

/// `StringUtils.substring(str, start, end)`:负下标从尾部算
fn commons_substring(s: &str, mut start: i32, end: Option<i32>) -> String {
    let len = s.encode_utf16().count() as i32;
    let mut end = end.unwrap_or(len);
    if end < 0 {
        end += len;
    }
    if start < 0 {
        start += len;
    }
    if end > len {
        end = len;
    }
    if start > end {
        return String::new();
    }
    utf16_slice(s, start.max(0), end.max(0))
}

fn substring_after(s: &str, sep: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }
    match s.find(sep) {
        Some(p) => s[p + sep.len()..].to_string(),
        None => String::new(),
    }
}

fn substring_after_last(s: &str, sep: &str) -> String {
    if s.is_empty() {
        return s.to_string();
    }
    if sep.is_empty() {
        return String::new();
    }
    match s.rfind(sep) {
        Some(p) if p + sep.len() < s.len() => s[p + sep.len()..].to_string(),
        _ => String::new(),
    }
}

fn substring_before(s: &str, sep: &str) -> String {
    if s.is_empty() || sep.is_empty() {
        return if sep.is_empty() && !s.is_empty() { String::new() } else { s.to_string() };
    }
    match s.find(sep) {
        Some(p) => s[..p].to_string(),
        None => s.to_string(),
    }
}

fn substring_before_last(s: &str, sep: &str) -> String {
    if s.is_empty() || sep.is_empty() {
        return s.to_string();
    }
    match s.rfind(sep) {
        Some(p) => s[..p].to_string(),
        None => s.to_string(),
    }
}

// ---------------------------------------------------------------------------
// 纯函数的单测。
//
// 本 crate 的信心此前全压在需要 JVM + gradle 的 xpath 差分套上,`cargo test`
// 对它是空跑 —— `java_double_to_string` 漏掉科学计数一支就是这么漏过去的
// (探针恰好只抽到 `1` 和 `12.3`)。凡是不需要树的口径,都在这里钉住。
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    /// `XValue` 没有 `PartialEq`(真身的 equals 是 `same_value`,语义另说),
    /// 数值分支这里比 `{:?}` 就够。
    fn num_dbg(v: XValue) -> String {
        format!("{v:?}")
    }

    /// `Constants.NUM_PATTERN = \d*\.?\d+` 的**首个** find()
    #[test]
    fn find_num_takes_the_first_match() {
        assert_eq!(find_num("第12.3章"), Some("12.3".into()));
        assert_eq!(find_num("abc"), None);
        assert_eq!(find_num(""), None);
        // `\d*\.?\d+`:小数点前可以没有数字
        assert_eq!(find_num("x.5y"), Some(".5".into()));
        // 但小数点后必须有数字,否则点不算进来
        assert_eq!(find_num("12."), Some("12".into()));
        assert_eq!(find_num("1.2.3"), Some("1.2".into()));
    }

    /// `num()` 的 Long / Double 分支 —— 真身走 BigDecimal,不是 f64
    #[test]
    fn num_long_or_double() {
        assert_eq!(num_dbg(num_of_decimal_literal("12")), "Num(Long(12))");
        // 小数部分全 0:BigDecimal.compareTo 忽略 scale ⇒ 仍是 Long
        assert_eq!(num_dbg(num_of_decimal_literal("12.0")), "Num(Long(12))");
        assert_eq!(num_dbg(num_of_decimal_literal("0.0")), "Num(Long(0))");
        assert_eq!(num_dbg(num_of_decimal_literal(".5")), "Num(Double(0.5))");
        assert_eq!(num_dbg(num_of_decimal_literal("12.3")), "Num(Double(12.3))");
    }

    /// 19 位整数:先 `parse::<f64>()` 再 `as i64` 会变 `…768`
    #[test]
    fn num_keeps_all_19_digits() {
        assert_eq!(
            num_dbg(num_of_decimal_literal("1234567890123456789")),
            "Num(Long(1234567890123456789))"
        );
    }

    /// 超出 long:`longValue()` 截低位 ⇒ compareTo != 0 ⇒ Double
    /// (Rust 的 `as i64` 是饱和,会给 i64::MAX)
    #[test]
    fn num_beyond_long_falls_to_double() {
        assert_eq!(num_dbg(num_of_decimal_literal("99999999999999999999")), "Num(Double(1e20))");
    }

    /// `XValue.asString()` 里的 Double 形态(下沉到 rubato-core 之前漏了科学计数)
    #[test]
    fn double_to_string_is_javas() {
        assert_eq!(java_double_to_string(12.3), "12.3");
        assert_eq!(java_double_to_string(1.0), "1.0");
        assert_eq!(java_double_to_string(1e7), "1.0E7");
        assert_eq!(java_double_to_string(1e-4), "1.0E-4");
        assert_eq!(java_double_to_string(12345678.5), "1.23456785E7");
    }

    /// `Sum.getNumFromStr`:**整串**匹配才算数
    #[test]
    fn num_from_str_is_whole_string() {
        assert_eq!(num_from_str("12.5"), Some(12.5));
        assert_eq!(num_from_str(".5"), Some(0.5));
        assert_eq!(num_from_str("12x"), None);
        assert_eq!(num_from_str("x12"), None);
        assert_eq!(num_from_str(""), None);
    }

    /// 下标是 **UTF-16 码元**,不是字节也不是字符
    #[test]
    fn substring_counts_utf16_units() {
        assert_eq!(java_substring("中文abc", 0, 2), "中文");
        assert_eq!(java_substring("中文abc", 2, 5), "abc");
        // 调用方先钳过,越界这里只截断
        assert_eq!(java_substring("abc", 1, 99), "bc");
        assert_eq!(java_substring("abc", 2, 1), "");
        // emoji 占两个码元 —— Java 的 substring 会把它劈开
        assert_eq!(java_substring("😀a", 1, 3), "\u{fffd}a");
    }

    /// `StringUtils.substring`:负下标从尾部算
    #[test]
    fn commons_substring_negative_index() {
        assert_eq!(commons_substring("abcdef", 2, None), "cdef");
        assert_eq!(commons_substring("abcdef", -2, None), "ef");
        assert_eq!(commons_substring("abcdef", 0, Some(-2)), "abcd");
        assert_eq!(commons_substring("abcdef", 4, Some(2)), "");
        assert_eq!(commons_substring("abcdef", 0, Some(99)), "abcdef");
    }

    /// `StringUtils.substringAfter/Before[Last]` 的空串口径(各不相同)
    #[test]
    fn substring_around_separator() {
        assert_eq!(substring_after("a=b=c", "="), "b=c");
        assert_eq!(substring_after("abc", "="), "");
        assert_eq!(substring_after("", "="), "");
        assert_eq!(substring_after_last("a=b=c", "="), "c");
        assert_eq!(substring_after_last("a=b=", "="), "");
        assert_eq!(substring_after_last("abc", ""), "");
        assert_eq!(substring_before("a=b=c", "="), "a");
        assert_eq!(substring_before("abc", "="), "abc");
        assert_eq!(substring_before("abc", ""), "");
        assert_eq!(substring_before_last("a=b=c", "="), "a=b");
        assert_eq!(substring_before_last("abc", "="), "abc");
        assert_eq!(substring_before_last("abc", ""), "abc");
    }

    /// `new BigDecimal(str)` 比 `f64::from_str` 严
    #[test]
    fn big_decimal_is_stricter_than_rust() {
        assert_eq!(parse_big_decimal("1.5"), Some(1.5));
        assert_eq!(parse_big_decimal("-1.5e3"), Some(-1500.0));
        assert_eq!(parse_big_decimal(".5"), Some(0.5));
        assert_eq!(parse_big_decimal("Infinity"), None);
        assert_eq!(parse_big_decimal("NaN"), None);
        assert_eq!(parse_big_decimal(" 1"), None);
        assert_eq!(parse_big_decimal("1e"), None);
        assert_eq!(parse_big_decimal(""), None);
    }

    /// `Math.round` 的口径是 HALF_UP(不是 Rust 的 half-away-from-zero-ties)
    #[test]
    fn half_up_rounds_toward_positive_infinity_on_ties() {
        assert_eq!(half_up(2.5), 3.0);
        assert_eq!(half_up(-2.5), -3.0);
        assert_eq!(half_up(2.4), 2.0);
        assert_eq!(half_up(-2.4), -2.0);
    }

    /// `~=` / `!~` 是 `Matcher.matches()` —— **整串**匹配
    #[test]
    fn java_matches_is_whole_string() {
        assert!(java_matches("a.c", "abc").unwrap());
        assert!(!java_matches("a.c", "xabcx").unwrap());
        assert!(java_matches("(", "x").is_err());
    }

    /// `compareTo` 比的是 UTF-16 码元序(补充平面字符会排在 U+E000..U+FFFF 之前)
    #[test]
    fn string_compare_is_utf16_order() {
        use std::cmp::Ordering;
        assert_eq!(java_str_cmp("a", "b"), Ordering::Less);
        assert_eq!(java_str_cmp("a", "a"), Ordering::Equal);
        // '\u{1F600}' 的代理对首元是 0xD83D,小于 '\u{FFFD}'
        assert_eq!(java_str_cmp("😀", "\u{fffd}"), Ordering::Less);
    }
}
