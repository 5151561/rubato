//! RuleAnalyzer(judge/engine RuleAnalyzer.kt,377 行)的逐行移植。
//!
//! 行为口径:与 Kotlin 裁判 diff 一致为准(tools/syntax_diff.sh)。
//! - 索引单位:Kotlin 按 UTF-16 code unit,这里按 char(Unicode 标量值)。
//!   对合法 Unicode 输入两者切出的子串一致,仅内部位置数值不同;
//!   增补平面字符(emoji 等)已在差分用例中覆盖。
//! - Kotlin 中会抛 StringIndexOutOfBoundsException 的路径(如 trim() 越过
//!   串尾、平衡组吃掉行尾转义符)映射为 `SplitError::IndexOutOfBounds`;
//!   splitRule 主动 throw Error("...后未平衡") 映射为 `SplitError::Unbalanced`,
//!   消息文本与裁判逐字对齐。

const ESC: char = '\\';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplitError {
    /// 平衡组不闭合;消息 = `queue.substring(0, start) + "后未平衡"`,与裁判一致
    Unbalanced(String),
    /// 对应 Kotlin 的 StringIndexOutOfBoundsException
    IndexOutOfBounds,
}

impl std::fmt::Display for SplitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SplitError::Unbalanced(msg) => f.write_str(msg),
            SplitError::IndexOutOfBounds => f.write_str("index_out_of_bounds"),
        }
    }
}

impl std::error::Error for SplitError {}

pub struct RuleAnalyzer {
    queue: Vec<char>, // 被处理字符串
    /// `queue[i]` 在 [`Self::src`] 里的字节偏移,长度 `queue.len() + 1`(末位是全长)。
    ///
    /// 为什么要它:切子串曾经是 `queue[from..to].iter().collect()` —— 逐 char
    /// 重新 UTF-8 编码一遍。裁判那边是 `String.substring`,一次 arraycopy 就完
    /// (实测 7.8 字符/ns,正好是 memcpy 的速度)。长规则上这一条差 25 倍:
    /// 111749 字符的 `exploreUrl`,裁判 13.1µs、被测 323.9µs,而且比值从 31k 到
    /// 111k 恒定 —— 是常数因子不是复杂度(见 docs/engine-perf.md §3④)。
    /// 有了这张表,`substring` 变成对 `src` 的一次字节切片。
    offs: Vec<u32>,
    src: String,           // 原串,子串直接从它上面切
    pos: usize,            // 当前处理到的位置
    start: usize,          // 当前处理字段的开始
    start_x: usize,        // 当前规则的开始
    rule: Vec<String>,     // 分割出的规则列表
    step: usize,           // 分割字符的长度
    elements_type: String, // 当前分割字符串
    code: bool,            // 平衡组模式:true 用 chompCodeBalanced,false 用 chompRuleBalanced
}

impl RuleAnalyzer {
    pub fn new(data: &str, code: bool) -> Self {
        // 一趟同时建 char 表与字节偏移表(两趟等于把 data 扫两遍)
        let mut queue = Vec::with_capacity(data.len());
        let mut offs = Vec::with_capacity(data.len() + 1);
        for (b, c) in data.char_indices() {
            queue.push(c);
            offs.push(b as u32);
        }
        offs.push(data.len() as u32);
        Self {
            queue,
            offs,
            src: data.to_string(),
            pos: 0,
            start: 0,
            start_x: 0,
            rule: Vec::new(),
            step: 0,
            elements_type: String::new(),
            code,
        }
    }

    pub fn elements_type(&self) -> &str {
        &self.elements_type
    }

    /// 修剪当前规则之前的"@"或者空白符。
    /// Kotlin 在整串都是可修剪字符时会越界抛异常,此处返回 Err 复刻该行为。
    pub fn trim(&mut self) -> Result<(), SplitError> {
        let c = *self.queue.get(self.pos).ok_or(SplitError::IndexOutOfBounds)?;
        if c == '@' || c < '!' {
            self.pos += 1;
            loop {
                let c = *self.queue.get(self.pos).ok_or(SplitError::IndexOutOfBounds)?;
                if c == '@' || c < '!' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            self.start = self.pos;
            self.start_x = self.pos;
        }
        Ok(())
    }

    /// 将 pos 重置为 0,方便复用
    pub fn re_set_pos(&mut self) {
        self.pos = 0;
        self.start_x = 0;
    }

    fn substring(&self, from: usize, to: usize) -> Result<String, SplitError> {
        if from > to || to > self.queue.len() {
            return Err(SplitError::IndexOutOfBounds);
        }
        // 字节切片 + 一次 memcpy;`offs` 的边界由 char_indices 建出,必落在字符边界上
        Ok(self.src[self.offs[from] as usize..self.offs[to] as usize].to_string())
    }

    fn substring_from(&self, from: usize) -> Result<String, SplitError> {
        self.substring(from, self.queue.len())
    }

    /// Java String.indexOf(needle, from) 语义:from 超界按 len 截断;
    /// 空 needle 返回 min(from, len)
    fn index_of(&self, needle: &[char], from: usize) -> Option<usize> {
        let len = self.queue.len();
        let n = needle.len();
        let from = from.min(len);
        if n > len {
            return None;
        }
        // 原来是 `(from..=len-n).find(|i| queue[i..i+n] == *needle)` —— **每个位置**
        // 做一次 slice 相等比较,单字符分隔符(`@`、书源里绝大多数)也照做。
        // 105280 字符的规则上实测这一句要 ~150µs,而裁判那边 `String.indexOf`
        // 是一条向量化的紧循环(整例才 10µs)。分成两支:单字符走 `position`
        // (编译成简单的 char 比较循环),多字符先按首字符筛再比其余。
        if n == 0 {
            return Some(from); // Java indexOf 的空串语义:min(from, len)
        }
        let first = needle[0];
        if n == 1 {
            return self.queue[from..].iter().position(|&c| c == first).map(|i| i + from);
        }
        (from..=len - n).find(|&i| self.queue[i] == first && self.queue[i..i + n] == *needle)
    }

    fn region_matches(&self, at: usize, needle: &[char]) -> bool {
        at + needle.len() <= self.queue.len() && self.queue[at..at + needle.len()] == *needle
    }

    /// 从剩余字串中拉出一个字符串,直到但不包括匹配序列(区分大小写)
    fn consume_to(&mut self, seq: &[char]) -> bool {
        self.start = self.pos;
        match self.index_of(seq, self.pos) {
            Some(offset) => {
                self.pos = offset;
                true
            }
            None => false,
        }
    }

    /// 直到匹配参数列表中任意一项;成功则设置 step 并同步 pos
    fn consume_to_any(&mut self, seqs: &[Vec<char>]) -> bool {
        let mut pos = self.pos;
        while pos != self.queue.len() {
            for s in seqs {
                if self.region_matches(pos, s) {
                    self.step = s.len();
                    self.pos = pos;
                    return true;
                }
            }
            pos += 1;
        }
        false
    }

    /// 返回从 pos 起第一个匹配字符的位置
    fn find_to_any(&self, seq: &[char]) -> Option<usize> {
        let mut pos = self.pos;
        while pos != self.queue.len() {
            if seq.contains(&self.queue[pos]) {
                return Some(pos);
            }
            pos += 1;
        }
        None
    }

    /// 拉出一个非内嵌代码平衡组,存在转义文本
    fn chomp_code_balanced(&mut self, open: char, close: char) -> Result<bool, SplitError> {
        let mut pos = self.pos;
        let mut depth: i64 = 0; // [] 嵌套深度
        let mut other_depth: i64 = 0; // 其他对称符号嵌套深度
        let mut in_single_quote = false;
        let mut in_double_quote = false;

        loop {
            if pos == self.queue.len() {
                break;
            }
            let c = *self.queue.get(pos).ok_or(SplitError::IndexOutOfBounds)?;
            pos += 1;
            if c != ESC {
                if c == '\'' && !in_double_quote {
                    in_single_quote = !in_single_quote;
                } else if c == '"' && !in_single_quote {
                    in_double_quote = !in_double_quote;
                }
                if !(in_single_quote || in_double_quote) {
                    if c == '[' {
                        depth += 1;
                    } else if c == ']' {
                        depth -= 1;
                    } else if depth == 0 {
                        // 仅 depth 为 0 时默认嵌套全部闭合,此字符才进行嵌套
                        if c == open {
                            other_depth += 1;
                        } else if c == close {
                            other_depth -= 1;
                        }
                    }
                }
            } else {
                pos += 1;
            }
            if !(depth > 0 || other_depth > 0) {
                break;
            }
        }

        if depth > 0 || other_depth > 0 {
            Ok(false)
        } else {
            self.pos = pos;
            Ok(true)
        }
    }

    /// 拉出一个规则平衡组;xpath/jsoup 中引号内转义字符无效
    fn chomp_rule_balanced(&mut self, open: char, close: char) -> Result<bool, SplitError> {
        let mut pos = self.pos;
        let mut depth: i64 = 0;
        let mut in_single_quote = false;
        let mut in_double_quote = false;

        loop {
            if pos == self.queue.len() {
                break;
            }
            let c = *self.queue.get(pos).ok_or(SplitError::IndexOutOfBounds)?;
            pos += 1;
            if c == '\'' && !in_double_quote {
                in_single_quote = !in_single_quote;
            } else if c == '"' && !in_single_quote {
                in_double_quote = !in_double_quote;
            }

            if in_single_quote || in_double_quote {
                if depth > 0 {
                    continue;
                }
                break;
            } else if c == '\\' {
                // 不在引号中的转义字符才将下个字符转义
                pos += 1;
                if depth > 0 {
                    continue;
                }
                break;
            }

            if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
            }

            if depth <= 0 {
                break;
            }
        }

        if depth > 0 {
            Ok(false)
        } else {
            self.pos = pos;
            Ok(true)
        }
    }

    fn chomp_balanced(&mut self, open: char, close: char) -> Result<bool, SplitError> {
        if self.code {
            self.chomp_code_balanced(open, close)
        } else {
            self.chomp_rule_balanced(open, close)
        }
    }

    /// 首段匹配(Kotlin: tailrec splitRule(vararg split)),elementsType 为空
    pub fn split_rule(&mut self, split: &[&str]) -> Result<Vec<String>, SplitError> {
        let seqs: Vec<Vec<char>> = split.iter().map(|s| s.chars().collect()).collect();

        if split.len() == 1 {
            self.elements_type = split[0].to_string();
            return if !self.consume_to(&seqs[0]) {
                let s = self.substring_from(self.start_x)?;
                self.rule.push(s);
                Ok(self.rule.clone())
            } else {
                self.step = seqs[0].len();
                self.split_rule_next() // 递归匹配
            };
        }

        // tailrec 展开为循环
        'recurse: loop {
            if !self.consume_to_any(&seqs) {
                // 未找到分隔符
                let s = self.substring_from(self.start_x)?;
                self.rule.push(s);
                return Ok(self.rule.clone());
            }

            let end = self.pos; // 记录分隔位置
            self.pos = self.start; // 重回开始,启动另一种查找

            loop {
                let st = self.find_to_any(&['[', '(']); // 查找筛选器位置

                match st {
                    None => {
                        // 压入分隔的首段规则到数组
                        self.rule = vec![self.substring(self.start_x, end)?];
                        // 设置组合类型
                        self.elements_type = self.substring(end, end + self.step)?;
                        self.pos = end + self.step; // 跳过分隔符

                        let et: Vec<char> = self.elements_type.chars().collect();
                        while self.consume_to(&et) {
                            let s = self.substring(self.start, self.pos)?;
                            self.rule.push(s);
                            self.pos += self.step;
                        }
                        let s = self.substring_from(self.pos)?;
                        self.rule.push(s);
                        return Ok(self.rule.clone());
                    }
                    Some(st) if st > end => {
                        // 分隔字串不在选择器中,将选择器前分隔字串分隔的字段依次压入数组
                        self.rule = vec![self.substring(self.start_x, end)?];
                        self.elements_type = self.substring(end, end + self.step)?;
                        self.pos = end + self.step;

                        let et: Vec<char> = self.elements_type.chars().collect();
                        while self.consume_to(&et) && self.pos < st {
                            let s = self.substring(self.start, self.pos)?;
                            self.rule.push(s);
                            self.pos += self.step;
                        }

                        return if self.pos > st {
                            self.start_x = self.start;
                            self.split_rule_next() // 首段已匹配,当前段匹配未完成,调用二段匹配
                        } else {
                            // 后面再无分隔字符
                            let s = self.substring_from(self.pos)?;
                            self.rule.push(s);
                            Ok(self.rule.clone())
                        };
                    }
                    Some(st) => {
                        self.pos = st; // 位置推移到筛选器处
                        let open = self.queue[self.pos];
                        let next = if open == '[' { ']' } else { ')' };
                        if !self.chomp_balanced(open, next)? {
                            return Err(SplitError::Unbalanced(format!(
                                "{}后未平衡",
                                self.substring(0, self.start)?
                            )));
                        }
                    }
                }

                if end <= self.pos {
                    break;
                }
            }

            self.start = self.pos; // 设置开始查找筛选器位置的起始位置
            continue 'recurse; // 递归调用首段匹配
        }
    }

    /// 二段匹配(Kotlin: tailrec splitRule()),elementsType 非空,直接按其查找
    fn split_rule_next(&mut self) -> Result<Vec<String>, SplitError> {
        let et: Vec<char> = self.elements_type.chars().collect();

        // tailrec 展开为循环
        'recurse: loop {
            let end = self.pos; // 记录分隔位置
            self.pos = self.start; // 重回开始,启动另一种查找

            loop {
                let st = self.find_to_any(&['[', '(']);

                match st {
                    None => {
                        let s = self.substring(self.start_x, end)?;
                        self.rule.push(s);
                        self.pos = end + self.step;

                        while self.consume_to(&et) {
                            let s = self.substring(self.start, self.pos)?;
                            self.rule.push(s);
                            self.pos += self.step;
                        }
                        let s = self.substring_from(self.pos)?;
                        self.rule.push(s);
                        return Ok(self.rule.clone());
                    }
                    Some(st) if st > end => {
                        let s = self.substring(self.start_x, end)?;
                        self.rule.push(s);
                        self.pos = end + self.step;

                        while self.consume_to(&et) && self.pos < st {
                            let s = self.substring(self.start, self.pos)?;
                            self.rule.push(s);
                            self.pos += self.step;
                        }

                        if self.pos > st {
                            self.start_x = self.start;
                            continue 'recurse; // 二段匹配 tailrec
                        } else {
                            let s = self.substring_from(self.pos)?;
                            self.rule.push(s);
                            return Ok(self.rule.clone());
                        }
                    }
                    Some(st) => {
                        self.pos = st;
                        let open = self.queue[self.pos];
                        let next = if open == '[' { ']' } else { ')' };
                        if !self.chomp_balanced(open, next)? {
                            return Err(SplitError::Unbalanced(format!(
                                "{}后未平衡",
                                self.substring(0, self.start)?
                            )));
                        }
                    }
                }

                if end <= self.pos {
                    break;
                }
            }

            self.start = self.pos;

            if !self.consume_to(&et) {
                let s = self.substring_from(self.start_x)?;
                self.rule.push(s);
                return Ok(self.rule.clone());
            }
            // else: 递归匹配 → continue
        }
    }

    /// 替换内嵌规则(如 `{$.rule}`)。inner 为起始标志(如 `{$.`),
    /// startStep/endStep 为不属于规则部分的前后置字符长度。
    /// fr 返回 None 或空串视为解析失败,该 inner 按普通字串跳过。
    /// 无任何替换发生时返回空串(与 Kotlin 一致)。
    pub fn inner_rule<F>(
        &mut self,
        inner: &str,
        start_step: usize,
        end_step: usize,
        mut fr: F,
    ) -> Result<String, SplitError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let inner_chars: Vec<char> = inner.chars().collect();
        let mut st = String::new();

        while self.consume_to(&inner_chars) {
            let pos_pre = self.pos; // 记录 consumeTo 匹配位置
            if self.chomp_code_balanced('{', '}')? {
                let arg_end = self.pos.checked_sub(end_step).ok_or(SplitError::IndexOutOfBounds)?;
                let arg = self.substring(pos_pre + start_step, arg_end)?;
                let frv = fr(&arg);
                if let Some(frv) = frv.filter(|v| !v.is_empty()) {
                    // 压入内嵌规则前的内容,及内嵌规则解析得到的字符串
                    st.push_str(&self.substring(self.start_x, pos_pre)?);
                    st.push_str(&frv);
                    self.start_x = self.pos; // 记录下次规则起点
                    continue;
                }
            }
            // 拉出字段不平衡,inner 只是个普通字串,跳到此 inner 后继续匹配
            self.pos += inner_chars.len();
        }

        if self.start_x == 0 {
            Ok(String::new())
        } else {
            st.push_str(&self.substring_from(self.start_x)?);
            Ok(st)
        }
    }

    /// 替换内嵌规则(startStr...endStr 包夹,如 `{{`/`}}`)。
    /// 注意与 Kotlin 一致:fr 返回 None 时拼入字面量 "null";
    /// 无任何替换发生时返回原串。
    pub fn inner_rule_str<F>(
        &mut self,
        start_str: &str,
        end_str: &str,
        mut fr: F,
    ) -> Result<String, SplitError>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let start_chars: Vec<char> = start_str.chars().collect();
        let end_chars: Vec<char> = end_str.chars().collect();
        let mut st = String::new();

        while self.consume_to(&start_chars) {
            self.pos += start_chars.len(); // 跳过开始字符串
            let pos_pre = self.pos;
            if self.consume_to(&end_chars) {
                let frv = fr(&self.substring(pos_pre, self.pos)?);
                let seg_end =
                    pos_pre.checked_sub(start_chars.len()).ok_or(SplitError::IndexOutOfBounds)?;
                st.push_str(&self.substring(self.start_x, seg_end)?);
                // Kotlin 的 String + String? 在 null 时拼 "null"
                match frv {
                    Some(v) => st.push_str(&v),
                    None => st.push_str("null"),
                }
                self.pos += end_chars.len(); // 跳过结束字符串
                self.start_x = self.pos; // 记录下次规则起点
            }
        }

        if self.start_x == 0 {
            Ok(self.src.clone())
        } else {
            st.push_str(&self.substring_from(self.start_x)?);
            Ok(st)
        }
    }
}

/// splitRule 拆出多支、各支求值之后的收尾:`%%` 按位交叉合并、
/// 其余(`&&` / `||`)顺次拼接。对齐 AnalyzeByJSoup / AnalyzeByJSonPath /
/// AnalyzeByXPath 三处逐字相同的合并循环,写一份供三个 crate 调。
pub fn merge<T: Clone>(results: &[Vec<T>], elements_type: &str) -> Vec<T> {
    let mut out = Vec::new();
    if results.is_empty() {
        return out;
    }
    if elements_type == "%%" {
        for i in 0..results[0].len() {
            for t in results {
                if i < t.len() {
                    out.push(t[i].clone());
                }
            }
        }
    } else {
        for t in results {
            out.extend(t.iter().cloned());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(data: &str, code: bool, seps: &[&str]) -> Result<Vec<String>, SplitError> {
        RuleAnalyzer::new(data, code).split_rule(seps)
    }

    #[test]
    fn multi_sep_only_splits_by_first_hit() {
        // 裁判语义:elementsType 取首个命中的分隔符,其余分隔符不参与切分
        assert_eq!(split("a&&b||c", false, &["&&", "||", "%%"]).unwrap(), vec!["a", "b||c"]);
        assert_eq!(split("a&&b&&c", false, &["&&", "||", "%%"]).unwrap(), vec!["a", "b", "c"]);
    }

    #[test]
    fn no_sep_returns_whole() {
        assert_eq!(
            split("class.chapter", false, &["&&", "||", "%%"]).unwrap(),
            vec!["class.chapter"]
        );
    }

    #[test]
    fn elements_type_is_first_sep() {
        let mut ra = RuleAnalyzer::new("a%%b&&c", false);
        assert_eq!(ra.split_rule(&["&&", "||", "%%"]).unwrap(), vec!["a", "b&&c"]);
        assert_eq!(ra.elements_type(), "%%");
    }

    #[test]
    fn sep_inside_attr_selector_protected() {
        assert_eq!(
            split("div[title=a&&b]&&span", false, &["&&", "||", "%%"]).unwrap(),
            vec!["div[title=a&&b]", "span"]
        );
    }

    #[test]
    fn single_sep_at() {
        assert_eq!(
            split("class.book@tag.a@text", false, &["@"]).unwrap(),
            vec!["class.book", "tag.a", "text"]
        );
    }

    #[test]
    fn trim_leading() {
        let mut ra = RuleAnalyzer::new("@@  div@text", false);
        ra.trim().unwrap();
        assert_eq!(ra.split_rule(&["@"]).unwrap(), vec!["div", "text"]);
    }

    #[test]
    fn trim_all_trimmable_errors_like_kotlin() {
        let mut ra = RuleAnalyzer::new("@@@", false);
        assert_eq!(ra.trim(), Err(SplitError::IndexOutOfBounds));
        let mut ra = RuleAnalyzer::new("", false);
        assert_eq!(ra.trim(), Err(SplitError::IndexOutOfBounds));
    }

    #[test]
    fn unbalanced_reports_kotlin_message() {
        // 多分隔符路径下 start 仍为 0,故前缀为空串(与裁判一致)
        let err = split("div[a&&b", false, &["&&", "||", "%%"]).unwrap_err();
        assert_eq!(err, SplitError::Unbalanced("后未平衡".into()));
    }

    #[test]
    fn code_mode_jsonpath() {
        assert_eq!(
            split("$.list[?(@.a&&@.b)]&&$.name", true, &["&&", "||"]).unwrap(),
            vec!["$.list[?(@.a&&@.b)]", "$.name"]
        );
    }

    #[test]
    fn inner_rule_basic() {
        let mut ra = RuleAnalyzer::new("xx{$.title}yy", false);
        let out = ra.inner_rule("{$.", 1, 1, |s| Some(format!("<{s}>"))).unwrap();
        assert_eq!(out, "xx<$.title>yy");
    }

    #[test]
    fn inner_rule_no_match_returns_empty() {
        let mut ra = RuleAnalyzer::new("plain", false);
        let out = ra.inner_rule("{$.", 1, 1, |s| Some(s.to_string())).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn inner_rule_str_url_template() {
        let mut ra = RuleAnalyzer::new("https://x.com/{{page}}?q={{key}}", false);
        let out = ra.inner_rule_str("{{", "}}", |s| Some(format!("[{s}]"))).unwrap();
        assert_eq!(out, "https://x.com/[page]?q=[key]");
    }

    #[test]
    fn inner_rule_str_no_match_returns_queue() {
        let mut ra = RuleAnalyzer::new("https://x.com/1", false);
        let out = ra.inner_rule_str("{{", "}}", |s| Some(s.to_string())).unwrap();
        assert_eq!(out, "https://x.com/1");
    }
}
