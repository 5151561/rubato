//! java.util.regex 行为兼容层(fancy-regex 垫片)。
//!
//! 覆盖书源规则层用到的正则面:`##pattern##replacement###` 的编译、匹配、
//! 替换语义,含 AnalyzeRule.replaceRegex(裁判 L482-505)的完整降级行为:
//! - 编译失败:replaceAll 路径降级为字面量替换;replaceFirst 路径直接返回 replacement
//! - 替换串展开抛错(坏的 $ 引用等):同上降级
//! - replaceFirst(###):取首个匹配的 value,再对 value 自身 replaceFirst
//!
//! 已知近似(与裁判 diff 观测):(?i) 用 Unicode 折叠近似 Java 的 ASCII 折叠;
//! 空匹配推进按 char(Java 按 UTF-16 code unit,仅增补平面字符有差)。

mod translate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JavaRegexError {
    /// 转译或 fancy 编译失败(对应 Java PatternSyntaxException)
    Compile(String),
    /// 匹配期失败(fancy 回溯上限等;Java 侧对应 StackOverflowError 一类)
    Runtime(String),
    /// 替换串展开失败(对应 Java IllegalArgumentException/IndexOutOfBoundsException)
    Replacement(String),
}

impl std::fmt::Display for JavaRegexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JavaRegexError::Compile(m) => write!(f, "compile: {m}"),
            JavaRegexError::Runtime(m) => write!(f, "runtime: {m}"),
            JavaRegexError::Replacement(m) => write!(f, "replacement: {m}"),
        }
    }
}
impl std::error::Error for JavaRegexError {}

/// 取锁。持锁期间不会 panic(只是查/插一张 map),中毒了也照常用里头那张表 ——
/// 因为一次编译失败而让整个进程的正则面永久瘫掉,比继续用更糟。
fn lock<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// fancy-regex 的**回溯步数预算**。默认 1_000_000 是给「短输入 + 病态正则」定的,
/// 而规则层的输入是**整张网页**:回落页底板 420 KB 时,一条没有病态回溯、
/// 只是从头扫到尾的书源正则(`(?<!\d,\h)"name":"…"` 那一族)就要 1~4 M 步 ——
/// 实测默认上限下 136 ms 后报 `Max limit for backtracking count exceeded`,
/// 而 **Java 的 Matcher 根本没有这种预算**:同一条正则在裁判侧顺利扫完、
/// 报「没匹配到」。于是同一步两侧一个 `toc_empty`、一个 `pipeline_error`
/// (2026-08-31 实测 3 个 case,见 fixtures/cases/pipeline-corpus-b/README.md)。
///
/// 取 `1 << 26`(≈ 6700 万):按实测的 ~8 步/字符,够扫 8 MB 的页面 ——
/// 比任何真实网页都宽;真碰上病态回溯时它仍是**兜底的刹车**(实测 ~30 ms/百万步,
/// 即最坏约 2 s),不至于把整个引擎挂死。这一位是**保真度与刹车的折中**:
/// 完全对齐 Java 就得取消上限,那等于把「书源写了个灾难性正则」变成永久挂起。
const BACKTRACK_LIMIT: usize = 1 << 26;

struct Compiled {
    re: fancy_regex::Regex,
    /// 捕获组个数(不含组 0)
    group_count: usize,
}

/// 编译结果缓存(按 `(java 原串, 是否整串匹配)` 索引)。
///
/// 为什么值得:`fancy-regex` 建程序比 `Pattern.compile` 贵**一到两个量级** ——
/// regex 差分套实测 p50 41.8µs vs 裁判 1.7µs,而且耗时跟着 **pattern 长度**走
/// (32.8µs → 115µs)、几乎不跟输入长度走(36.7 → 44.9µs),即花的是编译不是匹配
/// (docs/engine-perf.md §3①)。而书源里同一条正则会被反复调用:那套 8068 例
/// 只有 **1617 个不同 pattern**,缓存能省掉八成编译。
///
/// 两条口径:
/// - **只缓存成功的**。编译失败是少数,缓存 `Err` 要让错误串可克隆、还得防住
///   「失败也被复用」的疑虑,不值当。
/// - **满了整张清空,不做 LRU**。书源换源时 pattern 集合是整批换的,清空正好;
///   真要 LRU 得先拿出一份能证明它更好的数据 —— 现在没有。
///
/// 共享是安全的:`fancy_regex::Regex` 匹配走 `&self`,状态在调用方的 `Captures`
/// 上(与 Java 那个**有状态**的 `Matcher` 不同,那边每次调用也都新建一个)。
static CACHE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<(String, bool), std::sync::Arc<Compiled>>>,
> = std::sync::LazyLock::new(Default::default);

/// 缓存上限。够装下一整批书源的正则,又不至于让野生 pattern 把内存吃穿。
const CACHE_MAX: usize = 1024;

pub struct JavaRegex {
    inner: std::sync::Arc<Compiled>,
}

impl Clone for JavaRegex {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

impl JavaRegex {
    pub fn compile(java_pattern: &str) -> Result<Self, JavaRegexError> {
        Self::cached(java_pattern, false)
    }

    /// Kotlin `String.matches(Regex)` = Java `Matcher.matches()`:**整串**匹配。
    ///
    /// 不能拿 `^…$` 凑:Java 的 `matches()` 是「整个输入区域」,而 `$` 在默认
    /// 模式下允许末尾一个换行、开了 MULTILINE 更是每行都算。这里用 `\A…\z`
    /// 把两头钉死,和 `matches()` 的语义一致。
    pub fn compile_full_match(java_pattern: &str) -> Result<Self, JavaRegexError> {
        Self::cached(java_pattern, true)
    }

    /// 查缓存 → 未命中才转译 + 编译。**转译也在缓存这一侧**:命中时连
    /// `translate::translate` 都不跑(它自己也是一遍扫描)。
    fn cached(java_pattern: &str, full_match: bool) -> Result<Self, JavaRegexError> {
        let key = (java_pattern.to_string(), full_match);
        if let Some(hit) = lock(&CACHE).get(&key) {
            return Ok(Self { inner: hit.clone() });
        }
        let translated = translate::translate(java_pattern)
            .map_err(|e| JavaRegexError::Compile(e.to_string()))?;
        let inner = std::sync::Arc::new(if full_match {
            Self::build(&format!(r"\A(?:{translated})\z"), &translated)?
        } else {
            Self::build(&translated, &translated)?
        });
        let mut g = lock(&CACHE);
        if g.len() >= CACHE_MAX {
            g.clear();
        }
        g.insert(key, inner.clone());
        Ok(Self { inner })
    }

    fn build(pattern: &str, shown: &str) -> Result<Compiled, JavaRegexError> {
        let re = fancy_regex::RegexBuilder::new(pattern)
            .backtrack_limit(BACKTRACK_LIMIT)
            .build()
            .map_err(|e| JavaRegexError::Compile(format!("fancy: {e} <- {shown}")))?;
        let group_count = re.captures_len() - 1;
        Ok(Compiled { re, group_count })
    }

    /// 整串匹配上没有(配 [`Self::compile_full_match`] 用)
    pub fn is_match(&self, input: &str) -> Result<bool, JavaRegexError> {
        self.inner.re.is_match(input).map_err(|e| JavaRegexError::Runtime(e.to_string()))
    }

    /// 首个匹配的文本(Kotlin `regex.find(s)?.value`)
    pub fn find_first(&self, input: &str) -> Result<Option<String>, JavaRegexError> {
        let caps =
            self.inner.re.captures(input).map_err(|e| JavaRegexError::Runtime(e.to_string()))?;
        Ok(caps.map(|c| c.get(0).unwrap().as_str().to_string()))
    }

    /// 所有匹配的 (起, 止, 分组值)(组 0..N,未匹配的组为 None);cap 限制匹配数。
    /// 空匹配推进一个字符(Java 按 code unit,近似)——三个公开入口共用这一份口径,
    /// 免得同一段推进逻辑抄三份各改各的。
    fn find_all(
        &self,
        input: &str,
        cap: usize,
    ) -> Result<Vec<(usize, usize, Vec<Option<String>>)>, JavaRegexError> {
        let mut out = Vec::new();
        let mut pos = 0usize;
        while out.len() < cap {
            let Some(caps) = self
                .inner
                .re
                .captures_from_pos(input, pos)
                .map_err(|e| JavaRegexError::Runtime(e.to_string()))?
            else {
                break;
            };
            let m = caps.get(0).unwrap();
            out.push((
                m.start(),
                m.end(),
                (0..caps.len()).map(|i| caps.get(i).map(|g| g.as_str().to_string())).collect(),
            ));
            pos = if m.end() == m.start() {
                match input[m.end()..].chars().next() {
                    Some(c) => m.end() + c.len_utf8(),
                    None => break,
                }
            } else {
                m.end()
            };
        }
        Ok(out)
    }

    /// 所有匹配的分组值(组 0..N,未匹配的组为 None);cap 限制匹配数
    pub fn find_all_groups(
        &self,
        input: &str,
        cap: usize,
    ) -> Result<Vec<Vec<Option<String>>>, JavaRegexError> {
        Ok(self.find_all(input, cap)?.into_iter().map(|(_, _, groups)| groups).collect())
    }

    /// 所有匹配的 (起, 止, 分组值)(HtmlFormatter.formatKeepImg 用)
    pub fn find_all_with_ranges(
        &self,
        input: &str,
    ) -> Result<Vec<(usize, usize, Vec<Option<String>>)>, JavaRegexError> {
        self.find_all(input, usize::MAX)
    }

    /// Java Matcher.replaceAll 语义
    pub fn replace_all(&self, input: &str, replacement: &str) -> Result<String, JavaRegexError> {
        self.replace_impl(input, replacement, usize::MAX)
    }

    /// Java Matcher.replaceFirst 语义(无匹配时原样返回)
    pub fn replace_first(&self, input: &str, replacement: &str) -> Result<String, JavaRegexError> {
        self.replace_impl(input, replacement, 1)
    }

    fn replace_impl(
        &self,
        input: &str,
        replacement: &str,
        limit: usize,
    ) -> Result<String, JavaRegexError> {
        let mut out = String::with_capacity(input.len());
        let mut last_end = 0usize;
        for (start, end, groups) in self.find_all(input, limit)? {
            out.push_str(&input[last_end..start]);
            self.expand_replacement(&groups, replacement, &mut out)?;
            last_end = end;
        }
        out.push_str(&input[last_end..]);
        Ok(out)
    }

    /// Java Matcher.appendReplacement 的替换串展开:
    /// \x 转义、$n(贪婪吃数字但不超过组数);
    /// 坏引用抛 Replacement 错(供上层降级)。
    /// ${name} 随命名组一起不移植(语料零命中):`${` 落"$ 后非组引用"Err,
    /// 与 Java 侧"组名不存在"同类(IllegalArgumentException),两侧同走降级。
    fn expand_replacement(
        &self,
        groups: &[Option<String>],
        replacement: &str,
        out: &mut String,
    ) -> Result<(), JavaRegexError> {
        let chars: Vec<char> = replacement.chars().collect();
        let mut i = 0usize;
        while i < chars.len() {
            let c = chars[i];
            i += 1;
            match c {
                '\\' => {
                    if i >= chars.len() {
                        return Err(JavaRegexError::Replacement("末尾悬空的 \\".into()));
                    }
                    out.push(chars[i]);
                    i += 1;
                }
                '$' => {
                    let Some(d) = chars.get(i).and_then(|c| c.to_digit(10)) else {
                        return Err(JavaRegexError::Replacement("$ 后非组引用".into()));
                    };
                    i += 1;
                    let mut num = d as usize;
                    // Java:继续吃数字,直到并入后超过组数为止
                    while let Some(d) = chars.get(i).and_then(|c| c.to_digit(10)) {
                        let nv = num * 10 + d as usize;
                        if nv <= self.inner.group_count {
                            num = nv;
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    if num > self.inner.group_count {
                        return Err(JavaRegexError::Replacement(format!("无组 {num}")));
                    }
                    if let Some(g) = &groups[num] {
                        out.push_str(g);
                    }
                }
                _ => out.push(c),
            }
        }
        Ok(())
    }
}

/// AnalyzeRule.replaceRegex(裁判 L482-505)的完整移植,含所有降级路径。
/// 注意:缓存(regexCache)属于 rule-engine 层,这里每次现编译。
pub fn legado_replace_regex(
    result: &str,
    replace_regex: &str,
    replacement: &str,
    replace_first: bool,
) -> String {
    if replace_regex.is_empty() {
        return result.to_string();
    }
    let compiled = JavaRegex::compile(replace_regex).ok();
    if replace_first {
        /* ##match##replace### 获取第一个匹配到的结果并进行替换 */
        if let Some(re) = &compiled {
            let attempt = (|| -> Result<String, JavaRegexError> {
                match re.find_first(result)? {
                    Some(value) => re.replace_first(&value, replacement),
                    None => Ok(String::new()),
                }
            })();
            if let Ok(v) = attempt {
                return v;
            }
        }
        replacement.to_string()
    } else {
        /* ##match##replace 替换 */
        if let Some(re) = &compiled {
            if let Ok(v) = re.replace_all(result, replacement) {
                return v;
            }
        }
        // 字面量替换(Kotlin String.replace(literal))
        result.replace(replace_regex, replacement)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rall(pat: &str, input: &str, rep: &str) -> String {
        JavaRegex::compile(pat).unwrap().replace_all(input, rep).unwrap()
    }

    #[test]
    fn ascii_space_class() {
        // Java \s 不含全角空格 U+3000 与 NBSP
        assert_eq!(rall("\\s+", "a\u{3000}b c", "-"), "a\u{3000}b-c");
    }

    #[test]
    fn ascii_word_class() {
        // Java \w 不匹配汉字
        assert_eq!(rall("\\w+", "abc汉字def", "-"), "-汉字-");
    }

    #[test]
    fn dot_excludes_java_terminators() {
        assert_eq!(rall("a.c", "a\rc", "-"), "a\rc");
        assert_eq!(rall("(?s)a.c", "a\rc", "-"), "-");
    }

    #[test]
    fn dollar_before_final_newline() {
        // Java 非多行 $ 匹配末终止符之前
        assert_eq!(rall("c$", "abc\n", "-"), "ab-\n");
    }

    #[test]
    fn multiline_caret_not_at_eoi() {
        // (?m)^ 不匹配"末尾终止符之后"的位置
        let re = JavaRegex::compile("(?m)^").unwrap();
        let ms = re.find_all_groups("ab\ncd\n", 10).unwrap();
        assert_eq!(ms.len(), 2);
    }

    #[test]
    fn replacement_group_refs() {
        assert_eq!(rall("(a)(b)", "ab", "$2$1"), "ba");
        assert_eq!(rall("(a)", "a", "$12"), "a2"); // $1 后的 2 是字面量(组数=1)
        assert_eq!(rall("(a)", "a", "\\$1"), "$1");
    }

    #[test]
    fn replacement_bad_ref_errors() {
        let re = JavaRegex::compile("a").unwrap();
        assert!(matches!(re.replace_all("a", "$1"), Err(JavaRegexError::Replacement(_))));
        assert!(matches!(re.replace_all("a", "x$"), Err(JavaRegexError::Replacement(_))));
    }

    #[test]
    fn possessive_quantifier() {
        // a*+a 永不匹配(占有量词不回吐)
        let re = JavaRegex::compile("a*+a").unwrap();
        assert_eq!(re.find_first("aaa").unwrap(), None);
    }

    #[test]
    fn lookbehind_and_backref() {
        assert_eq!(rall("(?<=好)呀", "你好呀", "!"), "你好!");
        assert_eq!(rall("(.)\\1", "aabb", "<$1>"), "<a><b>");
    }

    #[test]
    fn word_boundary_ascii() {
        // Java \b 基于 ASCII \w:汉字不是词字符,"ab汉" 中 b 后是边界
        assert_eq!(rall("\\bab\\b", "ab汉", "-"), "-汉");
    }

    #[test]
    fn compile_error_on_bad_repetition() {
        // Java 对 a{b} 报 Illegal repetition(Rust 原生会当字面量)
        assert!(JavaRegex::compile("a{b}").is_err());
    }

    #[test]
    fn legado_composite_fallbacks() {
        // 编译失败 → 字面量替换
        assert_eq!(legado_replace_regex("x[y", "[", "-", false), "x-y");
        // 编译失败 + replaceFirst → 返回 replacement
        assert_eq!(legado_replace_regex("x[y", "[", "-", true), "-");
        // 坏 $ 引用 → 字面量替换(pattern 串本身被字面量替换)
        assert_eq!(legado_replace_regex("aXb", "X", "$9", false), "a$9b");
        // ### 无匹配 → 空串
        assert_eq!(legado_replace_regex("abc", "z", "-", true), "");
        // ### 有匹配:取 value 自替换
        assert_eq!(legado_replace_regex("第12章", "\\d+", "[$0]", true), "[12]");
        // ### 的 lookbehind 上下文丢失:value 单独匹配不上时原样返回 value
        assert_eq!(legado_replace_regex("ab", "(?<=a)b", "-", true), "b");
    }

    #[test]
    fn empty_match_advance() {
        assert_eq!(rall("x*", "ab", "-"), "-a-b-");
    }

    #[test]
    fn u_escape() {
        assert_eq!(rall("\\u002e+", "a..b", "-"), "a-b");
        // 八进制 \0nn 随语料零命中一起删了:现在是编译错(两侧同走降级)
        assert!(JavaRegex::compile("\\012").is_err());
    }

    #[test]
    fn horizontal_space() {
        // \h 含全角空格
        assert_eq!(rall("\\h+", "a\u{3000} b", "-"), "a-b");
    }

    #[test]
    fn class_dialect() {
        // 嵌套并集与交集
        assert_eq!(rall("[a[bc]]+", "abcd", "-"), "-d");
        assert_eq!(rall("[a-z&&[^bc]]+", "abcd", "-"), "-bc-");
        // 类内 \d
        assert_eq!(rall("[\\d，]+", "1，2x", "-"), "-x");
    }

    #[test]
    fn real_corpus_patterns_compile() {
        // 语料中的真实片段应能编译
        for p in [
            "(?mi)^\\h*(您可以在百度|请记住本书首发).+\\n?",
            "(?<=[，、‘《（【［「『])\\s+",
            "\\s+(?=[\\n，、’：；》）】］」』])",
            "(\\/)(?!.*\\1)",
            "Chapter \\d+(?=\\n)",
            "^([\\s\\S]{1,250}$)",
            "[\\u4e00-\\u9fa5\\dA-Za-z，]",
        ] {
            assert!(JavaRegex::compile(p).is_ok(), "should compile: {p}");
        }
    }
}
