//! 书源实体模型:`BookSource` + 五个规则对象,按裁判 GSON 语义从 JSON 反序列化
//! (契约:fixtures/cases/source/README.md;差分 tools/source_diff.sh)。
//!
//! 关键语义(对照 utils/GsonExtensions.kt + rule 实体的 jsonDeserializer):
//! - String 字段:基元一律 asString(数字/布尔转字面量),对象/数组转紧凑 JSON;
//! - Int 字段:**只接受数字**(字符串数字不转,落回默认值);
//! - Long/Boolean 字段:GSON 默认适配器(字符串数字/布尔可转,类型不合法
//!   让**整个解析失败**);
//! - 规则对象:接受对象或「字符串包 JSON」(字符串按 lenient 再解析),
//!   其它类型 → null;
//! - 未知键忽略;整体解析 lenient。

use crate::gson;
use serde_json::Value;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SearchRule {
    pub check_key_word: Option<String>,
    pub book_list: Option<String>,
    pub name: Option<String>,
    pub author: Option<String>,
    pub intro: Option<String>,
    pub kind: Option<String>,
    pub last_chapter: Option<String>,
    pub update_time: Option<String>,
    pub book_url: Option<String>,
    pub cover_url: Option<String>,
    pub word_count: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ExploreRule {
    pub book_list: Option<String>,
    pub name: Option<String>,
    pub author: Option<String>,
    pub intro: Option<String>,
    pub kind: Option<String>,
    pub last_chapter: Option<String>,
    pub update_time: Option<String>,
    pub book_url: Option<String>,
    pub cover_url: Option<String>,
    pub word_count: Option<String>,
}

/// `BookListRule` 接口的等价物:搜索/发现共用的列表规则视图
#[derive(Debug, Default, Clone, PartialEq)]
pub struct BookListRuleView {
    pub book_list: Option<String>,
    pub name: Option<String>,
    pub author: Option<String>,
    pub intro: Option<String>,
    pub kind: Option<String>,
    pub last_chapter: Option<String>,
    pub update_time: Option<String>,
    pub book_url: Option<String>,
    pub cover_url: Option<String>,
    pub word_count: Option<String>,
}

impl From<&SearchRule> for BookListRuleView {
    fn from(r: &SearchRule) -> Self {
        BookListRuleView {
            book_list: r.book_list.clone(),
            name: r.name.clone(),
            author: r.author.clone(),
            intro: r.intro.clone(),
            kind: r.kind.clone(),
            last_chapter: r.last_chapter.clone(),
            update_time: r.update_time.clone(),
            book_url: r.book_url.clone(),
            cover_url: r.cover_url.clone(),
            word_count: r.word_count.clone(),
        }
    }
}

impl From<&ExploreRule> for BookListRuleView {
    fn from(r: &ExploreRule) -> Self {
        BookListRuleView {
            book_list: r.book_list.clone(),
            name: r.name.clone(),
            author: r.author.clone(),
            intro: r.intro.clone(),
            kind: r.kind.clone(),
            last_chapter: r.last_chapter.clone(),
            update_time: r.update_time.clone(),
            book_url: r.book_url.clone(),
            cover_url: r.cover_url.clone(),
            word_count: r.word_count.clone(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct BookInfoRule {
    pub init: Option<String>,
    pub name: Option<String>,
    pub author: Option<String>,
    pub intro: Option<String>,
    pub kind: Option<String>,
    pub last_chapter: Option<String>,
    pub update_time: Option<String>,
    pub cover_url: Option<String>,
    pub toc_url: Option<String>,
    pub word_count: Option<String>,
    pub can_re_name: Option<String>,
    pub download_urls: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TocRule {
    pub pre_update_js: Option<String>,
    pub chapter_list: Option<String>,
    pub chapter_name: Option<String>,
    pub chapter_url: Option<String>,
    pub format_js: Option<String>,
    pub is_volume: Option<String>,
    pub is_vip: Option<String>,
    pub is_pay: Option<String>,
    pub update_time: Option<String>,
    pub next_toc_url: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ContentRule {
    pub content: Option<String>,
    pub sub_content: Option<String>,
    pub title: Option<String>,
    pub next_content_url: Option<String>,
    pub web_js: Option<String>,
    pub source_regex: Option<String>,
    pub replace_regex: Option<String>,
    pub image_style: Option<String>,
    pub image_decode: Option<String>,
    pub pay_action: Option<String>,
    pub call_back_js: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BookSource {
    pub book_source_url: String,
    pub book_source_name: String,
    pub book_source_group: Option<String>,
    pub book_source_type: i32,
    pub book_url_pattern: Option<String>,
    pub custom_order: i32,
    pub enabled: bool,
    pub enabled_explore: bool,
    pub js_lib: Option<String>,
    pub enabled_cookie_jar: Option<bool>,
    pub concurrent_rate: Option<String>,
    pub header: Option<String>,
    pub login_url: Option<String>,
    pub login_ui: Option<String>,
    pub login_check_js: Option<String>,
    pub cover_decode_js: Option<String>,
    /// LegadoTeam 新增:JS 单文件书源正文。Rubato 砍单(docs/plan.md §6),
    /// 只解析出来供上层判定 is_js_source() 后显式报「不支持」。
    pub main_js: Option<String>,
    pub book_source_comment: Option<String>,
    pub variable_comment: Option<String>,
    pub last_update_time: i64,
    pub respond_time: i64,
    pub weight: i32,
    pub explore_url: Option<String>,
    pub explore_screen: Option<String>,
    pub rule_explore: Option<ExploreRule>,
    pub search_url: Option<String>,
    pub rule_search: Option<SearchRule>,
    pub rule_book_info: Option<BookInfoRule>,
    pub rule_toc: Option<TocRule>,
    pub rule_content: Option<ContentRule>,
    /// **解析这份实体的原始 JSON**。书源 JS 里 `source` 绑定在真身里就是
    /// BookSource 实体本身,书源会读任意字段(`source.bookSourceComment`、
    /// `source.ruleSearch.bookList`…),故整份留着交给 [`host::SourceBinding::raw`]。
    ///
    /// 已知与真身的差:实体上**没填的字段是 Java null**,而这里缺席就是
    /// `undefined`;`rule*` 若写成「字符串包 JSON」,实体解出来是对象而这里
    /// 仍是字符串。两条都会在 pipeline-corpus-b 上现形,不会静默。
    pub raw: Option<Value>,
}

impl Default for BookSource {
    fn default() -> Self {
        BookSource {
            book_source_url: String::new(),
            book_source_name: String::new(),
            book_source_group: None,
            book_source_type: 0,
            book_url_pattern: None,
            custom_order: 0,
            enabled: true,
            enabled_explore: true,
            js_lib: None,
            enabled_cookie_jar: Some(true),
            concurrent_rate: None,
            header: None,
            login_url: None,
            login_ui: None,
            login_check_js: None,
            cover_decode_js: None,
            main_js: None,
            book_source_comment: None,
            variable_comment: None,
            last_update_time: 0,
            respond_time: 180_000,
            weight: 0,
            explore_url: None,
            explore_screen: None,
            rule_explore: None,
            search_url: None,
            rule_search: None,
            rule_book_info: None,
            rule_toc: None,
            rule_content: None,
            raw: None,
        }
    }
}

/// 解析错误(细节不进差分,两侧只比「成败」)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceParseError(pub String);

type R<T> = Result<T, SourceParseError>;

fn err<T>(msg: impl Into<String>) -> R<T> {
    Err(SourceParseError(msg.into()))
}

/// String 字段(StringJsonDeserializer):见模块注释
fn str_field(v: Option<&Value>) -> Option<String> {
    v.and_then(gson::string_field)
}

/// 非空 String 字段:null → 保持默认(GSON 对非空 Kotlin 字段的 null 跳过?
/// 不——String 是引用类型,GSON 会写入 null,Kotlin 非空字段被塞 null。
/// bookSourceUrl/bookSourceName 实际会变 null 串;投影时按 Kotlin 崩溃面回避,
/// 用例不生成 null 的这两个字段;这里 null → 空串。
fn str_field_nonnull(v: Option<&Value>, default: &str) -> String {
    match v {
        None => default.to_string(),
        Some(Value::Null) => default.to_string(),
        Some(x) => gson::string_field(x).unwrap_or_else(|| default.to_string()),
    }
}

/// Int 字段(IntJsonDeserializer):只接受数字;其余 → 默认
fn int_field(v: Option<&Value>, default: i32) -> i32 {
    match v {
        Some(Value::Number(n)) => {
            n.as_i64().map(|i| i as i32).or_else(|| n.as_f64().map(|f| f as i32)).unwrap_or(default)
        }
        _ => default,
    }
}

/// Long 字段(GSON 默认适配器):数字/数字字符串;非法 → 整体失败
fn long_field(v: Option<&Value>, default: i64) -> R<i64> {
    match v {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Number(n)) => {
            Ok(n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(default))
        }
        Some(Value::String(s)) => match s.parse::<i64>() {
            Ok(i) => Ok(i),
            Err(_) => match s.parse::<f64>() {
                // JsonReader.nextLong:double 表示但值为整数时可转
                Ok(f) if f == (f as i64) as f64 => Ok(f as i64),
                _ => err(format!("long:{s}")),
            },
        },
        Some(other) => err(format!("long:{other}")),
    }
}

/// Boolean 字段(GSON 默认适配器):bool / 字符串(Boolean.parseBoolean);
/// 数字等 → 整体失败;null → 默认
fn bool_field(v: Option<&Value>, default: bool) -> R<bool> {
    match v {
        None | Some(Value::Null) => Ok(default),
        Some(Value::Bool(b)) => Ok(*b),
        Some(Value::String(s)) => Ok(s.eq_ignore_ascii_case("true")),
        Some(other) => err(format!("bool:{other}")),
    }
}

fn opt_bool_field(v: Option<&Value>, default: Option<bool>) -> R<Option<bool>> {
    match v {
        None => Ok(default),
        Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(Value::String(s)) => Ok(Some(s.eq_ignore_ascii_case("true"))),
        Some(other) => err(format!("bool:{other}")),
    }
}

/// 规则对象字段:对象 / 「字符串包 JSON」(lenient 再解析)/ 其余 → None。
/// 字符串再解析失败 → 异常穿透 → 整体失败(GSON fromJson 抛 JsonSyntaxException)
fn rule_value<'a>(v: Option<&'a Value>, storage: &'a mut Option<Value>) -> R<Option<&'a Value>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(o @ Value::Object(_)) => Ok(Some(o)),
        Some(Value::String(s)) => {
            // fromJson(String) 是 lenient;空串 → null(GSON 空输入返回 null)
            if s.trim().is_empty() {
                return Ok(None);
            }
            match gson::parse_lenient(s) {
                Some(parsed @ Value::Object(_)) => {
                    *storage = Some(parsed);
                    Ok(storage.as_ref())
                }
                // 字符串里是基元/数组:GSON 反射适配器对非对象……
                // fromJson("[1]", TocRule) 抛 → 整体失败;基元同理
                Some(_) => err("rule:non-object"),
                None => err("rule:bad-json"),
            }
        }
        // 数字/布尔基元:jsonDeserializer 走 isJsonPrimitive 分支 → asString 再解析
        Some(Value::Bool(b)) => {
            let _ = b;
            err("rule:primitive")
        }
        Some(Value::Number(_)) => err("rule:primitive"),
        Some(Value::Array(_)) => Ok(None),
    }
}

macro_rules! sf {
    ($m:expr, $k:literal) => {
        str_field($m.get($k))
    };
}

fn parse_search_rule(v: &Value) -> SearchRule {
    let Some(m) = v.as_object() else { return SearchRule::default() };
    SearchRule {
        check_key_word: sf!(m, "checkKeyWord"),
        book_list: sf!(m, "bookList"),
        name: sf!(m, "name"),
        author: sf!(m, "author"),
        intro: sf!(m, "intro"),
        kind: sf!(m, "kind"),
        last_chapter: sf!(m, "lastChapter"),
        update_time: sf!(m, "updateTime"),
        book_url: sf!(m, "bookUrl"),
        cover_url: sf!(m, "coverUrl"),
        word_count: sf!(m, "wordCount"),
    }
}

fn parse_explore_rule(v: &Value) -> ExploreRule {
    let Some(m) = v.as_object() else { return ExploreRule::default() };
    ExploreRule {
        book_list: sf!(m, "bookList"),
        name: sf!(m, "name"),
        author: sf!(m, "author"),
        intro: sf!(m, "intro"),
        kind: sf!(m, "kind"),
        last_chapter: sf!(m, "lastChapter"),
        update_time: sf!(m, "updateTime"),
        book_url: sf!(m, "bookUrl"),
        cover_url: sf!(m, "coverUrl"),
        word_count: sf!(m, "wordCount"),
    }
}

fn parse_book_info_rule(v: &Value) -> BookInfoRule {
    let Some(m) = v.as_object() else { return BookInfoRule::default() };
    BookInfoRule {
        init: sf!(m, "init"),
        name: sf!(m, "name"),
        author: sf!(m, "author"),
        intro: sf!(m, "intro"),
        kind: sf!(m, "kind"),
        last_chapter: sf!(m, "lastChapter"),
        update_time: sf!(m, "updateTime"),
        cover_url: sf!(m, "coverUrl"),
        toc_url: sf!(m, "tocUrl"),
        word_count: sf!(m, "wordCount"),
        can_re_name: sf!(m, "canReName"),
        download_urls: sf!(m, "downloadUrls"),
    }
}

fn parse_toc_rule(v: &Value) -> TocRule {
    let Some(m) = v.as_object() else { return TocRule::default() };
    TocRule {
        pre_update_js: sf!(m, "preUpdateJs"),
        chapter_list: sf!(m, "chapterList"),
        chapter_name: sf!(m, "chapterName"),
        chapter_url: sf!(m, "chapterUrl"),
        format_js: sf!(m, "formatJs"),
        is_volume: sf!(m, "isVolume"),
        is_vip: sf!(m, "isVip"),
        is_pay: sf!(m, "isPay"),
        update_time: sf!(m, "updateTime"),
        next_toc_url: sf!(m, "nextTocUrl"),
    }
}

fn parse_content_rule(v: &Value) -> ContentRule {
    let Some(m) = v.as_object() else { return ContentRule::default() };
    ContentRule {
        content: sf!(m, "content"),
        sub_content: sf!(m, "subContent"),
        title: sf!(m, "title"),
        next_content_url: sf!(m, "nextContentUrl"),
        web_js: sf!(m, "webJs"),
        source_regex: sf!(m, "sourceRegex"),
        replace_regex: sf!(m, "replaceRegex"),
        image_style: sf!(m, "imageStyle"),
        image_decode: sf!(m, "imageDecode"),
        pay_action: sf!(m, "payAction"),
        call_back_js: sf!(m, "callBackJs"),
    }
}

impl BookSource {
    /// 真身 BookSource.isJsSource():mainJs 非空即 JS 单文件书源
    pub fn is_js_source(&self) -> bool {
        self.main_js.as_deref().is_some_and(|v| !v.trim().is_empty())
    }

    /// `GSON.fromJson(json, BookSource::class.java)` 的等价面
    pub fn parse(json: &str) -> R<BookSource> {
        let Some(v) = gson::parse_lenient(json) else {
            return err("bad json");
        };
        Self::from_value(&v)
    }

    pub fn from_value(v: &Value) -> R<BookSource> {
        let Some(m) = v.as_object() else {
            return err("not object");
        };
        let d = BookSource::default();
        let mut storage = (None, None, None, None, None);
        Ok(BookSource {
            book_source_url: str_field_nonnull(m.get("bookSourceUrl"), ""),
            book_source_name: str_field_nonnull(m.get("bookSourceName"), ""),
            book_source_group: str_field(m.get("bookSourceGroup")),
            book_source_type: int_field(m.get("bookSourceType"), d.book_source_type),
            book_url_pattern: str_field(m.get("bookUrlPattern")),
            custom_order: int_field(m.get("customOrder"), d.custom_order),
            enabled: bool_field(m.get("enabled"), d.enabled)?,
            enabled_explore: bool_field(m.get("enabledExplore"), d.enabled_explore)?,
            js_lib: str_field(m.get("jsLib")),
            enabled_cookie_jar: opt_bool_field(m.get("enabledCookieJar"), d.enabled_cookie_jar)?,
            concurrent_rate: str_field(m.get("concurrentRate")),
            header: str_field(m.get("header")),
            login_url: str_field(m.get("loginUrl")),
            login_ui: str_field(m.get("loginUi")),
            login_check_js: str_field(m.get("loginCheckJs")),
            cover_decode_js: str_field(m.get("coverDecodeJs")),
            main_js: str_field(m.get("mainJs")),
            book_source_comment: str_field(m.get("bookSourceComment")),
            variable_comment: str_field(m.get("variableComment")),
            last_update_time: long_field(m.get("lastUpdateTime"), d.last_update_time)?,
            respond_time: long_field(m.get("respondTime"), d.respond_time)?,
            weight: int_field(m.get("weight"), d.weight),
            explore_url: str_field(m.get("exploreUrl")),
            explore_screen: str_field(m.get("exploreScreen")),
            rule_explore: rule_value(m.get("ruleExplore"), &mut storage.0)?.map(parse_explore_rule),
            search_url: str_field(m.get("searchUrl")),
            rule_search: rule_value(m.get("ruleSearch"), &mut storage.1)?.map(parse_search_rule),
            rule_book_info: rule_value(m.get("ruleBookInfo"), &mut storage.2)?
                .map(parse_book_info_rule),
            rule_toc: rule_value(m.get("ruleToc"), &mut storage.3)?.map(parse_toc_rule),
            rule_content: rule_value(m.get("ruleContent"), &mut storage.4)?.map(parse_content_rule),
            raw: Some(v.clone()),
        })
    }

    // ---- BookSource 的规则 getter(空缺给默认对象)----

    pub fn search_rule(&self) -> SearchRule {
        self.rule_search.clone().unwrap_or_default()
    }

    pub fn explore_rule(&self) -> ExploreRule {
        self.rule_explore.clone().unwrap_or_default()
    }

    pub fn book_info_rule(&self) -> BookInfoRule {
        self.rule_book_info.clone().unwrap_or_default()
    }

    pub fn toc_rule(&self) -> TocRule {
        self.rule_toc.clone().unwrap_or_default()
    }

    pub fn content_rule(&self) -> ContentRule {
        self.rule_content.clone().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_as_string_json() {
        let s = BookSource::parse(
            r#"{"bookSourceUrl":"http://a","ruleToc":"{\"chapterList\":\"@css:li\"}"}"#,
        )
        .expect("解析");
        assert_eq!(s.rule_toc.expect("有").chapter_list.as_deref(), Some("@css:li"));
    }

    #[test]
    fn int_from_string_keeps_default() {
        let s = BookSource::parse(r#"{"bookSourceType":"2","customOrder":3}"#).expect("解析");
        assert_eq!(s.book_source_type, 0);
        assert_eq!(s.custom_order, 3);
    }

    #[test]
    fn string_from_object_is_compact_json() {
        let s = BookSource::parse(r#"{"header":{"User-Agent":"x"}}"#).expect("解析");
        assert_eq!(s.header.as_deref(), Some(r#"{"User-Agent":"x"}"#));
    }
}

// ---- 发现分类(`data/entities/rule/ExploreKind.kt` + `FlexChildStyle.kt`)----
//
// **这两个类没有注册 jsonDeserializer**(GsonExtensions.kt 的 `GSON` 只给五个
// 规则对象注册了),走的是 gson 的**反射适配器**。关键的一位:Kotlin 的
// data class **全部参数都有默认值**时会额外生成一个无参构造 —— gson 的
// `ConstructorConstructor` 找得到它,于是**缺席的字段保留 Kotlin 默认值**
// (`type = "url"`、`layout_flexShrink = 1F`、`layout_flexBasisPercent = -1F`),
// 而不是 Unsafe 分配出来的 null / 0。两个类都满足这个条件。

/// `FlexChildStyle`(发现分类在 FlexboxLayout 里的子项样式)
#[derive(Debug, Clone, PartialEq)]
pub struct FlexChildStyle {
    pub layout_flex_grow: f32,
    pub layout_flex_shrink: f32,
    /// **`Option` 不是多此一举**:Kotlin 那边类型是非空 `String`,但 gson 的
    /// 反射适配器对**非基元**字段照样写得进 null(`"layout_alignSelf": null`)——
    /// 于是真身会出现「非空类型持有 null」。`None` 表示的就是那个状态。
    pub layout_align_self: Option<String>,
    pub layout_flex_basis_percent: f32,
    pub layout_wrap_before: bool,
    /// 真身注释写着「自定义的内部水平对齐属性」—— 不进 FlexboxLayout 的 lp。
    /// `None` 同 [`FlexChildStyle::layout_align_self`]
    pub layout_justify_self: Option<String>,
}

impl Default for FlexChildStyle {
    /// 真身的构造默认值(`FlexChildStyle.defaultStyle` 就是它)
    fn default() -> Self {
        Self {
            layout_flex_grow: 0.0,
            layout_flex_shrink: 1.0,
            layout_align_self: Some("auto".into()),
            layout_flex_basis_percent: -1.0,
            layout_wrap_before: false,
            layout_justify_self: Some("auto".into()),
        }
    }
}

impl FlexChildStyle {
    /// `alignSelf()`:字符串 → FlexboxLayout 的常数,认不出给 -1(auto)
    pub fn align_self(&self) -> i32 {
        match self.layout_align_self.as_deref().unwrap_or("") {
            "auto" => -1,
            "flex_start" => 0,
            "flex_end" => 1,
            "center" => 2,
            "baseline" => 3,
            "stretch" => 4,
            _ => -1,
        }
    }

    /// gson 反射适配器:逐字段读,**缺席保留默认值**;类型不合法整条解析失败
    fn from_value(v: &Value) -> R<Self> {
        let Value::Object(m) = v else {
            return err("FlexChildStyle 不是对象");
        };
        let mut out = Self::default();
        for (k, val) in m {
            match k.as_str() {
                "layout_flexGrow" => out.layout_flex_grow = gson_float(val)?,
                "layout_flexShrink" => out.layout_flex_shrink = gson_float(val)?,
                "layout_flexBasisPercent" => out.layout_flex_basis_percent = gson_float(val)?,
                "layout_wrapBefore" => out.layout_wrap_before = gson_bool(val)?,
                // String 字段走 StringJsonDeserializer:**JSON null 给 Java null**
                "layout_alignSelf" => out.layout_align_self = gson::string_field(val),
                "layout_justifySelf" => out.layout_justify_self = gson::string_field(val),
                _ => {}
            }
        }
        Ok(out)
    }
}

/// `ExploreKind`(发现页的一格)
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ExploreKind {
    /// 同 [`FlexChildStyle::layout_align_self`]:Kotlin 是非空 `String`,
    /// 而 gson 写得进 null(`"title": null`)—— 真身那里 `title` 会是 null,
    /// 界面上任何 `title.xxx` 当场 NPE。这是真身的样子,照记。
    pub title: Option<String>,
    pub url: Option<String>,
    /// 真身字段名是 `type`(Rust 关键字,这里改名;JSON 键仍是 `type`)。
    /// 取值见真身 `ExploreKind.Type`:url / text / button / toggle / select。
    /// `None` 同 [`ExploreKind::title`]
    pub kind_type: Option<String>,
    pub action: Option<String>,
    pub chars: Option<Vec<Option<String>>>,
    /// 真身字段名是 `default`
    pub default_value: Option<String>,
    pub view_name: Option<String>,
    pub style: Option<FlexChildStyle>,
}

impl ExploreKind {
    /// 真身的默认值:`title = ""`、`type = "url"`,其余 null
    pub fn new(title: impl Into<String>, url: Option<String>) -> Self {
        Self { title: Some(title.into()), url, kind_type: Some("url".into()), ..Default::default() }
    }

    /// `style()`:没给就是 `FlexChildStyle.defaultStyle`
    pub fn style(&self) -> FlexChildStyle {
        self.style.clone().unwrap_or_default()
    }

    fn from_value(v: &Value) -> R<Self> {
        let Value::Object(m) = v else {
            return err("ExploreKind 不是对象");
        };
        // 无参构造 → Kotlin 默认值
        let mut out = Self {
            title: Some(String::new()),
            kind_type: Some("url".into()),
            ..Default::default()
        };
        for (k, val) in m {
            match k.as_str() {
                "title" => out.title = gson::string_field(val),
                "url" => out.url = gson::string_field(val),
                "type" => out.kind_type = gson::string_field(val),
                "action" => out.action = gson::string_field(val),
                "default" => out.default_value = gson::string_field(val),
                "viewName" => out.view_name = gson::string_field(val),
                "chars" => {
                    out.chars = match val {
                        Value::Null => None,
                        Value::Array(a) => Some(a.iter().map(gson::string_field).collect()),
                        _ => return err("chars 不是数组"),
                    }
                }
                "style" => {
                    out.style = match val {
                        Value::Null => None,
                        other => Some(FlexChildStyle::from_value(other)?),
                    }
                }
                _ => {}
            }
        }
        Ok(out)
    }

    /// `GSON.fromJsonArray<ExploreKind>(str).getOrThrow()`。
    /// 顶层必须是数组;元素必须是对象(gson 遇到 JSON null 会往列表里放 null,
    /// 而 `fromJsonArray` 见到 null 元素就抛「列表不能存在null元素」)。
    pub fn parse_json_array(json: &str) -> R<Vec<ExploreKind>> {
        let Some(v) = gson::parse_lenient(json) else {
            return err("json 解析失败");
        };
        let Value::Array(a) = v else {
            return err("顶层不是数组");
        };
        if a.iter().any(|e| e.is_null()) {
            return err("列表不能存在null元素");
        }
        a.iter().map(ExploreKind::from_value).collect()
    }
}

/// gson 内建 Float 适配器:`(float) in.nextDouble()`,STRING 记号也认
/// (lenient 下 `nextDouble` 会去 parse 那个串),parse 不动就抛
fn gson_float(v: &Value) -> R<f32> {
    match v {
        Value::Number(n) => Ok(n.as_f64().unwrap_or(f64::NAN) as f32),
        Value::String(s) => s
            .trim()
            .parse::<f64>()
            .map(|d| d as f32)
            .map_err(|_| SourceParseError(format!("不是数字:{s}"))),
        _ => err("不是数字"),
    }
}

/// gson 内建 Boolean 适配器:BOOLEAN 记号直接取,STRING 走
/// `Boolean.parseBoolean`(**只有 "true" 忽略大小写为真,其余一律假**)
fn gson_bool(v: &Value) -> R<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::String(s) => Ok(s.eq_ignore_ascii_case("true")),
        _ => err("不是布尔"),
    }
}

// ---- 书/章节实体(pipeline 用到的字段面;真身带 Room/Parcelize)----

/// 实体层变量(`variableMap` + `variable` JSON 串的非空性)
#[derive(Debug, Default, Clone, PartialEq)]
pub struct EntityVars {
    pub map: std::collections::HashMap<String, String>,
    /// 对应 `variable != null`(构造给了串,或 putVariable 写过)
    pub present: bool,
    /// 构造时给的原始串解析失败时保留(投影按原串输出)
    pub raw_unparsed: Option<String>,
}

impl EntityVars {
    /// `variable` 构造参数:GSON.fromJsonObject<HashMap>(variable) ?: hashMapOf()
    pub fn from_variable(variable: Option<&str>) -> EntityVars {
        let Some(s) = variable else {
            return EntityVars::default();
        };
        match gson::parse_lenient(s).and_then(|v| match v {
            Value::Object(m) => {
                let mut out = std::collections::HashMap::new();
                for (k, v) in m {
                    // GSON 反序列化 HashMap<String,String>:值必须是基元
                    out.insert(k, gson::string_field(&v)?);
                }
                Some(out)
            }
            _ => None,
        }) {
            Some(map) => EntityVars { map, present: true, raw_unparsed: None },
            None => EntityVars {
                map: std::collections::HashMap::new(),
                present: true,
                raw_unparsed: Some(s.to_string()),
            },
        }
    }

    /// `RuleDataInterface.putVariable`(<10000 路径;pipeline 不触大变量)
    pub fn put(&mut self, key: &str, value: &str) {
        self.map.insert(key.to_string(), value.to_string());
        self.present = true;
        self.raw_unparsed = None;
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Book {
    pub book_url: String,
    pub toc_url: String,
    pub origin: String,
    pub origin_name: String,
    pub origin_order: i32,
    pub name: String,
    pub author: String,
    pub kind: Option<String>,
    pub cover_url: Option<String>,
    pub intro: Option<String>,
    pub type_: i32,
    pub word_count: Option<String>,
    pub latest_chapter_title: Option<String>,
    pub dur_chapter_index: i32,
    pub dur_chapter_title: Option<String>,
    pub total_chapter_num: i32,
    pub last_check_count: i32,
    pub vars: EntityVars,
    pub info_html: Option<String>,
    pub toc_html: Option<String>,
    pub download_urls: Option<Vec<String>>,
    /// `config.fixedType`(默认 false)
    pub config_fixed_type: bool,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SearchBook {
    pub book_url: String,
    pub origin: String,
    pub origin_name: String,
    pub type_: i32,
    pub name: String,
    pub author: String,
    pub kind: Option<String>,
    pub cover_url: Option<String>,
    pub intro: Option<String>,
    pub word_count: Option<String>,
    pub latest_chapter_title: Option<String>,
    pub toc_url: String,
    pub vars: EntityVars,
    pub origin_order: i32,
    pub info_html: Option<String>,
    pub toc_html: Option<String>,
}

// 注:旧基准(legado-with-MD3)的 SearchBook 有个 init 块,对 kind / intro /
// latestChapterTitle 分别截到 1000 / 5000 / 200 字符。LegadoTeam@3046111c 的
// SearchBook **没有这个 init 块**,一律不截断 —— pipeline-corpus 差分抓出
// (真实源的规则在合成页上取到超长串时两侧才分岔)。

impl Book {
    /// `Book.toSearchBook()`
    pub fn to_search_book(&self) -> SearchBook {
        let mut sb = SearchBook {
            name: self.name.clone(),
            author: self.author.clone(),
            kind: self.kind.clone(),
            book_url: self.book_url.clone(),
            origin: self.origin.clone(),
            origin_name: self.origin_name.clone(),
            type_: self.type_,
            word_count: self.word_count.clone(),
            latest_chapter_title: self.latest_chapter_title.clone(),
            cover_url: self.cover_url.clone(),
            intro: self.intro.clone(),
            toc_url: self.toc_url.clone(),
            origin_order: self.origin_order,
            vars: self.vars.clone(),
            info_html: None,
            toc_html: None,
        };
        sb.info_html = self.info_html.clone();
        sb.toc_html = self.toc_html.clone();
        sb
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct BookChapter {
    pub url: String,
    pub title: String,
    pub base_url: String,
    pub book_url: String,
    pub index: i32,
    pub is_volume: bool,
    pub is_vip: bool,
    pub is_pay: bool,
    pub tag: Option<String>,
    /// `tocCountWords` 从 updateTime 信息里抽出的字数(抽走后 tag 只剩其余部分)
    pub word_count: Option<String>,
    pub vars: EntityVars,
}
