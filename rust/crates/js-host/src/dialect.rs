//! **Rhino 收、规范不收的写法** —— 书源里真实存在的方言,复刻它才叫兼容。
//!
//! 目前只有一条:**没加括号的解构箭头参**
//! (`list.map([title, url] => title + url)`)。Rhino 先把 `[title, url]` 当
//! 数组字面量解析,见到 `=>` 再把它改判成参数模式;规范(与 quickjs-ng)要求
//! 写成 `([title, url]) => …`,否则 `SyntaxError: unexpected token '=>'`。
//!
//! **为什么要接**:语料 1704 源里有 **5 个源**这么写(全是「探索页 JSON 用
//! `.map` 拼列表」那一批,js-host 套的 5db449eef0 / 6689925092 / a08f654b9a /
//! b67c744eaa / e302aeb992)。它们在 legado 上跑得动 —— 不接就是这五个源在
//! Rubato 上直接失效,是**用户看得见的退化**。反向的那一档(`class`、展开语法、
//! 正则里的 `\p`:quickjs-ng 收而 Rhino 不收)不在这里 —— 被测侧是超集,
//! 书源只会更跑得动,那一档记在 fixtures/cases/js-host/exemptions.json。
//!
//! **落地方式**:只在**已经报了 SyntaxError 之后**重写一次再跑。正常路径一个
//! 字节都不动;重写完还是错就报**原来那条错** —— 垫片不许把真因盖住
//! (「归一之前先把真因打到 stderr」的同一条规矩)。

/// 词法扫描里「上一个有意义的字符」允许 `/` 开正则的位置。
/// 分不清正则与除号会让扫描跑偏,但**跑偏只会少补或补错**:补错的结果解析不过,
/// 调用方就退回原来的错误。
fn regex_can_start(prev: Option<char>, prev_word: &str) -> bool {
    match prev {
        None => true,
        Some(c) if "(,=:[!&|?{};+-*%~^<>".contains(c) => true,
        Some(_) => matches!(
            prev_word,
            "return"
                | "typeof"
                | "case"
                | "in"
                | "of"
                | "instanceof"
                | "new"
                | "delete"
                | "void"
                | "do"
                | "else"
                | "yield"
                | "await"
        ),
    }
}

/// 关键字之后的 `[` 也可能是参数表(`return [a,b] => a`)
const KEYWORDS: [&str; 10] =
    ["return", "typeof", "case", "in", "of", "new", "do", "else", "yield", "await"];

/// `[` 落在「能开一个表达式」的位置(而不是 `a[i]` 这种下标 / 成员访问)
fn param_position(b: &[char], open: usize, prev_word: &str) -> bool {
    let mut i = open;
    while i > 0 {
        let c = b[i - 1];
        if c.is_whitespace() {
            i -= 1;
            continue;
        }
        if c.is_alphanumeric() || c == '_' || c == '$' {
            return KEYWORDS.contains(&prev_word);
        }
        return !matches!(c, ')' | ']' | '}' | '.' | '\'' | '"' | '`');
    }
    true // 整段的开头
}

/// 跳过一个字符串 / 模板串,返回收尾引号**之后**的下标
fn skip_string(b: &[char], start: usize) -> usize {
    let q = b[start];
    let mut i = start + 1;
    while i < b.len() {
        match b[i] {
            '\\' => i += 2,
            c if c == q => return i + 1,
            // 模板串里的 `${…}`:里面是完整表达式,可能再嵌字符串
            '$' if q == '`' && i + 1 < b.len() && b[i + 1] == '{' => {
                let mut depth = 1;
                i += 2;
                while i < b.len() && depth > 0 {
                    match b[i] {
                        '{' => depth += 1,
                        '}' => depth -= 1,
                        '\'' | '"' | '`' => {
                            i = skip_string(b, i);
                            continue;
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    i
}

/// 跳过一个正则字面量(含 `[…]` 字符组里的 `/`),返回标志位之后的下标
fn skip_regex(b: &[char], start: usize) -> usize {
    let mut i = start + 1;
    let mut in_class = false;
    while i < b.len() {
        match b[i] {
            '\\' => i += 1,
            '[' => in_class = true,
            ']' => in_class = false,
            '/' if !in_class => {
                i += 1;
                while i < b.len() && b[i].is_ascii_alphabetic() {
                    i += 1;
                }
                return i;
            }
            '\n' => return i, // 没闭合:不是正则,别把后面全吞掉
            _ => {}
        }
        i += 1;
    }
    i
}

/// 把 `[a, b] => …` 补成 `([a, b]) => …`;没有可补的地方返回 `None`。
pub(crate) fn parenthesize_arrow_params(src: &str) -> Option<String> {
    let b: Vec<char> = src.chars().collect();
    let mut edits: Vec<(usize, usize)> = Vec::new(); // (`[` 下标, `]` 下标)
    // 栈里连着记 `[` **之前**的那个词:到 `]` 的时候 prev_word 早清空了,
    // 而 param_position 判 `return [a,b]=>` 要用它
    let mut stack: Vec<(char, usize, String)> = Vec::new();
    // 刚刚闭合的那对方括号(`=>` 只认紧挨着的那一对,中间只许空白)
    let mut last_bracket: Option<(usize, usize, String)> = None;
    let mut prev: Option<char> = None;
    let mut prev_word = String::new();
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '\'' | '"' | '`' => {
                i = skip_string(&b, i);
                prev = Some(c);
                prev_word.clear();
                continue;
            }
            '/' if i + 1 < b.len() && b[i + 1] == '/' => {
                while i < b.len() && b[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '/' if i + 1 < b.len() && b[i + 1] == '*' => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
                continue;
            }
            '/' if regex_can_start(prev, &prev_word) => {
                i = skip_regex(&b, i);
                prev = Some('/');
                prev_word.clear();
                continue;
            }
            '=' if i + 1 < b.len() && b[i + 1] == '>' => {
                if let Some((oi, ci, word)) = last_bracket.take() {
                    if b[ci + 1..i].iter().all(|c| c.is_whitespace())
                        && param_position(&b, oi, &word)
                    {
                        edits.push((oi, ci));
                    }
                }
                i += 2;
                prev = Some('>');
                prev_word.clear();
                continue;
            }
            '(' | '[' | '{' => {
                stack.push((c, i, prev_word.clone()));
                last_bracket = None;
            }
            ')' | ']' | '}' => {
                match stack.pop() {
                    Some(('[', oi, word)) if c == ']' => {
                        last_bracket = Some((oi, i, word));
                    }
                    _ => last_bracket = None,
                }
                prev = Some(c);
                prev_word.clear();
                i += 1;
                continue;
            }
            _ => {}
        }
        if c.is_alphanumeric() || c == '_' || c == '$' {
            prev_word.push(c);
        } else {
            prev_word.clear();
        }
        prev = Some(c);
        i += 1;
    }

    if edits.is_empty() {
        return None;
    }
    let mut out = b;
    for (oi, ci) in edits.into_iter().rev() {
        out.insert(ci + 1, ')');
        out.insert(oi, '(');
    }
    Some(out.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::parenthesize_arrow_params as fix;

    #[test]
    fn wraps_unparenthesized_destructuring_param() {
        assert_eq!(fix("list.map([a,b]=>a+b)").unwrap(), "list.map(([a,b])=>a+b)");
        assert_eq!(fix("var f = [a, b] => a;").unwrap(), "var f = ([a, b]) => a;");
        // 多处 + 嵌套
        assert_eq!(fix("x.map([a]=>a).map([b]=>b)").unwrap(), "x.map(([a])=>a).map(([b])=>b)");
    }

    #[test]
    fn leaves_everything_else_alone() {
        // 下标 / 成员访问后面的 `=>` 不是参数表
        assert!(fix("a[0]=>b").is_none());
        assert!(fix("f()[x]=>1").is_none());
        // 已经加了括号的
        assert!(fix("list.map(([a,b])=>a)").is_none());
        // 普通代码
        assert!(fix("var x = [1,2,3]; x.map(function (v) { return v; })").is_none());
        assert!(fix("x => x + 1").is_none());
    }

    #[test]
    fn does_not_reach_into_strings_regex_or_comments() {
        assert!(fix("var s = '[a,b]=>c';").is_none());
        assert!(fix("var s = \"[a,b]=>c\";").is_none());
        assert!(fix("var r = /[ab]=>/;").is_none());
        assert!(fix("// [a,b]=>c\nvar x = 1;").is_none());
        assert!(fix("/* [a,b]=>c */ var x = 1;").is_none());
        assert!(fix("var s = `x${'[a]=>b'}y`;").is_none());
    }
}

// ---------------------------------------------------------------------------
// 完成值:`if` / `for` / `while` / `try` 收尾的脚本
// ---------------------------------------------------------------------------

/// 能开一个「完成值可能为空」的语句的关键字。
///
/// 规范(ES2024 §16.1.7,StatementList : StatementList StatementListItem →
/// `UpdateEmpty(s, sl)`)说:一条语句的完成值为**空**时,整段的完成值**留用上一条**。
/// `x=1; if(false){2}` 因此是 `1` —— Rhino 与 V8 都给 1。
///
/// **quickjs-ng 在这几个语句上不照办**:它在进入这些语句时把 eval 结果寄存器
/// 直接置成 undefined,于是同一段给 undefined。实测(fixtures/cases/js-host 的
/// `cv-` 一族探针):`;` / `{}` / `var` / `function` 四种它是对的,
/// `if` / `for` / `while` / `try` 四种是错的。
///
/// **这不是「裁判不合规范」那一档**,方向反过来 —— 被测侧少一块能力,
/// 书源会因此退化:`<js>` 以 `if` 收尾、条件不成立时,legado 交回上一句的值
/// (常常正是那个书单/元素表),Rubato 交回 null,整条规则当场空手。
const VALUE_EMPTY_HEADS: [&str; 7] = ["if", "for", "while", "do", "try", "switch", "with"];

/// `}` 之后接这几个词的不是新语句,是**上一条语句的续**。
/// (`while` 只在 `}` 之后才有歧义 —— do-while 的收尾;`;` 之后的 `while` 是新语句。)
const CONTINUATIONS: [&str; 3] = ["else", "catch", "finally"];

/// 跳过空白与注释,返回下一个有意义字符的下标
fn skip_trivia(b: &[char], mut i: usize) -> usize {
    loop {
        while i < b.len() && b[i].is_whitespace() {
            i += 1;
        }
        if i + 1 < b.len() && b[i] == '/' && b[i + 1] == '/' {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < b.len() && b[i] == '/' && b[i + 1] == '*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
            continue;
        }
        return i;
    }
}

/// 从 `i` 起读一个标识符(读不到给空串)
fn word_at(b: &[char], i: usize) -> String {
    let mut w = String::new();
    let mut j = i;
    while j < b.len() && (b[j].is_alphanumeric() || b[j] == '_' || b[j] == '$') {
        w.push(b[j]);
        j += 1;
    }
    w
}

/// 顶层语句边界:`;` 之后、以及把括号深度收回 0 的 `}` 之后。
/// 词法状态(字符串/模板串/正则/注释)与 [`parenthesize_arrow_params`] 同一套。
/// 返回 `(下标, 是不是由 `}` 产生的)`。
fn top_level_boundaries(b: &[char]) -> Vec<(usize, bool)> {
    let mut out = vec![(0usize, false)];
    let mut depth = 0i32;
    let mut prev: Option<char> = None;
    let mut prev_word = String::new();
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '\'' | '"' | '`' => {
                i = skip_string(b, i);
                prev = Some(c);
                prev_word.clear();
                continue;
            }
            '/' if i + 1 < b.len() && b[i + 1] == '/' => {
                while i < b.len() && b[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '/' if i + 1 < b.len() && b[i + 1] == '*' => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
                continue;
            }
            '/' if regex_can_start(prev, &prev_word) => {
                i = skip_regex(b, i);
                prev = Some('/');
                prev_word.clear();
                continue;
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 && c == '}' {
                    out.push((i + 1, true));
                }
            }
            ';' if depth == 0 => out.push((i + 1, false)),
            _ => {}
        }
        if c.is_alphanumeric() || c == '_' || c == '$' {
            prev_word.push(c);
        } else {
            prev_word.clear();
        }
        prev = Some(c);
        i += 1;
    }
    out
}

/// 把脚本切成**按顺序求值**的几段,末尾几段各是一条「完成值可能为空」的语句。
///
/// 调用方顺序求值每一段、取**最后一个非 undefined** 的完成值 —— 这正是
/// `UpdateEmpty` 那条规则,只是把「空完成值」近似成了 undefined。近似只在
/// 「末段的完成值**真的是** undefined」时失真(`if(true){undefined}`),
/// 那一档规范给 undefined、这里会退回上一段的值;语料里没有这种写法。
///
/// 段与段之间用**同一个 realm 的多次全局 eval**,`var` / `let` / `function` /
/// 未声明赋值四种绑定都是跨 eval 共享的(js-host 的 `cross_eval_scope_is_shared`
/// 单测钉住)。唯一的口径差是**提升**:末段里 `if(c){var y}` 的 `y`,不切时在
/// 整段开头就有(值 undefined),切开后要等那一段跑起来才有 —— 调用方靠
/// 「先拿 `if(0){…}` 把每段编译一遍」补上(既做语法预检,又把 var 提升做了)。
///
/// 返回 `None` = 不该切(末条语句不是这一档,或者切完头是空的)。
pub(crate) fn split_value_tail(src: &str) -> Option<Vec<String>> {
    let b: Vec<char> = src.chars().collect();
    let bounds = top_level_boundaries(&b);
    let mut cuts: Vec<usize> = Vec::new();
    let mut end = b.len();
    loop {
        // 从最后一个边界往前找「真正的语句开头」:`}` 之后的 else/catch/finally
        // 是上一条的续,`}` 之后的 while 是 do-while 的收尾
        let mut start = None;
        for &(pos, from_brace) in bounds.iter().rev() {
            if pos >= end {
                continue;
            }
            let t = skip_trivia(&b, pos);
            if t >= end {
                continue;
            }
            let w = word_at(&b, t);
            if CONTINUATIONS.contains(&w.as_str()) || (from_brace && w == "while") {
                continue;
            }
            start = Some((pos, t, w));
            break;
        }
        let Some((pos, t, w)) = start else { break };
        if !VALUE_EMPTY_HEADS.contains(&w.as_str()) {
            break;
        }
        // 头得有东西:整段就这一条语句时,quickjs 给 undefined 本来就是对的
        if skip_trivia(&b, 0) >= t {
            break;
        }
        cuts.push(t);
        end = pos.min(t);
    }
    if cuts.is_empty() {
        return None;
    }
    cuts.reverse();
    let mut segs = Vec::with_capacity(cuts.len() + 1);
    let mut from = 0usize;
    for c in cuts {
        segs.push(b[from..c].iter().collect::<String>());
        from = c;
    }
    segs.push(b[from..].iter().collect::<String>());
    Some(segs)
}

#[cfg(test)]
mod completion_tests {
    use super::split_value_tail as split;

    #[test]
    fn splits_off_a_value_empty_tail() {
        assert_eq!(split("x=1; if(false){2}").unwrap(), vec!["x=1; ", "if(false){2}"]);
        assert_eq!(split("x=1; while(false){3}").unwrap(), vec!["x=1; ", "while(false){3}"]);
        assert_eq!(split("x=1; try{}finally{}").unwrap(), vec!["x=1; ", "try{}finally{}"]);
        // `}` 之后的 else 是上一条的续,不是新语句
        assert_eq!(split("x=1; if(a){}else{}").unwrap(), vec!["x=1; ", "if(a){}else{}"]);
        // 连着两条:各切一刀,顺序求值取最后一个非 undefined
        assert_eq!(split("x=1; if(a){} if(b){}").unwrap(), vec!["x=1; ", "if(a){} ", "if(b){}"]);
    }

    #[test]
    fn leaves_everything_else_alone() {
        // 末条不是这一档 —— quickjs 本来就给对
        assert!(split("if(false){2} x=1").is_none());
        assert!(split("x=1; var y=2").is_none());
        assert!(split("x=1; function f(){}").is_none());
        assert!(split("x=1; y=2").is_none());
        // 整段就一条:给 undefined 是对的
        assert!(split("if(false){2}").is_none());
        assert!(split("   if(false){2}  ").is_none());
        // do-while 的收尾不是新的 while 语句
        assert!(split("do{x=1}while(false)").is_none());
        // 字符串 / 正则 / 注释里的分号与花括号不算边界
        assert!(split("x='a;if(b){}'").is_none());
        assert!(split("x=/;if(b){}/").is_none());
        assert!(split("// x=1; if(b){}\ny=2").is_none());
    }

    #[test]
    fn head_keeps_its_own_trailing_tail() {
        let segs = split("a=1;\nfunction f(){ if(x){} }\nif(y){}").unwrap();
        assert_eq!(segs.len(), 2);
        assert!(segs[0].contains("function f()"));
        assert_eq!(segs[1], "if(y){}");
    }
}
