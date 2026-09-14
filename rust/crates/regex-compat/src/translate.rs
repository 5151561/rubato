//! java.util.regex 模式串 → fancy-regex 模式串的转译器。
//!
//! 原则:
//! - Java 会编译失败的输入这里也要返回 Err(两侧同走"降级为字面量替换"),
//!   错误消息不要求一致,只要求"错/不错"一致;
//! - Java 与 Rust 语义不同的构造(ASCII 的 \w\s\d\b、行终止符相关的 . ^ $ \Z、
//!   \h、\uXXXX、possessive 量词、字符类方言)全部翻译成 fancy-regex 等价形式;
//! - 已知近似:(?i) 用 Unicode 折叠近似 Java 的 ASCII 折叠;(?x) 只在类外
//!   忽略空白;
//! - 语料零命中的 Java 构造(\p \P、\x、八进制 \0nn、\c、\a \e、\Q..\E、
//!   \k<name>、\v \V \H \R、命名组 (?<name>)、(?d)、输入侧原子组 (?>、
//!   \G \X (?U))不移植:统一落 Err,两侧同走降级;语料哪天出现,
//!   差分会立刻 FAIL 照出来(2026-08-31 审计:8001 差分例 + 1695 真实
//!   pattern 全零命中)。
//!
//! 行终止符口径(Java 默认):\n \r \r\n     。

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslateError(pub String);

impl std::fmt::Display for TranslateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pattern translate error: {}", self.0)
    }
}
impl std::error::Error for TranslateError {}

#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    case_insensitive: bool, // i
    multiline: bool,        // m
    dotall: bool,           // s
    comments: bool,         // x
}

struct Translator {
    chars: Vec<char>,
    i: usize,
    out: String,
    /// 每层组的 Flags(组关闭时弹出)
    flag_stack: Vec<Flags>,
    /// 已出现的捕获组数(含命名组),用于校验反向引用
    group_count: usize,
    /// 括号在 out 中的起始位置栈:(位置, 是否 lookaround 零宽组)
    group_out_start: Vec<(usize, bool)>,
    /// 上一个原子:(out 中起始位置, 是否零宽断言);None 表示当前位置不能接量词
    last_atom_start: Option<(usize, bool)>,
}

const JAVA_SPACE: &str = "\\t\\n\\x0B\\x0C\\r ";
const JAVA_WORD: &str = "0-9A-Za-z_";
const JAVA_HORIZ: &str =
    " \\t\\x{A0}\\x{1680}\\x{180E}\\x{2000}-\\x{200A}\\x{202F}\\x{205F}\\x{3000}";

pub fn translate(pattern: &str) -> Result<String, TranslateError> {
    let mut t = Translator {
        chars: pattern.chars().collect(),
        i: 0,
        out: String::with_capacity(pattern.len() * 2),
        flag_stack: vec![Flags::default()],
        group_count: 0,
        group_out_start: Vec::new(),
        last_atom_start: None,
    };
    t.run()?;
    Ok(t.out)
}

impl Translator {
    fn err<T>(&self, msg: &str) -> Result<T, TranslateError> {
        Err(TranslateError(format!("{msg} (近位置 {})", self.i)))
    }

    fn flags(&self) -> Flags {
        *self.flag_stack.last().unwrap()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.i += 1;
        }
        c
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn run(&mut self) -> Result<(), TranslateError> {
        while let Some(c) = self.next() {
            match c {
                _ if self.flags().comments && c.is_whitespace() => {} // (?x) 类外忽略空白
                '#' if self.flags().comments => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.i += 1;
                    }
                }
                '\\' => self.escape_outside()?,
                '[' => {
                    let start = self.out.len();
                    let cls = self.parse_class()?;
                    self.out.push_str(&cls);
                    self.last_atom_start = Some((start, false));
                }
                '(' => self.open_group()?,
                ')' => {
                    if self.group_out_start.is_empty() {
                        return self.err("多余的 )");
                    }
                    self.out.push(')');
                    self.last_atom_start = Some(self.group_out_start.pop().unwrap());
                    // (pos, is_lookaround):lookaround 组整体是零宽原子
                    if self.flag_stack.len() > 1 {
                        self.flag_stack.pop();
                    }
                }
                '*' | '+' | '?' => self.quantifier(c)?,
                '{' => self.curly_quantifier()?,
                '|' => {
                    self.out.push('|');
                    self.last_atom_start = None;
                }
                '^' => {
                    let start = self.out.len();
                    self.out.push_str(&self.caret_translation());
                    self.last_atom_start = Some((start, true));
                }
                '$' => {
                    let start = self.out.len();
                    self.out.push_str(&self.dollar_translation(self.flags().multiline));
                    self.last_atom_start = Some((start, true));
                }
                '.' => {
                    let start = self.out.len();
                    let f = self.flags();
                    if f.dotall {
                        self.out.push_str("(?s:.)");
                    } else {
                        self.out.push_str("[^\\n\\r\\x{85}\\x{2028}\\x{2029}]");
                    }
                    self.last_atom_start = Some((start, false));
                }
                ']' | '}' => {
                    // Java 中裸 ] } 是字面量
                    let start = self.out.len();
                    self.out.push('\\');
                    self.out.push(c);
                    self.last_atom_start = Some((start, false));
                }
                _ => {
                    let start = self.out.len();
                    self.push_literal(c);
                    self.last_atom_start = Some((start, false));
                }
            }
        }
        if !self.group_out_start.is_empty() {
            return self.err("( 未闭合");
        }
        Ok(())
    }

    fn caret_translation(&self) -> String {
        if !self.flags().multiline {
            // 包一层使其可量化(Java 允许 ^? 这类写法)
            "(?:\\A)".into()
        } else {
            // 行首 = 输入开头,或行终止符之后(且不在输入末尾)
            "(?:\\A|(?:(?<=[\\n\\x{85}\\x{2028}\\x{2029}])|(?<=\\r)(?!\\n))(?!\\z))".into()
        }
    }

    fn dollar_translation(&self, multiline: bool) -> String {
        if multiline {
            "(?:(?=[\\r\\x{85}\\x{2028}\\x{2029}])|(?<!\\r)(?=\\n)|\\z)".into()
        } else {
            "(?:\\z|(?=\\r\\n\\z)|(?<!\\r)(?=\\n\\z)|(?=[\\r\\x{85}\\x{2028}\\x{2029}]\\z))".into()
        }
    }

    fn push_literal(&mut self, c: char) {
        // 输出侧的字面量转义:交给 fancy 的元字符都转义掉
        match c {
            '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
            | '#' | '&' | '-' | '~' => {
                self.out.push('\\');
                self.out.push(c);
            }
            _ => self.out.push(c),
        }
    }

    fn open_group(&mut self) -> Result<(), TranslateError> {
        let out_start = self.out.len();
        if !self.eat('?') {
            // 普通捕获组
            self.group_count += 1;
            self.out.push('(');
            self.flag_stack.push(self.flags());
            self.group_out_start.push((out_start, false));
            self.last_atom_start = None;
            return Ok(());
        }
        match self.peek() {
            Some(':') | Some('=') | Some('!') => {
                let c = self.next().unwrap();
                self.out.push_str("(?");
                self.out.push(c);
                self.flag_stack.push(self.flags());
                // (?= / (?! 是零宽 lookaround
                self.group_out_start.push((out_start, c != ':'));
                self.last_atom_start = None;
                Ok(())
            }
            // 输入侧原子组 (?> 语料零命中,不移植(落 inline_flags 的 Err);
            // 下面 possessive 量词翻译自己 emit 的 (?> 不受影响
            Some('<') => {
                self.next();
                match self.peek() {
                    Some('=') | Some('!') => {
                        let c = self.next().unwrap();
                        self.out.push_str("(?<");
                        self.out.push(c);
                        self.flag_stack.push(self.flags());
                        self.group_out_start.push((out_start, true));
                        self.last_atom_start = None;
                        Ok(())
                    }
                    // 命名捕获组 (?<name>) 语料零命中,不移植;出现落这里的 Err 由差分照出
                    _ => self.err("非法的 (?< 构造"),
                }
            }
            _ => self.inline_flags(out_start),
        }
    }

    /// (?idmsuxU-idmsuxU) 或 (?flags:...)
    fn inline_flags(&mut self, out_start: usize) -> Result<(), TranslateError> {
        let mut f = self.flags();
        let mut on = true;
        // i/x 需要透传给 fancy;分正负两侧收集,最后拼 (?on-off)
        let mut dir_on = String::new();
        let mut dir_off = String::new();
        loop {
            match self.peek() {
                Some('-') if on => {
                    on = false;
                    self.i += 1;
                }
                Some(c @ ('i' | 'x')) => {
                    self.i += 1;
                    if c == 'i' {
                        f.case_insensitive = on;
                    } else {
                        f.comments = on;
                    }
                    if on {
                        dir_on.push(c);
                    } else {
                        dir_off.push(c);
                    }
                }
                Some('m') => {
                    self.i += 1;
                    f.multiline = on;
                }
                Some('s') => {
                    self.i += 1;
                    f.dotall = on;
                }
                // (?d) UNIX_LINES、(?U) UNICODE_CHARACTER_CLASS 语料零命中,
                // 不移植(落下面的通用 Err)
                Some('u') => {
                    // UNICODE_CASE:对本引擎无语义(此前也只写不读),吞掉即可
                    self.i += 1;
                }
                Some(')') => {
                    self.i += 1;
                    // 无作用域:改当前层 flags,只透传 i/x
                    *self.flag_stack.last_mut().unwrap() = f;
                    if !dir_on.is_empty() || !dir_off.is_empty() {
                        self.out.push_str("(?");
                        self.out.push_str(&dir_on);
                        if !dir_off.is_empty() {
                            self.out.push('-');
                            self.out.push_str(&dir_off);
                        }
                        self.out.push(')');
                    }
                    // (?m) 这类指令不是原子,Java 里其后接量词报错
                    self.last_atom_start = None;
                    return Ok(());
                }
                Some(':') => {
                    self.i += 1;
                    // 有作用域组
                    self.out.push_str("(?");
                    self.out.push_str(&dir_on);
                    if !dir_off.is_empty() {
                        self.out.push('-');
                        self.out.push_str(&dir_off);
                    }
                    self.out.push(':');
                    self.flag_stack.push(f);
                    self.group_out_start.push((out_start, false));
                    self.last_atom_start = None;
                    return Ok(());
                }
                _ => return self.err("非法的内联 flag"),
            }
        }
    }

    fn quantifier(&mut self, q: char) -> Result<(), TranslateError> {
        let Some((atom_start, zero_width)) = self.last_atom_start else {
            return self.err("量词前无可量化原子");
        };
        if zero_width {
            // fancy 拒绝对零宽断言做量词;静态消解:
            // A+ ≡ A;A?/A* ≡ (?:A|)(懒惰版偏好空分支 (?:|A))
            let possessive = self.eat('+');
            let lazy = !possessive && self.eat('?');
            match q {
                '+' => {}
                _ => self.make_optional_zero_width(atom_start, lazy),
            }
            self.last_atom_start = None;
            return Ok(());
        }
        if self.eat('+') {
            // possessive → 原子组
            self.out.insert_str(atom_start, "(?>");
            self.out.push(q);
            self.out.push(')');
        } else if self.eat('?') {
            self.out.push(q);
            self.out.push('?');
        } else {
            self.out.push(q);
        }
        // 量词后的原子 = 原原子+量词整体;Java 不允许再接量词(a** 报错)
        self.last_atom_start = None;
        Ok(())
    }

    /// 把 out 中 atom_start 起的零宽原子 A 改写为 (?:A|) / (?:|A)
    fn make_optional_zero_width(&mut self, atom_start: usize, lazy: bool) {
        if lazy {
            self.out.insert_str(atom_start, "(?:|");
            self.out.push(')');
        } else {
            self.out.insert_str(atom_start, "(?:");
            self.out.push_str("|)");
        }
    }

    fn curly_quantifier(&mut self) -> Result<(), TranslateError> {
        // Java:{ 若不构成合法 {n}/{n,}/{n,m} 直接编译错误
        let Some((atom_start, zero_width)) = self.last_atom_start else {
            return self.err("量词前无可量化原子");
        };
        let mut body = String::from("{");
        let mut saw_digit = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                saw_digit = true;
                body.push(c);
                self.i += 1;
            } else if c == ',' && !body.contains(',') && saw_digit {
                body.push(',');
                self.i += 1;
                // 上界数字可选,继续吃数字
                while let Some(c2) = self.peek() {
                    if c2.is_ascii_digit() {
                        body.push(c2);
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                break;
            } else {
                break;
            }
        }
        if !self.eat('}') || body == "{" || body.ends_with('{') {
            return self.err("非法的 {} 重复");
        }
        body.push('}');
        // {2} 形式要求至少一个数字
        if !body[1..body.len() - 1].chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return self.err("非法的 {} 重复");
        }
        if zero_width {
            // 零宽原子:{0,..} → 可选,{n>=1,..} → A 本身
            let possessive = self.eat('+');
            let lazy = !possessive && self.eat('?');
            let min_is_zero = body[1..].starts_with('0');
            if min_is_zero {
                self.make_optional_zero_width(atom_start, lazy);
            }
            self.last_atom_start = None;
            return Ok(());
        }
        if self.eat('+') {
            self.out.insert_str(atom_start, "(?>");
            self.out.push_str(&body);
            self.out.push(')');
        } else if self.eat('?') {
            self.out.push_str(&body);
            self.out.push('?');
        } else {
            self.out.push_str(&body);
        }
        self.last_atom_start = None;
        Ok(())
    }

    /// 类外转义
    fn escape_outside(&mut self) -> Result<(), TranslateError> {
        let start = self.out.len();
        let mut zw = false; // 该转义是否零宽断言
        let Some(c) = self.next() else {
            return self.err("末尾悬空的 \\");
        };
        match c {
            'd' => self.out.push_str("[0-9]"),
            'D' => self.out.push_str("[^0-9]"),
            's' => {
                self.out.push('[');
                self.out.push_str(JAVA_SPACE);
                self.out.push(']');
            }
            'S' => {
                self.out.push_str("[^");
                self.out.push_str(JAVA_SPACE);
                self.out.push(']');
            }
            'w' => {
                self.out.push('[');
                self.out.push_str(JAVA_WORD);
                self.out.push(']');
            }
            'W' => {
                self.out.push_str("[^");
                self.out.push_str(JAVA_WORD);
                self.out.push(']');
            }
            'h' => {
                self.out.push('[');
                self.out.push_str(JAVA_HORIZ);
                self.out.push(']');
            }
            // \H \v \V \R:语料零命中(\h 有 109 次,保留),不移植;
            // 出现会落下方"非法转义"Err,由差分照出
            'b' => {
                // Java ASCII 词边界,用 lookaround 精确表达(锚点也是可量化原子)
                self.out.push_str(
                    "(?:(?<=[0-9A-Za-z_])(?![0-9A-Za-z_])|(?<![0-9A-Za-z_])(?=[0-9A-Za-z_]))",
                );
                zw = true;
            }
            'B' => {
                self.out.push_str(
                    "(?:(?<=[0-9A-Za-z_])(?=[0-9A-Za-z_])|(?<![0-9A-Za-z_])(?![0-9A-Za-z_]))",
                );
                zw = true;
            }
            'A' => {
                self.out.push_str("(?:\\A)");
                zw = true;
            }
            'z' => {
                self.out.push_str("(?:\\z)");
                zw = true;
            }
            'Z' => {
                // 末终止符之前(等价非多行 $ 的位置断言)
                let s = self.dollar_translation(false);
                self.out.push_str(&s);
                zw = true;
            }
            // \G \X、\Q..\E、\k<name>、\p \P:语料零命中,不移植;
            // 全部落下方"非法转义"Err(Java 侧 \Q \k \p 合法,出现即差分 FAIL 照出)
            'u' => {
                let v = self.hex_fixed(4)?;
                self.out.push_str(&format!("\\x{{{v:X}}}"));
            }
            // \xHH \x{...}、八进制 \0nn、\cX、\a \e:语料零命中(\uXXXX 有命中,
            // 保留),不移植;落下方"非法转义"Err,由差分照出
            't' => self.out.push_str("\\t"),
            'n' => self.out.push_str("\\n"),
            'r' => self.out.push_str("\\r"),
            'f' => self.out.push_str("\\x{C}"),
            '1'..='9' => {
                // 反向引用:与 Java 一致,贪婪吃数字直到超过已见组数
                let mut num = c.to_digit(10).unwrap() as usize;
                while let Some(d) = self.peek().and_then(|c| c.to_digit(10)) {
                    let nv = num * 10 + d as usize;
                    if nv <= self.group_count {
                        num = nv;
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                if num > self.group_count {
                    return self.err("反向引用的组不存在");
                }
                self.out.push_str(&format!("\\{num}"));
            }
            _ if c.is_ascii_alphanumeric() => {
                // Java:仅转义未定义的 ASCII 字母/数字是编译错误;
                // \レ \作 这类非 ASCII 转义是合法字面量
                return self.err("非法转义");
            }
            _ => self.push_literal(c),
        }
        self.last_atom_start = Some((start, zw));
        Ok(())
    }

    fn hex_fixed(&mut self, n: usize) -> Result<u32, TranslateError> {
        let mut v = 0u32;
        for _ in 0..n {
            let d = self
                .peek()
                .and_then(|c| c.to_digit(16))
                .ok_or_else(|| TranslateError("非法十六进制转义".into()))?;
            v = v * 16 + d;
            self.i += 1;
        }
        Ok(v)
    }

    /// 字符类解析(Java 方言),返回翻译后的整段类
    fn parse_class(&mut self) -> Result<String, TranslateError> {
        let mut out = String::from("[");
        if self.eat('^') {
            out.push('^');
        }
        if self.peek() == Some(']') {
            // Java:类首的 ] 直接视为未闭合错误
            return self.err("字符类为空或未闭合");
        }
        let mut any_item = false;
        loop {
            let Some(c) = self.peek() else {
                return self.err("字符类未闭合");
            };
            match c {
                ']' => {
                    self.i += 1;
                    if !any_item {
                        return self.err("空字符类");
                    }
                    out.push(']');
                    return Ok(out);
                }
                '[' => {
                    // 嵌套类(并集)
                    self.i += 1;
                    let inner = self.parse_class()?;
                    out.push_str(&inner);
                    any_item = true;
                }
                '&' => {
                    self.i += 1;
                    if self.eat('&') {
                        out.push_str("&&");
                        // && 后可以直接接嵌套类或普通项,循环继续处理
                    } else {
                        out.push_str("\\&");
                        any_item = true;
                    }
                }
                _ => {
                    // 单项:字面量或转义,之后可能是 '-' 范围
                    let lo = self.class_single()?;
                    match lo {
                        ClassItem::Char(lo_c) => {
                            if self.peek() == Some('-')
                                && self
                                    .chars
                                    .get(self.i + 1)
                                    .copied()
                                    .is_some_and(|n| n != ']' && n != '[')
                            {
                                self.i += 1; // '-'
                                let hi = self.class_single()?;
                                match hi {
                                    ClassItem::Char(hi_c) => {
                                        if (lo_c as u32) > (hi_c as u32) {
                                            return self.err("非法字符范围");
                                        }
                                        push_class_char(&mut out, lo_c);
                                        out.push('-');
                                        push_class_char(&mut out, hi_c);
                                    }
                                    ClassItem::Set(_) => return self.err("范围端点非字符"),
                                }
                            } else {
                                push_class_char(&mut out, lo_c);
                            }
                        }
                        ClassItem::Set(s) => out.push_str(&s),
                    }
                    any_item = true;
                }
            }
        }
    }

    /// 类内单项:普通字符或转义;Set 表示展开后的子集(如 \d → "0-9")
    fn class_single(&mut self) -> Result<ClassItem, TranslateError> {
        let c = self.next().unwrap();
        if c != '\\' {
            return Ok(ClassItem::Char(c));
        }
        let Some(e) = self.next() else {
            return self.err("类内末尾悬空的 \\");
        };
        Ok(match e {
            'd' => ClassItem::Set("0-9".into()),
            'D' => ClassItem::Set("[^0-9]".into()),
            's' => ClassItem::Set(JAVA_SPACE.into()),
            'S' => ClassItem::Set(format!("[^{JAVA_SPACE}]")),
            'w' => ClassItem::Set(JAVA_WORD.into()),
            'W' => ClassItem::Set(format!("[^{JAVA_WORD}]")),
            'h' => ClassItem::Set(JAVA_HORIZ.into()),
            // 类内 \H \v \V、\p \P:语料零命中,不移植;落下方"类内非法转义"Err
            'u' => ClassItem::Char(
                char::from_u32(self.hex_fixed(4)?)
                    .ok_or_else(|| TranslateError("非法 \\u 码点".into()))?,
            ),
            // 类内 \xHH \x{...}、八进制 \0nn、\cX、\a \e:语料零命中,不移植;
            // 落下方"类内非法转义"Err,由差分照出
            't' => ClassItem::Char('\t'),
            'n' => ClassItem::Char('\n'),
            'r' => ClassItem::Char('\r'),
            'f' => ClassItem::Char('\x0C'),
            'b' => return self.err("类内 \\b 非法"), // Java 同样报错
            'Q' => return self.err("类内 \\Q 暂不支持"),
            _ if e.is_ascii_alphanumeric() => return self.err("类内非法转义"),
            _ => ClassItem::Char(e),
        })
    }
}

enum ClassItem {
    Char(char),
    /// 已翻译好的类内片段(如 "0-9"、"[^0-9]" 嵌套)
    Set(String),
}

fn push_class_char(out: &mut String, c: char) {
    match c {
        '\\' | ']' | '[' | '^' | '&' | '-' | '~' => {
            out.push('\\');
            out.push(c);
        }
        _ => out.push(c),
    }
}
