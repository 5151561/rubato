//! `AnalyzeRule`(judge/engine 973 行调度器)的移植。
//!
//! 责任:把一条书源规则串按 `<js>/@js:/@webjs:` 切成 SourceRule 序列,
//! 逐段在 jsoup / JSONPath / 正则 / JS 之间分发,并处理 `@put`/`@get`/
//! `{{}}`/`$N`/`##正则` 与 `isUrl` 绝对化。
//!
//! Phase 1 未建模(记在 fixtures/cases/rule-engine/README.md):
//! - content 是 Rhino `NativeObject` / gson `LinkedTreeMap` 的两条分支
//!   (只在 JS 宿主对象经手时出现,Phase 2 随 js-host 一起补);
//! - `reGetBook`/`refreshTocUrl`(preUpdateJs 专用,依赖 pipeline)。

pub mod java_util;
pub mod regex_elements;
pub mod source_rule;
pub mod value;

use html_compat::dsl::AnalyzeByJSoup;
use html_compat::entities::unescape_html4;
use json_compat::dsl::AnalyzeByJSonPath;
use regex_compat::legado_replace_regex;
use rubato_core::gson;
use rubato_core::host::{ElementHandle, ElementList, HostEnv, JsBindings, JsValue, VarStore};
use rubato_core::net_utils::{get_absolute_url, is_json, kotlin_trim};
use rubato_core::{JavaUrl, RuleData};
use scraper::Html;
use serde_json::Value;
use source_rule::{
    DEFAULT_RULE_TYPE, GET_RULE_TYPE, JS_PATTERN, JS_RULE_TYPE, Mode, SourceRule, WEB_JS_PATTERN,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use value::{Doc, RuleValue};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    /// RuleAnalyzer / AnalyzeByJSoup 抛出的解析异常
    Dsl(String),
    /// Kotlin `Regex(...)` 编译失败或运行期异常
    Regex(String),
    /// jayway `InvalidJsonException` / `PathNotFound` 一类
    Json(String),
    /// JS 求值异常
    Js(String),
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RuleError {}

/// **反调面的错误要带 Java 类别**:`java.getString/getElement(s)` 在真身里就是
/// AnalyzeRule 的方法,jayway 的 `PathNotFoundException` 会原样穿到 JS
/// (差分实测 `js-corpus-2c4b306c42` 的裁判结论是 `host:PathNotFoundException`)。
/// 被测侧只有错误文本,故在这一层——也只有这一层——把它翻回类名标记。
fn js_facing_error(e: RuleError) -> String {
    if let RuleError::Json(m) = &e {
        if m.starts_with("PathNotFound") || m.starts_with("path not found") {
            return "«host:PathNotFoundException»".to_string();
        }
    }
    e.to_string()
}

type R<T> = Result<T, RuleError>;

pub struct AnalyzeRule {
    content: Option<RuleValue>,
    base_url: Option<String>,
    redirect_url: Option<JavaUrl>,
    is_json: bool,
    is_regex: bool,
    analyze_by_jsoup: Option<(Doc, ego_tree::NodeId)>,
    analyze_by_json: Option<Rc<AnalyzeByJSonPath>>,
    /// `getAnalyzeByXPath(o)` 的 content 缓存(AnalyzeRule.kt L137)。
    /// 兄弟两个(jsoup / json)从一开始就有,只有它漏了 —— 于是
    /// `ruleBookInfo` 八个字段全是 XPath 的源要把整页 html5ever 解析八遍。
    /// 存的是**树与根上下文**而不是 `AnalyzeByXPath` 本身:XPath 选出来的
    /// 元素要连着这棵树往下传(见 [`RuleValue::JxList`])。
    analyze_by_xpath: Option<(Doc, Vec<ego_tree::NodeId>)>,
    string_rule_cache: HashMap<String, Vec<SourceRule>>,
    pub data: RuleData,
    pub host: Box<dyn HostEnv>,
    next_chapter_url: Option<String>,
    from_book_info: bool,
    /// `book` / `source` 两个 JS 绑定。由组装方(pipeline/engine)填,
    /// 缺席时 JS 里是 null —— 与真身「AnalyzeRule(null, null)」同形。
    pub book_binding: Option<rubato_core::host::BookBinding>,
    pub source_binding: Option<rubato_core::host::SourceBinding>,
    /// `chapter` 绑定(`AnalyzeRule.setChapter` 之后的那份实体)。**活的** ——
    /// 组装方每改一次章节实体就要刷新它,见 [`rubato_core::host::ChapterBinding`]。
    pub chapter_binding: Option<rubato_core::host::ChapterBinding>,
    /// JS 反调 `java.getElement(s)` 拿到的元素。JS 侧只过号(见 [`ElementHandle`]),
    /// 实体留在这里;一次 `eval_js` 用完即清。
    element_table: Vec<RuleValue>,
}

// ---- JS 侧拿到的元素句柄上的操作(jsoup Element/Elements 的可观察面)----
// 只对 Elements/Node 两个变体有意义;规则求出别的形态(字符串/JSON)时,
// 真身在 JS 里同样调不到这些方法 —— 这里退化成空串/空列表。

fn element_text(v: &RuleValue) -> String {
    match v {
        // jsoup `Elements.text()` 以空格相连(空项不产生分隔符,见 jsoup_join)
        RuleValue::Elements(doc, ids) => {
            let d = doc.borrow();
            let parts: Vec<String> = ids.iter().map(|&i| d.text_of(i)).collect();
            rubato_core::host::jsoup_join(&parts, " ")
        }
        RuleValue::Node(doc, id) => doc.borrow().text_of(*id),
        other => other.to_java_string(),
    }
}

fn element_inner_html(v: &RuleValue) -> String {
    match v {
        RuleValue::Elements(doc, ids) => {
            let d = doc.borrow();
            let parts: Vec<String> = ids.iter().map(|&i| d.inner_html_of(i)).collect();
            rubato_core::host::jsoup_join(&parts, "\n")
        }
        RuleValue::Node(doc, id) => doc.borrow().inner_html_of(*id),
        _ => String::new(),
    }
}

fn element_attr(v: &RuleValue, name: &str) -> String {
    match v {
        // jsoup `Elements.attr(k)` 取**第一个**有该属性的元素
        RuleValue::Elements(doc, ids) => {
            let d = doc.borrow();
            ids.iter().map(|&i| d.attr_of(i, name)).find(|a| !a.is_empty()).unwrap_or_default()
        }
        RuleValue::Node(doc, id) => doc.borrow().attr_of(*id, name),
        _ => String::new(),
    }
}

/// jsoup 的 `Selector.select(query, root)` 头一句是 `Validate.notEmpty(query)`
/// —— 空串抛的是 **ValidationException**,不是 SelectorParseException;
/// 全空白的串过得了这一关,到 `QueryParser` 里 trim 完才抛后者。
/// 两个类名书源都看得见(`try{...}catch(e)` 里按类别分支),故分开。
fn selector_error(css: &str) -> String {
    if css.is_empty() {
        rubato_core::host::host_tag("ValidationException")
    } else {
        rubato_core::host::host_tag("SelectorParseException")
    }
}

/// `Element.select(css)` / `Elements.select(css)` —— 逐个匹配的节点各成一个元素。
/// **选择器不合法要抛**(jsoup 如此),不能默默返回空 —— 见
/// [`rubato_core::host::RuleHost::el_select`]。
fn element_select(v: &RuleValue, css: &str) -> Result<Vec<RuleValue>, String> {
    let (doc, ids) = match v {
        RuleValue::Elements(doc, ids) => (doc.clone(), ids.clone()),
        RuleValue::Node(doc, id) => (doc.clone(), vec![*id]),
        _ => return Ok(Vec::new()),
    };
    // 空串在 jsoup 那里由 `Validate.notEmpty` 先拦下 —— 连解析器都进不去,
    // 所以哪怕这一组一个元素都没有,它照样抛
    if css.is_empty() {
        return Err(rubato_core::host::host_tag("ValidationException"));
    }
    // 先过 `QueryParser`:`Element.select` 走的是**纯 jsoup 选择器**那条路,
    // 不是 legado 的规则 DSL —— 报错的类别也由它决定
    html_compat::select::parse(css).map_err(|_| selector_error(css))?;
    let mut found = Vec::new();
    {
        let d = doc.borrow();
        for id in ids {
            found.extend(d.select_of(id, css).map_err(|_| selector_error(css))?);
        }
    }
    Ok(found.into_iter().map(|id| RuleValue::Node(doc.clone(), id)).collect())
}

/// `Elements.size()`。**单个 `Element` 不是 List** —— jsoup 里它没有 size/下标,
/// 故给 0。这一位还兼作代理层展开的终止条件:Elements 的子项是 Node,
/// Node 不再展开(否则 `element_at(Node, 0)` 给回自己会无限递归)。
fn element_size(v: &RuleValue) -> usize {
    match v {
        RuleValue::Elements(_, ids) => ids.len(),
        _ => 0,
    }
}

fn element_at(v: &RuleValue, i: usize) -> Option<RuleValue> {
    match v {
        RuleValue::Elements(doc, ids) => ids.get(i).map(|&id| RuleValue::Node(doc.clone(), id)),
        _ => None,
    }
}

/// `Element.remove()` / `Elements.remove()` —— 从文档里摘掉
fn element_remove(v: &RuleValue) {
    match v {
        RuleValue::Elements(doc, ids) => {
            let mut d = doc.borrow_mut();
            for &id in ids {
                d.detach(id);
            }
        }
        RuleValue::Node(doc, id) => doc.borrow_mut().detach(*id),
        _ => {}
    }
}

/// `Elements.not(css)` —— 把自己匹配 css 的那些去掉(见 `matches_query`)。
/// 底下是同一个 `QueryParser`,故错误通道同 `element_select`。
fn element_not(v: &RuleValue, css: &str) -> Result<Vec<RuleValue>, String> {
    let (doc, ids) = match v {
        RuleValue::Elements(doc, ids) => (doc.clone(), ids.clone()),
        RuleValue::Node(doc, id) => (doc.clone(), vec![*id]),
        _ => return Ok(Vec::new()),
    };
    // `Elements.not` → `Selector.filterOut(this, select(query, this))`,
    // 同样先过 notEmpty
    if css.is_empty() {
        return Err(rubato_core::host::host_tag("ValidationException"));
    }
    html_compat::select::parse(css).map_err(|_| selector_error(css))?;
    let keep: Vec<_> = {
        let d = doc.borrow();
        ids.into_iter().filter(|&id| !d.matches_query(id, css)).collect()
    };
    Ok(keep.into_iter().map(|id| RuleValue::Node(doc.clone(), id)).collect())
}

/// `Node.parentNode()`。Elements(一组)在 jsoup 里没有这个方法 ——
/// 那边是 `TypeError`,这里给 `None`(调用方给 JS 的 null)。
fn element_parent(v: &RuleValue) -> Option<RuleValue> {
    match v {
        RuleValue::Node(doc, id) => {
            let pid = doc.borrow().parent_of(*id)?;
            Some(RuleValue::Node(doc.clone(), pid))
        }
        _ => None,
    }
}

/// 占住 `AnalyzeRule.host` 的空壳。**全仓一份**:`eval_js` 期间自借用打破期
/// 用它(见那里的注释),而「只借 AnalyzeRule 的元素表、规则不经它求值」那几条路
/// (`engine::explore_kinds`、difftest 的 `js_case_runner`)拿的也是它 ——
/// 那三处从前各写各的,是同一个空壳的三份副本。
///
/// 它同时是 js-host 那三条裸指针(`VarPtr` / `NetP`)的**外层别名保护**:
/// 求值期间 host 被换出去,所以 `java.getString(rule)` 反调回来时不可能拿到
/// 同一个宿主的第二条 `&mut`。改这里之前先读 `js_host::host_env::VarPtr`
/// 的安全性说明(宿主自己还有一道 `in_eval` 自检,两道都在)。
///
/// **已知与真身的差别**:真身的 `evalJS` 是可重入的(Rhino 共享 scriptCache /
/// topScope,`java.getString('<js>…')` 能再进来一层),而这里报错。本套还没有
/// 用例踩到 —— 接 pipeline-corpus 的 B 层时它会变成真路径,那时要先拿探针把
/// 「递归几层、变量层怎么串」钉住,再决定是给 AnalyzeRule 一个备用宿主工厂
/// 还是照报错。见 docs/plan.md 的 M2h。
pub struct NoHost;

impl HostEnv for NoHost {
    fn eval_js(
        &mut self,
        _code: &str,
        _b: &JsBindings,
        _env: &mut dyn rubato_core::host::JsRuleEnv,
    ) -> Result<JsValue, String> {
        Err("eval_js 重入:host 已被换出(rule-engine 的自借用打破期);\
             真身在这里能递归,见 NoHost 的说明"
            .into())
    }

    fn web_js(&mut self, _req: &rubato_core::host::WebJsRequest<'_>) -> Result<String, String> {
        Err("web_js 重入:host 已被换出".into())
    }
}

/// 变量层:转给 `self.data`。真身里 `java.put/get` 打的是 AnalyzeRule 上的
/// 那条 chapter → book → ruleData → source 链,`RuleData` 已复刻。
impl VarStore for AnalyzeRule {
    fn put(&mut self, key: &str, value: &str) -> String {
        self.data.put(key, value)
    }

    fn get(&self, key: &str) -> String {
        self.data.get(key)
    }

    // `book.putVariable` / `chapter.putVariable` 那一层也要转下去 ——
    // 忘了转就落到 trait 的缺省空实现上,写进去的变量**静悄悄地丢**。
    fn put_entity(&mut self, entity: rubato_core::host::VarEntity, key: &str, value: &str) {
        self.data.put_entity(entity, key, value)
    }

    fn get_entity(&self, entity: rubato_core::host::VarEntity, key: &str) -> String {
        self.data.get_entity(entity, key)
    }
}

/// **`java` 的规则反调面**——真身的 `java` 就是 AnalyzeRule 本身。
/// 元素只过号,实体存在 `element_table` 里(见 [`RuleHost`])。
impl rubato_core::host::RuleHost for AnalyzeRule {
    fn rule_get_string(&mut self, rule: &str, content: Option<&str>) -> Result<String, String> {
        let c = content.map(|s| RuleValue::Str(s.to_string()));
        self.get_string(rule, c.as_ref(), false).map_err(js_facing_error)
    }

    fn rule_get_string_list(
        &mut self,
        rule: &str,
        content: Option<&str>,
    ) -> Result<Option<Vec<String>>, String> {
        let c = content.map(|s| RuleValue::Str(s.to_string()));
        self.get_string_list(rule, c.as_ref(), false).map_err(js_facing_error)
    }

    fn rule_get_element(&mut self, rule: &str) -> Result<Option<ElementHandle>, String> {
        let v = self.get_element(rule).map_err(js_facing_error)?;
        Ok(v.map(|v| self.intern_element(v)))
    }

    fn rule_get_elements(&mut self, rule: &str) -> Result<ElementList, String> {
        let (jsoup, vs) = self.get_elements_boxed(rule).map_err(js_facing_error)?;
        let items = vs.into_iter().map(|v| self.intern_element(v)).collect();
        Ok(ElementList { items, jsoup })
    }

    fn el_json(&mut self, h: ElementHandle) -> Option<serde_json::Value> {
        self.element(h).and_then(|v| v.to_gson_json())
    }

    fn el_java(&mut self, h: ElementHandle) -> Option<rubato_core::host::JavaValue> {
        let v = self.element(h)?;
        self.java_value(&v)
    }

    fn rule_parse_html(&mut self, html: &str) -> Result<ElementHandle, String> {
        // jsoup `Jsoup.parse` 给的是 Document(它本身也是个 Element)
        let doc: Doc = Rc::new(RefCell::new(AnalyzeByJSoup::from_html(html)));
        let root = doc.borrow().root();
        Ok(self.intern_element(RuleValue::Node(doc, root)))
    }

    fn rule_set_content(&mut self, content: &str, base_url: Option<&str>) -> Result<(), String> {
        self.set_content(RuleValue::Str(content.to_string()), base_url);
        Ok(())
    }

    fn el_to_string(&mut self, h: ElementHandle) -> String {
        self.element(h).map(|v| v.to_java_string()).unwrap_or_default()
    }

    fn el_text(&mut self, h: ElementHandle) -> String {
        self.element(h).map(|v| element_text(&v)).unwrap_or_default()
    }

    fn el_html(&mut self, h: ElementHandle) -> String {
        self.element(h).map(|v| element_inner_html(&v)).unwrap_or_default()
    }

    fn el_attr(&mut self, h: ElementHandle, name: &str) -> String {
        self.element(h).map(|v| element_attr(&v, name)).unwrap_or_default()
    }

    fn el_select(&mut self, h: ElementHandle, css: &str) -> Result<Vec<ElementHandle>, String> {
        let Some(v) = self.element(h) else { return Ok(Vec::new()) };
        Ok(element_select(&v, css)?.into_iter().map(|e| self.intern_element(e)).collect())
    }

    fn el_size(&mut self, h: ElementHandle) -> usize {
        self.element(h).map(|v| element_size(&v)).unwrap_or(0)
    }

    fn el_get(&mut self, h: ElementHandle, i: usize) -> Option<ElementHandle> {
        let v = self.element(h)?;
        element_at(&v, i).map(|e| self.intern_element(e))
    }

    fn el_parent(&mut self, h: ElementHandle) -> Option<ElementHandle> {
        let v = self.element(h)?;
        element_parent(&v).map(|e| self.intern_element(e))
    }

    fn el_remove(&mut self, h: ElementHandle) {
        if let Some(v) = self.element(h) {
            element_remove(&v);
        }
    }

    fn el_not(&mut self, h: ElementHandle, css: &str) -> Result<Vec<ElementHandle>, String> {
        let Some(v) = self.element(h) else { return Ok(Vec::new()) };
        Ok(element_not(&v, css)?.into_iter().map(|e| self.intern_element(e)).collect())
    }
}

impl AnalyzeRule {
    fn intern_element(&mut self, v: RuleValue) -> ElementHandle {
        self.element_table.push(v);
        (self.element_table.len() - 1) as ElementHandle
    }

    fn element(&self, h: ElementHandle) -> Option<RuleValue> {
        self.element_table.get(h as usize).cloned()
    }
}

impl AnalyzeRule {
    pub fn new(host: Box<dyn HostEnv>, data: RuleData) -> Self {
        Self {
            content: None,
            base_url: None,
            redirect_url: None,
            is_json: false,
            is_regex: false,
            analyze_by_jsoup: None,
            analyze_by_json: None,
            analyze_by_xpath: None,
            string_rule_cache: HashMap::new(),
            data,
            host,
            next_chapter_url: None,
            from_book_info: false,
            book_binding: None,
            source_binding: None,
            chapter_binding: None,
            element_table: Vec::new(),
        }
    }

    /// `setContent(null)`:详情页 init 规则可能给 null(pipeline 用)
    pub fn set_content_optional(
        &mut self,
        content: Option<RuleValue>,
        base_url: Option<&str>,
    ) -> &mut Self {
        match content {
            Some(v) => self.set_content(v, base_url),
            None => {
                // Kotlin `content.toString()` 对 null 是 "null" → isJson=false
                self.is_json = false;
                self.content = None;
                self.set_base_url(base_url);
                self.analyze_by_jsoup = None;
                self.analyze_by_json = None;
                self.analyze_by_xpath = None;
                self
            }
        }
    }

    /// `setContent`
    pub fn set_content(&mut self, content: RuleValue, base_url: Option<&str>) -> &mut Self {
        self.is_json = match &content {
            RuleValue::Node(..) => false,
            other => is_json(&other.to_java_string()),
        };
        self.content = Some(content);
        self.set_base_url(base_url);
        self.analyze_by_jsoup = None;
        self.analyze_by_json = None;
        self.analyze_by_xpath = None;
        self
    }

    pub fn set_base_url(&mut self, base_url: Option<&str>) -> &mut Self {
        if let Some(b) = base_url {
            self.base_url = Some(b.to_string());
        }
        self
    }

    /// 当前 redirectUrl(pipeline 的 formatKeepImg 用)
    pub fn redirect_url_ref(&self) -> Option<&JavaUrl> {
        self.redirect_url.as_ref()
    }

    pub fn set_next_chapter_url(&mut self, url: Option<&str>) {
        self.next_chapter_url = url.map(str::to_string);
    }

    pub fn set_from_book_info(&mut self, v: bool) {
        self.from_book_info = v;
    }

    /// `setRedirectUrl`:data URL 不改写;解析失败只记日志,保持旧值
    pub fn set_redirect_url(&mut self, url: &str) -> Option<&JavaUrl> {
        if rubato_core::net_utils::is_data_url(url) {
            return self.redirect_url.as_ref();
        }
        match JavaUrl::parse(url) {
            Ok(u) => self.redirect_url = Some(u),
            Err(e) => self.host.log(&format!("URL({url}) error\n{e}")),
        }
        self.redirect_url.as_ref()
    }

    // ---------- 解析器获取(与裁判一致:只对 content 本体缓存) ----------

    fn is_content(&self, v: &RuleValue) -> bool {
        self.content.as_ref().is_some_and(|c| c.same_as(v))
    }

    fn jsoup_for(&mut self, o: &RuleValue) -> (Doc, ego_tree::NodeId) {
        if self.is_content(o) {
            if self.analyze_by_jsoup.is_none() {
                self.analyze_by_jsoup = Some(new_jsoup(o));
            }
            return self.analyze_by_jsoup.clone().expect("刚填过");
        }
        new_jsoup(o)
    }

    /// `getAnalyzeByXPath(o)`。真身按 o 的类型分支:
    /// Document / Element / Elements 直接拿现成的树当根,其余
    /// `strToJXDocument(o.toString())` 重新解析。被测侧的 `Node` / `Elements`
    /// 就是"现成的树",借 `AnalyzeByJSoup::html()` 在**同一棵树**上求值;
    /// 其余形态(字符串 / JSON / List<JXNode>)走重新解析那条。
    fn xpath_tree(&mut self, o: &RuleValue) -> (Doc, Vec<ego_tree::NodeId>) {
        let parse = |o: &RuleValue| {
            let (html, root) =
                xpath_compat::AnalyzeByXPath::from_content_string(&o.to_java_string()).into_parts();
            (Rc::new(RefCell::new(AnalyzeByJSoup::from_tree(html))), root)
        };
        match o {
            RuleValue::Node(doc, id) => (doc.clone(), vec![*id]),
            RuleValue::Elements(doc, ids) => (doc.clone(), ids.clone()),
            // 重新解析那条:**只对 content 本体缓存**,与真身
            // `if (o != content) AnalyzeByXPath(o) else 缓存` 同口径,
            // 也与本文件的 jsoup_for / json_for 同形。
            other if self.is_content(other) => {
                if self.analyze_by_xpath.is_none() {
                    self.analyze_by_xpath = Some(parse(other));
                }
                self.analyze_by_xpath.clone().expect("刚填过")
            }
            other => parse(other),
        }
    }

    fn xpath_on<T>(
        &mut self,
        o: &RuleValue,
        f: impl Fn(&Html, &[ego_tree::NodeId]) -> Result<T, xpath_compat::XPathError>,
    ) -> R<T> {
        let (doc, root) = self.xpath_tree(o);
        let d = doc.borrow();
        f(d.html(), &root).map_err(|e: xpath_compat::XPathError| RuleError::Dsl(e.0))
    }

    fn json_for(&mut self, o: &RuleValue) -> R<Rc<AnalyzeByJSonPath>> {
        if self.is_content(o) {
            if self.analyze_by_json.is_none() {
                self.analyze_by_json = Some(Rc::new(new_json(o)?));
            }
            return Ok(self.analyze_by_json.clone().expect("刚填过"));
        }
        Ok(Rc::new(new_json(o)?))
    }

    // ---------- 规则切分 ----------

    fn split_source_rule_cache_string(&mut self, rule_str: &str) -> Vec<SourceRule> {
        if rule_str.is_empty() {
            return Vec::new();
        }
        if let Some(v) = self.string_rule_cache.get(rule_str) {
            return v.clone();
        }
        let v = self.split_source_rule(rule_str, false);
        self.string_rule_cache.insert(rule_str.to_string(), v.clone());
        v
    }

    /// `splitSourceRule`
    pub fn split_source_rule(&mut self, rule_str: &str, all_in_one: bool) -> Vec<SourceRule> {
        if rule_str.is_empty() {
            return Vec::new();
        }
        let mut rule_list: Vec<SourceRule> = Vec::new();
        let mut m_mode = Mode::Default;
        let mut start = 0usize;
        // 仅首字符为 : 时为 AllInOne
        if all_in_one && rule_str.starts_with(':') {
            m_mode = Mode::Regex;
            self.is_regex = true;
            start = 1;
        } else if self.is_regex {
            m_mode = Mode::Regex;
        }

        for m in JS_PATTERN.captures_iter(rule_str) {
            let whole = m.get(0).expect("整段");
            if whole.start() > start {
                let tmp = kotlin_trim_ascii(&rule_str[start..whole.start()]);
                if !tmp.is_empty() {
                    rule_list.push(SourceRule::new(&tmp, m_mode, self.is_json));
                }
            }
            let g2 = m.get(2).map(|x| x.as_str()).unwrap_or("");
            let g1 = m.get(1).map(|x| x.as_str()).unwrap_or("");
            let body = if g2.is_empty() { g1 } else { g2 };
            rule_list.push(SourceRule::new(body, Mode::Js, self.is_json));
            start = whole.end();
        }

        for m in WEB_JS_PATTERN.captures_iter(rule_str) {
            let whole = m.get(0).expect("整段");
            if whole.start() > start {
                let tmp = kotlin_trim_ascii(&rule_str[start..whole.start()]);
                if !tmp.is_empty() {
                    rule_list.push(SourceRule::new(&tmp, m_mode, self.is_json));
                }
            }
            let body = m.get(1).map(|x| x.as_str()).unwrap_or("");
            rule_list.push(SourceRule::new(body, Mode::WebJs, self.is_json));
            start = whole.end();
        }

        if rule_str.len() > start {
            let tmp = kotlin_trim_ascii(&rule_str[start..]);
            if !tmp.is_empty() {
                rule_list.push(SourceRule::new(&tmp, m_mode, self.is_json));
            }
        }
        rule_list
    }

    // ---------- put / get ----------

    fn put_rule(&mut self, put_map: &[(String, Option<String>)]) -> R<()> {
        let ordered = java_util::hash_map_order(put_map.to_vec());
        for (key, value) in ordered {
            let v = match value {
                Some(v) => self.get_string(&v, None, false)?,
                None => String::new(),
            };
            self.data.put(&key, &v);
        }
        Ok(())
    }

    pub fn put(&mut self, key: &str, value: &str) -> String {
        self.data.put(key, value)
    }

    pub fn get(&self, key: &str) -> String {
        self.data.get(key)
    }

    // ---------- makeUpRule ----------

    /// `SourceRule.makeUpRule`:回填 `@get:{}` / `{{}}` / `$N`,再切 `##`
    fn make_up_rule(&mut self, sr: &mut SourceRule, result: Option<&RuleValue>) -> R<()> {
        if !sr.rule_param.is_empty() {
            let mut info_val = String::new();
            let list = result.and_then(RuleValue::as_list);
            let mut index = sr.rule_param.len();
            while index > 0 {
                index -= 1;
                let reg_type = sr.rule_type[index];
                let param = sr.rule_param[index].clone();
                if reg_type > DEFAULT_RULE_TYPE {
                    match &list {
                        // result 不是 List → 原样保留 "$N"
                        None => info_val.insert_str(0, &param),
                        Some(l) => {
                            if l.len() > reg_type as usize {
                                match &l[reg_type as usize] {
                                    Some(s) => info_val.insert_str(0, s),
                                    // 元素为 null:Kotlin 的 ?.let 返回 null → 走 elvis
                                    None => info_val.insert_str(0, &param),
                                }
                            }
                            // size <= N:if 无 else 返回 Unit(非 null)→ 什么也不插
                        }
                    }
                } else if reg_type == JS_RULE_TYPE {
                    if is_rule(&param) {
                        let rl = self.get_or_create_single_source_rule(&param);
                        let s = self.get_string_with(rl, None, false, true)?;
                        info_val.insert_str(0, &s);
                    } else {
                        match self.eval_js(&param, result)? {
                            None => {}
                            Some(RuleValue::Str(s)) => info_val.insert_str(0, &s),
                            Some(RuleValue::Num(d)) if d % 1.0 == 0.0 => {
                                info_val.insert_str(0, &format!("{d:.0}"));
                            }
                            Some(v) => info_val.insert_str(0, &v.to_java_string()),
                        }
                    }
                } else if reg_type == GET_RULE_TYPE {
                    let v = self.get(&param);
                    info_val.insert_str(0, &v);
                } else {
                    info_val.insert_str(0, &param);
                }
            }
            sr.rule = info_val;
        }
        // 分离正则表达式
        let parts: Vec<String> = sr.rule.split("##").map(str::to_string).collect();
        sr.rule = kotlin_trim(&parts[0]);
        if parts.len() > 1 {
            sr.replace_regex = parts[1].clone();
        }
        if parts.len() > 2 {
            sr.replacement = parts[2].clone();
        }
        if parts.len() > 3 {
            sr.replace_first = true;
        }
        Ok(())
    }

    fn get_or_create_single_source_rule(&mut self, rule: &str) -> Vec<SourceRule> {
        if let Some(v) = self.string_rule_cache.get(rule) {
            return v.clone();
        }
        let v = vec![SourceRule::new(rule, Mode::Default, self.is_json)];
        if self.string_rule_cache.len() < 16 {
            self.string_rule_cache.insert(rule.to_string(), v.clone());
        }
        v
    }

    // ---------- JS ----------

    /// 规则链上的值 → **JS 里的 Java 形态**(见
    /// [`rubato_core::host::JavaValue`])。元素只过号;jayway/gson 的容器逐层
    /// 建成 NativeJavaMap / NativeJavaList;`None` = 这个值不是 Java 对象
    /// (Rhino 的 NativeObject —— 那是真 JS 对象,由 [`Self::bind_value`] 另走一支)。
    pub fn java_value(&mut self, v: &RuleValue) -> Option<rubato_core::host::JavaValue> {
        use crate::value::{JavaFlavor, java_value_of};
        use rubato_core::host::JavaValue;
        let boxed_strs = |l: &[String]| -> Vec<JavaValue> {
            l.iter().map(|s| JavaValue::Str(s.clone())).collect()
        };
        Some(match v {
            RuleValue::Str(s) => JavaValue::Str(s.clone()),
            RuleValue::Num(d) => JavaValue::Num(*d, v.to_java_string()),
            RuleValue::Bool(b) => JavaValue::Bool(*b),
            RuleValue::StrList(l) => JavaValue::List(boxed_strs(l), v.to_java_string()),
            RuleValue::StrListList(ll) => JavaValue::List(
                ll.iter()
                    .map(|x| JavaValue::List(boxed_strs(x), format!("[{}]", x.join(", "))))
                    .collect(),
                v.to_java_string(),
            ),
            // **出身决定数组的 toString**:文本解析出来的是 json-smart
            // (`JSONArray` → JSON 文本),Java 对象模型上读出来的是 gson 的
            // `ArrayList`(→ `[a, b]`)。见 value.rs 的 JavaFlavor。
            RuleValue::Json(j) => java_value_of(j, JavaFlavor::JsonSmart),
            RuleValue::Java(j) => java_value_of(j, JavaFlavor::Gson),
            RuleValue::GsonMap(m) => {
                java_value_of(&serde_json::Value::Object(m.clone()), JavaFlavor::Gson)
            }
            RuleValue::JsonArray(a) | RuleValue::JsonList(a) => JavaValue::List(
                a.iter().map(|x| java_value_of(x, JavaFlavor::JsonSmart)).collect(),
                v.to_java_string(),
            ),
            // `List<JXNode>`:容器是普通 ArrayList,而**项**可能是真元素
            // —— 项是元素时整表就过不了 GSON(裁判侧 normError JsonIOException)
            RuleValue::JxList(items) => {
                let s = v.to_java_string();
                let mut out = Vec::with_capacity(items.len());
                for it in items {
                    out.push(self.java_value(it)?);
                }
                JavaValue::List(out, s)
            }
            RuleValue::Elements(..) | RuleValue::Node(..) => {
                JavaValue::Element(self.intern_element(v.clone()))
            }
            // NativeObject / NativeArray 不是 Java 对象
            RuleValue::Native(_) => return None,
        })
    }

    /// 规则链上的值 → JS 绑定。**不拍平**:真身 `bindings["result"] = result`
    /// 绑的是对象本身,`result.anchor` / `result[0]` / `result.parentNode()`
    /// 都是它的方法面(见 [`rubato_core::host::BoundValue`])。
    ///
    /// 三支各走各的:jsoup 元素过号;jayway/gson 的 **Java 对象**逐层包成
    /// NativeJavaMap / NativeJavaList;`<js>` 造出来的 **NativeObject** 是真
    /// JS 对象。**标量不包**——`Context.javaToJS` 对 String/Number/Boolean
    /// 原样交回(探针 `js-bind-txt-typeof` / `-num-typeof` / `-flag-typeof`)。
    pub fn bind_value(&mut self, v: Option<&RuleValue>) -> Option<rubato_core::host::BoundValue> {
        use rubato_core::host::{BoundValue, JavaValue};
        let v = v?;
        Some(match v {
            RuleValue::Str(s) => BoundValue::Str(s.clone()),
            RuleValue::StrList(l) => BoundValue::StrList(l.clone()),
            // 数字 / 布尔要绑成 **JS 原生 number / boolean**:真身走
            // `Context.javaToJS`,对 Number/Boolean 原样交回。绑成串的话
            // `<js>1100000000+parseInt(result)</js>` 的结果再进 `{{result}}`
            // 就成了 `1.100000001E9`(Double.toString)而不是 `1100000001`
            // (`makeUpRule` 对整数 Double 走 `String.format("%.0f")`)。
            RuleValue::Num(d) => BoundValue::Num(*d),
            RuleValue::Bool(b) => BoundValue::Bool(*b),
            RuleValue::Elements(..) | RuleValue::Node(..) => {
                BoundValue::Element(self.intern_element(v.clone()))
            }
            // **NativeObject / NativeArray**:JS 自己造的对象,原样交回 JS
            // (`String(result)` 是 `[object Object]`、`hasOwnProperty` 在)
            RuleValue::Native(j) => match j {
                serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                    BoundValue::Native { json: j.clone(), str: v.to_java_string() }
                }
                // 载荷是标量时它本来就是 JS 的原生值
                serde_json::Value::String(s) => BoundValue::Str(s.clone()),
                serde_json::Value::Number(n) => BoundValue::Num(n.as_f64().unwrap_or(f64::NAN)),
                serde_json::Value::Bool(b) => BoundValue::Bool(*b),
                serde_json::Value::Null => return None,
            },
            other => match self.java_value(other)? {
                // 标量那几支 javaToJS 原样交回,**不装箱**
                JavaValue::Str(s) => BoundValue::Str(s),
                JavaValue::Num(d, _) => BoundValue::Num(d),
                JavaValue::Bool(b) => BoundValue::Bool(b),
                JavaValue::Null => return None,
                jv => BoundValue::Java(jv),
            },
        })
    }

    fn eval_js(&mut self, code: &str, result: Option<&RuleValue>) -> R<Option<RuleValue>> {
        let result_binding = self.bind_value(result);
        let src_binding = self.bind_value(self.content.clone().as_ref());
        let bindings = JsBindings {
            host: rubato_core::host::JsHost::Rule,
            source_login: false,
            result: result_binding,
            base_url: self.base_url.clone(),
            src: src_binding,
            title: self.data.chapter_title.clone(),
            next_chapter_url: self.next_chapter_url.clone(),
            from_book_info: self.from_book_info,
            // AnalyzeRule 不绑定 key/page(那是 AnalyzeUrl 的绑定)
            key: None,
            page: None,
            book: self.book_binding.clone(),
            source: self.source_binding.clone(),
            chapter: self.chapter_binding.clone(),
        };
        // **打破自借用**:`java` 在真身里就是 AnalyzeRule 本身,所以 JS 宿主要
        // 同时拿到 `&mut self`(变量层 + 规则反调面)。host 是 self 的字段,
        // 临时换出去、用完换回来 —— 期间 self.host 是个不会被调到的空壳
        // (真身里 `java.getString` 内部若再遇 `<js>` 会递归回 JS,那条路
        //  本套还没有用例;换回来之前不会走到)。
        let mut host = std::mem::replace(&mut self.host, Box::new(NoHost));
        let out = host.eval_js(code, &bindings, self);
        self.host = host;
        // 元素要**在清表之前**查回来(JS 那边只拿到号)
        let out = match out {
            Ok(JsValue::Elements { handles, strings, list }) => {
                Ok(self.elements_from_js(&handles, &strings, list))
            }
            Err(e) => Err(RuleError::Js(e)),
            Ok(JsValue::Null) => Ok(None),
            Ok(JsValue::Str(s)) => Ok(Some(RuleValue::Str(s))),
            Ok(JsValue::Num(d)) => Ok(Some(RuleValue::Num(d))),
            Ok(JsValue::Bool(b)) => Ok(Some(RuleValue::Bool(b))),
            Ok(JsValue::List(v)) => Ok(Some(RuleValue::StrList(v))),
            // **单个 NativeObject**:`<js>` 交回来的对象是 Rhino 的 NativeObject,
            // 不是 jayway 容器 —— `toString()` 是 `[object Object]`,而且
            // AnalyzeRule 对它有专用的「键值直接访问」分支(见 RuleValue::Native)。
            // 探针 js-bind-native-*。
            Ok(JsValue::Json(v)) => Ok(Some(RuleValue::Native(v))),
            Ok(JsValue::Native(a)) => Ok(Some(RuleValue::Native(Value::Array(a)))),
        };
        self.element_table.clear();
        out
    }

    /// JS 交回来的元素句柄 → 规则链上的值。
    ///
    /// `list == false` 是**单个** Element/Elements:原样交回(单个 Element
    /// 交给 `getElements` 时真身给空表,`explode_list` 同口径)。
    /// `list == true` 是数组 / jsoup Elements:合成一份 `Elements`;
    /// 里面若混着整组(`Elements` 项),按其节点摊平 —— 真身那边它们各自
    /// 还是一项,语料里没有这种写法,记在 README 上。
    /// 一个句柄都没有(空的 Elements)时没有文档可依,落回它的 `toString()`。
    fn elements_from_js(
        &self,
        handles: &[ElementHandle],
        strings: &[String],
        list: bool,
    ) -> Option<RuleValue> {
        if !list {
            return handles.first().and_then(|h| self.element(*h));
        }
        let mut doc: Option<Doc> = None;
        let mut ids = Vec::new();
        for h in handles {
            let (d, mut inner) = match self.element(*h) {
                Some(RuleValue::Node(d, id)) => (d, vec![id]),
                Some(RuleValue::Elements(d, inner)) => (d, inner),
                _ => continue,
            };
            // **NodeId 只在自己那棵树上有意义**:JS 里混进
            // `org.jsoup.Jsoup.parse(…)` 的新文档时,把它的号安在页面那棵树上
            // 会取到别的节点(`node()` 那里还会 panic)。混文档就整个落回串形态,
            // 也就是本变体出现之前的口径。
            if doc.as_ref().is_some_and(|first| !Rc::ptr_eq(first, &d)) {
                return Some(RuleValue::StrList(strings.to_vec()));
            }
            doc.get_or_insert(d);
            ids.append(&mut inner);
        }
        match doc {
            Some(d) => Some(RuleValue::Elements(d, ids)),
            None => Some(RuleValue::Str(rubato_core::host::jsoup_join(strings, "\n"))),
        }
    }

    /// `getWebJsResult(jsStr, result)`(AnalyzeRule.kt L179-196)。
    ///
    /// AnalyzeRule 这半边的三位交给宿主:`jsStr`、`GSON.toJson(result)`(当前值)、
    /// 以及 `url = baseUrl` / `html = content.toString()` —— 注意 `content` 是
    /// **AnalyzeRule 的字段**(setContent 存下的那份),不是当前值。
    fn web_js(&mut self, code: &str, result: Option<&RuleValue>) -> R<String> {
        let json = result.map(RuleValue::to_java_string).unwrap_or_else(|| "null".into());
        // `content.toString()`:字段是 `Any? = null`,Kotlin 的 toString 给 "null"
        let content =
            self.content.as_ref().map(RuleValue::to_java_string).unwrap_or_else(|| "null".into());
        let req = rubato_core::host::WebJsRequest {
            js: code,
            result_json: &json,
            base_url: self.base_url.as_deref(),
            content: &content,
        };
        self.host.web_js(&req).map_err(RuleError::Js)
    }

    // ---------- getString ----------

    pub fn get_string(
        &mut self,
        rule_str: &str,
        m_content: Option<&RuleValue>,
        is_url: bool,
    ) -> R<String> {
        if rule_str.is_empty() {
            return Ok(String::new());
        }
        let rule_list = self.split_source_rule_cache_string(rule_str);
        self.get_string_with(rule_list, m_content, is_url, true)
    }

    pub fn get_string_unescape(&mut self, rule_str: &str, unescape: bool) -> R<String> {
        if rule_str.is_empty() {
            return Ok(String::new());
        }
        let rule_list = self.split_source_rule_cache_string(rule_str);
        self.get_string_with(rule_list, None, false, unescape)
    }

    fn get_string_with(
        &mut self,
        rule_list: Vec<SourceRule>,
        m_content: Option<&RuleValue>,
        is_url: bool,
        unescape: bool,
    ) -> R<String> {
        let mut result: Option<RuleValue> = None;
        let content = m_content.cloned().or_else(|| self.content.clone());
        if let Some(content) = content {
            if !rule_list.is_empty() {
                if let RuleValue::GsonMap(m) = &content {
                    // 真身 `result is LinkedTreeMap` 短路:直取 first().rule 这个键,
                    // 不跑 putRule / makeUpRule / replaceRegex(AnalyzeRule L334-336)
                    let v = rule_list
                        .first()
                        .and_then(|sr| m.get(&sr.rule))
                        .filter(|v| !v.is_null())
                        .map(|v| RuleValue::Str(gson::object_to_string(v)));
                    return self.finish_get_string(v, is_url, unescape);
                }
                // 真身 `result is NativeObject`(AnalyzeRule L321-331):**只跑第一条**
                // 规则,putRule / makeUpRule / replaceRegex 都在,取值是
                // 「键值直接访问」;Js/Json 两个 mode 另有分支,`{{}}` 多参的原样给规则串
                if let RuleValue::Native(Value::Object(m)) = &content {
                    let m = m.clone();
                    let Some(sr) = rule_list.first().cloned() else {
                        return self.finish_get_string(None, is_url, unescape);
                    };
                    let mut sr = sr;
                    self.put_rule(&sr.put_map)?;
                    self.make_up_rule(&mut sr, Some(&content))?;
                    let picked = match sr.mode {
                        Mode::Js => self.eval_js(&sr.rule, Some(&content))?,
                        Mode::Json => {
                            let a = self.json_for(&content)?;
                            a.get_string(&sr.rule)
                                .map_err(|e| RuleError::Dsl(e.to_string()))?
                                .map(RuleValue::Str)
                        }
                        _ if sr.param_size() > 1 => Some(RuleValue::Str(sr.rule.clone())),
                        _ => m
                            .get(&sr.rule)
                            .filter(|v| !v.is_null())
                            .map(|v| RuleValue::Native(v.clone())),
                    };
                    let v = picked.map(|v| RuleValue::Str(replace_regex(&v.to_java_string(), &sr)));
                    return self.finish_get_string(v, is_url, unescape);
                }
                result = Some(content);
                for sr in rule_list {
                    let mut sr = sr;
                    self.put_rule(&sr.put_map)?;
                    self.make_up_rule(&mut sr, result.as_ref())?;
                    let Some(cur) = result.clone() else { continue };
                    let rule = sr.rule.clone();
                    if !kotlin_trim(&rule).is_empty() || sr.replace_regex.is_empty() {
                        result = match sr.mode {
                            Mode::WebJs => Some(RuleValue::Str(self.web_js(&rule, Some(&cur))?)),
                            Mode::Js => self.eval_js(&rule, Some(&cur))?,
                            Mode::Json => {
                                let a = self.json_for(&cur)?;
                                a.get_string(&rule)
                                    .map_err(|e| RuleError::Dsl(e.to_string()))?
                                    .map(RuleValue::Str)
                            }
                            Mode::XPath => self
                                .xpath_on(&cur, |d, r| xpath_compat::get_string_on(d, r, &rule))?
                                .map(RuleValue::Str),
                            Mode::Default => {
                                let (doc, root) = self.jsoup_for(&cur);
                                let mut d = doc.borrow_mut();
                                if is_url {
                                    Some(RuleValue::Str(
                                        d.get_string0_from(root, &rule)
                                            .map_err(|e| RuleError::Dsl(e.to_string()))?,
                                    ))
                                } else {
                                    d.get_string_from(root, &rule)
                                        .map_err(|e| RuleError::Dsl(e.to_string()))?
                                        .map(RuleValue::Str)
                                }
                            }
                            Mode::Regex => Some(RuleValue::Str(rule.clone())),
                        };
                    }
                    if let Some(cur) = result.clone() {
                        if !sr.replace_regex.is_empty() {
                            result =
                                Some(RuleValue::Str(replace_regex(&cur.to_java_string(), &sr)));
                        }
                    }
                }
            }
        }
        self.finish_get_string(result, is_url, unescape)
    }

    /// getString 的公共尾巴(`result == null → ""` 之后的部分)
    fn finish_get_string(
        &self,
        result: Option<RuleValue>,
        is_url: bool,
        unescape: bool,
    ) -> R<String> {
        let result_str = result.map(|v| v.to_java_string()).unwrap_or_default();
        let str = if unescape && result_str.contains('&') {
            unescape_html4(&result_str)
        } else {
            result_str
        };
        if is_url {
            return Ok(if kotlin_trim(&str).is_empty() {
                self.base_url.clone().unwrap_or_default()
            } else {
                get_absolute_url(self.redirect_url.as_ref(), &str)
            });
        }
        Ok(str)
    }

    // ---------- getStringList ----------

    pub fn get_string_list(
        &mut self,
        rule_str: &str,
        m_content: Option<&RuleValue>,
        is_url: bool,
    ) -> R<Option<Vec<String>>> {
        if rule_str.is_empty() {
            return Ok(None);
        }
        let rule_list = self.split_source_rule_cache_string(rule_str);
        self.get_string_list_with(rule_list, m_content, is_url)
    }

    fn get_string_list_with(
        &mut self,
        rule_list: Vec<SourceRule>,
        m_content: Option<&RuleValue>,
        is_url: bool,
    ) -> R<Option<Vec<String>>> {
        let mut result: Option<RuleValue> = None;
        let content = m_content.cloned().or_else(|| self.content.clone());
        let mut map_short_circuit = false;
        if let Some(content) = content {
            if !rule_list.is_empty() {
                if let RuleValue::GsonMap(m) = &content {
                    // 真身短路(AnalyzeRule L242-244):`result = result[first().rule]`,
                    // 原样保留取出来的对象,由尾巴的 `as? List<String>` 决定死活
                    map_short_circuit = true;
                    result = rule_list
                        .first()
                        .and_then(|sr| m.get(&sr.rule))
                        .filter(|v| !v.is_null())
                        .map(|v| match v {
                            Value::String(s) => RuleValue::Str(s.clone()),
                            // ArrayList<Object> → `as? List<String>` 因擦除而成立
                            Value::Array(a) => {
                                RuleValue::StrList(a.iter().map(gson::object_to_string).collect())
                            }
                            // 嵌套 Map / 数字 / 布尔:不是 List → 尾巴给 null
                            other => RuleValue::Json(other.clone()),
                        });
                }
                if !map_short_circuit {
                    result = Some(content);
                    for sr in rule_list {
                        let mut sr = sr;
                        self.put_rule(&sr.put_map)?;
                        self.make_up_rule(&mut sr, result.as_ref())?;
                        let Some(cur) = result.clone() else { continue };
                        let rule = sr.rule.clone();
                        if !rule.is_empty() {
                            result = match sr.mode {
                                Mode::WebJs => {
                                    let s = self.web_js(&rule, Some(&cur))?;
                                    // GSON.fromJsonArray<String> 成功则当列表用
                                    Some(match parse_json_string_array(&s) {
                                        Some(list) => RuleValue::StrList(list),
                                        None => RuleValue::Str(s),
                                    })
                                }
                                Mode::Js => self.eval_js(&rule, Some(&cur))?,
                                Mode::Json => {
                                    let a = self.json_for(&cur)?;
                                    Some(RuleValue::StrList(
                                        a.get_string_list(&rule)
                                            .map_err(|e| RuleError::Dsl(e.to_string()))?,
                                    ))
                                }
                                Mode::XPath => {
                                    Some(RuleValue::StrList(self.xpath_on(&cur, |d, r| {
                                        xpath_compat::get_string_list_on(d, r, &rule)
                                    })?))
                                }
                                Mode::Default => {
                                    let (doc, root) = self.jsoup_for(&cur);
                                    let mut d = doc.borrow_mut();
                                    Some(RuleValue::StrList(
                                        d.get_string_list_from(root, &rule)
                                            .map_err(|e| RuleError::Dsl(e.to_string()))?,
                                    ))
                                }
                                Mode::Regex => Some(RuleValue::Str(rule.clone())),
                            };
                        }
                        if !sr.replace_regex.is_empty() {
                            let cur = result.clone();
                            result = match cur.as_ref().and_then(RuleValue::as_list) {
                                Some(l) => Some(RuleValue::StrList(
                                    l.iter()
                                        .map(|x| replace_regex(x.as_deref().unwrap_or("null"), &sr))
                                        .collect(),
                                )),
                                None => Some(RuleValue::Str(replace_regex(
                                    &cur.map(|v| v.to_java_string())
                                        .unwrap_or_else(|| "null".into()),
                                    &sr,
                                ))),
                            };
                        }
                    }
                } // if !map_short_circuit
            }
        }
        let Some(result) = result else { return Ok(None) };
        // String → 按 \n 切
        let list: Option<Vec<String>> = match &result {
            RuleValue::Str(s) => Some(s.split('\n').map(str::to_string).collect()),
            other => other
                .as_list()
                .map(|l| l.into_iter().map(|x| x.unwrap_or_else(|| "null".into())).collect()),
        };
        if is_url {
            let mut urls: Vec<String> = Vec::new();
            if let Some(l) = &list {
                for u in l {
                    let abs = get_absolute_url(self.redirect_url.as_ref(), u);
                    if !abs.is_empty() && !urls.contains(&abs) {
                        urls.push(abs);
                    }
                }
            }
            return Ok(Some(urls));
        }
        Ok(list)
    }

    // ---------- getElement / getElements ----------

    pub fn get_element(&mut self, rule_str: &str) -> R<Option<RuleValue>> {
        if rule_str.is_empty() {
            return Ok(None);
        }
        let mut result: Option<RuleValue> = None;
        let content = self.content.clone();
        let rule_list = self.split_source_rule(rule_str, true);
        if let (Some(content), false) = (content, rule_list.is_empty()) {
            result = Some(content);
            for sr in rule_list {
                let mut sr = sr;
                self.put_rule(&sr.put_map)?;
                self.make_up_rule(&mut sr, result.as_ref())?;
                let Some(cur) = result.clone() else { continue };
                let rule = sr.rule.clone();
                result = self.dispatch_element(&sr, &rule, &cur, false)?;
                if !sr.replace_regex.is_empty() {
                    let s = result.map(|v| v.to_java_string()).unwrap_or_else(|| "null".into());
                    result = Some(RuleValue::Str(replace_regex(&s, &sr)));
                }
            }
        }
        Ok(result)
    }

    /// `getElements`——注意裁判这里**不调 makeUpRule**(引擎原样如此)
    pub fn get_elements(&mut self, rule_str: &str) -> R<Vec<RuleValue>> {
        Ok(self.get_elements_boxed(rule_str)?.1)
    }

    /// `getElements` + **容器本身是不是 jsoup 的 `Elements`**。
    ///
    /// 展开成 `Vec<RuleValue>` 会把容器类型抹掉,而书源在 JS 里看得见它:
    /// `Elements.toString()` 是各 outerHtml 换行相连、过不了 GSON,普通
    /// `ArrayList` 是 `[a, b]`、过得了。见 [`rubato_core::host::ElementList`]。
    pub fn get_elements_boxed(&mut self, rule_str: &str) -> R<(bool, Vec<RuleValue>)> {
        let mut result: Option<RuleValue> = None;
        let content = self.content.clone();
        let rule_list = self.split_source_rule(rule_str, true);
        if let (Some(content), false) = (content, rule_list.is_empty()) {
            result = Some(content);
            for sr in rule_list {
                self.put_rule(&sr.put_map)?;
                let Some(cur) = result.clone() else { continue };
                let rule = sr.rule.clone();
                result = self.dispatch_element(&sr, &rule, &cur, true)?;
            }
        }
        // LegadoTeam 基准:不再 `result as List<Any>` 强转,而是按 List / Array /
        // NativeArray 三分支展开并过滤 null 与 Scriptable.NOT_FOUND;其余类型
        // (String/Map/标量)一律落到 `return ArrayList()` —— 旧基准这里抛
        // ClassCastException,是本次基准切换的语义漂移点之一。
        match result {
            None => Ok((false, Vec::new())),
            Some(v) => {
                let jsoup = matches!(v, RuleValue::Elements(..) | RuleValue::Node(..));
                Ok((jsoup, explode_list(&v).unwrap_or_default()))
            }
        }
    }

    fn dispatch_element(
        &mut self,
        sr: &SourceRule,
        rule: &str,
        cur: &RuleValue,
        plural: bool,
    ) -> R<Option<RuleValue>> {
        Ok(match sr.mode {
            Mode::Regex => {
                let regs: Vec<String> = split_not_blank(rule, "&&");
                if regs.is_empty() {
                    return Err(RuleError::Regex("空正则组".into()));
                }
                let src = cur.to_java_string();
                if plural {
                    Some(RuleValue::StrListList(
                        regex_elements::get_elements(&src, &regs, 0)
                            .map_err(|e| RuleError::Regex(e.to_string()))?,
                    ))
                } else {
                    regex_elements::get_element(&src, &regs, 0)
                        .map_err(|e| RuleError::Regex(e.to_string()))?
                        .map(RuleValue::StrList)
                }
            }
            Mode::WebJs => {
                let s = self.web_js(rule, Some(cur))?;
                // getElement 用 fromJsonObject<Map<..>>、getElements 用
                // fromJsonArray<Map<..>>:形状不符时 gson 抛异常 → getOrNull() 给 null
                match serde_json::from_str::<Value>(&s) {
                    Ok(Value::Object(m)) if !plural => Some(RuleValue::Json(Value::Object(m))),
                    Ok(Value::Array(a))
                        if plural && a.iter().all(|x| x.is_object() || x.is_null()) =>
                    {
                        Some(RuleValue::JsonList(a.into_iter().filter(|x| !x.is_null()).collect()))
                    }
                    _ => None,
                }
            }
            Mode::Js => self.eval_js(rule, Some(cur))?,
            Mode::Json => {
                // content 是 Java 对象模型(LinkedTreeMap)时,读出来的容器是
                // ArrayList/LinkedTreeMap 而不是 json-smart 的 JSONArray
                let java = matches!(cur, RuleValue::GsonMap(_) | RuleValue::Java(_));
                let wrap = |v: Value| if java { RuleValue::Java(v) } else { RuleValue::Json(v) };
                let a = self.json_for(cur)?;
                if plural {
                    let list = a.get_list(rule).map_err(|e| RuleError::Dsl(e.to_string()))?;
                    if java {
                        Some(RuleValue::Java(Value::Array(list)))
                    } else {
                        Some(RuleValue::JsonArray(list))
                    }
                } else {
                    match a.get_object_scalar(rule) {
                        // `AnalyzeByJSonPath.getObject(rule): Any = ctx.read(rule)`
                        // 返回类型**非空**:读到 JSON null 时 Kotlin 的空检查抛 NPE
                        Some((v, _)) if v.is_null() => {
                            return Err(RuleError::Json(format!("NullPointerException: {rule}")));
                        }
                        // indefinite 路径的结果是 jayway 新建的 JSONArray,
                        // 不随模型走 Java 口径
                        Some((v, scalar)) => {
                            Some(if scalar { wrap(v) } else { RuleValue::Json(v) })
                        }
                        None => return Err(RuleError::Json(format!("PathNotFound: {rule}"))),
                    }
                }
            }
            // `getAnalyzeByXPath(result).getElements(rule)` 给的是 `List<JXNode>`。
            // JXNode 不是 jsoup Element:它可能是元素、也可能是字符串,而且**不进
            // 元素表** —— 真身把这个 List 往下传时,下一条规则拿到的是
            // `List<JXNode>`,`AnalyzeByXPath(o)` / `AnalyzeByJSoup(o)` 都落到
            // `o.toString()` 那条(Java 集合口径 `[a, b]`)再重新解析。
            // 被测侧用 `StrList` 表示它:`to_java_string()` 同样给 `[a, b]`,
            // 展开成列表时同样是各 `JXNode.toString()`(= asString())。
            Mode::XPath => {
                let (doc, root) = self.xpath_tree(cur);
                let items = {
                    let d = doc.borrow();
                    xpath_compat::get_elements_on(d.html(), &root, rule)
                        .map_err(|e| RuleError::Dsl(e.0))?
                        .map(|ns| {
                            ns.iter()
                                .map(|n| match n {
                                    // 真实树上的元素:**保住元素身份**往下传
                                    xpath_compat::JxNode::El(id) => {
                                        RuleValue::Node(doc.clone(), *id)
                                    }
                                    // 合成元素 / 字符串:树上没有它,只能拿串继续
                                    other => RuleValue::Str(xpath_compat::node_as_string(
                                        d.html(),
                                        other,
                                    )),
                                })
                                .collect::<Vec<_>>()
                        })
                };
                items.map(RuleValue::JxList)
            }
            Mode::Default => {
                let (doc, root) = self.jsoup_for(cur);
                let ids = doc
                    .borrow()
                    .get_elements_of(root, rule)
                    .map_err(|e| RuleError::Dsl(e.to_string()))?;
                Some(RuleValue::Elements(doc, ids))
            }
        })
    }
}

// ---------- 自由函数 ----------

fn new_jsoup(o: &RuleValue) -> (Doc, ego_tree::NodeId) {
    match o {
        // `AnalyzeByJSoup(doc)` 里 `doc is Element` 直接复用该元素为根
        RuleValue::Node(doc, id) => (doc.clone(), *id),
        other => {
            let d = Rc::new(RefCell::new(AnalyzeByJSoup::from_html(&other.to_java_string())));
            let root = d.borrow().root();
            (d, root)
        }
    }
}

fn new_json(o: &RuleValue) -> R<AnalyzeByJSonPath> {
    match o {
        RuleValue::Json(v) => Ok(AnalyzeByJSonPath::from_value(v.clone())),
        // 裁判 `JsonPath.parse(Object)`:LinkedTreeMap 直接当 JSON 模型用,
        // 不经过 toString() 再解析
        RuleValue::GsonMap(m) => Ok(AnalyzeByJSonPath::from_value(Value::Object(m.clone()))),
        RuleValue::Java(v) => Ok(AnalyzeByJSonPath::from_value(v.clone())),
        // NativeObject/NativeArray:jayway 读得动(Rhino 的 NativeObject 实现了
        // `java.util.Map`),但读出来的**还是 Rhino 的对象** —— `getString` 那一步
        // 的 `toString()` 给 `[object Object]`,不是 json-smart 的 `{k=v}`。
        // 见 `ModelFlavor`(pb02434 就是这么照出来的)。
        RuleValue::Native(v) => Ok(AnalyzeByJSonPath::from_native(v.clone())),
        RuleValue::JsonArray(a) | RuleValue::JsonList(a) => {
            Ok(AnalyzeByJSonPath::from_value(Value::Array(a.clone())))
        }
        // 裁判 `AnalyzeByJSonPath(json: Any)` 只有 **String** 那一支走
        // `JsonPath.parse(String)`(带 notEmpty / notNull 两道);别的对象走
        // Object 重载,空的 jsoup Element、`List<String>` 之类照样收得下 ——
        // 我们这边一律先 `to_java_string()`,所以那两道**只能**按原来的类型加。
        RuleValue::Str(s) => {
            AnalyzeByJSonPath::parse_string(s).map_err(|_| RuleError::Json("InvalidJson".into()))
        }
        other => AnalyzeByJSonPath::parse_permissive(&other.to_java_string())
            .map_err(|_| RuleError::Json("InvalidJson".into())),
    }
}

/// `AnalyzeRule.replaceRegex`——与 regex-compat 的 `legado_replace_regex` 同源
fn replace_regex(result: &str, sr: &SourceRule) -> String {
    legado_replace_regex(result, &sr.replace_regex, &sr.replacement, sr.replace_first)
}

/// `SourceRule.isRule`
fn is_rule(rule_str: &str) -> bool {
    rule_str.starts_with('@')
        || rule_str.starts_with("$.")
        || rule_str.starts_with("$[")
        || rule_str.starts_with("//")
}

/// `String.splitNotBlank(vararg delimiter)`
fn split_not_blank(s: &str, delim: &str) -> Vec<String> {
    s.split(delim).map(kotlin_trim).filter(|x| !x.is_empty()).collect()
}

/// Kotlin `trim { it <= ' ' }`(splitSourceRule 里用的是这个,不是默认 trim)
fn kotlin_trim_ascii(s: &str) -> String {
    s.trim_matches(|c: char| c <= ' ').to_string()
}

/// `GSON.fromJsonArray<String>(...)`:必须是 JSON 数组,元素按 gson
/// StringJsonDeserializer 取串,null 元素被 filterNotNull 丢掉
fn parse_json_string_array(s: &str) -> Option<Vec<String>> {
    let v: Value = serde_json::from_str(s).ok()?;
    let Value::Array(a) = v else { return None };
    // `Gson.fromJsonArray<String>`:源码里写着「含 null 元素就抛」,但**实测**
    // (judge 探针 `@webjs:[null]` / `["a",null]` / `["a", , "b"]`)null 元素是被
    // **静默丢弃**的,列表照常返回 —— 以裁判实测为准。
    // 元素按 StringJsonDeserializer 取串:基元 asString、结构体 JsonElement.toString()
    a.iter().filter(|x| !x.is_null()).map(gson::string_field).collect()
}

/// `result as List<Any>`:把列表值摊成逐元素的 RuleValue
fn explode_list(v: &RuleValue) -> Option<Vec<RuleValue>> {
    match v {
        RuleValue::StrList(l) => Some(l.iter().map(|s| RuleValue::Str(s.clone())).collect()),
        RuleValue::StrListList(l) => {
            Some(l.iter().map(|x| RuleValue::StrList(x.clone())).collect())
        }
        RuleValue::JsonArray(a) | RuleValue::JsonList(a) => {
            Some(a.iter().map(|x| RuleValue::Json(x.clone())).collect())
        }
        RuleValue::Java(Value::Array(a)) => {
            Some(a.iter().map(|x| RuleValue::Java(x.clone())).collect())
        }
        RuleValue::Elements(doc, ids) => {
            Some(ids.iter().map(|&i| RuleValue::Node(doc.clone(), i)).collect())
        }
        // `List<JXNode>`:项已经各自是元素或串了
        RuleValue::JxList(v) => Some(v.clone()),
        // NativeArray:元素原样留着(对象仍是 NativeObject,下游按键取值)
        RuleValue::Native(Value::Array(a)) => {
            Some(a.iter().map(|v| RuleValue::Native(v.clone())).collect())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rubato_core::VarLayer;

    struct EchoHost;
    impl HostEnv for EchoHost {
        fn eval_js(
            &mut self,
            code: &str,
            b: &JsBindings,
            vars: &mut dyn rubato_core::host::JsRuleEnv,
        ) -> Result<JsValue, String> {
            Ok(match code {
                // 绑定是对象不是串(元素要查表),桩这里只要它的 toString 形态
                "#result" => JsValue::Str(
                    b.result.as_ref().map(|v| v.to_java_string(vars)).unwrap_or_default(),
                ),
                _ => JsValue::Str(code.to_string()),
            })
        }
        fn web_js(&mut self, req: &rubato_core::host::WebJsRequest<'_>) -> Result<String, String> {
            Ok(req.js.to_string())
        }
    }

    const HTML: &str = r#"<ul class="l"><li class="i"><a href="/1.html">甲</a></li>
        <li class="i"><a href="2.html">乙</a></li></ul>"#;

    fn rule(data: RuleData) -> AnalyzeRule {
        let mut ar = AnalyzeRule::new(Box::new(EchoHost), data);
        ar.set_content(RuleValue::Str(HTML.into()), Some("http://e.com/a/b.html"));
        ar.set_redirect_url("http://e.com/a/b.html");
        ar
    }

    #[test]
    fn jsoup_string_and_list() {
        let mut ar = rule(RuleData::default());
        assert_eq!(ar.get_string("class.i@text", None, false).unwrap(), "甲\n乙");
        assert_eq!(
            ar.get_string_list("class.i@text", None, false).unwrap(),
            Some(vec!["甲".to_string(), "乙".to_string()])
        );
    }

    #[test]
    fn is_url_absolutizes() {
        let mut ar = rule(RuleData::default());
        assert_eq!(
            ar.get_string_list("class.i@a@href", None, true).unwrap(),
            Some(vec!["http://e.com/1.html".into(), "http://e.com/a/2.html".into()])
        );
        // 空结果时 isUrl 回落到 baseUrl
        assert_eq!(ar.get_string("class.none@href", None, true).unwrap(), "http://e.com/a/b.html");
    }

    #[test]
    fn replace_regex_and_unescape() {
        let mut ar = rule(RuleData::default());
        assert_eq!(ar.get_string("class.i@text##甲##A", None, false).unwrap(), "A\n乙");
        // getString 末尾的 unescapeHtml4:text() 已解一层,这里再解一层
        let mut ar2 = AnalyzeRule::new(Box::new(EchoHost), RuleData::default());
        ar2.set_content(RuleValue::Str("<p>a&amp;amp;b&amp;nbsp;c</p>".into()), None);
        assert_eq!(ar2.get_string("tag.p@text", None, false).unwrap(), "a&b\u{a0}c");
        // unescape=false 时保留原样
        assert_eq!(ar2.get_string_unescape("tag.p@text", false).unwrap(), "a&amp;b&nbsp;c");
    }

    #[test]
    fn put_and_get_variables() {
        let data = RuleData { rule_data: Some(VarLayer::default()), ..Default::default() };
        let mut ar = rule(data);
        let out = ar.get_string(r#"@put:{"k":"class.i@a@text"}@get:{k}"#, None, false).unwrap();
        assert_eq!(out, "甲\n乙");
        assert_eq!(ar.get("k"), "甲\n乙");
    }

    #[test]
    fn js_segment_receives_result() {
        let mut ar = rule(RuleData::default());
        assert_eq!(ar.get_string("class.i@text<js>#result</js>", None, false).unwrap(), "甲\n乙");
    }
}
