//! `AnalyzeUrl`(judge/engine 992 行)的移植:把书源里的 url 规则编译成一次
//! 请求的全部要素(url / method / header / body / 编码后的 query 与 form /
//! charset / retry / webView 等),再经 [`client`] 的重定向/重试/cookie 循环
//! 发出去(传输层抽象见 [`transport`],产品侧实现见 [`live`],feature `live`)。
//!
//! 离线面的差分口径见 fixtures/cases/analyze-url/README.md,
//! 全链路(含 cookie 合并)由 fetch 套钉住。

pub mod charset_detector;
pub mod client;
pub mod cookie;
pub mod encode;
pub mod encoding_detect;
pub mod fetch;
pub mod http_url;
pub mod icu4j_data;
#[cfg(feature = "live")]
pub mod live;
pub mod response;
pub mod source_header;
pub mod transport;
pub mod url_option;
/// `BackstageWebView` 的**策略层**(webView 的可差分那一半),见 webview.rs 头注释
pub mod verification;
pub mod webview;

use encode::encode_params;
use regex::Regex;
use rubato_core::RuleData;
use rubato_core::gson;
use rubato_core::host::{
    ElementHandle, HostEnv, JsBindings, JsValue, NetResponse, RuleHost, VarStore,
};
use rubato_core::net_utils::{get_absolute_url_str, get_base_url, get_sub_domain, is_json, is_xml};
use rule_syntax::RuleAnalyzer;
use std::sync::LazyLock;
use url_option::UrlOption;

/// `<(.*?)>`——Java 的 `.` 不匹配行终止符
static PAGE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("<([^\r\n\u{85}\u{2028}\u{2029}]*?)>").expect("page 模式"));

/// `paramPattern = \s*,\s*(?=\{)`——url 与 option JSON 的分界。
/// regex crate 不支持先行断言,按 Java 语义手写:找到第一个「跳过空白后是 `{`」
/// 的逗号,匹配区间从该逗号前的空白串起点到 `{` 前一位(Java 的 \s 是 ASCII 集)。
/// (pub:HtmlFormatter.formatKeepImg 的图片 option 切分也用它)
pub fn param_split(s: &str) -> Option<(usize, usize)> {
    fn is_ws(b: u8) -> bool {
        matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
    }
    let b = s.as_bytes();
    for i in 0..b.len() {
        if b[i] != b',' {
            continue;
        }
        let mut j = i + 1;
        while j < b.len() && is_ws(b[j]) {
            j += 1;
        }
        if j < b.len() && b[j] == b'{' {
            let mut start = i;
            while start > 0 && is_ws(b[start - 1]) {
                start -= 1;
            }
            return Some((start, j));
        }
    }
    None
}
pub(crate) static JS_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<js>([\s\S]*?)</js>|@js:([\s\S]*)").expect("js 模式"));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestMethod {
    Get,
    Post,
    Head,
}

impl RequestMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            RequestMethod::Get => "GET",
            RequestMethod::Post => "POST",
            RequestMethod::Head => "HEAD",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlError {
    /// RuleAnalyzer 抛出的解析异常
    Split(String),
    /// JS 求值异常
    Js(String),
    /// `charset(name)` 不认识的编码
    UnsupportedCharset(String),
    /// page 规则越界(`pages[page-1]`)
    IndexOutOfBounds,
}

impl std::fmt::Display for UrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for UrlError {}

type R<T> = Result<T, UrlError>;

/// 构造参数(对应 AnalyzeUrl 的主构造器里差分覆盖得到的那些)
#[derive(Default)]
pub struct UrlArgs<'a> {
    pub m_url: &'a str,
    pub base_url: &'a str,
    pub key: Option<&'a str>,
    pub page: Option<i32>,
    pub speak_text: Option<&'a str>,
    pub speak_speed: Option<i32>,
    /// headerMapF(为 None 时取 source 的 header)
    pub header_map_f: Option<Vec<(String, String)>>,
    pub source_header: Option<Vec<(String, String)>>,
    /// `source.getKey()`(bookSourceUrl)——cookie 的 domain 归一优先用它
    pub source_key: Option<&'a str>,
    /// `source.enabledCookieJar`
    pub enabled_cookie_jar: bool,
    /// `bindings["book"] = ruleData as? Book`(AnalyzeUrl.kt L387)
    pub book: Option<rubato_core::host::BookBinding>,
    /// `bindings["source"] = source`(同上 L388)
    pub source: Option<rubato_core::host::SourceBinding>,
    /// **元素面的提供方**(`org.jsoup.Jsoup.parse` 那条 LiveConnect 路)。
    /// net 建不出 AnalyzeRule(依赖方向不允许),由上层注入;
    /// 缺席时书源里的 `org.jsoup.*` 报 `«no-rule-host»`。见 [`SplitEnv`]。
    pub rule_host: Option<Box<dyn RuleHost>>,
}

pub struct AnalyzeUrl {
    pub rule_url: String,
    pub url: String,
    pub url_no_query: String,
    pub base_url: String,
    /// LinkedHashMap:插入序有意义
    pub header_map: Vec<(String, String)>,
    pub body: Option<String>,
    pub encoded_form: Option<String>,
    pub encoded_query: Option<String>,
    pub charset: Option<String>,
    pub method: RequestMethod,
    pub proxy: Option<String>,
    pub type_: Option<String>,
    pub retry: i32,
    pub use_web_view: bool,
    pub web_js: Option<String>,
    pub body_js: Option<String>,
    pub dns_ip: Option<String>,
    /// urlOption 的 `timeout`(毫秒)→ okhttp readTimeout
    pub read_timeout_ms: Option<i64>,
    /// urlOption 里显式配过 timeout(决定是否派生 callTimeout / 走 Cronet 拦截器)
    pub url_timeout_configured: bool,
    /// urlOption 的 `followRedirects`
    pub follow_redirects: Option<bool>,
    pub server_id: Option<i64>,
    pub web_view_delay_time: i64,
    pub data: RuleData,
    /// cookie 键:`sourceKey?.takeIf { getBaseUrl(it) == null } ?: getSubDomain(url)`
    pub domain: String,
    /// `source?.getKey()` 原样(webView 分支的 `tag` 就是它 —— cookie 回抄的键)。
    /// 与 [`Self::domain`] 不是一回事:那一位是「链接就退回请求域名」之后的结果。
    pub source_key: Option<String>,
    pub enabled_cookie_jar: bool,
    host: Box<dyn HostEnv>,
    key: Option<String>,
    page: Option<i32>,
    book: Option<rubato_core::host::BookBinding>,
    source: Option<rubato_core::host::SourceBinding>,
    rule_host: Option<Box<dyn RuleHost>>,
}

/// `AnalyzeUrl` 求值时交给 JS 宿主的环境:变量层是 `AnalyzeUrl` 自己那份
/// `RuleData`(真身 `AnalyzeUrl.put/get` 打的就是 ruleData),而**元素面**
/// 另外接。
///
/// 为什么要拆:`java` 在这个宿主上是 AnalyzeUrl,**没有** getString/getElements
/// (探针实测 TypeError,见 fixtures/cases/js-host/README.md「两个宿主」);
/// 但 `org.jsoup.Jsoup.parse(...)` 是 Rhino 的 **LiveConnect 全局面**,与 `java`
/// 是谁无关 —— 书源在 searchUrl 的 `@js:` 里直接 parse 回来的 HTML 很常见。
/// 差分抓到:少了它,那些源在被测侧直接 TypeError。
struct SplitEnv<'a> {
    vars: &'a mut RuleData,
    rules: &'a mut dyn RuleHost,
}

impl VarStore for SplitEnv<'_> {
    fn put(&mut self, key: &str, value: &str) -> String {
        self.vars.put(key, value)
    }
    fn get(&self, key: &str) -> String {
        self.vars.get(key)
    }
}

/// 逐字转发给内层 `rules`(签名照 `RuleHost` 原样列出,便于对着真身核对)
macro_rules! forward_to_rules {
    ($(fn $name:ident(&mut self $(, $arg:ident: $ty:ty)*) -> $ret:ty;)+) => {
        $(fn $name(&mut self $(, $arg: $ty)*) -> $ret {
            self.rules.$name($($arg),*)
        })+
    };
}

/// 规则求值那几个(getString/getElements/setContent)**不转发** —— 这个宿主
/// 上真身就没有它们,保持缺省实现给 `«no-rule-host»`。只转发元素面与 parse。
impl RuleHost for SplitEnv<'_> {
    forward_to_rules! {
        fn rule_parse_html(&mut self, html: &str) -> Result<ElementHandle, String>;
        fn el_to_string(&mut self, h: ElementHandle) -> String;
        fn el_text(&mut self, h: ElementHandle) -> String;
        fn el_html(&mut self, h: ElementHandle) -> String;
        fn el_attr(&mut self, h: ElementHandle, name: &str) -> String;
        fn el_select(&mut self, h: ElementHandle, css: &str) -> Result<Vec<ElementHandle>, String>;
        fn el_size(&mut self, h: ElementHandle) -> usize;
        fn el_get(&mut self, h: ElementHandle, i: usize) -> Option<ElementHandle>;
        fn el_not(&mut self, h: ElementHandle, css: &str) -> Result<Vec<ElementHandle>, String>;
        fn el_json(&mut self, h: ElementHandle) -> Option<serde_json::Value>;
        fn el_java(&mut self, h: ElementHandle) -> Option<rubato_core::host::JavaValue>;
        fn rule_get_elements(&mut self, rule: &str) -> Result<rubato_core::host::ElementList, String>;
    }
}

pub(crate) fn header_get<'a>(map: &'a [(String, String)], key: &str) -> Option<&'a str> {
    map.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

pub(crate) use crate::transport::header_get_ci;

pub(crate) fn header_put(map: &mut Vec<(String, String)>, key: &str, value: &str) {
    match map.iter_mut().find(|(k, _)| k == key) {
        Some(slot) => slot.1 = value.to_string(),
        None => map.push((key.to_string(), value.to_string())),
    }
}

/// Kotlin `trim { it <= ' ' }`
fn trim_ascii(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

impl AnalyzeUrl {
    pub fn new(args: UrlArgs<'_>, host: Box<dyn HostEnv>, data: RuleData) -> R<Self> {
        let mut base_url = args.base_url.to_string();
        if let Some((start, _)) = param_split(&base_url) {
            base_url = base_url[..start].to_string();
        }
        let mut header_map: Vec<(String, String)> = Vec::new();
        let mut proxy = None;
        if let Some(h) = args.header_map_f.or(args.source_header) {
            for (k, v) in h {
                header_put(&mut header_map, &k, &v);
            }
            if let Some(p) = header_get(&header_map, "proxy") {
                proxy = Some(p.to_string());
                header_map.retain(|(k, _)| k != "proxy");
            }
        }

        let mut me = AnalyzeUrl {
            rule_url: String::new(),
            url: String::new(),
            url_no_query: String::new(),
            base_url,
            header_map,
            body: None,
            encoded_form: None,
            encoded_query: None,
            charset: None,
            method: RequestMethod::Get,
            proxy,
            type_: None,
            retry: 0,
            use_web_view: false,
            web_js: None,
            body_js: None,
            dns_ip: None,
            read_timeout_ms: None,
            url_timeout_configured: false,
            follow_redirects: None,
            server_id: None,
            web_view_delay_time: 0,
            data,
            domain: String::new(),
            enabled_cookie_jar: args.enabled_cookie_jar,
            source_key: args.source_key.map(str::to_string),
            host,
            key: args.key.map(str::to_string),
            page: args.page,
            book: args.book,
            source: args.source,
            rule_host: args.rule_host,
        };
        let source_key = me.source_key.clone();
        me.init_url(args.m_url)?;
        // 真身(AnalyzeUrl L149-151,initUrl() 之后):
        //   val sourceKey = source?.getKey()
        //   domain = sourceKey?.takeIf { NetworkUtils.getBaseUrl(it) == null }
        //       ?: NetworkUtils.getSubDomain(url)
        // sourceKey **不是** http(s) 链接(书源名之类)时直接当键用;
        // 是链接时反而不取它的子域名,而是取**本次请求 url** 的子域名。
        me.domain = match source_key.as_deref() {
            Some(k) if get_base_url(k).is_none() => k.to_string(),
            _ => get_sub_domain(&me.url),
        };
        Ok(me)
    }

    fn init_url(&mut self, m_url: &str) -> R<()> {
        self.rule_url = m_url.to_string();
        self.analyze_js()?;
        self.replace_key_page_js()?;
        self.analyze_url()?;
        Ok(())
    }

    /// `analyzeJs`:`<js>`/`@js:` 段之外的文本用 `@result` 占位拼接
    fn analyze_js(&mut self) -> R<()> {
        let rule_url = self.rule_url.clone();
        let mut start = 0usize;
        let mut result = rule_url.clone();
        for m in JS_PATTERN.captures_iter(&rule_url) {
            let whole = m.get(0).expect("整段");
            if whole.start() > start {
                let seg = rule_url[start..whole.start()].trim();
                if !seg.is_empty() {
                    result = seg.replace("@result", &result);
                }
            }
            let g2 = m.get(2).map(|x| x.as_str()).unwrap_or("");
            let g1 = m.get(1).map(|x| x.as_str()).unwrap_or("");
            let body = if g2.is_empty() { g1 } else { g2 };
            // Kotlin 的 `evalJS(...).toString()`:null 变成字面量 "null"
            result = match self.eval_js(body, Some(&result))? {
                Some(v) => js_to_string(&v),
                None => "null".to_string(),
            };
            start = whole.end();
        }
        if rule_url.len() > start {
            let seg = rule_url[start..].trim();
            if !seg.is_empty() {
                result = seg.replace("@result", &result);
            }
        }
        self.rule_url = result;
        Ok(())
    }

    /// `replaceKeyPageJs`:先 `{{js}}` 后 `<a,b,c>` 页码
    fn replace_key_page_js(&mut self) -> R<()> {
        if self.rule_url.contains("{{") && self.rule_url.contains("}}") {
            let src = self.rule_url.clone();
            let mut analyze = RuleAnalyzer::new(&src, false);
            let mut js_err: Option<UrlError> = None;
            let url = analyze
                .inner_rule_str("{{", "}}", |code| match self.eval_js(code, None) {
                    Ok(None) => Some(String::new()),
                    Ok(Some(v)) => Some(js_number_or_string(&v)),
                    Err(e) => {
                        js_err.get_or_insert(e);
                        Some(String::new())
                    }
                })
                .map_err(|e| UrlError::Split(e.to_string()))?;
            if let Some(e) = js_err {
                return Err(e);
            }
            if !url.is_empty() {
                self.rule_url = url;
            }
        }
        if let Some(page) = self.page {
            let src = self.rule_url.clone();
            for m in PAGE_PATTERN.captures_iter(&src) {
                let whole = m.get(0).expect("整段").as_str();
                let pages: Vec<&str> = m.get(1).expect("组1").as_str().split(',').collect();
                // 有符号比较(裁判 `if (page < pages.size)`):page 为负时 `as usize`
                // 会绕成极大值走 last() 那支,真身却是下一行的越界
                let pick = if (page as i64) < pages.len() as i64 {
                    let idx = usize::try_from(page - 1).map_err(|_| UrlError::IndexOutOfBounds)?;
                    pages.get(idx).ok_or(UrlError::IndexOutOfBounds)?
                } else {
                    pages.last().expect("split 至少一段")
                };
                self.rule_url = self.rule_url.replace(whole, trim_ascii(pick));
            }
        }
        Ok(())
    }

    /// `analyzeUrl`:切 option JSON、绝对化、套用 option、编码 query/form
    fn analyze_url(&mut self) -> R<()> {
        let rule_url = self.rule_url.clone();
        let url_match = param_split(&rule_url);
        let url_no_option = match url_match {
            Some((start, _)) => &rule_url[..start],
            None => &rule_url[..],
        };
        self.url = get_absolute_url_str(Some(&self.base_url), url_no_option);
        if let Some(b) = rubato_core::net_utils::get_base_url(&self.url) {
            self.base_url = b;
        }
        if let Some((_, end)) = url_match {
            let option_str = &rule_url[end..];
            // 真身两档解析:GSONStrict 先来,失败再用宽松档 —— **宽松档救回来时
            // 会记一条 Debug 日志**(AnalyzeUrl.kt L245)。语料里这种写法极多
            // (option 用单引号),日志那一面比不上就是这里漏了。
            let option = match gson::parse_strict(option_str).and_then(UrlOption::from_value) {
                Some(o) => Some(o),
                None => match gson::parse_lenient(option_str).and_then(UrlOption::from_value) {
                    Some(o) => {
                        self.host.log("链接参数 JSON 格式不规范，请改为规范格式");
                        Some(o)
                    }
                    None => None,
                },
            };
            if let Some(option) = option {
                if let Some(method) = option.method() {
                    self.method = match method.to_uppercase().as_str() {
                        "POST" => RequestMethod::Post,
                        "HEAD" => RequestMethod::Head,
                        _ => RequestMethod::Get,
                    };
                }
                if let Some(headers) = option.header_map() {
                    for (k, v) in headers {
                        header_put(&mut self.header_map, &k, &v);
                    }
                }
                if let Some(b) = option.body() {
                    self.body = Some(b);
                }
                self.type_ = option.type_();
                self.charset = option.charset();
                self.retry = option.retry();
                self.use_web_view = option.use_web_view();
                self.web_js = option.web_js();
                self.body_js = option.body_js();
                // LegadoTeam 新增:timeout / followRedirects(真身 L268-272)
                if let Some(t) = option.timeout() {
                    self.read_timeout_ms = Some(t);
                    self.url_timeout_configured = true;
                }
                self.follow_redirects = option.follow_redirects();
                self.dns_ip = option.dns_ip();
                if let Some(js) = option.js() {
                    let cur = self.url.clone();
                    if let Some(v) = self.eval_js(&js, Some(&cur))? {
                        self.url = js_to_string(&v);
                    }
                }
                self.server_id = option.server_id();
                self.web_view_delay_time = option.web_view_delay_time().unwrap_or(0).max(0);
            }
        }
        self.url_no_query = self.url.clone();
        match self.method {
            RequestMethod::Get => {
                if let Some(pos) = self.url.find('?') {
                    let query = self.url[pos + 1..].to_string();
                    self.encoded_query =
                        Some(encode_params(&query, self.charset.as_deref(), true)?);
                    self.url_no_query = self.url[..pos].to_string();
                }
            }
            RequestMethod::Post => {
                if let Some(body) = self.body.clone() {
                    if !is_json(&body)
                        && !is_xml(&body)
                        && header_get(&self.header_map, "Content-Type").is_none_or(str::is_empty)
                    {
                        self.encoded_form =
                            Some(encode_params(&body, self.charset.as_deref(), false)?);
                    }
                }
            }
            RequestMethod::Head => {}
        }
        Ok(())
    }

    pub(crate) fn eval_js(&mut self, code: &str, result: Option<&str>) -> R<Option<JsValue>> {
        let bindings = JsBindings {
            // AnalyzeUrl.evalJS —— 绑定面与 AnalyzeRule 不同,见 JsHost
            host: rubato_core::host::JsHost::Url,
            // `@result` 这条路上它一直是串(AnalyzeUrl 的规则链上没有元素)
            result: result.map(|s| rubato_core::host::BoundValue::Str(s.to_string())),
            base_url: Some(self.base_url.clone()),
            key: self.key.clone(),
            page: self.page,
            // 真身绑的是 `ruleData as? Book` 与 `source` —— 书源的 `@js:`/`{{}}`
            // 里 `book.name` / `source.bookSourceUrl` 很常见,漏绑就是 TypeError
            book: self.book.clone(),
            source: self.source.clone(),
            ..Default::default()
        };
        let r = match self.rule_host.as_mut() {
            Some(rh) => {
                let mut env = SplitEnv { vars: &mut self.data, rules: rh.as_mut() };
                self.host.eval_js(code, &bindings, &mut env)
            }
            None => self.host.eval_js(code, &bindings, &mut self.data),
        };
        match r {
            Err(e) => Err(UrlError::Js(e)),
            Ok(JsValue::Null) => Ok(None),
            Ok(v) => Ok(Some(v)),
        }
    }

    /// `loginCheckJs`:真身 `analyzeUrl.evalJS(checkJs, res) as StrResponse`
    /// (WebBook.kt L75-81)—— JS 真的跑,**返回值强转 StrResponse**,转不成
    /// 就是 ClassCastException,一路穿到四步之外。
    ///
    /// 返回 `Ok(true)` = 交回来的还是那个响应(强转成立,四步照常往下走);
    /// `Ok(false)` = 返回的是别的东西(真身在这里 ClassCastException)。
    ///
    /// **还没做的一支**:JS 返回**另一个** StrResponse(比如
    /// `java.connect(u)` 的结果)时真身会拿那一个继续走,这里认不出来,
    /// 按 `false` 报错。语料里 7 例全是 `cookie.removeCookie(…)\nresult`。
    ///
    /// 绑进去的 `StrResponse` 只带 url / body / code:`StrResponse` 这边没留
    /// 响应头(见 fetch.rs),所以 JS 里 `result.headers()` 是空的。语料里
    /// checkJs 没有读它的。
    pub fn eval_login_check_js(&mut self, code: &str, res: &crate::fetch::StrResponse) -> R<bool> {
        let bound = NetResponse {
            url: res.url.clone(),
            body: res.body.clone(),
            code: res.status as i32,
            ..Default::default()
        };
        let bindings = JsBindings {
            host: rubato_core::host::JsHost::Url,
            result: Some(rubato_core::host::BoundValue::Response(bound)),
            base_url: Some(self.base_url.clone()),
            key: self.key.clone(),
            page: self.page,
            book: self.book.clone(),
            source: self.source.clone(),
            ..Default::default()
        };
        let r = match self.rule_host.as_mut() {
            Some(rh) => {
                let mut env = SplitEnv { vars: &mut self.data, rules: rh.as_mut() };
                self.host.eval_js(code, &bindings, &mut env)
            }
            None => self.host.eval_js(code, &bindings, &mut self.data),
        };
        match r {
            Err(e) => Err(UrlError::Js(e)),
            // 绑定对象原样交回来:`__strResponse` 那一位还在(见 js-host 的 set_bound)
            Ok(JsValue::Json(v)) => {
                Ok(v.get("__strResponse") == Some(&serde_json::Value::Bool(true)))
            }
            Ok(_) => Ok(false),
        }
    }

    /// `getUserAgent()`
    pub fn user_agent(&self, default_ua: &str) -> String {
        header_get_ci(&self.header_map, "User-Agent").unwrap_or(default_ua).to_string()
    }

    pub fn is_post(&self) -> bool {
        self.method == RequestMethod::Post
    }
}

/// Kotlin `Any.toString()` 对 JS 返回值的效果
pub(crate) fn js_to_string(v: &JsValue) -> String {
    match v {
        JsValue::Null => "null".into(),
        JsValue::Str(s) => s.clone(),
        JsValue::Num(d) => gson::java_double(*d),
        JsValue::Bool(b) => b.to_string(),
        JsValue::List(l) => format!("[{}]", l.join(", ")),
        JsValue::Json(j) => gson::object_to_string(j),
        // jsoup 的 toString:单个 Element 是 outerHtml,一组是各 outerHtml 以
        // 换行相连。**JS 数组装着元素**时真身给的是身份哈希(`NativeArray@…`,
        // 逐次运行都不同)—— 那一支不可复现,这里不区分,按一组走。
        // AnalyzeUrl 这条路上元素只能来自 `org.jsoup` 那面,语料里没有。
        JsValue::Elements { strings, .. } => strings.join("\n"),
        // NativeArray 的 Kotlin `toString()` 是身份哈希,逐次运行都不同 ——
        // 归一成同一个标记(同 RuleValue::Native)
        JsValue::Native(_) => "«identity»".into(),
    }
}

/// `replaceKeyPageJs` 里的整数特判:`Double % 1.0 == 0.0` → `%.0f`
fn js_number_or_string(v: &JsValue) -> String {
    match v {
        JsValue::Str(s) => s.clone(),
        JsValue::Num(d) if d % 1.0 == 0.0 => format!("{d:.0}"),
        other => js_to_string(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use difftest_stub::Stub;

    mod difftest_stub {
        use rubato_core::host::{HostEnv, JsBindings, JsValue};
        pub struct Stub;
        impl HostEnv for Stub {
            fn eval_js(
                &mut self,
                code: &str,
                b: &JsBindings,
                _v: &mut dyn rubato_core::host::JsRuleEnv,
            ) -> Result<JsValue, String> {
                Ok(match code {
                    "#key" => JsValue::Str(b.key.clone().unwrap_or_default()),
                    _ => JsValue::Str(code.to_string()),
                })
            }
            fn web_js(
                &mut self,
                req: &rubato_core::host::WebJsRequest<'_>,
            ) -> Result<String, String> {
                Ok(req.js.to_string())
            }
        }
    }

    fn build(m_url: &str) -> AnalyzeUrl {
        AnalyzeUrl::new(
            UrlArgs {
                m_url,
                base_url: "http://e.com/a/b.html",
                key: Some("关键字"),
                page: Some(2),
                ..Default::default()
            },
            Box::new(Stub),
            RuleData::default(),
        )
        .expect("构造成功")
    }

    #[test]
    fn plain_url_and_query() {
        let a = build("/s?q=中文&p=1");
        assert_eq!(a.url, "http://e.com/s?q=中文&p=1");
        assert_eq!(a.url_no_query, "http://e.com/s");
        assert_eq!(a.encoded_query.as_deref(), Some("q=%E4%B8%AD%E6%96%87&p=1"));
        assert_eq!(a.method, RequestMethod::Get);
    }

    #[test]
    fn option_json() {
        let a = build(r#"/s,{"method":"POST","body":"q=中文","charset":"gbk","retry":2}"#);
        assert_eq!(a.method, RequestMethod::Post);
        assert_eq!(a.retry, 2);
        assert_eq!(a.encoded_form.as_deref(), Some("q=%D6%D0%CE%C4"));
        // gson 不走 setter:空白/原值原样留着
        assert_eq!(a.charset.as_deref(), Some("gbk"));
    }

    #[test]
    fn trailing_comma_kills_option() {
        // gson 即使 lenient 也不接受尾逗号 → option 整个作废,退回 GET
        let a = build(r#"/s,{"method":"POST","body":"x",}"#);
        assert_eq!(a.method, RequestMethod::Get);
        assert_eq!(a.body, None);
    }

    #[test]
    fn page_pattern() {
        let a = build("/list<1,2,3>.html");
        assert_eq!(a.url, "http://e.com/list2.html");
    }

    #[test]
    fn body_object_is_pretty_printed() {
        let a = build(r#"/s,{"method":"POST","body":{"a":1}}"#);
        assert_eq!(a.body.as_deref(), Some("{\n  \"a\": 1\n}"));
    }
}
