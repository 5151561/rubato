//! `BackstageWebView`(judge/engine help/http/BackstageWebView.kt,394 行)的
//! **策略层**逐行移植。
//!
//! plan §3 写的「BackstageWebView 不进自动差分」只对 **WebView 本身**成立。
//! 那 394 行里 WebView 之外**全是策略** —— 默认 JS、补 900 ms、`100 + delayTime`
//! 起跑、`200/400/600/800/1000` 的重试梯子、`retry > 30` 报「js执行超时」、
//! 结果过 `unescapeJson` 再剥首尾引号、跟过转向就包 `priorResponse(302)`、
//! `sourceRegex`/`overrideUrlRegex` 换嗅探客户端回**资源 URL 本身**、
//! 页面加载完把 cookie 抄进 `CookieStore`。这一半两侧都得逐字节一致,
//! 而它完全可以在「WebView 换成同一份剧本」的前提下比 —— 判据见
//! `fixtures/cases/webview/README.md`,裁判是 `judge/wvharness`。
//!
//! 平台那一半(真的去加载一个页面)是 [`WebViewHost`] 后面的事:Rust 不持有
//! webview(plan §1),Android 走 flutter_inappwebview 的 headless 实例、
//! 桌面降档走可见窗口,都在 Dart 侧。
//!
//! **时间是虚拟的**:真身靠 `Handler.postDelayed` 与 `withTimeout` 排时序,
//! 这里把「现在几点」当成状态推进(见 [`BackstageWebView::get_str_response`]),
//! 于是延时策略可比、可测,产品侧再由平台把它翻成真的等待。

use crate::http_url::HttpUrl;
use rubato_core::host::{WebViewEvent, WebViewHost, WebViewLoad, WebViewSettings};

/// `WebSettings.LOAD_DEFAULT`
pub const LOAD_DEFAULT: i32 = -1;
/// `WebSettings.LOAD_CACHE_ELSE_NETWORK`
pub const LOAD_CACHE_ELSE_NETWORK: i32 = 1;

/// 真身 `companion object { const val JS = … }`
pub const DEFAULT_JS: &str = "document.documentElement.outerHTML";

/// `CookieManager.cookieJarHeader`——内部网络选项,**不许发给网站**
const COOKIE_JAR_HEADER: &str = "CookieJar";
const UA_NAME: &str = "User-Agent";

/// `EvalJsRunnable.intervals`
const INTERVALS: [i64; 5] = [200, 400, 600, 800, 1000];

/// 取回来的东西(真身 `StrResponse` 里被上层读到的那三位)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WvResponse {
    /// `StrResponse.url()`——过 okhttp `Request.Builder().url()` 规范化之后的形态
    pub url: String,
    pub body: Option<String>,
    /// `raw.priorResponse != null`:跟过转向。上层(WebBook)靠它判重定向
    pub is_redirect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WvError {
    /// `withTimeout(timeout ?: 60000)` 到点
    Timeout,
    /// `NoStackTraceException("js执行超时")`(retry > 30)
    JsTimeout,
    /// 用户点了停。真身那边是协程被取消(`invokeOnCancellation` 还 WebView),
    /// 这边是 [`WebViewHost::cancelled`] 说了话 —— **差分侧永远到不了这一档**
    /// (剧本 host 的 `cancelled()` 恒假)
    Cancelled,
    /// `url` 是 null 且没有 html —— 真身 `webView.loadUrl(url!!)` 抛 NPE,
    /// 被 `load()` 里那层 `catch (e: Exception)` 收成 onError
    NullUrl,
    /// 别的异常(类名),如重定向分支里 `Request.Builder().url()` 拒了地址
    Other(String),
}

/// 差分标签(裁判侧 `wvharness/Main.kt::errorTag` 的同一套名字)。
/// 上层(`net::fetch`)把它当错误串往外带,于是「webView 那半边报的是什么」
/// 在 fetch / rule-engine 那几套里也说得清。
impl std::fmt::Display for WvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WvError::Timeout => write!(f, "timeout"),
            WvError::JsTimeout => write!(f, "js_timeout"),
            WvError::Cancelled => write!(f, "cancelled"),
            WvError::NullUrl => write!(f, "NullPointerException"),
            WvError::Other(s) => write!(f, "{s}"),
        }
    }
}

/// cookie 回抄的去处(真身是 `CookieStore` 单例;产品侧接 store,差分侧录调用)
pub trait WebViewCookieSink {
    /// `CookieStore.setCookie(tag, CookieManager.getInstance().getCookie(url))`
    fn set_cookie(&mut self, tag: &str, cookie: Option<&str>);
}

/// `CookieStore` 那一路的 sink:真身 `CookieStore.setCookie(tag, cookie)` 与
/// 网络层写的是**同一张表**,所以直接落在本次调用的 [`CookieEnv`] 上。
/// `CookieManager.getCookie(url)` 为 null 时真身存的是**空串**
/// (`CookieStore.saveCookie` 的 `cookie ?: ""`)。
pub struct CookieStoreSink<'a>(pub &'a mut dyn rubato_core::host::CookieEnv);

impl WebViewCookieSink for CookieStoreSink<'_> {
    fn set_cookie(&mut self, tag: &str, cookie: Option<&str>) {
        self.0.set_cookie(tag, cookie.unwrap_or(""));
    }
}

/// 构造参数**逐字对齐真身**(BackstageWebView.kt L51-64):名字一样、缺省一样,
/// 改的时候能一行对一行地核。
#[derive(Debug, Clone, Default)]
pub struct BackstageWebView {
    pub url: Option<String>,
    pub html: Option<String>,
    pub encode: Option<String>,
    pub tag: Option<String>,
    /// 真身是 `HashMap<String, String>?`;null 与空表在 `toWebViewRequestConfig`
    /// 里等价(它整段是 null-safe 的),所以这边只用一张有序表
    pub header_map: Vec<(String, String)>,
    pub source_regex: Option<String>,
    pub override_url_regex: Option<String>,
    pub java_script: Option<String>,
    pub delay_time: i64,
    pub cache_first: bool,
    pub timeout: Option<i64>,
    pub result: Option<String>,
    pub is_rule: bool,
}

impl BackstageWebView {
    /// `getEncoding()`
    fn encoding(&self) -> String {
        self.encode.clone().unwrap_or_else(|| "utf-8".to_string())
    }

    /// `getJs()`:`javaScript` 非空就用它,否则默认那句
    fn js(&self) -> String {
        match self.java_script.as_deref() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => DEFAULT_JS.to_string(),
        }
    }

    /// 走嗅探客户端还是取网页源码的客户端(`createWebView`)
    fn is_sniffer(&self) -> bool {
        !is_null_or_blank(&self.source_regex) || !is_null_or_blank(&self.override_url_regex)
    }

    /// `getStrResponse()`。**时间是虚拟的**:返回值里不带时间,时间轴由
    /// [`WebViewHost`] 的实现(差分侧的剧本 / 产品侧的真等待)自己感知 ——
    /// 差分侧把每次 `eval` 的时刻记下来对拍。
    pub fn get_str_response(
        &self,
        host: &mut dyn WebViewHost,
        cookies: &mut dyn WebViewCookieSink,
    ) -> Result<WvResponse, WvError> {
        // `if (javaScript == null && delayTime == 0L) delayTime = 900L`
        // **只看 javaScript 是不是 null**(空串不算),与真身一致
        let delay_time =
            if self.java_script.is_none() && self.delay_time == 0 { 900 } else { self.delay_time };
        let deadline = self.timeout.unwrap_or(60_000);
        // 当前虚拟时刻(以 `loadUrl` 那一刻为 0);排期与事件在同一根轴上
        let mut now: i64;

        // ---- load():建 WebView + 发起加载 ----
        let (user_agent, headers) =
            to_web_view_request_config(&self.header_map, crate::client::user_agent());
        // `createWebView(requestConfig)` 先于加载 —— 哪怕接着就为 url == null 抛
        host.create(&WebViewSettings {
            user_agent,
            block_network_image: true,
            cache_mode: if self.cache_first { LOAD_CACHE_ELSE_NETWORK } else { LOAD_DEFAULT },
        });
        let html_nonempty = self.html.as_deref().is_some_and(|h| !h.is_empty());
        if !html_nonempty && self.url.is_none() {
            // `webView.loadUrl(url!!)` 的 NPE:真身在 load() 里收成 onError + destroy
            host.destroy();
            return Err(WvError::NullUrl);
        }
        let req = WebViewLoad {
            url: self.url.clone(),
            html: if html_nonempty { self.html.clone() } else { None },
            encoding: self.encoding(),
            headers,
        };
        host.load(&req);

        // ---- 事件回放 + 排期,一条循环 ----
        //
        // 真身是一个 `Handler`:WebView 的回调与 `postDelayed` 的求值排在同一个
        // 队列上,**后来的 `onPageFinished` 会 `removeCallbacks` 掉还没跑的求值、
        // 重新 `postDelayed(100 + delayTime)`**。跳转站因此有两次「加载完成」,
        // 真身取的是最后一次之后的页面。这里照搬那个语义:每一轮都「等到排期那一刻,
        // 期间来了事件就先处理事件」。
        let mut is_redirect = false;
        // `runnable ?: EvalJsRunnable(view, url, …)`:runnable 只建一次,
        // 于是 `buildStrResponse` 用的是**第一次** onPageFinished 的地址
        let mut first_finished: Option<String> = None;
        // `HtmlWebViewClient`:求值的排期(只有一份,每次加载完成重排)
        let mut due: Option<i64> = None;
        // `SnifferWebClient`:每次加载完成排**一发**新的 LoadJsRunnable,
        // 不撤旧的(真身那边没有 removeCallbacks)——所以是一串
        let mut sniff_due: Vec<i64> = Vec::new();
        let mut retry = 0usize;
        let js = if self.is_rule { format!("{INJECTION}\n{}", self.js()) } else { self.js() };

        loop {
            let next_due = if self.is_sniffer() { sniff_due.first().copied() } else { due };
            // 排期越过 deadline 就不必等了 —— 真身那边虚拟队列里的下一件事
            // 排在 `withTimeout` 之后,到点即抛
            let until = next_due.unwrap_or(deadline).min(deadline);
            let got = host.next_event_until(until);
            // **点了停就当场收摊**:这一步可以长达 60 秒(等页面加载完),
            // 而取消在别处是「步与步之间」的粒度 —— 不在这里问,用户点完停
            // 还得再等一分钟。平台侧的等待也因此是分片的(见 `ffi::platform`)
            if host.cancelled() {
                host.destroy();
                return Err(WvError::Cancelled);
            }
            let Some((at, ev)) = got else {
                // 没有排期(还在等页面加载完 / 嗅探一直没命中)= 等到点也没结果
                let Some(d) = next_due else {
                    host.destroy();
                    return Err(WvError::Timeout);
                };
                if d > deadline {
                    host.destroy();
                    return Err(WvError::Timeout);
                }
                now = d;
                if self.is_sniffer() {
                    // `LoadJsRunnable`:`loadUrl("javascript:…")` —— 跑一下,**结果不收**
                    sniff_due.remove(0);
                    if let Some(j) = self.java_script.as_deref().filter(|s| !s.is_empty()) {
                        host.eval_void(&format!("javascript:{j}"));
                    }
                    continue;
                }
                // `EvalJsRunnable.run()`
                let raw = host.eval(&js);
                if !raw.is_empty() && raw != "null" {
                    let content = strip_quotes(&unescape_json(&raw)?);
                    let url = first_finished.clone().unwrap_or_default();
                    let out = self.build_str_response(&url, is_redirect, content);
                    host.destroy();
                    return out;
                }
                if retry > 30 {
                    host.destroy();
                    return Err(WvError::JsTimeout);
                }
                let next_delay = INTERVALS[retry.min(INTERVALS.len() - 1)];
                retry += 1;
                due = Some(now + next_delay);
                continue;
            };
            now = at;
            match ev {
                WebViewEvent::OverrideUrl { url, is_redirect: r } => {
                    if self.is_sniffer() {
                        // `SnifferWebClient.shouldOverrideUrlLoading`
                        if self.matches(&self.override_url_regex, &url)? {
                            let body = url;
                            let out = self.str_response(self.url.clone().unwrap_or_default(), body);
                            host.destroy();
                            return out;
                        }
                    } else {
                        // `HtmlWebViewClient.shouldOverrideUrlLoading`:SDK ≥ N 用
                        // `request.isRedirect`(垫片与产品都钉在 N 之上)
                        is_redirect = is_redirect || r;
                    }
                }
                WebViewEvent::LoadResource { url } => {
                    if self.is_sniffer() && self.matches(&self.source_regex, &url)? {
                        let out = self.str_response(self.url.clone().unwrap_or_default(), url);
                        host.destroy();
                        return out;
                    }
                }
                WebViewEvent::PageFinished { url } => {
                    // `setCookie(url)`:两个客户端的 onPageFinished 都做,
                    // 而且**每一次**都做(地址是这一次那张页的)
                    if let Some(tag) = self.tag.as_deref() {
                        let c = host.page_cookie(&url);
                        cookies.set_cookie(tag, c.as_deref());
                    }
                    if self.is_sniffer() {
                        // `SnifferWebClient.onPageFinished`:javaScript 非空才排
                        if self.java_script.as_deref().is_some_and(|s| !s.is_empty()) {
                            sniff_due.push(now + 100 + delay_time);
                        }
                        continue;
                    }
                    if self.result.is_some() {
                        // `view.evaluateJavascript("window.result = …", null)`:不收结果,
                        // **在 onPageFinished 里当场跑**(不排期),每来一次跑一次
                        host.eval_void(&format!(
                            "window.result = {WEB_CACHE_NAME}.getFromMemory('webview_result')"
                        ));
                    }
                    if first_finished.is_none() {
                        first_finished = Some(url);
                    }
                    // `removeCallbacks(runnable)` + `postDelayed(runnable, 100 + delayTime)`
                    // —— **重排**,而 `retry` 是 runnable 的字段、不跟着归零
                    due = Some(now + 100 + delay_time);
                }
            }
        }
    }

    /// `SnifferWebClient` 的两处 `requestUrl.matches(it.toRegex())` —— Kotlin 的
    /// `matches` 是**整串**匹配。正则编译不了在真身那边是 `PatternSyntaxException`
    fn matches(&self, pattern: &Option<String>, input: &str) -> Result<bool, WvError> {
        let Some(p) = pattern.as_deref().filter(|s| !s.trim().is_empty()) else {
            return Ok(false);
        };
        let re = regex_compat::JavaRegex::compile_full_match(p)
            .map_err(|_| WvError::Other("PatternSyntaxException".into()))?;
        re.is_match(input).map_err(|_| WvError::Other("PatternSyntaxException".into()))
    }

    /// `StrResponse(url, body)`:构造器里 `Request.Builder().url()` 抛就退回
    /// `http://localhost/`(**不抛**)
    fn str_response(&self, url: String, body: String) -> Result<WvResponse, WvError> {
        Ok(WvResponse { url: str_response_url(&url), body: Some(body), is_redirect: false })
    }

    /// `buildStrResponse(content)`
    fn build_str_response(
        &self,
        url: &str,
        is_redirect: bool,
        content: String,
    ) -> Result<WvResponse, WvError> {
        if !is_redirect {
            return Ok(WvResponse {
                url: str_response_url(url),
                body: Some(content),
                is_redirect: false,
            });
        }
        // 这一支是**手搭 Response**:两处 `Request.Builder().url()` 都会为非法地址
        // 抛(不像 StrResponse 的构造器那样吞),异常穿到 handleResult 的 catch
        let origin = self.url.clone().unwrap_or_else(|| url.to_string());
        HttpUrl::parse(&origin).map_err(|_| WvError::Other("IllegalArgumentException".into()))?;
        let final_url =
            HttpUrl::parse(url).map_err(|_| WvError::Other("IllegalArgumentException".into()))?;
        Ok(WvResponse {
            // `StrResponse.url()`:没有 networkResponse 就取 `raw.request.url`
            url: final_url.to_string(),
            body: Some(content),
            is_redirect: true,
        })
    }
}

/// `isRule = true` 时拼在 JS 前面的那段(真身的名字是每进程随机的 ——
/// 差分侧钉成固定串,与 `judge/wvharness` 的垫片同值)
const WEB_CACHE_NAME: &str = "__wvCache";
const INJECTION: &str = "try{var cache=__wvCache,source=__wvSource,java=__wvJava;}catch(e){}";

fn is_null_or_blank(s: &Option<String>) -> bool {
    s.as_deref().is_none_or(|v| v.trim().is_empty())
}

/// `StrResponse(url, body)` 构造器:url 过 okhttp,非法 → `http://localhost/`
fn str_response_url(raw: &str) -> String {
    match HttpUrl::parse(raw) {
        Ok(u) => u.to_string(),
        Err(_) => "http://localhost/".to_string(),
    }
}

/// `handleResult` 的 `.replace("^\"|\"$".toRegex(), "")`:**两头各剥一个引号**
/// (Regex.replace 是全局替换,而这条正则最多命中首尾各一次)
fn strip_quotes(s: &str) -> String {
    let mut out = s;
    if let Some(rest) = out.strip_prefix('"') {
        out = rest;
    }
    if let Some(rest) = out.strip_suffix('"') {
        out = rest;
    }
    out.to_string()
}

/// `toWebViewRequestConfig`(judge/engine help/webView/WebViewRequestConfig.kt 真身)
///
/// 返回 (UA, 其余头)。UA 先找**精确**的 `User-Agent`,再找忽略大小写的,
/// 都没有才用默认;`User-Agent` / `CookieJar` / `proxy` 三个不进转发头
/// —— 后两个是内部网络选项,发给网站就是漏。
pub fn to_web_view_request_config(
    header_map: &[(String, String)],
    default_user_agent: &str,
) -> (String, Vec<(String, String)>) {
    let exact = header_map
        .iter()
        .find(|(k, v)| k == UA_NAME && !v.trim().is_empty())
        .map(|(_, v)| v.clone());
    let user_agent = exact
        .or_else(|| {
            header_map
                .iter()
                .find(|(k, v)| k.eq_ignore_ascii_case(UA_NAME) && !v.trim().is_empty())
                .map(|(_, v)| v.clone())
        })
        .unwrap_or_else(|| default_user_agent.to_string());
    let headers = header_map
        .iter()
        .filter(|(k, _)| {
            !k.eq_ignore_ascii_case(UA_NAME)
                && !k.eq_ignore_ascii_case(COOKIE_JAR_HEADER)
                && !k.eq_ignore_ascii_case("proxy")
        })
        .cloned()
        .collect();
    (user_agent, headers)
}

/// `org.apache.commons.text.StringEscapeUtils.unescapeJson`
/// (= `UNESCAPE_JAVA`:八进制 → `\uXXXX` → 控制字符表 → `\\`/`\"`/`\'`/裸 `\`)。
///
/// 顺序是**它自己的**:AggregateTranslator 在每个位置按序试,谁先吃掉算谁的。
/// 于是 `\/` 落到最后那条「裸 `\` 吃掉、什么都不输出」上 → 变成 `/`;
/// 而 `\u` 后面不足四个十六进制位时 commons-text **抛**(真身没接这个异常,
/// 会穿过 `handleResult`)—— 这里照抛。
pub fn unescape_json(input: &str) -> Result<String, WvError> {
    let b: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != '\\' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        // ① OctalUnescaper:`\` + 1~3 位八进制(首位 0-3 才允许三位)
        if i + 1 < b.len() {
            if let Some(extra) = octal_extra(b[i + 1]) {
                let max = extra + 1;
                let mut n = 0usize;
                let mut val = 0u32;
                while n < max && i + 1 + n < b.len() && b[i + 1 + n].is_digit(8) {
                    val = val * 8 + b[i + 1 + n].to_digit(8).unwrap();
                    n += 1;
                }
                out.push(char::from_u32(val).unwrap_or('\u{fffd}'));
                i += 1 + n;
                continue;
            }
        }
        // ② UnicodeUnescaper:`\` + 一个或多个 `u` + 四位十六进制
        if i + 1 < b.len() && b[i + 1] == 'u' {
            let mut j = i + 1;
            while j < b.len() && b[j] == 'u' {
                j += 1;
            }
            if j + 4 > b.len() {
                return Err(WvError::Other("IllegalArgumentException".into()));
            }
            let hex: String = b[j..j + 4].iter().collect();
            let val = u32::from_str_radix(&hex, 16)
                .map_err(|_| WvError::Other("IllegalArgumentException".into()))?;
            out.push(char::from_u32(val).unwrap_or('\u{fffd}'));
            i = j + 4;
            continue;
        }
        // ③ 控制字符表 ④ 反斜杠自身 / 引号 / 单引号 / 裸反斜杠(吃掉)
        if i + 1 < b.len() {
            let c = b[i + 1];
            let mapped = match c {
                'b' => Some('\u{8}'),
                'n' => Some('\n'),
                't' => Some('\t'),
                'f' => Some('\u{c}'),
                'r' => Some('\r'),
                '\\' => Some('\\'),
                '"' => Some('"'),
                '\'' => Some('\''),
                _ => None,
            };
            if let Some(m) = mapped {
                out.push(m);
                i += 2;
                continue;
            }
        }
        // 裸 `\`:LookupTranslator 的最后一条 —— 吃掉,不输出
        i += 1;
    }
    Ok(out)
}

/// commons-text `OctalUnescaper.additionalDigits`
fn octal_extra(c: char) -> Option<usize> {
    match c {
        '0'..='3' => Some(2),
        '4'..='7' => Some(1),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescape_and_strip() {
        assert_eq!(unescape_json(r#""a\nb""#).unwrap(), "\"a\nb\"");
        assert_eq!(strip_quotes("\"x\""), "x");
        // `\/` 落到「裸反斜杠吃掉」那一条
        assert_eq!(unescape_json(r"a\/b").unwrap(), "a/b");
        assert_eq!(unescape_json(r"A").unwrap(), "A");
        assert_eq!(unescape_json(r"\101").unwrap(), "A");
        assert!(unescape_json(r"\u00").is_err());
    }

    /// 点了停:**当场收摊并把 WebView 还回去**,不等满 60 秒。
    /// (差分侧到不了这一档 —— 剧本 host 的 `cancelled()` 恒假)
    #[test]
    fn cancel_stops_the_wait_and_releases_the_webview() {
        #[derive(Default)]
        struct StoppedHost {
            released: u32,
            waits: u32,
        }
        impl WebViewHost for StoppedHost {
            fn create(&mut self, _s: &WebViewSettings) {}
            fn load(&mut self, _l: &WebViewLoad) {}
            fn next_event_until(&mut self, _until: i64) -> Option<(i64, WebViewEvent)> {
                // 平台侧看见令牌就提前回来(没到点也交 None)
                self.waits += 1;
                None
            }
            fn eval(&mut self, _js: &str) -> String {
                panic!("取消之后不该再求值")
            }
            fn eval_void(&mut self, _js: &str) {}
            fn page_cookie(&mut self, _url: &str) -> Option<String> {
                None
            }
            fn cancelled(&self) -> bool {
                true
            }
            fn destroy(&mut self) {
                self.released += 1;
            }
        }
        struct NoCookies;
        impl WebViewCookieSink for NoCookies {
            fn set_cookie(&mut self, _tag: &str, _cookie: Option<&str>) {}
        }

        let bwv =
            BackstageWebView { url: Some("https://a.example/p".into()), ..Default::default() };
        let mut host = StoppedHost::default();
        let err = bwv.get_str_response(&mut host, &mut NoCookies).unwrap_err();
        assert_eq!(err, WvError::Cancelled);
        assert_eq!(err.to_string(), "cancelled");
        assert_eq!(host.released, 1, "取消也要把 WebView 还回去");
        assert_eq!(host.waits, 1, "问过一次就该收摊");
    }

    #[test]
    fn request_config_filters_internal_options() {
        let h = vec![
            ("user-agent".to_string(), "UA/2".to_string()),
            ("CookieJar".to_string(), "1".to_string()),
            ("proxy".to_string(), "p".to_string()),
            ("referer".to_string(), "r".to_string()),
        ];
        let (ua, rest) = to_web_view_request_config(&h, "def");
        assert_eq!(ua, "UA/2");
        assert_eq!(rest, vec![("referer".to_string(), "r".to_string())]);
    }
}
