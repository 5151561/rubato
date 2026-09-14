//! `AnalyzeRule.SourceRule`(内部类)的移植:模式识别、`@put` 分离、
//! `@get:{}` / `{{}}` / `$N` 的拆分。
//!
//! `makeUpRule` 需要回调 AnalyzeRule(求 JS、取变量),所以放在 lib.rs。

use crate::java_util::{parse_put_json_lenient, parse_put_json_strict};
use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    XPath,
    Json,
    Default,
    Js,
    Regex,
    WebJs,
}

pub(crate) const GET_RULE_TYPE: i32 = -2;
pub(crate) const JS_RULE_TYPE: i32 = -1;
pub(crate) const DEFAULT_RULE_TYPE: i32 = 0;

pub(crate) static PUT_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)@put:(\{[^}]+?\})").expect("put 模式"));
pub(crate) static EVAL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)@get:\{[^}]+?\}|\{\{[\s\S]*?\}\}").expect("eval 模式"));
pub(crate) static REGEX_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$[0-9]{1,2}").expect("$N 模式"));
pub(crate) static JS_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<js>([\s\S]*?)</js>|@js:([\s\S]*)").expect("js 模式"));
pub(crate) static WEB_JS_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)@webjs:([\s\S]{5,})").expect("webjs 模式"));

#[derive(Debug, Clone)]
pub struct SourceRule {
    pub mode: Mode,
    pub rule: String,
    pub replace_regex: String,
    pub replacement: String,
    pub replace_first: bool,
    /// `@put` 解析结果;值为 None 对应 JSON null(取值时按空串走)
    pub put_map: Vec<(String, Option<String>)>,
    pub(crate) rule_param: Vec<String>,
    pub(crate) rule_type: Vec<i32>,
}

fn starts_with_ci(s: &str, p: &str) -> bool {
    // 按字节比,避免规则以多字节字符开头时切在 char 边界内
    let (b, pb) = (s.as_bytes(), p.as_bytes());
    b.len() >= pb.len() && b[..pb.len()].eq_ignore_ascii_case(pb)
}

/// `splitPutRule`:摘掉全部 `@put:{...}`,先严格 JSON 后宽松 JSON
fn split_put_rule(rule_str: &str, put_map: &mut Vec<(String, Option<String>)>) -> String {
    let mut v = rule_str.to_string();
    for m in PUT_PATTERN.captures_iter(rule_str) {
        let whole = m.get(0).expect("整段").as_str();
        v = v.replacen(whole, "", usize::MAX);
        let json = m.get(1).expect("JSON 段").as_str();
        let parsed = parse_put_json_strict(json).or_else(|| parse_put_json_lenient(json));
        if let Some(entries) = parsed {
            for (k, val) in entries {
                // HashMap.put:已存在的键保持原位置、只换值
                match put_map.iter_mut().find(|(ek, _)| *ek == k) {
                    Some(slot) => slot.1 = val,
                    None => put_map.push((k, val)),
                }
            }
        }
    }
    v
}

impl SourceRule {
    /// `SourceRule(ruleStr, mode)` 的 init 块
    pub fn new(rule_str: &str, mode: Mode, is_json: bool) -> Self {
        let mut mode = mode;
        let mut rule = if mode == Mode::Js || mode == Mode::Regex {
            rule_str.to_string()
        } else if starts_with_ci(rule_str, "@CSS:") {
            mode = Mode::Default;
            rule_str.to_string()
        } else if let Some(rest) = rule_str.strip_prefix("@@") {
            mode = Mode::Default;
            rest.to_string()
        } else if starts_with_ci(rule_str, "@XPath:") {
            mode = Mode::XPath;
            rule_str[7..].to_string()
        } else if starts_with_ci(rule_str, "@Json:") {
            mode = Mode::Json;
            rule_str[6..].to_string()
        } else if is_json || rule_str.starts_with("$.") || rule_str.starts_with("$[") {
            mode = Mode::Json;
            rule_str.to_string()
        } else if rule_str.starts_with('/') {
            // XPath 特征很明显,无需配置单独的识别标头
            mode = Mode::XPath;
            rule_str.to_string()
        } else {
            rule_str.to_string()
        };

        let mut put_map: Vec<(String, Option<String>)> = Vec::new();
        rule = split_put_rule(&rule, &mut put_map);

        let mut me = SourceRule {
            mode,
            rule: String::new(),
            replace_regex: String::new(),
            replacement: String::new(),
            replace_first: false,
            put_map,
            rule_param: Vec::new(),
            rule_type: Vec::new(),
        };

        // @get,{{ }} 拆分
        let mut start = 0usize;
        if let Some(first) = EVAL_PATTERN.find(&rule) {
            let tmp = &rule[start..first.start()];
            if me.mode != Mode::Js
                && me.mode != Mode::Regex
                && (first.start() == 0 || !tmp.contains("##"))
            {
                me.mode = Mode::Regex;
            }
        }
        for m in EVAL_PATTERN.find_iter(&rule) {
            if m.start() > start {
                let tmp = rule[start..m.start()].to_string();
                me.split_regex(&tmp);
            }
            let tmp = m.as_str();
            if starts_with_ci(tmp, "@get:") {
                me.rule_type.push(GET_RULE_TYPE);
                // `@get:{key}` → key
                me.rule_param.push(tmp[6..tmp.len() - 1].to_string());
            } else if tmp.starts_with("{{") {
                me.rule_type.push(JS_RULE_TYPE);
                me.rule_param.push(tmp[2..tmp.len() - 2].to_string());
            } else {
                let tmp = tmp.to_string();
                me.split_regex(&tmp);
            }
            start = m.end();
        }
        if rule.len() > start {
            let tmp = rule[start..].to_string();
            me.split_regex(&tmp);
        }
        me.rule = rule;
        me
    }

    /// `splitRegex`:拆 `$\d{1,2}`(只在 `##` 之前的段里找,但切片取自整串)
    fn split_regex(&mut self, rule_str: &str) {
        let mut start = 0usize;
        let head = rule_str.split("##").next().unwrap_or("");
        if REGEX_PATTERN.find(head).is_some() && self.mode != Mode::Js && self.mode != Mode::Regex {
            self.mode = Mode::Regex;
        }
        for m in REGEX_PATTERN.find_iter(head) {
            if m.start() > start {
                self.rule_type.push(DEFAULT_RULE_TYPE);
                self.rule_param.push(rule_str[start..m.start()].to_string());
            }
            let tmp = m.as_str();
            self.rule_type.push(tmp[1..].parse().expect("$N 已由模式保证是数字"));
            self.rule_param.push(tmp.to_string());
            start = m.end();
        }
        if rule_str.len() > start {
            self.rule_type.push(DEFAULT_RULE_TYPE);
            self.rule_param.push(rule_str[start..].to_string());
        }
    }

    pub fn param_size(&self) -> usize {
        self.rule_param.len()
    }
}
