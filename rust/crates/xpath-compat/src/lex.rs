//! JsoupXpath 2.5.5 的 ANTLR 词法器(`XpathLexer`)复刻。
//!
//! 记号表逐条抄自 jar 里 `XpathLexer.makeLiteralNames/makeSymbolicNames` 与
//! 序列化 ATN(ANTLR 的序列化把每个码点 +2,解出来就是标准 XML 的
//! NCNameStartChar / NCNameChar 两张表)。
//!
//! 两条 ANTLR 规矩决定了很多"怪"行为,必须照做:
//! 1. **最长匹配**,长度相同时**先声明的规则赢**。规则声明序是
//!    `'processing-instruction' > 'or' > 'and' > '$' > NodeType > Number >
//!     AxisName > …符号… > Literal > Whitespace > NCName`。
//!    于是 `html` / `text` / `node` / `num` / `comment` / `allText` /
//!    `outerHtml` 是 **NodeType 保留字**,写成元素名根本选不中 ——
//!    `/html` 在真身里是语法错误,不是"选不到"。
//!    而 `htmls` 更长,落回 NCName。
//! 2. **词法错误只打印不抛**(抛的是 parser 的 DoFailOnErrorHandler)。
//!    于是 `//td%%//td` 里的 `%%` 被**整个丢掉**,剩下的照常解析成
//!    `//td//td` —— 差分实测如此。

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    /// 'processing-instruction'(独立字面量,优先于 NodeType)
    ProcInstr,
    Or,
    And,
    Dollar,
    NodeType(String),
    Number(f64),
    AxisName(String),
    PathSep, // /
    AbrPath, // //
    LPar,
    RPar,
    LBrac,
    RBrac,
    Minus,
    Plus,
    Dot,
    Mul,      // *
    Division, // `div`
    Modulo,   // `mod`
    DotDot,
    At,
    Comma,
    Pipe,
    Less,
    More,
    Le,
    Ge,
    Equality,
    Inequality,
    StartWith,     // ^=
    EndWith,       // $=
    ContainWith,   // *=
    RegexpWith,    // ~=
    RegexpNotWith, // !~
    Colon,
    Cc, // ::
    Literal(String),
    NCName(String),
    Eof,
}

const NODE_TYPES: [&str; 8] =
    ["comment", "text", "processing-instruction", "node", "num", "allText", "outerHtml", "html"];

const AXIS_NAMES: [&str; 15] = [
    "ancestor",
    "ancestor-or-self",
    "attribute",
    "child",
    "descendant",
    "descendant-or-self",
    "following",
    "following-sibling",
    "parent",
    "preceding",
    "preceding-sibling",
    "self",
    "following-sibling-one",
    "preceding-sibling-one",
    "sibling",
];

fn is_ncname_start(c: char) -> bool {
    matches!(c,
        'A'..='Z' | '_' | 'a'..='z'
        | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}'
        | '\u{200C}'..='\u{200D}' | '\u{2070}'..='\u{218F}'
        | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}')
}

fn is_ncname_char(c: char) -> bool {
    is_ncname_start(c)
        || matches!(c,
            '-' | '.' | '0'..='9' | '\u{B7}'
            | '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}')
}

/// 切一遍记号。词法层不报错:认不出的字符直接丢(ANTLR 的默认
/// `LexerErrorListener` 只打印 `token recognition error`)。
pub fn lex(src: &str) -> Vec<Tok> {
    let cs: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < cs.len() {
        let c = cs[i];
        if matches!(c, ' ' | '\t' | '\r' | '\n') {
            i += 1;
            continue;
        }
        // 关键字与名字:先按最长 NCName 取,再看它是不是保留字
        if is_ncname_start(c) {
            let start = i;
            while i < cs.len() && is_ncname_char(cs[i]) {
                i += 1;
            }
            let word: String = cs[start..i].iter().collect();
            // 'processing-instruction' / 'or' / 'and' 声明在 NodeType 之前
            out.push(match word.as_str() {
                "processing-instruction" => Tok::ProcInstr,
                "or" => Tok::Or,
                "and" => Tok::And,
                w if NODE_TYPES.contains(&w) => Tok::NodeType(word.clone()),
                w if AXIS_NAMES.contains(&w) => Tok::AxisName(word.clone()),
                _ => Tok::NCName(word.clone()),
            });
            continue;
        }
        // Number: Digits ('.' Digits?)? | '.' Digits
        if c.is_ascii_digit() {
            let start = i;
            while i < cs.len() && cs[i].is_ascii_digit() {
                i += 1;
            }
            if i < cs.len() && cs[i] == '.' {
                i += 1;
                while i < cs.len() && cs[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let s: String = cs[start..i].iter().collect();
            out.push(Tok::Number(s.parse().unwrap_or(0.0)));
            continue;
        }
        if c == '.' && i + 1 < cs.len() && cs[i + 1].is_ascii_digit() {
            let start = i;
            i += 1;
            while i < cs.len() && cs[i].is_ascii_digit() {
                i += 1;
            }
            let s: String = cs[start..i].iter().collect();
            out.push(Tok::Number(s.parse().unwrap_or(0.0)));
            continue;
        }
        // Literal:引号内**不做转义**,到下一个同款引号为止;
        // 不闭合就整段吃不下(ANTLR 词法失败 → 跳一个字符继续)
        if c == '"' || c == '\'' {
            if let Some(end) = cs[i + 1..].iter().position(|&x| x == c) {
                let s: String = cs[i + 1..i + 1 + end].iter().collect();
                // 真身把整段(含引号)当 Literal 文本,exprStr() 再剥引号 ——
                // 这里直接给剥好的值,语义等价
                out.push(Tok::Literal(s));
                i += end + 2;
            } else {
                i += 1; // 认不出,丢
            }
            continue;
        }
        // `div` / `mod`
        if c == '`' {
            let rest: String = cs[i..].iter().collect();
            if rest.starts_with("`div`") {
                out.push(Tok::Division);
                i += 5;
                continue;
            }
            if rest.starts_with("`mod`") {
                out.push(Tok::Modulo);
                i += 5;
                continue;
            }
            i += 1;
            continue;
        }
        let two: Option<(char, char)> = cs.get(i + 1).map(|&n| (c, n));
        let (tok, len): (Option<Tok>, usize) = match two {
            Some(('/', '/')) => (Some(Tok::AbrPath), 2),
            Some(('.', '.')) => (Some(Tok::DotDot), 2),
            Some((':', ':')) => (Some(Tok::Cc), 2),
            Some(('<', '=')) => (Some(Tok::Le), 2),
            Some(('>', '=')) => (Some(Tok::Ge), 2),
            Some(('!', '=')) => (Some(Tok::Inequality), 2),
            Some(('^', '=')) => (Some(Tok::StartWith), 2),
            Some(('$', '=')) => (Some(Tok::EndWith), 2),
            Some(('*', '=')) => (Some(Tok::ContainWith), 2),
            Some(('~', '=')) => (Some(Tok::RegexpWith), 2),
            Some(('!', '~')) => (Some(Tok::RegexpNotWith), 2),
            _ => (None, 0),
        };
        if let Some(t) = tok {
            out.push(t);
            i += len;
            continue;
        }
        let one = match c {
            '/' => Some(Tok::PathSep),
            '(' => Some(Tok::LPar),
            ')' => Some(Tok::RPar),
            '[' => Some(Tok::LBrac),
            ']' => Some(Tok::RBrac),
            '-' => Some(Tok::Minus),
            '+' => Some(Tok::Plus),
            '.' => Some(Tok::Dot),
            '*' => Some(Tok::Mul),
            '@' => Some(Tok::At),
            ',' => Some(Tok::Comma),
            '|' => Some(Tok::Pipe),
            '<' => Some(Tok::Less),
            '>' => Some(Tok::More),
            '=' => Some(Tok::Equality),
            ':' => Some(Tok::Colon),
            '$' => Some(Tok::Dollar),
            _ => None,
        };
        i += 1;
        if let Some(t) = one {
            out.push(t);
        }
        // None:token recognition error —— 丢掉,继续
    }
    out.push(Tok::Eof);
    out
}
