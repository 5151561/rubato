//! WebBook 四步(搜索/发现/详情/目录/正文)的移植。
//! 逐行对照 judge/engine 的 model/webBook/{WebBook,BookList,BookInfo,
//! BookChapterList,BookContent}.kt;Debug 日志、协程取消、并发页
//! (差分固定 threadCount=1,串行序)不进被测面。
//!
//! Phase 1 契约(fixtures/cases/pipeline/README.md):
//! - loginCheckJs 非空 → 两侧一致报错(裁判是桩 JS 的 ClassCastException);
//! - source.header 走 BaseSource.getHeaderMap()(JSON 解析 + 默认 UA 注入);
//!   内联 `<js>` 头两侧都走桩(真 JS 是 Phase 2);
//! - 净化/简繁被砍:displayTitle 只去 \r\n。

use crate::helpers::{
    chapter_absolute_url, chapter_display_title, format_book_author, format_book_name, is_true,
    word_count_format,
};
use crate::html_formatter;
use net::fetch::{FetchError, StrResponse};
use net::transport::HttpTransport;
use net::{AnalyzeUrl, UrlArgs};
use regex_compat::JavaRegex;
use rubato_core::entities::{Book, BookChapter, BookListRuleView, BookSource, SearchBook};
use rubato_core::host::{CookieEnv, HostEnv, JsBindings, JsValue, WebViewHost};
use rubato_core::net_utils::{get_absolute_url_str, kotlin_trim};
use rubato_core::rule_data::{RuleData, VarLayer};
use rule_engine::AnalyzeRule;
use rule_engine::value::RuleValue;

// BookType(constant/BookType.kt)
const TYPE_TEXT: i32 = 0b1000;
const TYPE_AUDIO: i32 = 0b100000;
const TYPE_IMAGE: i32 = 0b1000000;
const TYPE_WEB_FILE: i32 = 0b10000000;
const TYPE_VIDEO: i32 = 0b100;
/// `BookType.allBookType`(含 video)
const ALL_BOOK_TYPE: i32 = TYPE_VIDEO | TYPE_TEXT | TYPE_IMAGE | TYPE_AUDIO | TYPE_WEB_FILE;

/// `AppConfig.tocCountWords`(真身 PreferKey 默认 true;差分垫片同值)
const TOC_COUNT_WORDS: bool = true;

/// `AppPattern.wordCountRegex`
static WORD_COUNT_REGEX: std::sync::LazyLock<JavaRegex> = std::sync::LazyLock::new(|| {
    JavaRegex::compile("(?:^|字数[：:、]?|\\s+)([0-9万千百\\.]{1,6}字)").expect("wordCountRegex")
});

#[derive(Debug)]
pub enum PipelineError {
    /// 保留 fetch 的可比对错误串(snapshot_miss / too_many_redirects)
    Fetch(FetchError),
    TocEmpty,
    ContentEmpty,
    Other(String),
}

impl PipelineError {
    /// 与裁判 fetch/pipeline op 相同的归一化
    pub fn to_diff_string(&self) -> String {
        match self {
            PipelineError::Fetch(e) => {
                let s = format!("{e}");
                if s.starts_with("snapshot_miss:") || s == "too_many_redirects" {
                    s
                } else {
                    "pipeline_error".to_string()
                }
            }
            PipelineError::TocEmpty => "toc_empty".to_string(),
            PipelineError::ContentEmpty => "content_empty".to_string(),
            PipelineError::Other(_) => "pipeline_error".to_string(),
        }
    }
}

type R<T> = Result<T, PipelineError>;

fn other<T>(msg: impl Into<String>) -> R<T> {
    Err(PipelineError::Other(msg.into()))
}

pub struct PipelineEnv<'a> {
    pub transport: &'a dyn HttpTransport,
    pub cookies: &'a mut dyn CookieEnv,
    /// JS 宿主工厂。**带 `RuleData`** —— 宿主的网络面(`java.ajax`)在真身里
    /// 是 `AnalyzeUrl(url, source, ruleData = book)`,url 里的 `{{…}}` 变量
    /// 要靠这一份才解得出值(见 `rubato_core::host::NetProvider::ajax`)。
    pub host_factory: &'a dyn Fn(&RuleData) -> Box<dyn HostEnv>,
    /// webView 平台面的工厂。**每次 `BackstageWebView` 都要一个新的** ——
    /// 真身那边是从池子里 acquire 一个 WebView、用完 release。
    /// `None` = 这一侧还没接(产品在 PlatformHooks 落地前),
    /// `{"webView": true}` 的书源就走 `FetchError::WebViewUnsupported`。
    pub web_view: Option<&'a dyn Fn() -> Box<dyn WebViewHost>>,
}

/// `BookSource.getBookType()`
/// `source` 绑定。真身里 `bindings["source"]` **就是 BookSource 实体**,
/// 书源 JS 读任意字段(`source.bookSourceComment` 再 eval 它是常见写法),
/// 故 `raw` 带整份原始 JSON —— 见 [`BookSource::raw`] 顶部记的两条已知差。
fn source_binding(source: &BookSource) -> rubato_core::host::SourceBinding {
    rubato_core::host::SourceBinding {
        key: source.book_source_url.clone(),
        tag: source.book_source_name.clone(),
        raw: source.raw.clone(),
    }
}

/// `book` 绑定,但实体是**正在装配的 SearchBook**。
///
/// 真身 `getSearchItem` 一上来就 `analyzeRule.setRuleData(searchBook)`
/// (BookList.kt L216),而 `book get() = ruleData as? BaseBook` —— SearchBook
/// **就是** BaseBook,所以搜索/发现两步的 `book` 绑定**不是 null**,是这本
/// 正在填的书。而且是**活的**:填到哪一步就读得到哪一步(`coverUrl` 规则里
/// `book.origin + …` 是常见写法,pb02759)。
///
/// (M2i 的 README 说「搜索/发现真身也是 null」—— 那句话只对 BookList 自己
/// 那个 AnalyzeRule 成立,`setRuleData` 之后就不是了。)
fn search_book_binding(sb: &SearchBook) -> rubato_core::host::BookBinding {
    rubato_core::host::BookBinding {
        name: sb.name.clone(),
        author: sb.author.clone(),
        book_url: sb.book_url.clone(),
        origin: sb.origin.clone(),
        kind: sb.kind.clone(),
        toc_url: sb.toc_url.clone(),
    }
}

/// `book` 绑定。真身是 `ruleData as? BaseBook`(AnalyzeRule.kt L68)——
/// 详情/目录/正文三步传的是 Book 实体;搜索/发现两步传的是 SearchBook,
/// 见 [`search_book_binding`]。
fn book_binding(book: &Book) -> rubato_core::host::BookBinding {
    rubato_core::host::BookBinding {
        name: book.name.clone(),
        author: book.author.clone(),
        book_url: book.book_url.clone(),
        origin: book.origin.clone(),
        kind: book.kind.clone(),
        toc_url: book.toc_url.clone(),
    }
}

/// `chapter` 绑定(`AnalyzeRule.setChapter` 之后那份实体)。真身绑的是实体本身,
/// 规则跑到哪一步它就已经被写到哪一步 —— 故每改一次要重新调一次。
fn chapter_binding(ch: &BookChapter) -> rubato_core::host::ChapterBinding {
    rubato_core::host::ChapterBinding {
        url: ch.url.clone(),
        title: ch.title.clone(),
        base_url: ch.base_url.clone(),
        book_url: ch.book_url.clone(),
        index: ch.index,
        is_volume: ch.is_volume,
        is_vip: ch.is_vip,
        is_pay: ch.is_pay,
        tag: ch.tag.clone(),
    }
}

fn source_book_type(source: &BookSource) -> i32 {
    match source.book_source_type {
        3 => TYPE_TEXT | TYPE_WEB_FILE, // BookSourceType.file
        2 => TYPE_IMAGE,
        1 => TYPE_AUDIO,
        4 => TYPE_VIDEO, // BookSourceType.video
        _ => TYPE_TEXT,
    }
}

fn book_is_type(book: &Book, t: i32) -> bool {
    book.type_ & t > 0
}

fn reset_book_type(book: &mut Book, source: &BookSource) {
    if !book.config_fixed_type {
        book.type_ &= !ALL_BOOK_TYPE;
        book.type_ |= source_book_type(source);
    }
}

/// JsValue 的 Kotlin `toString()`(webBook 的 formatJs 结果转标题)
fn js_to_string(v: &JsValue) -> String {
    match v {
        JsValue::Null => "null".into(),
        JsValue::Str(s) => s.clone(),
        JsValue::Num(d) => rubato_core::gson::java_double(*d),
        JsValue::Bool(b) => b.to_string(),
        JsValue::List(l) => format!("[{}]", l.join(", ")),
        JsValue::Json(j) => rubato_core::gson::object_to_string(j),
        // jsoup 的 toString(一组以换行相连),同 net::js_to_string
        JsValue::Elements { strings, .. } => strings.join("\n"),
        // NativeArray 的 Kotlin `toString()` 是身份哈希 —— 归一成标记
        JsValue::Native(_) => "«identity»".into(),
    }
}

/// 实体变量层 ↔ RuleData 的桥
fn entity_rule_data(vars: &rubato_core::entities::EntityVars, name: &str) -> RuleData {
    RuleData {
        book: Some(VarLayer { vars: vars.map.clone(), ..Default::default() }),
        book_name: Some(name.to_string()),
        ..Default::default()
    }
}

fn write_back_entity_vars(vars: &mut rubato_core::entities::EntityVars, layer: Option<&VarLayer>) {
    if let Some(l) = layer {
        if l.vars != vars.map {
            vars.map = l.vars.clone();
            vars.present = true;
            vars.raw_unparsed = None;
        }
    }
}

/// 带章节层的变量装配:实体层([`entity_rule_data`])之上再垫 chapter 的
/// vars 与标题 —— 正文那一支的每次 fetch / 求值都要这三行,收成一处。
fn chapter_rule_data(book: &Book, chapter: &BookChapter) -> RuleData {
    let mut data = entity_rule_data(&book.vars, &book.name);
    data.chapter = Some(VarLayer { vars: chapter.vars.map.clone(), ..Default::default() });
    data.chapter_title = Some(chapter.title.clone());
    data
}

/// [`fetch`] 的常见形态:没有搜索词、没有页码、没有 webJs/sourceRegex ——
/// 详情/目录/正文的翻页抓取全走这一壳,免得十个位置参数抄一排 `None`。
fn fetch_page(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    m_url: &str,
    base_url: &str,
    data: RuleData,
    book: Option<&Book>,
) -> R<(AnalyzeUrl, StrResponse)> {
    fetch(env, source, m_url, None, None, base_url, data, book, None, None)
}

/// RuleData(plain 层)的 `getVariable()`:空 map → None
fn rule_data_variable(data: &RuleData) -> Option<rubato_core::entities::EntityVars> {
    let layer = data.rule_data.as_ref()?;
    if layer.vars.is_empty() {
        return None;
    }
    Some(rubato_core::entities::EntityVars {
        map: layer.vars.clone(),
        present: true,
        raw_unparsed: None,
    })
}

/// AnalyzeUrl 构造 + getStrResponse(loginCheckJs 检查内联)
#[allow(clippy::too_many_arguments)]
fn fetch(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    m_url: &str,
    key: Option<&str>,
    page: Option<i32>,
    base_url: &str,
    data: RuleData,
    // `AnalyzeUrl(ruleData = …)`:真身把它 `as? Book` 之后当 `book` 绑定
    // (AnalyzeUrl.kt L387)。搜索/发现两步传 None —— 那里的 ruleData 是裸 RuleData。
    book: Option<&Book>,
    js_str: Option<&str>,
    source_regex: Option<&str>,
) -> R<(AnalyzeUrl, StrResponse)> {
    // BaseSource.getHeaderMap():LegadoTeam 基准起 source.header 进入契约
    // (header 规则 JSON 解析 + 默认 UA 注入),不再被忽略
    let source_header = {
        // header 里的 java.put/get 打的是 source 级 CacheManager,不是本次的 ruleData
        // (绑定面由 source_header_map 按 BaseSource 宿主自己搭)
        let mut header_vars = RuleData::default();
        let mut host = (env.host_factory)(&header_vars);
        net::source_header::source_header_map(
            source.header.as_deref(),
            net::client::user_agent(),
            None,
            host.as_mut(),
            &source_binding(source),
            &mut header_vars,
        )
    };
    let args = UrlArgs {
        m_url,
        base_url,
        key,
        page,
        source_header: Some(source_header),
        source_key: Some(&source.book_source_url),
        enabled_cookie_jar: source.enabled_cookie_jar == Some(true),
        // **两个 JS 绑定**:此前一直是 None —— 于是书源 `@js:` 里但凡读
        // `source.*` / `book.*` 就是 TypeError(产品路径同样,差分照不到:
        // A 层那套两侧都是确定性桩,压根不碰绑定)。
        book: book.map(book_binding),
        source: Some(source_binding(source)),
        // **元素面**:`org.jsoup.Jsoup.parse(...)` 是 Rhino 的 LiveConnect 全局面,
        // 与 `java` 是谁无关 —— searchUrl 的 `@js:` 里 parse 回来的 HTML 很常见。
        // net 建不出 AnalyzeRule(依赖方向不允许),由这里注入(见 net::SplitEnv)。
        // 此前一直缺席:产品里这些书源直接 `«no-rule-host»`。
        rule_host: Some(Box::new(AnalyzeRule::new(
            (env.host_factory)(&RuleData::default()),
            RuleData::default(),
        ))),
        ..Default::default()
    };
    let mut au = AnalyzeUrl::new(args, (env.host_factory)(&data), data)
        .map_err(|e| PipelineError::Other(format!("url:{e}")))?;
    // webView 那条路要的平台原语:每次一个新的(真身是从池子里 acquire 一个
    // WebView,用完 release)
    let mut wv = env.web_view.map(|f| f());
    let res = au
        .get_str_response(env.transport, env.cookies, js_str, source_regex, true, wv.as_deref_mut())
        .map_err(PipelineError::Fetch)?;
    // loginCheckJs:真身 `analyzeUrl.evalJS(checkJs, res) as StrResponse`
    // —— JS 真的跑,返回值强转 StrResponse,转不成就是 ClassCastException。
    // (Phase 1 这里是一句 `other("login_check_js")`:那时裁判侧的桩 JS 返回值
    //  必然转型失败,两侧同归 pipeline_error;裁判换成真 Rhino 之后它成了单边差异。)
    //
    // **还没做的一支**:`getStrResponse` 自己失败时真身还会拿
    // `getErrStrResponse(throwable)` 再跑一遍 checkJs,返回值 code() != 500
    // 就当成功往下走(WebBook.kt L84-98)。被测侧那条路仍是直接把 Fetch 错误抛出去。
    if let Some(check_js) = source.login_check_js.as_deref().filter(|s| !s.trim().is_empty()) {
        let ok = au
            .eval_login_check_js(check_js, &res)
            .map_err(|e| PipelineError::Other(format!("login_check_js:{e}")))?;
        if !ok {
            return other("login_check_js:ClassCastException");
        }
    }
    Ok((au, res))
}

// ---------------- BookList(搜索/发现列表)----------------

/// `WebBook.searchBookAwait` 的**前半截**:把搜索地址抓回来,还没解析。
///
/// 单拆出来是给诊断口用的([`crate::web_book::search_book`] 与
/// `Engine::search_source_page` 共用它)——**分成两份实现就会出现
/// 「诊断说抓到了、搜索却没有」那种最糟的报告**。
pub fn search_fetch(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    key: &str,
    page: Option<i32>,
) -> R<(AnalyzeUrl, StrResponse)> {
    let search_url = match &source.search_url {
        Some(u) if !u.chars().all(char::is_whitespace) => u.clone(),
        _ => return other("搜索url不能为空"),
    };
    let data = RuleData { rule_data: Some(VarLayer::default()), ..Default::default() };
    fetch(
        env,
        source,
        &search_url,
        Some(key),
        page,
        &source.book_source_url,
        data,
        None,
        None,
        None,
    )
}

/// `WebBook.searchBookAwait`
pub fn search_book(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    key: &str,
    page: Option<i32>,
) -> R<Vec<SearchBook>> {
    let (au, res) = search_fetch(env, source, key, page)?;
    let body = res.body.clone();
    analyze_book_list(env, source, au, &res.url, body.as_deref(), true, res.prior_is_redirect)
}

/// `WebBook.exploreBookAwait`
pub fn explore_fetch(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    url: &str,
    page: Option<i32>,
) -> R<(AnalyzeUrl, StrResponse)> {
    let data = RuleData { rule_data: Some(VarLayer::default()), ..Default::default() };
    fetch(env, source, url, None, page, &source.book_source_url, data, None, None, None)
}

/// `WebBook.exploreBookAwait`
pub fn explore_book(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    url: &str,
    page: Option<i32>,
) -> R<Vec<SearchBook>> {
    let (au, res) = explore_fetch(env, source, url, page)?;
    let body = res.body.clone();
    analyze_book_list(env, source, au, &res.url, body.as_deref(), false, res.prior_is_redirect)
}

/// `BookList.analyzeBookList`
#[allow(clippy::too_many_arguments)]
fn analyze_book_list(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    au: AnalyzeUrl,
    base_url: &str,
    body: Option<&str>,
    is_search: bool,
    is_redirect: bool,
) -> R<Vec<SearchBook>> {
    let Some(body) = body else {
        return other(format!("error_get_web_content:{}", au.rule_url));
    };
    let mut book_list: Vec<SearchBook> = Vec::new();

    // AnalyzeRule(ruleData, bookSource):共享 AnalyzeUrl 用过的变量层
    let base_data = au.data.clone();
    let mut ar = AnalyzeRule::new((env.host_factory)(&base_data), base_data);
    // `AnalyzeRule(ruleData, bookSource)`:这一步的 ruleData 是裸 `RuleData`,
    // 故 `book` 绑定为 null(真身 `ruleData as? BaseBook` 同样给 null)
    ar.source_binding = Some(source_binding(source));
    ar.set_content(RuleValue::Str(body.to_string()), Some(base_url));
    ar.set_redirect_url(base_url);

    // 搜索 + bookUrlPattern 命中 → 详情页解析
    if is_search {
        if let Some(pattern) = &source.book_url_pattern {
            let matched = JavaRegex::compile(&format!("^(?:{pattern})$"))
                .map_err(|e| PipelineError::Other(format!("pattern:{e:?}")))?
                .find_first(base_url)
                .map_err(|e| PipelineError::Other(format!("pattern:{e:?}")))?
                .map(|m| m.len() == base_url.len())
                .unwrap_or(false);
            if matched {
                if let Some(mut sb) =
                    get_info_item(env, source, &mut ar, &au, body, base_url, is_redirect)?
                {
                    sb.info_html = Some(body.to_string());
                    book_list.push(sb);
                }
                return Ok(book_list);
            }
        }
    }

    // 搜索,或发现规则的 bookList 为空白 → 都落到搜索规则(BookList.kt 的 when 同型)
    let use_search_rule = is_search
        || source
            .explore_rule()
            .book_list
            .as_deref()
            .is_none_or(|s| s.chars().all(char::is_whitespace));
    let book_list_rule: BookListRuleView = if use_search_rule {
        (&source.search_rule()).into()
    } else {
        (&source.explore_rule()).into()
    };

    let mut rule_list: String = book_list_rule.book_list.clone().unwrap_or_default();
    let mut reverse = false;
    if rule_list.starts_with('-') {
        reverse = true;
        rule_list = rule_list[1..].to_string();
    }
    if rule_list.starts_with('+') {
        rule_list = rule_list[1..].to_string();
    }

    let collections =
        ar.get_elements(&rule_list).map_err(|e| PipelineError::Other(format!("rule:{e:?}")))?;

    if collections.is_empty() && source.book_url_pattern.as_deref().is_none_or(str::is_empty) {
        if let Some(mut sb) = get_info_item(env, source, &mut ar, &au, body, base_url, is_redirect)?
        {
            sb.info_html = Some(body.to_string());
            book_list.push(sb);
        }
    } else {
        // 裁判在循环里逐次取 `ruleData.getVariable()`,而 ruleData 是**被 getElements
        // 里的 @put 写过**的那一份(AnalyzeRule.setRuleData 只换 AnalyzeRule 的引用,
        // 不动这个局部变量)。所以快照要在 getElements 之后、逐项替换 data 之前取。
        let variable = rule_data_variable(&ar.data);
        for item in collections {
            if let Some(sb) =
                get_search_item(source, &mut ar, item, base_url, variable.clone(), &book_list_rule)?
            {
                let mut sb = sb;
                if base_url == sb.book_url {
                    sb.info_html = Some(body.to_string());
                }
                book_list.push(sb);
            }
        }
        // LinkedHashSet 去重(SearchBook equals = bookUrl,保首个)
        let mut seen = std::collections::HashSet::new();
        book_list.retain(|b| seen.insert(b.book_url.clone()));
        if reverse {
            book_list.reverse();
        }
    }
    Ok(book_list)
}

/// `BookList.getInfoItem`
fn get_info_item(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    ar: &mut AnalyzeRule,
    au: &AnalyzeUrl,
    body: &str,
    base_url: &str,
    is_redirect: bool,
) -> R<Option<SearchBook>> {
    let mut book =
        Book { vars: rule_data_variable(&au.data).unwrap_or_default(), ..Default::default() };
    book.book_url = if is_redirect {
        base_url.to_string()
    } else {
        get_absolute_url_str(Some(&au.url), &au.rule_url)
    };
    book.origin = source.book_source_url.clone();
    book.origin_name = source.book_source_name.clone();
    book.origin_order = source.custom_order;
    book.type_ = source_book_type(source);
    // setRuleData(book):**`book` 绑定也跟着换** —— `book get() = ruleData as? BaseBook`
    ar.data = entity_rule_data(&book.vars, &book.name);
    ar.book_binding = Some(book_binding(&book));
    let r = analyze_book_info_with(env, source, &mut book, body, ar, base_url, base_url, false);
    // 真身的 ruleData **就是** book(`setRuleData(book)`),`@put:` 直接写进
    // `book.variableMap`,`toSearchBook()` 再把 variable 带走;这边 ruleData 是
    // 一份拷贝,不写回就丢 —— 与 `analyze_book_info_entry` 里那一行同源。
    write_back_entity_vars(&mut book.vars, ar.data.book.as_ref());
    r?;
    if !book.name.chars().all(char::is_whitespace) && !book.name.is_empty() {
        return Ok(Some(book.to_search_book()));
    }
    Ok(None)
}

/// `BookList.getSearchItem`
fn get_search_item(
    source: &BookSource,
    ar: &mut AnalyzeRule,
    item: RuleValue,
    base_url: &str,
    variable: Option<rubato_core::entities::EntityVars>,
    rule: &BookListRuleView,
) -> R<Option<SearchBook>> {
    let mut sb = SearchBook {
        vars: variable.unwrap_or_default(),
        type_: source_book_type(source),
        origin: source.book_source_url.clone(),
        origin_name: source.book_source_name.clone(),
        origin_order: source.custom_order,
        ..Default::default()
    };
    ar.data = entity_rule_data(&sb.vars, &sb.name);
    ar.book_binding = Some(search_book_binding(&sb));
    ar.set_content(item, None);

    let get =
        |ar: &mut AnalyzeRule, r: &Option<String>| -> Result<String, rule_engine::RuleError> {
            ar.get_string(r.as_deref().unwrap_or(""), None, false)
        };
    let get_url =
        |ar: &mut AnalyzeRule, r: &Option<String>| -> Result<String, rule_engine::RuleError> {
            ar.get_string(r.as_deref().unwrap_or(""), None, true)
        };

    sb.name =
        format_book_name(&get(ar, &rule.name).map_err(|e| PipelineError::Other(format!("{e:?}")))?);
    ar.data.book_name = Some(sb.name.clone());
    ar.book_binding = Some(search_book_binding(&sb));
    if !sb.name.is_empty() {
        sb.author = format_book_author(
            &get(ar, &rule.author).map_err(|e| PipelineError::Other(format!("{e:?}")))?,
        );
        ar.book_binding = Some(search_book_binding(&sb));
        // kind(错误吞掉)
        if let Ok(kind) = ar.get_string_list(rule.kind.as_deref().unwrap_or(""), None, false) {
            // BookList.kt L230:`joinToString(",")`,**不截断**
            sb.kind = kind.map(|l| l.join(","));
        }
        ar.book_binding = Some(search_book_binding(&sb));
        // wordCount
        if let Ok(wc) = get(ar, &rule.word_count) {
            if let Ok(w) = word_count_format(Some(&wc)) {
                sb.word_count = Some(w);
            }
        }
        // lastChapter
        if let Ok(lc) = get(ar, &rule.last_chapter) {
            sb.latest_chapter_title = Some(lc);
        }
        // intro:HtmlFormatter.formatIntro(段首不缩进),裁判不截断
        if let Ok(intro) = get(ar, &rule.intro) {
            sb.intro = Some(html_formatter::format_intro(Some(&intro)));
        }
        // coverUrl
        if let Ok(cover) = get(ar, &rule.cover_url) {
            if !cover.is_empty() {
                sb.cover_url = Some(get_absolute_url_str(Some(base_url), &cover));
            }
        }
        sb.book_url =
            get_url(ar, &rule.book_url).map_err(|e| PipelineError::Other(format!("{e:?}")))?;
        if sb.book_url.is_empty() {
            sb.book_url = base_url.to_string();
        }
        write_back_entity_vars(&mut sb.vars, ar.data.book.as_ref());
        return Ok(Some(sb));
    }
    write_back_entity_vars(&mut sb.vars, ar.data.book.as_ref());
    Ok(None)
}

// ---------------- BookInfo(详情)----------------

/// `WebBook.getBookInfoAwait`
pub fn get_book_info(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    book: &mut Book,
    can_re_name: bool,
) -> R<()> {
    reset_book_type(book, source);
    if book.info_html.as_deref().is_some_and(|s| !s.is_empty()) {
        let body = book.info_html.clone();
        let base = book.book_url.clone();
        let data = entity_rule_data(&book.vars, &book.name);
        let mut ar = AnalyzeRule::new((env.host_factory)(&data), data);
        ar.book_binding = Some(book_binding(book));
        ar.source_binding = Some(source_binding(source));
        analyze_book_info_entry(
            env,
            source,
            book,
            body.as_deref(),
            &mut ar,
            &base,
            &base,
            can_re_name,
        )
    } else {
        let data = entity_rule_data(&book.vars, &book.name);
        let m_url = book.book_url.clone();
        let (au, res) = fetch_page(env, source, &m_url, &source.book_source_url, data, Some(book))?;
        write_back_entity_vars(&mut book.vars, au.data.book.as_ref());
        let mut ar = AnalyzeRule::new((env.host_factory)(&au.data), au.data);
        ar.book_binding = Some(book_binding(book));
        ar.source_binding = Some(source_binding(source));
        let base = book.book_url.clone();
        let redirect = res.url.clone();
        analyze_book_info_entry(
            env,
            source,
            book,
            res.body.as_deref(),
            &mut ar,
            &base,
            &redirect,
            can_re_name,
        )
    }
}

/// `BookInfo.analyzeBookInfo`(带 body 判空的入口重载)
#[allow(clippy::too_many_arguments)]
fn analyze_book_info_entry(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    book: &mut Book,
    body: Option<&str>,
    ar: &mut AnalyzeRule,
    base_url: &str,
    redirect_url: &str,
    can_re_name: bool,
) -> R<()> {
    let Some(body) = body else {
        return other(format!("error_get_web_content:{base_url}"));
    };
    ar.set_content(RuleValue::Str(body.to_string()), Some(base_url));
    ar.set_redirect_url(redirect_url);
    let r =
        analyze_book_info_with(env, source, book, body, ar, base_url, redirect_url, can_re_name);
    write_back_entity_vars(&mut book.vars, ar.data.book.as_ref());
    r
}

/// `BookInfo.analyzeBookInfo`(核心重载)
#[allow(clippy::too_many_arguments)]
fn analyze_book_info_with(
    _env: &mut PipelineEnv<'_>,
    source: &BookSource,
    book: &mut Book,
    body: &str,
    ar: &mut AnalyzeRule,
    base_url: &str,
    redirect_url: &str,
    can_re_name: bool,
) -> R<()> {
    let info_rule = source.book_info_rule();
    if let Some(init) = &info_rule.init {
        if !init.chars().all(char::is_whitespace) {
            let el = ar.get_element(init).map_err(|e| PipelineError::Other(format!("{e:?}")))?;
            ar.set_content_optional(el, None);
        }
    }
    let m_can_re_name = can_re_name
        && !info_rule.can_re_name.as_deref().is_none_or(|s| s.chars().all(char::is_whitespace));

    let name = format_book_name(
        &ar.get_string(info_rule.name.as_deref().unwrap_or(""), None, false)
            .map_err(|e| PipelineError::Other(format!("{e:?}")))?,
    );
    if !name.is_empty() && (m_can_re_name || book.name.is_empty()) {
        book.name = name;
        ar.data.book_name = Some(book.name.clone());
    }
    // **绑定是活的**:真身绑的是实体本身,填到哪一步下面的规则就读得到哪一步
    // (`tocUrl: {{book.kind}}` 读的是刚填好的 kind —— 快照版本给的是空串)。
    // 同 chapter 绑定,见 rubato_core::host::ChapterBinding。
    ar.book_binding = Some(book_binding(book));
    let author = format_book_author(
        &ar.get_string(info_rule.author.as_deref().unwrap_or(""), None, false)
            .map_err(|e| PipelineError::Other(format!("{e:?}")))?,
    );
    if !author.is_empty() && (m_can_re_name || book.author.is_empty()) {
        book.author = author;
    }
    ar.book_binding = Some(book_binding(book));
    // kind(吞错)
    if let Ok(Some(l)) = ar.get_string_list(info_rule.kind.as_deref().unwrap_or(""), None, false) {
        // BookInfo.kt L85-88:`joinToString(",")`,**不截断**
        let joined: String = l.join(",");
        if !joined.is_empty() {
            book.kind = Some(joined);
        }
    }
    ar.book_binding = Some(book_binding(book));
    // wordCount(吞错)
    if let Ok(wc) = ar.get_string(info_rule.word_count.as_deref().unwrap_or(""), None, false) {
        if let Ok(w) = word_count_format(Some(&wc)) {
            if !w.is_empty() {
                book.word_count = Some(w);
            }
        }
    }
    ar.book_binding = Some(book_binding(book));
    // lastChapter(吞错)
    if let Ok(lc) = ar.get_string(info_rule.last_chapter.as_deref().unwrap_or(""), None, false) {
        if !lc.is_empty() {
            book.latest_chapter_title = Some(lc);
        }
    }
    ar.book_binding = Some(book_binding(book));
    // intro(吞错):`<usehtml>` / `<md>` / `<useweb>` 前缀直通(不过 formatIntro),
    // 其余走 HtmlFormatter.formatIntro(段首不缩进)
    if let Ok(intro) = ar.get_string(info_rule.intro.as_deref().unwrap_or(""), None, false) {
        let trim_start = intro.trim_start_matches(|c: char| c.is_whitespace());
        if trim_start.starts_with("<usehtml>")
            || trim_start.starts_with("<md>")
            || trim_start.starts_with("<useweb>")
        {
            book.intro = Some(trim_start.to_string());
        } else {
            let f = html_formatter::format_intro(Some(&intro));
            if !f.is_empty() {
                book.intro = Some(f);
            }
        }
    }
    ar.book_binding = Some(book_binding(book));
    // coverUrl(吞错)
    if let Ok(cover) = ar.get_string(info_rule.cover_url.as_deref().unwrap_or(""), None, false) {
        if !cover.is_empty() {
            book.cover_url = Some(get_absolute_url_str(Some(redirect_url), &cover));
        }
    }
    ar.book_binding = Some(book_binding(book));
    if !book_is_type(book, TYPE_WEB_FILE) {
        book.toc_url = ar
            .get_string(info_rule.toc_url.as_deref().unwrap_or(""), None, true)
            .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
        if book.toc_url.is_empty() {
            book.toc_url = base_url.to_string();
        }
        if book.toc_url == base_url {
            book.toc_html = Some(body.to_string());
        }
    } else {
        let urls = ar
            .get_string_list(info_rule.download_urls.as_deref().unwrap_or(""), None, true)
            .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
        match urls {
            Some(u) if !u.is_empty() => book.download_urls = Some(u),
            _ => return other("下载链接为空"),
        }
    }
    Ok(())
}

// ---------------- BookChapterList(目录)----------------

/// `WebBook.getChapterListAwait`(runPerJs=false)
pub fn get_chapter_list(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    book: &mut Book,
) -> R<Vec<BookChapter>> {
    reset_book_type(book, source);
    if book.book_url == book.toc_url && book.toc_html.as_deref().is_some_and(|s| !s.is_empty()) {
        let body = book.toc_html.clone();
        let base = book.toc_url.clone();
        analyze_chapter_list_entry(env, source, book, &base, &base.clone(), body.as_deref())
    } else {
        let data = entity_rule_data(&book.vars, &book.name);
        let m_url = book.toc_url.clone();
        let base = book.book_url.clone();
        let (au, res) = fetch_page(env, source, &m_url, &base, data, Some(book))?;
        write_back_entity_vars(&mut book.vars, au.data.book.as_ref());
        let toc_url = book.toc_url.clone();
        let redirect = res.url.clone();
        analyze_chapter_list_entry(env, source, book, &toc_url, &redirect, res.body.as_deref())
    }
}

fn analyze_chapter_list_entry(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    book: &mut Book,
    base_url: &str,
    redirect_url: &str,
    body: Option<&str>,
) -> R<Vec<BookChapter>> {
    let Some(body) = body else {
        return other(format!("error_get_web_content:{base_url}"));
    };
    let toc_rule = source.toc_rule();
    let mut next_url_list = vec![redirect_url.to_string()];
    let mut reverse = false;
    let mut list_rule = toc_rule.chapter_list.clone().unwrap_or_default();
    if list_rule.starts_with('-') {
        reverse = true;
        list_rule = list_rule[1..].to_string();
    }
    if list_rule.starts_with('+') {
        list_rule = list_rule[1..].to_string();
    }

    let mut chapter_list: Vec<BookChapter> = Vec::new();
    let (first, next_urls) = analyze_chapter_page(
        env,
        source,
        book,
        base_url,
        redirect_url,
        body,
        &toc_rule,
        &list_rule,
        true,
    )?;
    chapter_list.extend(first);
    match next_urls.len() {
        0 => {}
        1 => {
            let mut next_url = next_urls[0].clone();
            while !next_url.is_empty() && !next_url_list.contains(&next_url) {
                next_url_list.push(next_url.clone());
                let data = entity_rule_data(&book.vars, &book.name);
                let (au, res) =
                    fetch_page(env, source, &next_url, &source.book_source_url, data, Some(book))?;
                write_back_entity_vars(&mut book.vars, au.data.book.as_ref());
                if let Some(next_body) = res.body.as_deref() {
                    let (chapters, urls) = analyze_chapter_page(
                        env,
                        source,
                        book,
                        &next_url.clone(),
                        &next_url,
                        next_body,
                        &toc_rule,
                        &list_rule,
                        false,
                    )?;
                    next_url = urls.first().cloned().unwrap_or_default();
                    chapter_list.extend(chapters);
                }
            }
        }
        _ => {
            // 并发页(差分 threadCount=1 → 顺序执行,mapAsync 保序)
            for url_str in &next_urls {
                let data = entity_rule_data(&book.vars, &book.name);
                let (au, res) =
                    fetch_page(env, source, url_str, &source.book_source_url, data, Some(book))?;
                write_back_entity_vars(&mut book.vars, au.data.book.as_ref());
                let body =
                    res.body.as_deref().ok_or_else(|| PipelineError::Other("body_null".into()))?;
                let (chapters, _) = analyze_chapter_page(
                    env, source, book, url_str, &res.url, body, &toc_rule, &list_rule, false,
                )?;
                chapter_list.extend(chapters);
            }
        }
    }
    if chapter_list.is_empty() {
        return Err(PipelineError::TocEmpty);
    }
    if !reverse {
        chapter_list.reverse();
    }
    // LinkedHashSet 去重(BookChapter equals = url)
    let mut seen = std::collections::HashSet::new();
    chapter_list.retain(|c| seen.insert(c.url.clone()));
    // getReverseToc() 恒 false(config 默认)→ 再反转
    chapter_list.reverse();
    for (i, ch) in chapter_list.iter_mut().enumerate() {
        ch.index = i as i32;
    }
    // formatJs
    if let Some(format_js) =
        toc_rule.format_js.as_deref().filter(|s| !s.chars().all(char::is_whitespace))
    {
        let mut dummy = RuleData::default();
        let mut host = (env.host_factory)(&dummy);
        for (i, ch) in chapter_list.iter_mut().enumerate() {
            let bindings = JsBindings {
                title: Some(ch.title.clone()),
                page: Some(i as i32 + 1), // index 绑定(1 起)
                ..Default::default()
            };
            if let Ok(v) = host.eval_js(format_js, &bindings, &mut dummy) {
                if v != JsValue::Null {
                    ch.title = js_to_string(&v);
                }
            }
        }
    }
    // durChapterTitle / totalChapterNum / latestChapterTitle
    let replaced_title = |ch: &BookChapter| chapter_display_title(ch);
    let dur = chapter_list
        .get(book.dur_chapter_index as usize)
        .unwrap_or_else(|| chapter_list.last().expect("非空"));
    book.dur_chapter_title = Some(replaced_title(dur));
    if book.total_chapter_num < chapter_list.len() as i32 {
        book.last_check_count = chapter_list.len() as i32 - book.total_chapter_num;
    }
    book.total_chapter_num = chapter_list.len() as i32;
    // simulatedTotalChapterNum = totalChapterNum(vendor 精简)
    let latest = chapter_list
        .get((book.total_chapter_num - 1).max(0) as usize)
        .unwrap_or_else(|| chapter_list.last().expect("非空"));
    book.latest_chapter_title = Some(replaced_title(latest));
    Ok(chapter_list)
}

/// `BookChapterList.analyzeChapterList`(私有重载:单页解析)
#[allow(clippy::too_many_arguments)]
fn analyze_chapter_page(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    book: &mut Book,
    base_url: &str,
    redirect_url: &str,
    body: &str,
    toc_rule: &rubato_core::entities::TocRule,
    list_rule: &str,
    get_next_url: bool,
) -> R<(Vec<BookChapter>, Vec<String>)> {
    let data = entity_rule_data(&book.vars, &book.name);
    let mut ar = AnalyzeRule::new((env.host_factory)(&data), data);
    ar.book_binding = Some(book_binding(book));
    ar.source_binding = Some(source_binding(source));
    ar.set_content(RuleValue::Str(body.to_string()), Some(base_url));
    ar.set_redirect_url(redirect_url);

    let mut chapter_list = Vec::new();
    let elements =
        ar.get_elements(list_rule).map_err(|e| PipelineError::Other(format!("{e:?}")))?;
    let mut next_url_list = Vec::new();
    if get_next_url {
        if let Some(next_rule) = toc_rule.next_toc_url.as_deref().filter(|s| !s.is_empty()) {
            // **异常要穿出去**:真身这里没有 try/catch(BookChapterList.kt L214),
            // 目录下一页的 JS 抛了就一路穿到四步之外。此前这里是
            // `if let Ok(Some(items)) = …`,把 Err 咽了 —— 于是「裁判 pipeline_error /
            // 被测成功」与「裁判 pipeline_error / 被测 toc_empty」两种单边差异。
            let items = ar
                .get_string_list(next_rule, None, true)
                .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
            for item in items.into_iter().flatten() {
                if item != redirect_url {
                    next_url_list.push(item);
                }
            }
        }
    }
    if !elements.is_empty() {
        let g =
            |ar: &mut AnalyzeRule, r: &Option<String>| -> Result<String, rule_engine::RuleError> {
                ar.get_string(r.as_deref().unwrap_or(""), None, false)
            };
        for (index, item) in elements.into_iter().enumerate() {
            ar.set_content(item, None);
            let mut ch = BookChapter {
                book_url: book.book_url.clone(),
                base_url: redirect_url.to_string(),
                ..Default::default()
            };
            // setChapter(chapter):chapter 层进变量链 + `chapter` JS 绑定。
            // 绑定是**活的** —— 真身绑的是实体本身,所以 chapterUrl 的 JS 读得到
            // 刚算出来的 chapter.title;这边每改一次就重新绑一次。
            ar.data.chapter = Some(VarLayer { vars: ch.vars.map.clone(), ..Default::default() });
            ar.data.chapter_title = Some(ch.title.clone());
            ar.chapter_binding = Some(chapter_binding(&ch));
            ch.title = g(&mut ar, &toc_rule.chapter_name)
                .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
            ar.data.chapter_title = Some(ch.title.clone());
            ar.chapter_binding = Some(chapter_binding(&ch));
            ch.url = g(&mut ar, &toc_rule.chapter_url)
                .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
            let info = g(&mut ar, &toc_rule.update_time)
                .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
            let is_volume = g(&mut ar, &toc_rule.is_volume)
                .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
            ch.is_volume = is_true(Some(&is_volume));
            if ch.is_volume {
                ch.tag = Some(info);
            } else if TOC_COUNT_WORDS {
                // AppConfig.tocCountWords:从章节信息里抽出「x万字」并从 tag 里剔掉
                let first = WORD_COUNT_REGEX
                    .find_all_groups(&info, 1)
                    .ok()
                    .and_then(|v| v.into_iter().next());
                match first.and_then(|g| {
                    let whole = g.first().cloned().flatten()?;
                    let g1 = g.get(1).cloned().flatten()?;
                    Some((whole, g1))
                }) {
                    Some((whole, g1)) => {
                        ch.word_count = Some(kotlin_trim(&g1).to_string());
                        // Kotlin String.replaceFirst(String, String):字面量替换首个
                        ch.tag = Some(info.replacen(&whole, "", 1));
                    }
                    None => ch.tag = Some(info),
                }
            } else {
                ch.tag = Some(info);
            }
            if ch.url.is_empty() {
                if ch.is_volume {
                    ch.url = format!("{}{}", ch.title, index);
                } else {
                    ch.url = base_url.to_string();
                }
            }
            if !ch.title.is_empty() {
                let is_vip = g(&mut ar, &toc_rule.is_vip)
                    .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
                let is_pay = g(&mut ar, &toc_rule.is_pay)
                    .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
                if is_true(Some(&is_vip)) {
                    ch.is_vip = true;
                }
                if is_true(Some(&is_pay)) {
                    ch.is_pay = true;
                }
                write_back_entity_vars(&mut ch.vars, ar.data.chapter.as_ref());
                chapter_list.push(ch);
            }
            ar.data.chapter = None;
            ar.data.chapter_title = None;
        }
    }
    let _ = env;
    write_back_entity_vars(&mut book.vars, ar.data.book.as_ref());
    Ok((chapter_list, next_url_list))
}

// ---------------- BookContent(正文)----------------

/// `WebBook.getContentAwait`(needSave=false)
pub fn get_content(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    book: &mut Book,
    chapter: &mut BookChapter,
    next_chapter_url: Option<&str>,
) -> R<String> {
    let content_rule = source.content_rule();
    if content_rule.content.as_deref().is_none_or(str::is_empty) {
        return Ok(chapter.url.clone());
    }
    if chapter.is_volume && chapter.url.starts_with(&chapter.title) {
        return Ok(chapter.tag.clone().unwrap_or_default());
    }
    if chapter.url == book.book_url && book.toc_html.as_deref().is_some_and(|s| !s.is_empty()) {
        let abs = chapter_absolute_url(chapter);
        let body = book.toc_html.clone();
        analyze_content_entry(
            env,
            source,
            book,
            chapter,
            &abs,
            &abs.clone(),
            body.as_deref(),
            next_chapter_url,
        )
    } else {
        let abs = chapter_absolute_url(chapter);
        let data = chapter_rule_data(book, chapter);
        let toc_url = book.toc_url.clone();
        let (au, res) = fetch(
            env,
            source,
            &abs,
            None,
            None,
            &toc_url,
            data,
            Some(book),
            content_rule.web_js.as_deref(),
            content_rule.source_regex.as_deref(),
        )?;
        write_back_entity_vars(&mut book.vars, au.data.book.as_ref());
        write_back_entity_vars(&mut chapter.vars, au.data.chapter.as_ref());
        let redirect = res.url.clone();
        analyze_content_entry(
            env,
            source,
            book,
            chapter,
            &abs,
            &redirect,
            res.body.as_deref(),
            next_chapter_url,
        )
    }
}

/// `BookContent.analyzeContent`(公开重载:分页循环 + 收尾)
#[allow(clippy::too_many_arguments)]
fn analyze_content_entry(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    book: &mut Book,
    chapter: &mut BookChapter,
    base_url: &str,
    redirect_url: &str,
    body: Option<&str>,
    next_chapter_url: Option<&str>,
) -> R<String> {
    let Some(body) = body else {
        return other(format!("error_get_web_content:{base_url}"));
    };
    let m_next_chapter_url = next_chapter_url.filter(|s| !s.is_empty());
    let mut page_count = 0;
    let mut content_builder = String::new();
    let append_content = |cb: &mut String, content: &str, page_count: &mut i32| {
        if *page_count > 0 {
            cb.push('\n');
        }
        cb.push_str(content);
        *page_count += 1;
    };
    let mut next_url_list = vec![redirect_url.to_string()];
    let content_rule = source.content_rule();

    // 外层 analyzeRule(title/replaceRegex/subContent 用)
    let outer_data = chapter_rule_data(book, chapter);
    let mut outer = AnalyzeRule::new((env.host_factory)(&outer_data), outer_data);
    outer.book_binding = Some(book_binding(book));
    outer.source_binding = Some(source_binding(source));
    outer.chapter_binding = Some(chapter_binding(chapter));
    outer.set_content(RuleValue::Str(body.to_string()), Some(base_url));
    outer.set_redirect_url(redirect_url);
    outer.set_next_chapter_url(m_next_chapter_url);

    let (first_content, mut next_urls) = analyze_content_page(
        env,
        source,
        book,
        chapter,
        base_url,
        redirect_url,
        body,
        &content_rule,
        m_next_chapter_url,
        true,
    )?;
    append_content(&mut content_builder, &first_content, &mut page_count);

    if next_urls.len() == 1 {
        let mut next_url = next_urls[0].clone();
        while !next_url.is_empty() && !next_url_list.contains(&next_url) {
            if let Some(n) = m_next_chapter_url {
                if get_absolute_url_str(Some(redirect_url), &next_url)
                    == get_absolute_url_str(Some(redirect_url), n)
                {
                    break;
                }
            }
            next_url_list.push(next_url.clone());
            let data = chapter_rule_data(book, chapter);
            let (au, res) =
                fetch_page(env, source, &next_url, &source.book_source_url, data, Some(book))?;
            write_back_entity_vars(&mut book.vars, au.data.book.as_ref());
            if let Some(next_body) = res.body.as_deref() {
                let (content, urls) = analyze_content_page(
                    env,
                    source,
                    book,
                    chapter,
                    &next_url.clone(),
                    &res.url,
                    next_body,
                    &content_rule,
                    m_next_chapter_url,
                    true,
                )?;
                next_url = urls.first().cloned().unwrap_or_default();
                append_content(&mut content_builder, &content, &mut page_count);
            }
        }
    } else if next_urls.len() > 1 {
        for url_str in &next_urls {
            let data = chapter_rule_data(book, chapter);
            let (au, res) =
                fetch_page(env, source, url_str, &source.book_source_url, data, Some(book))?;
            write_back_entity_vars(&mut book.vars, au.data.book.as_ref());
            let body =
                res.body.as_deref().ok_or_else(|| PipelineError::Other("body_null".into()))?;
            let (content, _) = analyze_content_page(
                env,
                source,
                book,
                chapter,
                url_str,
                &res.url,
                body,
                &content_rule,
                m_next_chapter_url,
                false,
            )?;
            append_content(&mut content_builder, &content, &mut page_count);
        }
    }
    next_urls.clear();

    // subContent
    if let Some(rule) =
        content_rule.sub_content.as_deref().filter(|s| !s.chars().all(char::is_whitespace))
    {
        let raw_sub = outer
            .get_string_unescape(rule, false)
            .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
        let raw_sub = kotlin_trim(&raw_sub);
        let sub_content = if raw_sub.to_lowercase().starts_with("http") {
            let data = chapter_rule_data(book, chapter);
            let (_, res) =
                fetch_page(env, source, &raw_sub, &source.book_source_url, data, Some(book))?;
            res.body.unwrap_or_default()
        } else {
            raw_sub.clone()
        };
        if !sub_content.chars().all(char::is_whitespace) && !sub_content.is_empty() {
            let is_online_txt = !book_is_type(book, 0b100000000) && book_is_type(book, TYPE_TEXT);
            if is_online_txt {
                append_content(&mut content_builder, &sub_content, &mut page_count);
            } else if book_is_type(book, TYPE_AUDIO) {
                chapter.vars.put("lyric", &sub_content);
            }
        }
    }

    let mut content_str = content_builder;

    // title 规则(先正文再标题)
    if let Some(title_rule) =
        content_rule.title.as_deref().filter(|s| !s.chars().all(char::is_whitespace))
    {
        if let Ok(title) = outer.get_string(title_rule, None, false) {
            if !title.is_empty() && !title.chars().all(char::is_whitespace) {
                static IMG_REGEX: std::sync::LazyLock<JavaRegex> = std::sync::LazyLock::new(|| {
                    JavaRegex::compile("(.*)((?:data|https?):[\\s\\S]+)$").expect("imgRegex")
                });
                let mut title = title;
                if let Ok(ms) = IMG_REGEX.find_all_with_ranges(&title) {
                    if let Some((_, _, groups)) = ms.first() {
                        let group1 = groups.get(1).and_then(|g| g.clone()).unwrap_or_default();
                        title = if !group1.is_empty() { group1 } else { chapter.title.clone() };
                        // reviewImg 不进 Phase 1 投影
                    }
                }
                chapter.title = title;
            }
        }
    }

    // 全文替换(replaceRegex)
    if let Some(replace_regex) = content_rule.replace_regex.as_deref().filter(|s| !s.is_empty()) {
        let joined: Vec<String> = content_str.split('\n').map(kotlin_trim).collect();
        content_str = joined.join("\n");
        content_str = outer
            .get_string(replace_regex, Some(&RuleValue::Str(content_str.clone())), false)
            .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
        content_str =
            content_str.split('\n').map(|l| format!("　　{l}")).collect::<Vec<_>>().join("\n");
    }
    write_back_entity_vars(&mut chapter.vars, outer.data.chapter.as_ref());
    write_back_entity_vars(&mut book.vars, outer.data.book.as_ref());

    if !chapter.is_volume && content_str.chars().all(char::is_whitespace) {
        return Err(PipelineError::ContentEmpty);
    }
    Ok(content_str)
}

/// 取正文里的一张图片(`BookHelp.saveImage`:
/// `AnalyzeUrl(src, source = bookSource).getByteArrayAwait()`)。
///
/// 为什么不在 Dart 侧 `Image.network`:书源的图片十之八九要带着**这个源自己的**
/// header(Referer / UA)与 cookie 才取得下来,而那两样都在这条链路上
/// (`source_header_map` + `CookieEnv`)。绕过去等于把防盗链站点的图片全丢掉。
///
/// `src` 是块表交出来的**原样**那一串,可能带 `,{"width":"50%"}` 之类的
/// option JSON —— `AnalyzeUrl` 自己会切(`param_split`),这里不预处理。
///
/// **没做**:真身取回字节之后还有一步 `ImageUtils.decode`(书源的 `decodeJs`
/// 解密图片)。产品里用到它的源极少,而那一步要把 JS 宿主接进图片链路;
/// 记在计划书 M2b 的账上,不在这里悄悄留个半截实现。
pub fn get_image(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    book: &Book,
    src: &str,
) -> R<Vec<u8>> {
    let source_header = {
        let mut header_vars = RuleData::default();
        let mut host = (env.host_factory)(&header_vars);
        net::source_header::source_header_map(
            source.header.as_deref(),
            net::client::user_agent(),
            None,
            host.as_mut(),
            &source_binding(source),
            &mut header_vars,
        )
    };
    let data = RuleData::default();
    let args = UrlArgs {
        m_url: src,
        base_url: &book.book_url,
        source_header: Some(source_header),
        source_key: Some(&source.book_source_url),
        enabled_cookie_jar: source.enabled_cookie_jar == Some(true),
        book: Some(book_binding(book)),
        source: Some(source_binding(source)),
        ..Default::default()
    };
    let mut au = AnalyzeUrl::new(args, (env.host_factory)(&data), data)
        .map_err(|e| PipelineError::Other(format!("url:{e}")))?;
    let (bytes, _hops) =
        au.get_byte_array(env.transport, env.cookies).map_err(PipelineError::Fetch)?;
    Ok(bytes)
}

/// `BookContent.analyzeContent`(私有重载:单页)
#[allow(clippy::too_many_arguments)]
fn analyze_content_page(
    env: &mut PipelineEnv<'_>,
    source: &BookSource,
    book: &mut Book,
    chapter: &mut BookChapter,
    base_url: &str,
    redirect_url: &str,
    body: &str,
    content_rule: &rubato_core::entities::ContentRule,
    next_chapter_url: Option<&str>,
    get_next_page_url: bool,
) -> R<(String, Vec<String>)> {
    let ar_data = chapter_rule_data(book, chapter);
    let mut ar = AnalyzeRule::new((env.host_factory)(&ar_data), ar_data);
    ar.book_binding = Some(book_binding(book));
    ar.source_binding = Some(source_binding(source));
    ar.chapter_binding = Some(chapter_binding(chapter));
    ar.set_content(RuleValue::Str(body.to_string()), Some(base_url));
    let _ = ar.set_redirect_url(redirect_url);
    ar.set_next_chapter_url(next_chapter_url);

    let mut content = ar
        .get_string_unescape(content_rule.content.as_deref().unwrap_or(""), false)
        .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
    if !book_is_type(book, TYPE_AUDIO) && !book_is_type(book, TYPE_VIDEO) {
        // adaptSpecialStyle 恒 false → useHtml 占位路径跳过
        let r_url = ar.redirect_url_ref().cloned();
        content = html_formatter::format_keep_img(Some(&content), r_url.as_ref());
        if content.contains('&') {
            content = html_compat::entities::unescape_html4(&content);
        }
    }
    let mut next_url_list = Vec::new();
    if get_next_page_url {
        if let Some(next_rule) = content_rule.next_content_url.as_deref().filter(|s| !s.is_empty())
        {
            // 同上:真身 BookContent.kt L262 也没有 try/catch
            let items = ar
                .get_string_list(next_rule, None, true)
                .map_err(|e| PipelineError::Other(format!("{e:?}")))?;
            next_url_list.extend(items.into_iter().flatten());
        }
    }
    write_back_entity_vars(&mut chapter.vars, ar.data.chapter.as_ref());
    write_back_entity_vars(&mut book.vars, ar.data.book.as_ref());
    let _ = env;
    Ok((content, next_url_list))
}
