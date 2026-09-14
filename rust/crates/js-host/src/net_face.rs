//! **JS 网络面的本体**:`java.ajax` / `connect` / `get`/`head`/`post`。
//!
//! 产品侧(`engine::js_net::LiveNet`,接 okhttp 等价的 LiveTransport)与差分侧
//! (`difftest::js_net::ReplayNet`,接 HTTP 录放)从前是两份近乎逐行相同的
//! 实现 —— 连 Set-Cookie 解析那十行和注释都一字不差,改一处漏一处。
//! 真正不同的只有四样,全部收进 [`JsNetIo`] 由两侧各自注入:
//! - transport / cookies 的具体类型(两个类型参数);
//! - 嵌套宿主怎么建(`nested_factory`:差分侧要确定性垫片,产品侧要取消钩子);
//! - `AnalyzeUrl` 构造错误怎么归一(`construct_error`:差分侧要对齐裁判的
//!   异常类别串,产品侧原样);
//! - hops 观察面(`hops_fn`:差分侧接 RecordingTransport;产品侧缺席 ——
//!   LiveTransport 没有记录器,见 [`NetProvider::hops`] 缺省实现处的说明)。
//!
//! **两条不同的路**,与真身一一对应:
//! - `ajax` / `connect` 走 `AnalyzeUrl.getStrResponse` —— okhttp,带拦截器链
//!   (补 UA / Keep-Alive)、跟进重定向、按 urlOption 解析 `,{...}` 参数;
//! - `get` / `head` / `post` 走 **jsoup** 的 `Connection` —— 不经那条拦截器链,
//!   `followRedirects(false)`,头是调用方给的原样,直接打一跳。

use crate::host_env::QuickJsHost;
use net::transport::{HttpTransport, RequestBody};
use net::{AnalyzeUrl, UrlArgs};
use rubato_core::RuleData;
use rubato_core::host::{
    CookieEnv, HostEnv, JsBindings, JsValue, NetError, NetProvider, NetResponse, WebViewHost,
};
use std::cell::RefCell;
use std::rc::Rc;

/// 两侧各自注入的差异面 + 公共配置。见模块注释。
pub struct JsNetIo<T: HttpTransport, C: CookieEnv> {
    pub transport: T,
    pub cookies: C,
    pub source_key: String,
    pub source_header: Option<Vec<(String, String)>>,
    pub enabled_cookie_jar: bool,
    /// `AnalyzeRule.ajax` 的覆写会把 `ruleData = book` 传进 AnalyzeUrl ——
    /// url 里的 `{{...}}` / 变量在那条路上才解得出值。存的是**建宿主时**的快照
    /// (求值中途 `java.put` 改的变量不回灌;语料里没有这种写法)。
    pub rule_data: RuleData,
    /// 嵌套 `AnalyzeUrl`(`ajax`/`connect` 拿 url 串再建一个)里 `<js>`/`@js:`
    /// 的求值宿主。环境必须与外层同一份(cache / cookie 两张表在真身是进程级
    /// 单例,差分侧还要确定性垫片、产品侧要中断钩子)—— 都由工厂闭包捕获。
    pub nested_factory: Box<dyn Fn() -> QuickJsHost>,
    /// `AnalyzeUrl` 构造(initUrl)错误 → [`NetError`]。
    /// 差分侧把 `UrlError::Js` 归一成裁判的 `«host:ScriptException»`,产品侧原样。
    pub construct_error: fn(net::UrlError) -> NetError,
    /// hops 观察面(差分用;产品侧 `None` 走 [`NetProvider::hops`] 的缺省空表)
    pub hops_fn: Option<Box<dyn Fn() -> Vec<(String, String)>>>,
    /// `{"webView": true}` 那条路的平台原语工厂(每次调用要一个新的)。
    /// `None` = 这一侧还没接 —— `ajax`/`connect` 在那一档报
    /// [`rubato_core::host::NET_UNSUPPORTED`],与从前那句「未接」标记同串。
    pub web_view: Option<Box<dyn Fn() -> Box<dyn WebViewHost>>>,
    /// 「请用户出手」那三件(`startBrowser` / `startBrowserAwait` /
    /// `getVerificationCode`)的策略层 —— 真身 `SourceVerificationHelp`。
    /// **两侧共用同一份**(`net::verification`),不同的只是界面那一头
    /// ([`net::verification::VerifyUi`]:产品是真界面,差分是剧本)。
    /// `None` = 这一侧没接界面,三件都报「未接」。
    pub verify: Option<std::sync::Arc<net::verification::Verification>>,
    /// 书源名(`getSource()?.getTag()`)与类型(`getSourceType()`)——
    /// 弹界面时要往上写,别处用不着
    pub source_name: String,
    pub source_type: i32,
    /// 用户点停了吗(等用户过盾可以等很久,不给它就退不出来)
    pub cancel: Option<rubato_core::host::CancelFn>,
}

/// 嵌套宿主:自己记的 Debug 日志汇到共享槽(真身落的是全局 `Debug`,
/// 外层看得见),由 [`NetProvider::take_logs`] 交回外层。
struct NestedHost {
    inner: QuickJsHost,
    sink: Rc<RefCell<Vec<String>>>,
}

impl HostEnv for NestedHost {
    fn eval_js(
        &mut self,
        code: &str,
        b: &JsBindings,
        env: &mut dyn rubato_core::host::JsRuleEnv,
    ) -> Result<JsValue, String> {
        self.inner.eval_js(code, b, env)
    }

    fn web_js(&mut self, req: &rubato_core::host::WebJsRequest<'_>) -> Result<String, String> {
        self.inner.web_js(req)
    }

    fn log(&mut self, msg: &str) {
        self.sink.borrow_mut().push(msg.to_string());
    }
}

pub struct JsNet<T: HttpTransport, C: CookieEnv> {
    io: JsNetIo<T, C>,
    /// 嵌套 `AnalyzeUrl` 落下的 Debug 日志(见 [`NetProvider::take_logs`])
    nested_logs: Rc<RefCell<Vec<String>>>,
}

impl<T: HttpTransport, C: CookieEnv> JsNet<T, C> {
    pub fn new(io: JsNetIo<T, C>) -> JsNet<T, C> {
        JsNet { io, nested_logs: Rc::new(RefCell::new(Vec::new())) }
    }

    fn nested_host(&self) -> Box<dyn HostEnv> {
        Box::new(NestedHost { inner: (self.io.nested_factory)(), sink: self.nested_logs.clone() })
    }

    /// `AnalyzeUrl(urlStr, source = getSource()[, ruleData = book])`。
    /// `with_rule_data` 分开两个调用点:`JsExtensions.ajax`(JsExtensions.kt L140)
    /// **不带**,`AnalyzeRule.ajax` 的覆写(AnalyzeRule.kt L957)**带**。
    fn build(&self, url: &str, with_rule_data: bool) -> Result<AnalyzeUrl, NetError> {
        AnalyzeUrl::new(
            UrlArgs {
                m_url: url,
                source_key: Some(&self.io.source_key),
                source_header: self.io.source_header.clone(),
                enabled_cookie_jar: self.io.enabled_cookie_jar,
                ..Default::default()
            },
            self.nested_host(),
            if with_rule_data { self.io.rule_data.clone() } else { RuleData::default() },
        )
        .map_err(self.io.construct_error)
    }

    fn run(&mut self, au: &mut AnalyzeUrl) -> Result<net::fetch::StrResponse, NetError> {
        // webView 那条路每次要一个新的平台原语(理由同 pipeline::web_book::fetch)
        let mut wv = self.io.web_view.as_ref().map(|f| f());
        au.get_str_response(
            &self.io.transport,
            &mut self.io.cookies,
            None,
            None,
            true,
            wv.as_deref_mut(),
        )
        .map_err(|e| NetError::Request(e.to_string()))
    }

    /// `WebViewModel.initData` + `WebViewActivity` 那三行:**可见浏览器到底
    /// 怎么加载这一页**。真身在界面层做(Activity 拿得到书源与 okhttp),
    /// 我们的界面在 Dart —— 于是在这里算完,随 [`VerifyOpen`] 一起交过去。
    ///
    /// 逐条对齐真身:
    /// - `AnalyzeUrl(url, source = source)` 拆开 `url,{…}` → `baseUrl` 与 `headerMap`;
    /// - `headerMap.toWebViewRequestConfig(AppConfig.userAgent)` 把 **UA 单拎出来**
    ///   (它属于 WebSettings,跳转与子资源才会一致),`CookieJar` / `proxy`
    ///   两个内部选项**不许发给网站**;
    /// - `isPost()` 时先由 okhttp 那条路(`useWebView = false`)取回 HTML,
    ///   **覆盖**入参那份;body 为空则退回「直接加载地址」那一支
    ///   (真身 `if (html.isNullOrEmpty()) loadUrl(...)`)。
    ///
    /// UA 这一位是**过盾的关键**:Cloudflare 的 clearance 认 UA,可见窗口若用
    /// 系统 WebView 的缺省 UA,用户点过之后书源再 `java.ajax`,会被当成另一位
    /// 客户端重新拦下。
    ///
    /// **出错交回 `None`**:真身 `initData` 的 `execute {}` 把异常收进
    /// `onError`(弹个 toast),界面照开、书源那一侧毫不知情 —— 所以这里不能
    /// 把它抛成 JS 异常。
    ///
    /// 未照搬:真身给「入参 html」那一支注入 `JS_INJECTION2`(页面里的
    /// `java.*`),`WebJsExtensions` 整族在砍单里。
    fn browser_load(
        &mut self,
        url: &str,
        supplied_html: Option<&str>,
    ) -> Option<net::verification::BrowserLoad> {
        let mut au = self.build(url, false).ok()?;
        let request_url = au.url.clone();
        let (user_agent, headers) =
            net::webview::to_web_view_request_config(&au.header_map, net::client::user_agent());
        let mut html = supplied_html.map(str::to_string);
        if au.is_post() {
            html = au
                .get_str_response(&self.io.transport, &mut self.io.cookies, None, None, false, None)
                .ok()?
                .body;
        }
        Some(net::verification::BrowserLoad {
            url: request_url,
            headers,
            user_agent,
            html: html.filter(|s| !s.is_empty()),
        })
    }
}

/// 「请用户出手」那三件的错误 → JS 里抛的串。**带上真身的异常类别**
/// (`«host:X»` 是差分侧认的标记),后面缀一句人看得懂的话。
fn verify_err(e: net::verification::VerifyError) -> NetError {
    NetError::Request(format!("{} {e}", rubato_core::host::host_tag(e.class())))
}

impl<T: HttpTransport, C: CookieEnv> JsNet<T, C> {
    /// 书源那三位,交给 `SourceVerificationHelp`(真身 `BaseSource` 的三个 getter)
    fn source_ref(&self) -> net::verification::SourceRef<'_> {
        net::verification::SourceRef {
            key: &self.io.source_key,
            name: &self.io.source_name,
            kind: self.io.source_type,
        }
    }
}

impl<T: HttpTransport, C: CookieEnv> NetProvider for JsNet<T, C> {
    fn take_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut *self.nested_logs.borrow_mut())
    }

    fn ajax(
        &mut self,
        url: &str,
        _call_timeout: Option<i64>,
        with_rule_data: bool,
    ) -> Result<String, NetError> {
        let mut au = self.build(url, with_rule_data)?;
        // `{"webView": true}` 走真策略(`net::webview`);这一侧没接平台原语时
        // 才退回「未接」标记 —— 裁判侧 `BackstageWebView` 的垫片抛同一个串
        if au.use_web_view && self.io.web_view.is_none() {
            return Err(NetError::Request(rubato_core::host::NET_UNSUPPORTED.into()));
        }
        let r = self.run(&mut au)?;
        Ok(r.body.unwrap_or_default())
    }

    /// `JsExtensions.webView` / `webViewGetSource` / `webViewGetOverrideUrl`
    /// (JsExtensions.kt L245-328)。三个入口只差哪一个正则非空,故合成一个。
    ///
    /// 书源那半边由本宿主补:`headerMap = getSource()?.getHeaderMap(true)`
    /// (= 建宿主时算好的那份)、`tag = getSource()?.getKey()`。
    /// 失败**不吞**:真身那三处都没有 runCatching,异常直接穿到 JS。
    fn web_view(
        &mut self,
        req: &rubato_core::host::WebViewReq<'_>,
    ) -> Result<Option<String>, NetError> {
        let Some(factory) = self.io.web_view.as_ref() else {
            return Err(NetError::Request(rubato_core::host::NET_UNSUPPORTED.into()));
        };
        let mut host = factory();
        let bwv = net::webview::BackstageWebView {
            url: req.url.map(str::to_string),
            html: req.html.map(str::to_string),
            tag: Some(self.io.source_key.clone()),
            header_map: self.io.source_header.clone().unwrap_or_default(),
            source_regex: req.source_regex.map(str::to_string),
            override_url_regex: req.override_url_regex.map(str::to_string),
            java_script: req.js.map(str::to_string),
            delay_time: req.delay_time,
            cache_first: req.cache_first,
            ..Default::default()
        };
        let mut sink = net::webview::CookieStoreSink(&mut self.io.cookies);
        let res = bwv
            .get_str_response(host.as_mut(), &mut sink)
            // webView 那半边报的错**穿到 JS**。串按差分约定加壳,由 runner 归一成
            // 裁判的异常类别(见 difftest::js_case_runner 的 tag)
            .map_err(|e| NetError::Request(format!("«webview:{e}»")))?;
        Ok(res.body)
    }

    /// `AnalyzeRule.getWebJsResult` 的网络那半边(AnalyzeRule.kt L179-196)。
    ///
    /// AnalyzeRule 给的三位在 `req` 里;**书源那半边**由本宿主补:
    /// `headerMap = getSource()?.getHeaderMap(true)`(= 建宿主时算好的那份)、
    /// `tag = getSource()?.getKey()`。余下的常量逐字对齐真身:
    /// `cacheFirst = true` / `timeout = 10000` / `isRule = true`。
    fn web_js(&mut self, req: &rubato_core::host::WebJsRequest<'_>) -> Result<String, NetError> {
        let Some(factory) = self.io.web_view.as_ref() else {
            return Err(NetError::Request(rubato_core::host::NET_UNSUPPORTED.into()));
        };
        let mut host = factory();
        let bwv = net::webview::BackstageWebView {
            url: req.base_url.map(str::to_string),
            html: Some(req.content.to_string()),
            tag: Some(self.io.source_key.clone()),
            header_map: self.io.source_header.clone().unwrap_or_default(),
            java_script: Some(req.js.to_string()),
            cache_first: true,
            timeout: Some(10_000),
            result: Some(req.result_json.to_string()),
            is_rule: true,
            ..Default::default()
        };
        let mut sink = net::webview::CookieStoreSink(&mut self.io.cookies);
        let res = bwv
            .get_str_response(host.as_mut(), &mut sink)
            .map_err(|e| NetError::Request(e.to_string()))?;
        // `.getStrResponse().body.toString()`:body 为 null 时是字符串 "null"
        Ok(res.body.unwrap_or_else(|| "null".to_string()))
    }

    fn ajax_all(&mut self, urls: &[String]) -> Result<Vec<NetResponse>, NetError> {
        let mut out = Vec::with_capacity(urls.len());
        for url in urls {
            // 与 `ajax` 同一条 `AnalyzeUrl` 路,但**不带 ruleData**
            // (JsExtensions.kt L157 建的是 `AnalyzeUrl(url, source = getSource())`)
            let mut au = self.build(url, false)?;
            if au.use_web_view {
                return Err(NetError::Request(rubato_core::host::NET_UNSUPPORTED.into()));
            }
            let r = self.run(&mut au)?;
            out.push(NetResponse {
                url: r.url,
                body: r.body,
                code: r.status as i32,
                headers: Vec::new(),
                cookies: Vec::new(),
            });
        }
        Ok(out)
    }

    /// `JsExtensions.startBrowser`:弹出来就走
    fn start_browser(
        &mut self,
        url: &str,
        title: &str,
        html: Option<&str>,
    ) -> Result<(), NetError> {
        let Some(v) = self.io.verify.clone().filter(|v| v.ui_available()) else {
            // 没接界面(Dart 侧还没登记)= 「未接」那一档 —— 不能让书源
            // 等一个永远不来的答复
            return Err(NetError::Request(rubato_core::host::NET_UNSUPPORTED.into()));
        };
        let load = self.browser_load(url, html);
        v.start_browser(&self.source_ref(), url, title, html, load).map_err(verify_err)
    }

    /// `JsExtensions.startBrowserAwait`:弹浏览器、等用户过盾。
    /// **两种结局**(真身 `when (val result = getVerificationResult(...))`):
    /// 用户把页面源码带回来了,或者别人刚过完同一个盾 —— 后者自己重抓一次
    /// (`AnalyzeUrl(url).getStrResponse(useWebView = false)`)。
    fn start_browser_await(
        &mut self,
        url: &str,
        title: &str,
        refetch_after_success: bool,
        html: Option<&str>,
    ) -> Result<NetResponse, NetError> {
        let Some(v) = self.io.verify.clone().filter(|v| v.ui_available()) else {
            // 没接界面(Dart 侧还没登记)= 「未接」那一档 —— 不能让书源
            // 等一个永远不来的答复
            return Err(NetError::Request(rubato_core::host::NET_UNSUPPORTED.into()));
        };
        let load = self.browser_load(url, html);
        let cancel = self.io.cancel.clone();
        let out = v
            .get_verification_result(
                &self.source_ref(),
                url,
                title,
                true,
                refetch_after_success,
                html,
                load,
                cancel.as_ref(),
            )
            .map_err(verify_err)?;
        match out {
            // `StrResponse(url2.ifEmpty { url }, body)`
            net::verification::VerifyOutcome::Response(u, body) => Ok(NetResponse {
                url: if u.is_empty() { url.to_string() } else { u },
                body: Some(body),
                ..Default::default()
            }),
            net::verification::VerifyOutcome::Refetch => {
                // **不走 webView**(真身 `getStrResponse(useWebView = false)`)
                let mut au = self.build(url, false)?;
                au.use_web_view = false;
                let r = self.run(&mut au)?;
                Ok(NetResponse { url: r.url, body: r.body, ..Default::default() })
            }
        }
    }

    /// `JsExtensions.getVerificationCode`:弹图片验证码,等用户把字打进去。
    /// 真身 `check(result is Response)` —— 归并那一档在这里进不来
    /// (`canJoinVerificationFlight` 要求 useBrowser)
    fn get_verification_code(&mut self, image_url: &str) -> Result<String, NetError> {
        let Some(v) = self.io.verify.clone().filter(|v| v.ui_available()) else {
            // 没接界面(Dart 侧还没登记)= 「未接」那一档 —— 不能让书源
            // 等一个永远不来的答复
            return Err(NetError::Request(rubato_core::host::NET_UNSUPPORTED.into()));
        };
        let cancel = self.io.cancel.clone();
        let out = v
            .get_verification_result(
                &self.source_ref(),
                image_url,
                "",
                false,
                true,
                None,
                None,
                cancel.as_ref(),
            )
            .map_err(verify_err)?;
        match out {
            net::verification::VerifyOutcome::Response(_, r) => Ok(r),
            net::verification::VerifyOutcome::Refetch => {
                Err(NetError::Request("verification result is not a response".into()))
            }
        }
    }

    fn connect(
        &mut self,
        url: &str,
        header_json: Option<&str>,
        _call_timeout: Option<i64>,
    ) -> Result<NetResponse, NetError> {
        // `connect(url, header)`:header 是 JSON 串,真身走 GSON.fromJsonObject,
        // 解不出来给 null(不是报错)
        let header_map = header_json.and_then(parse_header_json);
        let mut au = AnalyzeUrl::new(
            UrlArgs {
                m_url: url,
                source_key: Some(&self.io.source_key),
                header_map_f: header_map,
                source_header: self.io.source_header.clone(),
                enabled_cookie_jar: self.io.enabled_cookie_jar,
                ..Default::default()
            },
            self.nested_host(),
            RuleData::default(),
        )
        .map_err(self.io.construct_error)?;
        let r = self.run(&mut au)?;
        Ok(NetResponse {
            url: r.url,
            body: r.body,
            code: r.status as i32,
            headers: Vec::new(),
            cookies: Vec::new(),
        })
    }

    fn jsoup(
        &mut self,
        method: &str,
        url: &str,
        headers_json: Option<&str>,
        body: Option<&str>,
    ) -> Result<NetResponse, NetError> {
        // 真身 `parseJsRequestHeaders`:JS 对象 → Map<String,String>
        let mut headers: Vec<(String, String)> =
            headers_json.and_then(parse_header_json).unwrap_or_default();
        if self.io.enabled_cookie_jar {
            headers.push(("CookieJar".to_string(), "1".to_string()));
        }
        let req_body =
            body.map(|b| RequestBody { bytes: b.as_bytes().to_vec(), content_type: None });
        // jsoup 那条路**不跟进重定向**、**不过 okhttp 的拦截器链**,直接打一跳
        // (裁判侧与 ReplayHttp.executeRaw 同形)
        let raw = self
            .io
            .transport
            .execute_hop(method, url, &headers, req_body.as_ref())
            .map_err(|e| NetError::Request(format!("{e:?}")))?;
        let mut cookies = Vec::new();
        for (k, v) in &raw.headers {
            if k.eq_ignore_ascii_case("set-cookie") {
                // jsoup 的 `cookies()`:取 `name=value` 的头一段
                let first = v.split(';').next().unwrap_or("");
                if let Some((n, val)) = first.split_once('=') {
                    cookies.push((n.trim().to_string(), val.trim().to_string()));
                }
            }
        }
        Ok(NetResponse {
            url: url.to_string(),
            body: Some(String::from_utf8_lossy(&raw.body).to_string()),
            code: raw.status as i32,
            headers: raw.headers.clone(),
            cookies,
        })
    }

    fn hops(&self) -> Vec<(String, String)> {
        match &self.io.hops_fn {
            Some(f) => f(),
            None => Vec::new(),
        }
    }
}

/// JS 对象 / GSON 对象 → `Vec<(名, 值)>`。非字符串值按 `toString` 铺平,
/// 与 `parseJsRequestHeaders` 同口径。
pub fn parse_header_json(h: &str) -> Option<Vec<(String, String)>> {
    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(h).ok().map(|m| {
        m.into_iter()
            .map(|(k, v)| (k, v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())))
            .collect()
    })
}

/// `BaseSource.getHeaderMap(hasLoginHeader = true)`:header 规则(可能是 JS,
/// 跑在**第三个宿主** `BaseSource.evalJS` 上)+ 默认 UA 注入 + **登录头合并**。
/// 登录头从 CacheManager 的 `loginHeader_<key>` 来,故 `host.cache` 必须先
/// seed 好。产品侧(`engine::shared::JsEnv`)与差分侧(difftest 的
/// pipeline_case)从前各抄一份逐行相同的实现,收成这一处;宿主的建法
/// (`with_config` 还是 `for_difftest`、cookie 表接哪张)仍由调用方定。
///
/// 算 header 的这个宿主**不给网络面**:header 规则里再发请求会绕回还没建好的
/// 环境;真身那里也是先有 header 再有请求。
pub fn source_header_for(
    host: &mut QuickJsHost,
    source_key: &str,
    source_name: &str,
    header_rule: Option<&str>,
) -> Vec<(String, String)> {
    let login_header: Option<Vec<(String, String)>> = host
        .cache
        .get_str(&format!("loginHeader_{source_key}"))
        .and_then(|s| parse_header_json(&s));
    let binding = rubato_core::host::SourceBinding {
        key: source_key.to_string(),
        tag: source_name.to_string(),
        raw: None,
    };
    let mut vars = RuleData::default();
    net::source_header::source_header_map(
        header_rule,
        net::client::user_agent(),
        login_header.as_deref(),
        host,
        &binding,
        &mut vars,
    )
}
