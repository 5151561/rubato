//! jayway JSONPath 3.0.0 行为对齐的求值器(书源规则用到的方言面)。
//!
//! 对齐点(以 judge/harness 差分为准):
//! - definite 路径(无 ..、*、切片、多选、过滤器):命中返回标量,
//!   未命中报 PathNotFound;indefinite 路径永远返回列表(可为空);
//! - 过滤器:存在性 `[?(@.x)]`(null 也算存在)、比较 == != < <= > >=
//!   (数值跨 int/float 比较,字符串字典序),&& || ! 与括号;
//! - `..` 递归下降:先当前节点后子节点(对象按键序、数组按下标)的先序;
//! - Java toString 渲染:Map→`{k=v, k2=v2}`、List→`[a, b]`、Double→Java
//!   Double.toString 布局(含 1.0E10 科学计数)、null→"null"。

pub mod dsl;

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonPathError {
    InvalidPath(String),
    PathNotFound(String),
}

impl std::fmt::Display for JsonPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonPathError::InvalidPath(m) => write!(f, "invalid path: {m}"),
            JsonPathError::PathNotFound(m) => write!(f, "path not found: {m}"),
        }
    }
}
impl std::error::Error for JsonPathError {}

/// read 的结果形态:jayway definite 路径给标量,indefinite 给列表
#[derive(Debug, Clone, PartialEq)]
pub enum ReadResult {
    Scalar(Value),
    List(Vec<Value>),
}

#[derive(Debug, Clone)]
enum Segment {
    Key(String),
    Keys(Vec<String>),
    Index(i64),
    Indices(Vec<i64>),
    Wildcard,
    Slice(Option<i64>, Option<i64>),
    /// `..`:作用于其后一个段
    Scan,
    Filter(Expr),
}

#[derive(Debug, Clone)]
enum Expr {
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Compare(Operand, CmpOp, Operand),
    Exists(Vec<Segment>, bool), // (相对路径, 是否 $ 根路径)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone)]
enum Operand {
    RelPath(Vec<Segment>),  // @.x.y
    RootPath(Vec<Segment>), // $.x.y
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

pub struct JsonPath {
    segments: Vec<Segment>,
    definite: bool,
    source: String,
}

impl JsonPath {
    pub fn compile(path: &str) -> Result<Self, JsonPathError> {
        // jayway 的 JsonPath 构造:**不 trim**,不以 $ / @ 开头就补 "$."
        // ——书源里 `data.books` 这种省略根的写法全靠这条。
        // 别在这里加 trim:`&&` 拆出来的支常带前导 `\n`
        // (`$.a&&\n$.b` → 第二支是 `"\n$.b"`),真身把它当**属性名**的一部分
        // (实测 `PathNotFoundException: Missing property in path $['\n$']`),
        // trim 掉就成了「我们读得到而裁判读不到」。尾部空格另有一条:
        // 它由属性名读取器吃掉,见 parse_bare_name。
        let normalized = if path.starts_with('$') || path.starts_with('@') {
            path.to_string()
        } else {
            format!("$.{path}")
        };
        let mut p = Parser::new(&normalized);
        p.expect_root()?;
        let segments = p.parse_segments()?;
        // PropertyPathToken.isTokenDefinite() 恒为 true(注释原文:multi props
        // still means single node),所以多键选择也算 definite
        let definite = segments
            .iter()
            .all(|s| matches!(s, Segment::Key(_) | Segment::Index(_) | Segment::Keys(_)));
        Ok(Self { segments, definite, source: path.to_string() })
    }

    pub fn read(&self, root: &Value) -> Result<ReadResult, JsonPathError> {
        let mut out: Vec<Value> = Vec::new();
        collect(&self.segments, root, root, false, &mut out)
            .map_err(|_| JsonPathError::PathNotFound(self.source.clone()))?;
        if self.definite {
            match out.into_iter().next() {
                Some(v) => Ok(ReadResult::Scalar(v)),
                None => Err(JsonPathError::PathNotFound(self.source.clone())),
            }
        } else {
            Ok(ReadResult::List(out))
        }
    }
}

/// 在 node 上依次应用 segments,把命中值追加进 out(root 供根路径过滤器用)。
/// jayway 语义:扇出算子(通配/扫描/过滤/切片/多选)之前的缺失抛
/// PathNotFound;进入扇出后(lenient=true)各分支的缺失静默跳过。
fn collect(
    segments: &[Segment],
    node: &Value,
    root: &Value,
    lenient: bool,
    out: &mut Vec<Value>,
) -> Result<(), JsonPathError> {
    fn miss(lenient: bool) -> Result<(), JsonPathError> {
        if lenient { Ok(()) } else { Err(JsonPathError::PathNotFound(String::new())) }
    }
    let Some((seg, rest)) = segments.split_first() else {
        out.push(node.clone());
        return Ok(());
    };
    match seg {
        Segment::Key(k) => match node.as_object().and_then(|m| m.get(k.as_str())) {
            Some(v) => collect(rest, v, root, lenient, out),
            None => miss(lenient),
        },
        Segment::Keys(ks) => match node.as_object() {
            Some(m) => {
                if rest.is_empty() {
                    // multiPropertyMergeCase:叶子上的多键选择合并成一个 Map
                    // (缺失的键直接跳过),整条路径仍算 definite
                    let mut merged = serde_json::Map::new();
                    for k in ks {
                        if let Some(v) = m.get(k.as_str()) {
                            merged.insert(k.clone(), v.clone());
                        }
                    }
                    out.push(Value::Object(merged));
                } else {
                    // multiPropertyIterationCase:非叶子逐键下钻
                    for k in ks {
                        if let Some(v) = m.get(k.as_str()) {
                            collect(rest, v, root, true, out)?;
                        }
                    }
                }
                Ok(())
            }
            None => miss(lenient),
        },
        Segment::Index(i) => {
            match node.as_array().and_then(|a| norm_index(*i, a.len()).map(|i| &a[i])) {
                Some(v) => collect(rest, v, root, lenient, out),
                None => miss(lenient),
            }
        }
        Segment::Indices(is) => match node.as_array() {
            Some(a) => {
                for i in is {
                    if let Some(v) = norm_index(*i, a.len()).map(|i| &a[i]) {
                        collect(rest, v, root, true, out)?;
                    }
                }
                Ok(())
            }
            None => miss(lenient),
        },
        Segment::Wildcard => match node {
            Value::Object(m) => {
                for v in m.values() {
                    collect(rest, v, root, true, out)?;
                }
                Ok(())
            }
            Value::Array(a) => {
                for v in a {
                    collect(rest, v, root, true, out)?;
                }
                Ok(())
            }
            // jayway:通配符打在已存在的标量上返回空,不抛错
            _ => Ok(()),
        },
        Segment::Slice(start, end) => match node.as_array() {
            Some(a) => {
                let len = a.len() as i64;
                // 四种端点形态先按 jayway 各自的循环边界算出同一串下标,再统一收集
                let idxs: Vec<i64> = match (start, end) {
                    // 双端 [from:to]:jayway 是朴素 for i in from..to,
                    // 负 i 映射 len+i,越界跳过(产生"环绕"行为,实测对齐)
                    (Some(from), Some(to)) => {
                        (*from..*to).map(|i| if i < 0 { len + i } else { i }).collect()
                    }
                    // 单端 [from:] / [:to]:规范化负值后截取
                    (Some(from), None) => {
                        ((if *from < 0 { len + *from } else { *from }).max(0)..len).collect()
                    }
                    (None, Some(to)) => {
                        (0..(if *to < 0 { len + *to } else { *to }).min(len)).collect()
                    }
                    (None, None) => (0..len).collect(),
                };
                for idx in idxs {
                    if (0..len).contains(&idx) {
                        collect(rest, &a[idx as usize], root, true, out)?;
                    }
                }
                Ok(())
            }
            None => miss(lenient),
        },
        Segment::Scan => {
            // 先序遍历每个节点,对其应用剩余段(全程 lenient)
            fn walk(node: &Value, rest: &[Segment], root: &Value, out: &mut Vec<Value>) {
                let _ = collect(rest, node, root, true, out);
                match node {
                    Value::Object(m) => {
                        for v in m.values() {
                            walk(v, rest, root, out);
                        }
                    }
                    Value::Array(a) => {
                        for v in a {
                            walk(v, rest, root, out);
                        }
                    }
                    _ => {}
                }
            }
            walk(node, rest, root, out);
            Ok(())
        }
        Segment::Filter(expr) => match node {
            Value::Array(a) => {
                for v in a {
                    if eval_expr(expr, v, root) {
                        collect(rest, v, root, true, out)?;
                    }
                }
                Ok(())
            }
            // jayway:过滤器打在对象上时对对象本身求值
            Value::Object(_) => {
                if eval_expr(expr, node, root) {
                    collect(rest, node, root, true, out)?;
                }
                Ok(())
            }
            _ => miss(lenient),
        },
    }
}

fn norm_index(i: i64, len: usize) -> Option<usize> {
    let len = len as i64;
    let idx = if i < 0 { len + i } else { i };
    if (0..len).contains(&idx) { Some(idx as usize) } else { None }
}

fn eval_expr(e: &Expr, current: &Value, root: &Value) -> bool {
    match e {
        Expr::Or(a, b) => eval_expr(a, current, root) || eval_expr(b, current, root),
        Expr::And(a, b) => eval_expr(a, current, root) && eval_expr(b, current, root),
        Expr::Not(a) => !eval_expr(a, current, root),
        Expr::Exists(segs, is_root) => {
            let base = if *is_root { root } else { current };
            let mut out = Vec::new();
            let _ = collect(segs, base, root, true, &mut out);
            !out.is_empty()
        }
        Expr::Compare(l, op, r) => {
            let lv = resolve(l, current, root);
            let rv = resolve(r, current, root);
            let (Some(lv), Some(rv)) = (lv, rv) else {
                // jayway:路径缺失时 != 为 true,其余为 false
                return *op == CmpOp::Ne;
            };
            compare(&lv, *op, &rv)
        }
    }
}

fn resolve(o: &Operand, current: &Value, root: &Value) -> Option<Value> {
    match o {
        Operand::RelPath(segs) => {
            let mut out = Vec::new();
            let _ = collect(segs, current, root, true, &mut out);
            out.into_iter().next()
        }
        Operand::RootPath(segs) => {
            let mut out = Vec::new();
            let _ = collect(segs, root, root, true, &mut out);
            out.into_iter().next()
        }
        Operand::Str(s) => Some(Value::String(s.clone())),
        Operand::Num(n) => serde_json::Number::from_f64(*n).map(Value::Number),
        Operand::Bool(b) => Some(Value::Bool(*b)),
        Operand::Null => Some(Value::Null),
    }
}

fn compare(l: &Value, op: CmpOp, r: &Value) -> bool {
    use CmpOp::*;
    use std::cmp::Ordering;
    // 先算出序关系再对 op 断一次:数值跨类型比较(jayway 用 BigDecimal)、
    // 字符串按字典序;其余类型只支持 ==/!=,有序比较一律 false
    let ord: Option<Ordering> = if let (Some(a), Some(b)) = (as_f64(l), as_f64(r)) {
        a.partial_cmp(&b)
    } else if let (Value::String(a), Value::String(b)) = (l, r) {
        Some(a.cmp(b))
    } else {
        return match op {
            Eq => l == r,
            Ne => l != r,
            _ => false,
        };
    };
    match op {
        Eq => ord == Some(Ordering::Equal),
        Ne => ord != Some(Ordering::Equal),
        Lt => ord == Some(Ordering::Less),
        Le => matches!(ord, Some(Ordering::Less | Ordering::Equal)),
        Gt => ord == Some(Ordering::Greater),
        Ge => matches!(ord, Some(Ordering::Greater | Ordering::Equal)),
    }
}

fn as_f64(v: &Value) -> Option<f64> {
    v.as_number().and_then(|n| n.as_f64())
}

// ---------- 路径解析 ----------

struct Parser {
    chars: Vec<char>,
    i: usize,
}

impl Parser {
    fn new(s: &str) -> Self {
        Self { chars: s.chars().collect(), i: 0 }
    }

    fn err<T>(&self, msg: &str) -> Result<T, JsonPathError> {
        Err(JsonPathError::InvalidPath(format!("{msg} (位置 {})", self.i)))
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect_root(&mut self) -> Result<(), JsonPathError> {
        if !self.eat('$') && !self.eat('@') {
            return self.err("路径必须以 $ 或 @ 开头");
        }
        Ok(())
    }

    fn parse_segments(&mut self) -> Result<Vec<Segment>, JsonPathError> {
        let mut segs = Vec::new();
        loop {
            match self.peek() {
                None => return Ok(segs),
                Some('.') => {
                    self.i += 1;
                    if self.eat('.') {
                        segs.push(Segment::Scan);
                        // `..` 后可以直接接 [ 或名字
                        if self.peek() == Some('[') {
                            continue;
                        }
                    }
                    if self.peek() == Some('[') {
                        // `.[` 容忍为 [
                        continue;
                    }
                    if self.eat('*') {
                        segs.push(Segment::Wildcard);
                        continue;
                    }
                    let name = self.parse_bare_name()?;
                    segs.push(Segment::Key(name));
                }
                Some('[') => {
                    self.i += 1;
                    segs.push(self.parse_bracket()?);
                }
                Some(c) => return self.err(&format!("意外字符 {c:?}")),
            }
        }
    }

    /// jayway `PathCompiler.readProperty` 的裸属性名。
    ///
    /// **空格是唯一特殊的空白**,而且只有「后面到路径末尾全是空格」时才算
    /// 名字结束(尾部空格被吃掉);中间遇到空格是 InvalidPath
    /// (真身原话:Use bracket notion ['my prop'])。`\t` / `\n` 都是名字的
    /// 一部分。实测:
    ///
    /// | 路径 | jayway |
    /// |---|---|
    /// | `$.data.total ` | 读得到(编译成 `$['data']['total']`) |
    /// | `$.data.total  x` / `$.data .total` | InvalidPath |
    /// | `$.data.total\n` | 属性名是 `total\n` → PathNotFound |
    /// | `\n$.data.total` | 补 `$.` 之后属性名是 `\n$` → PathNotFound |
    fn parse_bare_name(&mut self) -> Result<String, JsonPathError> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c == '.' || c == '[' {
                break;
            }
            if c == ' ' {
                if self.chars[self.i..].iter().all(|&c| c == ' ') {
                    self.i = self.chars.len();
                    break;
                }
                return self.err("属性名里有空格(真身要求写成 ['my prop'])");
            }
            s.push(c);
            self.i += 1;
        }
        if s.is_empty() {
            return self.err("空属性名");
        }
        Ok(s)
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|c| c == ' ' || c == '\t') {
            self.i += 1;
        }
    }

    fn parse_bracket(&mut self) -> Result<Segment, JsonPathError> {
        self.skip_ws();
        match self.peek() {
            Some('*') => {
                self.i += 1;
                self.skip_ws();
                if !self.eat(']') {
                    return self.err("[* 未闭合");
                }
                Ok(Segment::Wildcard)
            }
            Some('\'') | Some('"') => {
                let mut names = Vec::new();
                loop {
                    self.skip_ws();
                    let q = self.peek().filter(|&c| c == '\'' || c == '"');
                    let Some(q) = q else { return self.err("期望引号属性名") };
                    self.i += 1;
                    let mut name = String::new();
                    loop {
                        match self.peek() {
                            None => return self.err("引号未闭合"),
                            Some(c) if c == q => {
                                self.i += 1;
                                break;
                            }
                            Some('\\') => {
                                self.i += 1;
                                let Some(c) = self.peek() else {
                                    return self.err("转义悬空");
                                };
                                name.push(c);
                                self.i += 1;
                            }
                            Some(c) => {
                                name.push(c);
                                self.i += 1;
                            }
                        }
                    }
                    names.push(name);
                    self.skip_ws();
                    if self.eat(',') {
                        continue;
                    }
                    if self.eat(']') {
                        break;
                    }
                    return self.err("['..'] 未闭合");
                }
                if names.len() == 1 {
                    Ok(Segment::Key(names.pop().unwrap()))
                } else {
                    Ok(Segment::Keys(names))
                }
            }
            Some('?') => {
                self.i += 1;
                if !self.eat('(') {
                    return self.err("[? 后期望 (");
                }
                let expr = self.parse_or()?;
                self.skip_ws();
                if !self.eat(')') {
                    return self.err("过滤器括号未闭合");
                }
                self.skip_ws();
                if !self.eat(']') {
                    return self.err("过滤器 ] 未闭合");
                }
                Ok(Segment::Filter(expr))
            }
            _ => {
                // 数字:索引/多索引/切片
                let mut nums: Vec<Option<i64>> = Vec::new();
                let mut is_slice = false;
                let mut cur = String::new();
                loop {
                    self.skip_ws();
                    match self.peek() {
                        Some(c) if c.is_ascii_digit() || c == '-' => {
                            cur.push(c);
                            self.i += 1;
                        }
                        Some(':') => {
                            is_slice = true;
                            nums.push(if cur.is_empty() {
                                None
                            } else {
                                Some(
                                    cur.parse()
                                        .map_err(|_| JsonPathError::InvalidPath("坏索引".into()))?,
                                )
                            });
                            cur.clear();
                            self.i += 1;
                        }
                        Some(',') => {
                            nums.push(Some(
                                cur.parse()
                                    .map_err(|_| JsonPathError::InvalidPath("坏索引".into()))?,
                            ));
                            cur.clear();
                            self.i += 1;
                        }
                        Some(']') => {
                            self.i += 1;
                            nums.push(if cur.is_empty() {
                                None
                            } else {
                                Some(
                                    cur.parse()
                                        .map_err(|_| JsonPathError::InvalidPath("坏索引".into()))?,
                                )
                            });
                            break;
                        }
                        _ => return self.err("非法下标"),
                    }
                }
                if is_slice {
                    if nums.len() != 2 {
                        return self.err("切片仅支持 start:end");
                    }
                    Ok(Segment::Slice(nums[0], nums[1]))
                } else {
                    let vals: Vec<i64> = nums.into_iter().flatten().collect();
                    if vals.is_empty() {
                        return self.err("空下标");
                    }
                    if vals.len() == 1 {
                        Ok(Segment::Index(vals[0]))
                    } else {
                        Ok(Segment::Indices(vals))
                    }
                }
            }
        }
    }

    // 过滤器表达式:or → and → unary → cmp
    fn parse_or(&mut self) -> Result<Expr, JsonPathError> {
        let mut l = self.parse_and()?;
        loop {
            self.skip_ws();
            if self.peek() == Some('|') && self.chars.get(self.i + 1) == Some(&'|') {
                self.i += 2;
                let r = self.parse_and()?;
                l = Expr::Or(Box::new(l), Box::new(r));
            } else {
                return Ok(l);
            }
        }
    }

    fn parse_and(&mut self) -> Result<Expr, JsonPathError> {
        let mut l = self.parse_unary()?;
        loop {
            self.skip_ws();
            if self.peek() == Some('&') && self.chars.get(self.i + 1) == Some(&'&') {
                self.i += 2;
                let r = self.parse_unary()?;
                l = Expr::And(Box::new(l), Box::new(r));
            } else {
                return Ok(l);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, JsonPathError> {
        self.skip_ws();
        if self.eat('!') {
            let e = self.parse_unary()?;
            return Ok(Expr::Not(Box::new(e)));
        }
        if self.eat('(') {
            let e = self.parse_or()?;
            self.skip_ws();
            if !self.eat(')') {
                return self.err("括号未闭合");
            }
            return Ok(e);
        }
        // 比较或存在性
        let left = self.parse_operand()?;
        self.skip_ws();
        let op = if self.eat('=') {
            if self.eat('=') {
                Some(CmpOp::Eq)
            } else {
                return self.err("单个 = 非法");
            }
        } else if self.peek() == Some('!') && self.chars.get(self.i + 1) == Some(&'=') {
            self.i += 2;
            Some(CmpOp::Ne)
        } else if self.eat('<') {
            Some(if self.eat('=') { CmpOp::Le } else { CmpOp::Lt })
        } else if self.eat('>') {
            Some(if self.eat('=') { CmpOp::Ge } else { CmpOp::Gt })
        } else {
            None
        };
        match op {
            None => match left {
                Operand::RelPath(segs) => Ok(Expr::Exists(segs, false)),
                Operand::RootPath(segs) => Ok(Expr::Exists(segs, true)),
                _ => self.err("过滤器缺少比较符"),
            },
            Some(op) => {
                let right = self.parse_operand()?;
                Ok(Expr::Compare(left, op, right))
            }
        }
    }

    fn parse_operand(&mut self) -> Result<Operand, JsonPathError> {
        self.skip_ws();
        match self.peek() {
            Some('@') => {
                self.i += 1;
                let segs = self.parse_filter_path()?;
                Ok(Operand::RelPath(segs))
            }
            Some('$') => {
                self.i += 1;
                let segs = self.parse_filter_path()?;
                Ok(Operand::RootPath(segs))
            }
            Some(q @ ('\'' | '"')) => {
                self.i += 1;
                let mut s = String::new();
                loop {
                    match self.peek() {
                        None => return self.err("字符串未闭合"),
                        Some(c) if c == q => {
                            self.i += 1;
                            break;
                        }
                        Some('\\') => {
                            self.i += 1;
                            let Some(c) = self.peek() else {
                                return self.err("转义悬空");
                            };
                            s.push(match c {
                                'n' => '\n',
                                't' => '\t',
                                'r' => '\r',
                                other => other,
                            });
                            self.i += 1;
                        }
                        Some(c) => {
                            s.push(c);
                            self.i += 1;
                        }
                    }
                }
                Ok(Operand::Str(s))
            }
            Some(c) if c.is_ascii_digit() || c == '-' => {
                let mut s = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit()
                        || c == '.'
                        || c == '-'
                        || c == 'e'
                        || c == 'E'
                        || c == '+'
                    {
                        s.push(c);
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                s.parse::<f64>()
                    .map(Operand::Num)
                    .map_err(|_| JsonPathError::InvalidPath("坏数字".into()))
            }
            _ => {
                // true / false / null
                let mut s = String::new();
                while let Some(c) = self.peek() {
                    if c.is_ascii_alphabetic() {
                        s.push(c);
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                match s.as_str() {
                    "true" => Ok(Operand::Bool(true)),
                    "false" => Ok(Operand::Bool(false)),
                    "null" => Ok(Operand::Null),
                    _ => self.err("非法操作数"),
                }
            }
        }
    }

    /// 过滤器里 @ / $ 后面的路径(遇比较符/逻辑符/右括号即止)
    fn parse_filter_path(&mut self) -> Result<Vec<Segment>, JsonPathError> {
        let mut segs = Vec::new();
        loop {
            match self.peek() {
                Some('.') => {
                    self.i += 1;
                    if self.eat('.') {
                        segs.push(Segment::Scan);
                    }
                    if self.peek() == Some('[') {
                        continue;
                    }
                    let mut name = String::new();
                    while let Some(c) = self.peek() {
                        if c == '.'
                            || c == '['
                            || c == ' '
                            || c == ')'
                            || c == '='
                            || c == '!'
                            || c == '<'
                            || c == '>'
                            || c == '&'
                            || c == '|'
                            || c == ','
                        {
                            break;
                        }
                        name.push(c);
                        self.i += 1;
                    }
                    if name.is_empty() {
                        return self.err("过滤器路径空属性名");
                    }
                    segs.push(Segment::Key(name));
                }
                Some('[') => {
                    self.i += 1;
                    segs.push(self.parse_bracket()?);
                }
                _ => return Ok(segs),
            }
        }
    }
}

// ---------- Java toString 渲染 ----------

/// 按引擎观察到的 jayway/json-smart toString 规则渲染 JSON 值:
/// - 对象:LinkedHashMap.toString 风格 `{k=v, k2=v2}`(值递归按本规则);
/// - 数组:json-smart JSONArray.toString = 严格 JSON 文本(字符串带引号
///   转义、无空格),嵌套对象在数组内也是 JSON 形态;
/// - 标量:字符串裸串、Double 按 Java Double.toString、null→"null"。
pub fn java_to_string(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => java_number_to_string(n),
        Value::String(s) => s.clone(),
        Value::Array(_) => json_smart_to_json(v),
        Value::Object(m) => {
            let items: Vec<String> =
                m.iter().map(|(k, v)| format!("{k}={}", java_to_string(v))).collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}

fn java_number_to_string(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        i.to_string()
    } else if let Some(u) = n.as_u64() {
        u.to_string()
    } else {
        java_double_to_string(n.as_f64().unwrap_or(f64::NAN))
    }
}

/// json-smart 的 JSON 序列化(JSONArray/嵌套内容 toString 用):
/// 无空格;字符串转义 " \ 与控制字符(\u 形式),非 ASCII 原样;数字按 Java 形态
pub fn json_smart_to_json(v: &Value) -> String {
    fn esc(s: &str, out: &mut String) {
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\u{8}' => out.push_str("\\b"),
                '\u{c}' => out.push_str("\\f"),
                '/' => out.push_str("\\/"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => {
                    out.push_str(&format!("\\u{:04X}", c as u32));
                }
                c => out.push(c),
            }
        }
        out.push('"');
    }
    fn ser(v: &Value, out: &mut String) {
        match v {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&java_number_to_string(n)),
            Value::String(s) => esc(s, out),
            Value::Array(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    ser(v, out);
                }
                out.push(']');
            }
            Value::Object(m) => {
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    esc(k, out);
                    out.push(':');
                    ser(v, out);
                }
                out.push('}');
            }
        }
    }
    let mut out = String::new();
    ser(v, &mut out);
    out
}

/// Java `Double.toString`。本体在 `rubato_core::java_num` —— 全仓唯一一份
/// (`use json_compat::java_double_to_string` 的老调用点照旧能用)。
pub use rubato_core::java_double_to_string;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read(json: &Value, path: &str) -> Result<ReadResult, JsonPathError> {
        JsonPath::compile(path)?.read(json)
    }

    #[test]
    fn definite_scalar() {
        let v = json!({"a": {"b": 5}});
        assert_eq!(read(&v, "$.a.b").unwrap(), ReadResult::Scalar(json!(5)));
    }

    #[test]
    fn definite_missing_is_error() {
        let v = json!({"a": 1});
        assert!(matches!(read(&v, "$.b"), Err(JsonPathError::PathNotFound(_))));
    }

    #[test]
    fn definite_null_is_scalar_null() {
        let v = json!({"a": null});
        assert_eq!(read(&v, "$.a").unwrap(), ReadResult::Scalar(Value::Null));
    }

    #[test]
    fn indefinite_returns_list() {
        let v = json!({"list": [{"n": 1}, {"n": 2}]});
        assert_eq!(read(&v, "$.list[*].n").unwrap(), ReadResult::List(vec![json!(1), json!(2)]));
        // 扇出前的定值前缀缺失:jayway 抛 PathNotFound(而非空列表)
        assert!(matches!(read(&v, "$.nope[*]"), Err(JsonPathError::PathNotFound(_))));
    }

    #[test]
    fn recursive_preorder() {
        let v = json!({"name": "a", "sub": {"name": "b", "deep": {"name": "c"}}});
        assert_eq!(
            read(&v, "$..name").unwrap(),
            ReadResult::List(vec![json!("a"), json!("b"), json!("c")])
        );
    }

    #[test]
    fn negative_index_and_slice() {
        let v = json!([10, 20, 30, 40]);
        assert_eq!(read(&v, "$[-1]").unwrap(), ReadResult::Scalar(json!(40)));
        assert_eq!(read(&v, "$[1:3]").unwrap(), ReadResult::List(vec![json!(20), json!(30)]));
        assert_eq!(
            read(&v, "$[1:]").unwrap(),
            ReadResult::List(vec![json!(20), json!(30), json!(40)])
        );
        // jayway 双端切片是朴素 from..to 循环 + 负下标映射:[-1:0] = 末元素
        assert_eq!(read(&v, "$[-1:0]").unwrap(), ReadResult::List(vec![json!(40)]));
        assert_eq!(read(&v, "$[-1:1]").unwrap(), ReadResult::List(vec![json!(40), json!(10)]));
    }

    #[test]
    fn filters() {
        let v = json!({"items": [
            {"t": "a", "n": 1, "flag": true},
            {"t": "b", "n": 2},
            {"n": 3, "flag": null}
        ]});
        // 存在性:null 也算存在
        let ReadResult::List(l) = read(&v, "$.items[?(@.flag)]").unwrap() else { panic!() };
        assert_eq!(l.len(), 2);
        // 比较
        let ReadResult::List(l) = read(&v, "$.items[?(@.t=='a')].n").unwrap() else { panic!() };
        assert_eq!(l, vec![json!(1)]);
        let ReadResult::List(l) = read(&v, "$.items[?(@.n > 1)].n").unwrap() else { panic!() };
        assert_eq!(l, vec![json!(2), json!(3)]);
        // != 对缺失路径为 true
        let ReadResult::List(l) = read(&v, "$.items[?(@.t != 'a')].n").unwrap() else { panic!() };
        assert_eq!(l, vec![json!(2), json!(3)]);
    }

    #[test]
    fn java_tostring_rendering() {
        assert_eq!(java_to_string(&json!({"a": 1, "b": "x"})), "{a=1, b=x}");
        // 数组按 json-smart JSON 文本(带引号、无空格);对象内数组同理
        assert_eq!(java_to_string(&json!([1, "a", null, true])), "[1,\"a\",null,true]");
        assert_eq!(
            java_to_string(&json!({"tags": ["仙侠", "热血"], "n": 1.5})),
            "{tags=[\"仙侠\",\"热血\"], n=1.5}"
        );
        assert_eq!(java_to_string(&json!([{"k": "v"}])), "[{\"k\":\"v\"}]");
        assert_eq!(java_to_string(&json!(1.5)), "1.5");
        assert_eq!(java_to_string(&json!(3)), "3");
    }

    #[test]
    fn java_double_format() {
        assert_eq!(java_double_to_string(1.0), "1.0");
        assert_eq!(java_double_to_string(0.001), "0.001");
        assert_eq!(java_double_to_string(1e7), "1.0E7");
        assert_eq!(java_double_to_string(1e-4), "1.0E-4");
        assert_eq!(java_double_to_string(1234567.5), "1234567.5");
        assert_eq!(java_double_to_string(-2.5e10), "-2.5E10");
    }

    #[test]
    fn multi_name_and_index() {
        let v = json!({"a": 1, "b": 2, "arr": [0, 1, 2, 3]});
        // 叶子上的多键选择是 jayway 的 multiPropertyMergeCase:合并成一个 Map
        // 且整条路径算 definite(差分 re00764 抓出,裁判返回 "{}" 而非 "[]")
        assert_eq!(read(&v, "$['a','b']").unwrap(), ReadResult::Scalar(json!({"a": 1, "b": 2})));
        assert_eq!(read(&v, "$['a','z']").unwrap(), ReadResult::Scalar(json!({"a": 1})));
        assert_eq!(read(&v, "$['y','z']").unwrap(), ReadResult::Scalar(json!({})));
        // 省略根的写法会被补成 "$."
        assert_eq!(read(&v, "a").unwrap(), ReadResult::Scalar(json!(1)));
        assert_eq!(read(&v, "$.arr[0,2]").unwrap(), ReadResult::List(vec![json!(0), json!(2)]));
        assert_eq!(read(&v, "$['a']").unwrap(), ReadResult::Scalar(json!(1)));
    }
}
