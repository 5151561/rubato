//! 剧本 WebView(差分侧的 [`WebViewHost`])。
//!
//! 平台那一半录不下来,于是**换成一份剧本**:case 里写清「加载这一页会依次
//! 发生什么」,两侧照着放事件。契约与字段名见 `fixtures/cases/webview/README.md`
//! ——裁判侧的同一份在 `judge/*/shims/…WebkitShims.kt`(剧本 WebView)
//! 与 `Stage.kt`(剧本本体)。
//!
//! 用它的不止 webview 那一套:`{"webView": true}` 的书源经 `net::fetch` 走
//! `BackstageWebView` 的策略层,fetch / rule-engine / pipeline 那几套的 case
//! 因此也可以带一个 `webview` 剧本(**不带就是缺省剧本**:页面加载得完、
//! 每次求值都回 `"null"` —— 于是重试梯子跑满、报「js执行超时」。
//! 那正是「这一页录不下来」的诚实结局,不是假装取到了内容)。

use rubato_core::host::{WebViewEvent, WebViewHost, WebViewLoad, WebViewSettings};
use serde_json::Value;

/// 一次加载的剧本(case 里的 `webview` 对象)
#[derive(Debug, Clone, Default)]
pub struct WvScript {
    pub override_urls: Vec<(String, bool)>,
    pub load_resources: Vec<String>,
    /// 这一次加载**会不会**完成(缺省会)。false = 一直加载不完,只能等超时
    pub page_finished: bool,
    pub page_finished_url: Option<String>,
    /// **后来的**加载完成:`(第几毫秒, 那张页的地址)`。跳转站(`Redirecting…`)
    /// 就是这个形状 —— 真身每来一次 `onPageFinished` 都 `removeCallbacks` 掉
    /// 还没跑的求值、重排 `100 + delayTime`,所以取到的是**最后一次**之后的页面
    pub later_page_finished: Vec<(i64, Option<String>)>,
    /// 每一次求值的回值(**JS 侧的原样串**);用完之后一直重复最后一个,
    /// 空表一律当 `"null"`
    pub evals: Vec<String>,
    pub cookie: Option<String>,
}

impl WvScript {
    /// 从 case 的 `webview` 字段解析;字段缺席 = 缺省剧本
    pub fn parse(case: &Value) -> WvScript {
        let o = case.get("webview");
        let get = |k: &str| o.and_then(|s| s.get(k));
        let arr = |k: &str| -> Vec<Value> {
            get(k).and_then(Value::as_array).cloned().unwrap_or_default()
        };
        let strs = |k: &str| -> Vec<String> {
            arr(k).iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
        };
        let text = |k: &str| get(k).and_then(Value::as_str).map(str::to_string);
        WvScript {
            override_urls: arr("overrideUrls")
                .iter()
                .map(|e| {
                    (
                        e.get("url").and_then(Value::as_str).unwrap_or_default().to_string(),
                        e.get("isRedirect").and_then(Value::as_bool).unwrap_or(false),
                    )
                })
                .collect(),
            load_resources: strs("loadResources"),
            page_finished: get("pageFinished").and_then(Value::as_bool).unwrap_or(true),
            page_finished_url: text("pageFinishedUrl"),
            later_page_finished: arr("laterPageFinished")
                .iter()
                .map(|e| {
                    (
                        e.get("at").and_then(Value::as_i64).unwrap_or(0),
                        e.get("url").and_then(Value::as_str).map(str::to_string),
                    )
                })
                .collect(),
            evals: strs("evals"),
            cookie: text("cookie"),
        }
    }
}

/// 剧本 WebView:按剧本回放事件、按剧本回求值,并把痕迹记下来。
///
/// **时间**在被测侧是策略层自己推的(`net::webview` 每次 `eval` 前把时刻交过来),
/// 这里只负责记 —— 裁判那边同一条时间轴由虚拟时钟给出。
pub struct ScriptedHost {
    script: WvScript,
    event_idx: usize,
    eval_idx: usize,
    // ---- 痕迹 ----
    pub settings: Option<WebViewSettings>,
    pub load: Option<WebViewLoad>,
    pub eval_log: Vec<(i64, String)>,
    pub released: u32,
    in_use: bool,
    now: i64,
}

impl ScriptedHost {
    pub fn new(script: WvScript) -> ScriptedHost {
        ScriptedHost {
            script,
            event_idx: 0,
            eval_idx: 0,
            settings: None,
            load: None,
            eval_log: Vec::new(),
            released: 0,
            in_use: false,
            now: 0,
        }
    }

    /// 这一次加载的**请求地址**:`loadUrl` 那一支就是它自己;
    /// `loadDataWithBaseURL(baseUrl = null, …)` 那一支是 `about:blank`
    pub fn request_url(l: &WebViewLoad) -> String {
        l.url.clone().unwrap_or_else(|| "about:blank".to_string())
    }

    /// 事件顺序照真 WebView:① `shouldOverrideUrlLoading`* ② `onLoadResource`*
    /// ③ `onPageFinished`(地址缺省 = 请求地址)④ `laterPageFinished` 各按自己的时刻。
    /// 前三档都在第 0 毫秒 —— 真身那边 `loadUrl` 之后的回调是同一拍里下来的。
    fn peek_event(&self) -> Option<(i64, WebViewEvent)> {
        let i = self.event_idx;
        let overrides = self.script.override_urls.len();
        let resources = self.script.load_resources.len();
        let finished = usize::from(self.script.page_finished);
        let request_url = || self.load.as_ref().map(ScriptedHost::request_url).unwrap_or_default();
        if i < overrides {
            let (url, is_redirect) = self.script.override_urls[i].clone();
            return Some((0, WebViewEvent::OverrideUrl { url, is_redirect }));
        }
        if i < overrides + resources {
            let url = self.script.load_resources[i - overrides].clone();
            return Some((0, WebViewEvent::LoadResource { url }));
        }
        if i == overrides + resources && self.script.page_finished {
            let url = self.script.page_finished_url.clone().unwrap_or_else(request_url);
            return Some((0, WebViewEvent::PageFinished { url }));
        }
        // 页面一次都没加载完的剧本里,后来的那些也送不出来(真身:没加载就没有回调)
        if !self.script.page_finished {
            return None;
        }
        let j = i - overrides - resources - finished;
        let (at, url) = self.script.later_page_finished.get(j)?.clone();
        Some((at, WebViewEvent::PageFinished { url: url.unwrap_or_else(request_url) }))
    }

    /// 剧本里第 N 条回复;**用完之后一直重复最后一条**(与裁判侧 `Stage.nextEval` 同)
    fn reply(&mut self) -> String {
        let evals = &self.script.evals;
        let r = evals.get(self.eval_idx).or_else(|| evals.last()).cloned();
        self.eval_idx += 1;
        r.unwrap_or_else(|| "null".to_string())
    }
}

impl WebViewHost for ScriptedHost {
    fn create(&mut self, settings: &WebViewSettings) {
        self.settings = Some(settings.clone());
        self.in_use = true;
    }

    fn load(&mut self, load: &WebViewLoad) {
        self.load = Some(load.clone());
    }

    /// 剧本的时间轴:头一串事件都在第 0 毫秒(`loadUrl` 之后那一拍),
    /// `laterPageFinished` 各在自己那一刻。**到点之前有事件就先交事件**。
    fn next_event_until(&mut self, until_ms: i64) -> Option<(i64, WebViewEvent)> {
        let Some((at, ev)) = self.peek_event() else {
            self.now = until_ms;
            return None;
        };
        if at > until_ms {
            self.now = until_ms;
            return None;
        }
        self.event_idx += 1;
        self.now = at;
        Some((at, ev))
    }

    fn eval(&mut self, js: &str) -> String {
        let reply = self.reply();
        self.eval_log.push((self.now, js.to_string()));
        reply
    }

    fn eval_void(&mut self, js: &str) {
        // 裁判侧 `evaluateJavascript(js, null)` 与 `loadUrl("javascript:…")`
        // 都走同一个记账口(`Stage.nextEval`)——**回复也照样消耗一条**
        let _ = self.reply();
        self.eval_log.push((self.now, js.to_string()));
    }

    fn page_cookie(&mut self, _url: &str) -> Option<String> {
        self.script.cookie.clone()
    }

    fn destroy(&mut self) {
        if self.in_use {
            self.in_use = false;
            self.released += 1;
        }
    }
}
