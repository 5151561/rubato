//! JsoupXpath 的 ANTLR 文法(`Xpath.g4`)递归下降复刻。
//!
//! AST 与 ANTLR 的规则一一对应(而不是化简成常规表达式树),
//! 因为求值器 `eval.rs` 是照着 `XpathProcessor` 的 visit 方法逐个写的 ——
//! 结构对齐才能把"哪一步 push 了 scope、哪一步改了 context"抄准。
//!
//! 文法(记号见 lex.rs;与 jar 里 XpathParser 的 ruleNames 同名):
//! ```text
//! main               : expr          ← **没有 EOF**
//! expr               : orExpr
//! orExpr             : andExpr ('or' andExpr)*
//! andExpr            : equalityExpr ('and' equalityExpr)*
//! equalityExpr       : relationalExpr (op=('='|'!=') relationalExpr)*
//! relationalExpr     : additiveExpr (op=('<'|'>'|'<='|'>='|'^='|'$='|'*='|'~='|'!~') additiveExpr)*
//! additiveExpr       : multiplicativeExpr (('+'|'-') multiplicativeExpr)*
//! multiplicativeExpr : unaryExprNoRoot (op=('*'|`div`|`mod`) multiplicativeExpr)?
//! unaryExprNoRoot    : sign='-'? unionExprNoRoot
//! unionExprNoRoot    : pathExprNoRoot (op='|' unionExprNoRoot)?
//! pathExprNoRoot     : locationPath | filterExpr (op=('/'|'//') relativeLocationPath)?
//! filterExpr         : primaryExpr predicate*
//! primaryExpr        : '$' qName | '(' expr ')' | Literal | Number | functionCall
//! functionCall       : qName '(' (expr (',' expr)*)? ')'
//! locationPath       : relativeLocationPath | absoluteLocationPathNoroot
//! absoluteLocationPathNoroot : op=('/'|'//') relativeLocationPath
//! relativeLocationPath : step (op=('/'|'//') step)*
//! step               : axisSpecifier nodeTest predicate* | abbreviatedStep
//! axisSpecifier      : AxisName '::' | '@'?
//! nodeTest           : nameTest | NodeType '(' ')' | 'processing-instruction' '(' Literal ')'
//! nameTest           : '*' | nCName ':' '*' | qName
//! predicate          : '[' expr ']'
//! abbreviatedStep    : '.' | '..'
//! qName              : nCName (':' nCName)*
//! nCName             : NCName | AxisName
//! ```
//! 注意 `nCName` **不含 NodeType** —— 于是 `/html` 是语法错误(见 lex.rs)。

use crate::lex::{Tok, lex};

#[derive(Debug, Clone)]
pub struct OrExpr(pub Vec<AndExpr>);
#[derive(Debug, Clone)]
pub struct AndExpr(pub Vec<EqualityExpr>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EqOp {
    Eq,
    Ne,
}
#[derive(Debug, Clone)]
pub struct EqualityExpr {
    pub items: Vec<RelationalExpr>,
    pub op: Option<EqOp>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RelOp {
    Lt,
    Gt,
    Le,
    Ge,
    StartWith,
    EndWith,
    ContainWith,
    RegexpWith,
    RegexpNotWith,
}
#[derive(Debug, Clone)]
pub struct RelationalExpr {
    pub items: Vec<AdditiveExpr>,
    pub op: Option<RelOp>,
}

#[derive(Debug, Clone)]
pub struct AdditiveExpr {
    pub first: MultiplicativeExpr,
    /// `('+'|'-', 右操作数)`,按源序
    pub rest: Vec<(char, MultiplicativeExpr)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MulOp {
    Mul,
    Div,
    Mod,
}
#[derive(Debug, Clone)]
pub struct MultiplicativeExpr {
    pub left: UnaryExpr,
    pub rest: Option<(MulOp, Box<MultiplicativeExpr>)>,
}

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub neg: bool,
    pub inner: UnionExpr,
}

#[derive(Debug, Clone)]
pub struct UnionExpr {
    pub path: PathExprNoRoot,
    pub rest: Option<Box<UnionExpr>>,
}

#[derive(Debug, Clone)]
pub enum PathExprNoRoot {
    Location(LocationPath),
    /// filterExpr(+ 可选的 `/`/`//` 尾巴)
    Filter {
        primary: PrimaryExpr,
        /// 语法上收得下,但 visitFilterExpr 只 visit primaryExpr —— 谓词被丢掉
        preds: Vec<OrExpr>,
        tail: Option<(bool, RelativeLocationPath)>,
    },
}

#[derive(Debug, Clone)]
pub enum PrimaryExpr {
    /// `$name` —— 真身直接抛 "not support variableReference"
    VarRef,
    Paren(Box<OrExpr>),
    Literal(String),
    Number(f64),
    Call {
        name: String,
        args: Vec<OrExpr>,
    },
}

#[derive(Debug, Clone)]
pub enum LocationPath {
    Relative(RelativeLocationPath),
    Absolute { abr: bool, rel: RelativeLocationPath },
}

#[derive(Debug, Clone)]
pub struct RelativeLocationPath {
    pub first: Step,
    /// `(是不是 //, 下一个 step)`
    pub rest: Vec<(bool, Step)>,
}

#[derive(Debug, Clone)]
pub enum Step {
    Dot,
    DotDot,
    Full { axis: Axis, test: NodeTest, preds: Vec<OrExpr> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Axis {
    /// 没写轴 —— visitStep 的 isAxisOk = false
    None,
    Named(String),
    At,
}

#[derive(Debug, Clone)]
pub enum NodeTest {
    /// nameTest:`*` / qName / `ns:*`(后者真身只取 ns 那一半,见 visitNameTest)。
    ///
    /// `expr_str` 对应 `XValue.isExprStr()`:`visitNCName` 与 `*` 都调了
    /// `.exprStr()`,**而带冒号的 qName 那支没调**(visitQName 的 size>1 分支
    /// 直接 `XValue.create(join(":"))`)。于是 `//p:q` 在 visitStep 里既不是
    /// 名字测试也不是节点集 —— 整个 step 把这个字符串**当结果抬走**,
    /// 结果就是字面量 `"p:q"`。语料里 `//javascript:gotochapter(…)` 靠的就是它。
    Name { name: String, expr_str: bool },
    /// `NodeType '(' ')'`
    Type(String),
    /// `processing-instruction '(' Literal ')'`
    ProcInstr,
}

#[derive(Debug)]
pub struct ParseError;

pub type PResult<T> = Result<T, ParseError>;

/// `main : expr`。**文法里没有 EOF** —— 表达式吃完就收工,尾巴上的记号
/// 一概不管。语料里大量"其实是相对 URL"的规则(`/api/book-info?id=X1`)
/// 就靠这条不报错:`?` `&` 被词法丢掉,`/api/book-info` 当路径求值(选不中),
/// 剩下的 `id = X1 source_id = 1` 直接被无视 → 结果是空串而不是语法错误。
pub fn parse(src: &str) -> PResult<OrExpr> {
    let toks = lex(src);
    let mut p = Parser { t: toks, i: 0 };
    p.or_expr()
}

struct Parser {
    t: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        self.t.get(self.i).unwrap_or(&Tok::Eof)
    }
    fn at(&self, n: usize) -> &Tok {
        self.t.get(self.i + n).unwrap_or(&Tok::Eof)
    }
    fn bump(&mut self) -> Tok {
        let t = self.t.get(self.i).cloned().unwrap_or(Tok::Eof);
        self.i += 1;
        t
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == t {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: Tok) -> PResult<()> {
        if self.eat(&t) { Ok(()) } else { Err(ParseError) }
    }

    fn or_expr(&mut self) -> PResult<OrExpr> {
        let mut v = vec![self.and_expr()?];
        while self.eat(&Tok::Or) {
            v.push(self.and_expr()?);
        }
        Ok(OrExpr(v))
    }

    fn and_expr(&mut self) -> PResult<AndExpr> {
        let mut v = vec![self.equality_expr()?];
        while self.eat(&Tok::And) {
            v.push(self.equality_expr()?);
        }
        Ok(AndExpr(v))
    }

    fn equality_expr(&mut self) -> PResult<EqualityExpr> {
        let mut items = vec![self.relational_expr()?];
        let mut op = None;
        loop {
            let o = match self.peek() {
                Tok::Equality => EqOp::Eq,
                Tok::Inequality => EqOp::Ne,
                _ => break,
            };
            self.bump();
            op = Some(o); // ANTLR 的 op 标签只记最后一个
            items.push(self.relational_expr()?);
        }
        Ok(EqualityExpr { items, op })
    }

    fn relational_expr(&mut self) -> PResult<RelationalExpr> {
        let mut items = vec![self.additive_expr()?];
        let mut op = None;
        loop {
            let o = match self.peek() {
                Tok::Less => RelOp::Lt,
                Tok::More => RelOp::Gt,
                Tok::Le => RelOp::Le,
                Tok::Ge => RelOp::Ge,
                Tok::StartWith => RelOp::StartWith,
                Tok::EndWith => RelOp::EndWith,
                Tok::ContainWith => RelOp::ContainWith,
                Tok::RegexpWith => RelOp::RegexpWith,
                Tok::RegexpNotWith => RelOp::RegexpNotWith,
                _ => break,
            };
            self.bump();
            op = Some(o);
            items.push(self.additive_expr()?);
        }
        Ok(RelationalExpr { items, op })
    }

    fn additive_expr(&mut self) -> PResult<AdditiveExpr> {
        let first = self.multiplicative_expr()?;
        let mut rest = Vec::new();
        loop {
            let c = match self.peek() {
                Tok::Plus => '+',
                Tok::Minus => '-',
                _ => break,
            };
            self.bump();
            rest.push((c, self.multiplicative_expr()?));
        }
        Ok(AdditiveExpr { first, rest })
    }

    fn multiplicative_expr(&mut self) -> PResult<MultiplicativeExpr> {
        let left = self.unary_expr()?;
        let op = match self.peek() {
            Tok::Mul => Some(MulOp::Mul),
            Tok::Division => Some(MulOp::Div),
            Tok::Modulo => Some(MulOp::Mod),
            _ => None,
        };
        let rest = match op {
            Some(o) => {
                self.bump();
                Some((o, Box::new(self.multiplicative_expr()?)))
            }
            None => None,
        };
        Ok(MultiplicativeExpr { left, rest })
    }

    fn unary_expr(&mut self) -> PResult<UnaryExpr> {
        // ANTLR 的 DefaultErrorStrategy.sync():在可选块 / 循环入口上,
        // 前瞻记号不在预测集里就 **consumeUntil(follow set)** —— 只报
        // "unwanted token" 不抛(DoFailOnErrorHandler 只覆写了 recover /
        // recoverInline,没覆写 sync)。`unaryExprNoRoot : sign='-'? …`
        // 的可选块就是语料里 `[@href!~='…']` 那个多余 `=` 被吞掉的地方。
        while !self.starts_unary() && self.peek() != &Tok::Eof {
            self.bump();
        }
        let neg = self.eat(&Tok::Minus);
        Ok(UnaryExpr { neg, inner: self.union_expr()? })
    }

    fn starts_unary(&self) -> bool {
        matches!(
            self.peek(),
            Tok::Minus
                | Tok::Dollar
                | Tok::LPar
                | Tok::Literal(_)
                | Tok::Number(_)
                | Tok::PathSep
                | Tok::AbrPath
                | Tok::Dot
                | Tok::DotDot
                | Tok::Mul
                | Tok::At
                | Tok::NCName(_)
                | Tok::AxisName(_)
                | Tok::NodeType(_)
                | Tok::ProcInstr
        )
    }

    fn union_expr(&mut self) -> PResult<UnionExpr> {
        let path = self.path_expr_no_root()?;
        let rest = if self.eat(&Tok::Pipe) { Some(Box::new(self.union_expr()?)) } else { None };
        Ok(UnionExpr { path, rest })
    }

    /// `locationPath | filterExpr (op relativeLocationPath)?`
    ///
    /// 两支的 FIRST 集重叠(`NCName` 既能是元素名也能是函数名)。ANTLR 用
    /// 自适应预测挑,这里等价地看:能起 primaryExpr 的记号(`$` `(` 字面量
    /// 数字)或"qName 后面紧跟 `(`"就是 filterExpr,否则走 locationPath。
    /// NodeType 不能当函数名(nCName 里没有它),所以 `text()` 永远是 nodeTest。
    fn path_expr_no_root(&mut self) -> PResult<PathExprNoRoot> {
        if self.starts_filter_expr() {
            let primary = self.primary_expr()?;
            let mut preds = Vec::new();
            while self.peek() == &Tok::LBrac {
                preds.push(self.predicate()?);
            }
            let tail = match self.peek() {
                Tok::PathSep => {
                    self.bump();
                    Some((false, self.relative_location_path()?))
                }
                Tok::AbrPath => {
                    self.bump();
                    Some((true, self.relative_location_path()?))
                }
                _ => None,
            };
            return Ok(PathExprNoRoot::Filter { primary, preds, tail });
        }
        Ok(PathExprNoRoot::Location(self.location_path()?))
    }

    fn starts_filter_expr(&self) -> bool {
        match self.peek() {
            Tok::Dollar | Tok::LPar | Tok::Literal(_) | Tok::Number(_) => true,
            Tok::NCName(_) | Tok::AxisName(_) => {
                // qName = nCName (':' nCName)*,跳过去看是不是 '('
                let mut k = 1;
                while matches!(self.at(k), Tok::Colon)
                    && matches!(self.at(k + 1), Tok::NCName(_) | Tok::AxisName(_))
                {
                    k += 2;
                }
                matches!(self.at(k), Tok::LPar)
            }
            _ => false,
        }
    }

    fn primary_expr(&mut self) -> PResult<PrimaryExpr> {
        match self.peek().clone() {
            Tok::Dollar => {
                self.bump();
                self.q_name()?;
                Ok(PrimaryExpr::VarRef)
            }
            Tok::LPar => {
                self.bump();
                let e = self.or_expr()?;
                self.expect(Tok::RPar)?;
                Ok(PrimaryExpr::Paren(Box::new(e)))
            }
            Tok::Literal(s) => {
                self.bump();
                Ok(PrimaryExpr::Literal(s))
            }
            Tok::Number(n) => {
                self.bump();
                Ok(PrimaryExpr::Number(n))
            }
            Tok::NCName(_) | Tok::AxisName(_) => {
                let name = self.q_name()?;
                self.expect(Tok::LPar)?;
                let mut args = Vec::new();
                if self.peek() != &Tok::RPar {
                    args.push(self.or_expr()?);
                    while self.eat(&Tok::Comma) {
                        args.push(self.or_expr()?);
                    }
                }
                self.expect(Tok::RPar)?;
                Ok(PrimaryExpr::Call { name, args })
            }
            _ => Err(ParseError),
        }
    }

    fn location_path(&mut self) -> PResult<LocationPath> {
        match self.peek() {
            Tok::PathSep => {
                self.bump();
                Ok(LocationPath::Absolute { abr: false, rel: self.relative_location_path()? })
            }
            Tok::AbrPath => {
                self.bump();
                Ok(LocationPath::Absolute { abr: true, rel: self.relative_location_path()? })
            }
            _ => Ok(LocationPath::Relative(self.relative_location_path()?)),
        }
    }

    fn relative_location_path(&mut self) -> PResult<RelativeLocationPath> {
        let first = self.step()?;
        let mut rest = Vec::new();
        loop {
            let abr = match self.peek() {
                Tok::PathSep => false,
                Tok::AbrPath => true,
                _ => break,
            };
            self.bump();
            // **ANTLR 的单记号删除**(`DefaultErrorStrategy.singleTokenDeletion`):
            // 当前记号开不了 step、而**下一个**开得了 → 删掉当前这个再来一次。
            // 裁判(JsoupXpath 2.5.5 跑在 ANTLR 运行时上)就是这么恢复的 ——
            // 实测 `/pc/book/{$.id}/catalog` 在它那边**不报错、返回空**:
            // 词法把 `{`/`}` 丢掉(token recognition error,我们的 lex 已经这么做),
            // 语法报 `extraneous input '$'` 后把 `$` 删掉,剩下 `/pc/book/.`
            // 求值为空,尾巴上的 `id/catalog` 被「文法没有 EOF」那条无视。
            //
            // **为什么非接不可**:语料里 20 条规则长这样,全是「其实是相对 URL、
            // 里面嵌了 `{{$.id}}` 这种规则」的写法(`/novel/{{$.novelId}}`、
            // `/cdn/book/chapterList/{{$.id}}.html`…)。它们以 `/` 开头,真身照
            // `ruleStr.startsWith("/")` 判成 XPath(AnalyzeRule.kt L688)。
            // 被测侧此前一律硬报错 —— **整步当场死掉**,而裁判只是拿到空串。
            //
            // **只在这里恢复,不在 `first` 上恢复** —— 这一条是探出来的,不是推的:
            // `/{{$.book_id}}.html` 在裁判那边**照样抛**(`no viable alternative
            // at input '/$'`)。差别在 ANTLR 的两种错法:`locationPath` 的头部是
            // 一个**预测点**(relativeLocationPath 还是 absoluteLocationPathNoroot),
            // 预测失败是 NoViableAlt,默认策略**不做**单记号删除;而 `/` 之后
            // 再匹配 step 是普通的记号失配(InputMismatch),走 recoverInline。
            // 放宽到 `first` 上会让这一条从「抛」变成「返回空」,方向反了。
            if !Self::starts_step(self.peek()) && Self::starts_step(self.at(1)) {
                self.bump();
            }
            rest.push((abr, self.step()?));
        }
        Ok(RelativeLocationPath { first, rest })
    }

    /// 一个记号能不能**开一个 step** —— 与 ANTLR 报的那份 expecting 集合同一份:
    /// `{'processing-instruction', NodeType, AxisName, '.', '*', '..', '@', NCName}`
    fn starts_step(t: &Tok) -> bool {
        matches!(
            t,
            Tok::Dot
                | Tok::DotDot
                | Tok::At
                | Tok::Mul
                | Tok::NCName(_)
                | Tok::AxisName(_)
                | Tok::NodeType(_)
                | Tok::ProcInstr
        )
    }

    fn step(&mut self) -> PResult<Step> {
        if self.eat(&Tok::DotDot) {
            return Ok(Step::DotDot);
        }
        if self.eat(&Tok::Dot) {
            return Ok(Step::Dot);
        }
        let axis = match self.peek().clone() {
            Tok::AxisName(a) if self.at(1) == &Tok::Cc => {
                self.bump();
                self.bump();
                Axis::Named(a)
            }
            Tok::At => {
                self.bump();
                Axis::At
            }
            _ => Axis::None,
        };
        let test = self.node_test()?;
        let mut preds = Vec::new();
        while self.peek() == &Tok::LBrac {
            preds.push(self.predicate()?);
        }
        Ok(Step::Full { axis, test, preds })
    }

    fn node_test(&mut self) -> PResult<NodeTest> {
        match self.peek().clone() {
            Tok::ProcInstr => {
                self.bump();
                self.expect(Tok::LPar)?;
                if !matches!(self.peek(), Tok::Literal(_)) {
                    return Err(ParseError);
                }
                self.bump();
                self.expect(Tok::RPar)?;
                Ok(NodeTest::ProcInstr)
            }
            Tok::NodeType(t) => {
                self.bump();
                self.expect(Tok::LPar)?;
                self.expect(Tok::RPar)?;
                Ok(NodeTest::Type(t))
            }
            Tok::Mul => {
                self.bump();
                Ok(NodeTest::Name { name: "*".into(), expr_str: true })
            }
            Tok::NCName(_) | Tok::AxisName(_) => {
                let first = self.n_c_name()?;
                // `ns:*` —— visitNameTest 走 nCName 那支,只留 ns 那一半
                if self.peek() == &Tok::Colon && self.at(1) == &Tok::Mul {
                    self.bump();
                    self.bump();
                    return Ok(NodeTest::Name { name: first, expr_str: true });
                }
                let mut parts = vec![first];
                while self.peek() == &Tok::Colon
                    && matches!(self.at(1), Tok::NCName(_) | Tok::AxisName(_))
                {
                    self.bump();
                    parts.push(self.n_c_name()?);
                }
                let expr_str = parts.len() == 1;
                Ok(NodeTest::Name { name: parts.join(":"), expr_str })
            }
            _ => Err(ParseError),
        }
    }

    fn predicate(&mut self) -> PResult<OrExpr> {
        self.expect(Tok::LBrac)?;
        let e = self.or_expr()?;
        self.expect(Tok::RBrac)?;
        Ok(e)
    }

    fn q_name(&mut self) -> PResult<String> {
        let mut parts = vec![self.n_c_name()?];
        while self.peek() == &Tok::Colon && matches!(self.at(1), Tok::NCName(_) | Tok::AxisName(_))
        {
            self.bump();
            parts.push(self.n_c_name()?);
        }
        Ok(parts.join(":"))
    }

    fn n_c_name(&mut self) -> PResult<String> {
        match self.bump() {
            Tok::NCName(s) | Tok::AxisName(s) => Ok(s),
            _ => Err(ParseError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    /// ANTLR 的单记号删除 —— 每一条的期望值都是拿 JsoupXpath 2.5.5 的 jar
    /// 实跑出来的(scratch 的 X2.java),不是照文法推的。
    #[test]
    fn antlr_single_token_deletion() {
        // `/` 之后再匹配 step 是记号失配 → 删掉再来 → 解析得过、求值为空。
        // 语料里 20 条「其实是相对 URL」的规则靠这条不报错。
        for ok in [
            "/pc/book/{$.id}/catalog",
            "/novel/{{$.novelId}}/chapters?readNum=1",
            "/cdn/book/content/@get:{bid}/{{$.id}}.html",
            "/api/book/{$.bookInfo._id}/comment?type=&page=1",
        ] {
            assert!(parse(ok).is_ok(), "应当恢复得了:{ok}");
        }
        // **头一个 step 上不恢复**:`locationPath` 的头部是预测点,
        // 预测失败在 ANTLR 那边是 NoViableAlt,默认策略不做单记号删除。
        // 实测裁判对这条**照样抛**(no viable alternative at input '/$')。
        assert!(parse(r#"/{{$.book_id}}.html?t=1,{"headers":{}}"#).is_err());
    }
}
