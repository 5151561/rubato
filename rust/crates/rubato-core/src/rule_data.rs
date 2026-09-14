//! `RuleDataInterface` 四层变量存储(chapter → book → ruleData → source)。

use crate::host::{VarEntity, VarStore};
use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct VarLayer {
    pub vars: HashMap<String, String>,
    /// **大值不进 `variableMap`**:`RuleDataInterface.putVariable` 以
    /// `value.length < 10000` 分流,够大的走 `putBigVariable`(真身是
    /// RuleBigDataHelp 的另一张表,按 bookUrl / chapterUrl 键),
    /// 同时**从 variableMap 里删掉**。读取按 `variableMap[key] ?: getBigVariable(key)`。
    ///
    /// 可观察的后果:实体的 `variable` JSON 里**没有**这一条(书源把整页
    /// `java.put('real_chapter', src)` 存起来是常见写法,那一定超过 10000)。
    pub big: HashMap<String, String>,
}

/// `value.length < 10000` —— Kotlin 的 `length` 是 **UTF-16 码元数**,
/// 不是字符数(汉字仍是 1,emoji 之类是 2)
fn is_big(value: &str) -> bool {
    value.encode_utf16().count() >= 10000
}

impl VarLayer {
    pub fn put(&mut self, key: &str, value: &str) {
        if is_big(value) {
            self.vars.remove(key);
            self.big.insert(key.to_string(), value.to_string());
        } else {
            self.big.remove(key);
            self.vars.insert(key.to_string(), value.to_string());
        }
    }
    pub fn get(&self, key: &str) -> &str {
        self.vars.get(key).or_else(|| self.big.get(key)).map(String::as_str).unwrap_or("")
    }
}

/// AnalyzeRule 的 put/get 优先级链。层不存在(None)时整层跳过,
/// 这正是裁判 `chapter?.putVariable(...) ?: book?.putVariable(...)` 的语义。
#[derive(Debug, Default, Clone)]
pub struct RuleData {
    pub chapter: Option<VarLayer>,
    pub book: Option<VarLayer>,
    pub rule_data: Option<VarLayer>,
    pub source: Option<VarLayer>,
    /// `get("bookName")` 的特判来源
    pub book_name: Option<String>,
    /// `get("title")` 的特判来源(chapter 存在才生效)
    pub chapter_title: Option<String>,
}

/// RuleData **没有**规则反调面(它只是变量层)——`java.getString` 打到它身上
/// 就是「这个宿主没有那个方法」,与真身的 AnalyzeUrl 同形。见 [`RuleHost`]。
impl crate::host::RuleHost for RuleData {}

impl VarStore for RuleData {
    /// `put`:只写命中的第一层
    fn put(&mut self, key: &str, value: &str) -> String {
        let first = [&mut self.chapter, &mut self.book, &mut self.rule_data, &mut self.source]
            .into_iter()
            .flatten()
            .next();
        if let Some(l) = first {
            l.put(key, value);
        }
        value.to_string()
    }

    /// `putVariable`:实体自己那一层。层不在(没 setChapter / 没 book)就**丢掉**
    /// —— 真身那里根本没有这个绑定可调。
    fn put_entity(&mut self, entity: VarEntity, key: &str, value: &str) {
        let l = match entity {
            VarEntity::Book => &mut self.book,
            VarEntity::Chapter => &mut self.chapter,
        };
        if let Some(l) = l {
            l.put(key, value);
        }
    }

    /// `getVariable`:同样只看自己那一层,取不到给空串
    fn get_entity(&self, entity: VarEntity, key: &str) -> String {
        let l = match entity {
            VarEntity::Book => &self.book,
            VarEntity::Chapter => &self.chapter,
        };
        l.as_ref().map(|l| l.get(key).to_string()).unwrap_or_default()
    }

    /// `get`:bookName/title 特判在前,随后逐层取第一个非空值
    fn get(&self, key: &str) -> String {
        // Kotlin: `book?.let { return it.name }`——book 存在就直接返回(可为空串)
        if key == "bookName" && self.book.is_some() {
            return self.book_name.clone().unwrap_or_default();
        }
        if key == "title" {
            if self.chapter.is_some() {
                return self.chapter_title.clone().unwrap_or_default();
            }
        }
        for l in [&self.chapter, &self.book, &self.rule_data, &self.source].into_iter().flatten() {
            let v = l.get(key);
            if !v.is_empty() {
                return v.to_string();
            }
        }
        String::new()
    }
}
