//! `AnalyzeByRegex`(judge/engine 57 行)的逐行移植。
//!
//! 注意 Kotlin `Regex(...)` 编译失败会直接抛,不像 replaceRegex 有降级链;
//! 且 `groupValues` 对未参与匹配的组给空串而非 null。

use regex_compat::{JavaRegex, JavaRegexError};

fn group_values(re: &JavaRegex, input: &str) -> Result<Vec<Vec<String>>, JavaRegexError> {
    Ok(re
        .find_all_groups(input, usize::MAX)?
        .into_iter()
        .map(|g| g.into_iter().map(Option::unwrap_or_default).collect())
        .collect())
}

/// `AnalyzeByRegex.getElement`
pub fn get_element(
    res: &str,
    regs: &[String],
    index: usize,
) -> Result<Option<Vec<String>>, JavaRegexError> {
    let re = JavaRegex::compile(&regs[index])?;
    let all = group_values(&re, res)?;
    let Some(first) = all.first() else {
        return Ok(None);
    };
    if index + 1 == regs.len() {
        return Ok(Some(first.clone()));
    }
    let joined: String = all.iter().map(|g| g[0].as_str()).collect();
    get_element(&joined, regs, index + 1)
}

/// `AnalyzeByRegex.getElements`
pub fn get_elements(
    res: &str,
    regs: &[String],
    index: usize,
) -> Result<Vec<Vec<String>>, JavaRegexError> {
    let re = JavaRegex::compile(&regs[index])?;
    let all = group_values(&re, res)?;
    if all.is_empty() {
        return Ok(Vec::new());
    }
    if index + 1 == regs.len() {
        return Ok(all);
    }
    let joined: String = all.iter().map(|g| g[0].as_str()).collect();
    get_elements(&joined, regs, index + 1)
}
