//! 引擎内部的共享件:取消令牌、跨线程 cookie 视图、JS 宿主装配。

use js_host::host_env::{QuickJsHost, SharedMap};
use js_host::java_api::HostConfig;
use rubato_core::RuleData;
use rubato_core::entities::BookSource;
use rubato_core::host::{
    BoundValue, CookieEnv, HostEnv, JavaValue, JsBindings, JsHost, JsValue, SetCookie,
    SourceBinding, WebViewProvider,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use store::CookieStore;

/// 任务取消。FRB 没有自动取消(计划书 §1),句柄由上层显式持有。
///
/// **粒度**:流水线内部的一次 HTTP 调用不可中断,取消只在**步与步之间**、
/// 以及并发搜索的**源与源之间**生效 —— 但 JS 求值是个例外:令牌接到了
/// QuickJS 的中断钩子上(见 [`JsEnv::host`]),死循环的书源当场就停。
#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// 包成 [`rubato_core::host::CancelFn`] 交给平台侧(webView 那条路要跨线程问)
    pub fn as_fn(&self) -> rubato_core::host::CancelFn {
        let flag = self.0.clone();
        std::sync::Arc::new(move || flag.load(Ordering::SeqCst))
    }
}

/// 多线程搜索时的 cookie 视图:每次调用短暂持锁,
/// 语义与独占 [`CookieStore`] 一致(CookieStore 自身不是 Sync)。
pub struct SharedCookies(pub Arc<Mutex<CookieStore>>);

impl CookieEnv for SharedCookies {
    fn get_cookie(&mut self, url: &str) -> String {
        self.0.lock().expect("cookie 锁").get_cookie(url)
    }

    fn load_request_cookie(&mut self, url: &str, request_cookie: Option<&str>) -> Option<String> {
        self.0.lock().expect("cookie 锁").load_request_cookie(url, request_cookie)
    }

    fn save_response_cookies(&mut self, url: &str, cookies: &[SetCookie]) {
        self.0.lock().expect("cookie 锁").save_response_cookies(url, cookies)
    }

    fn set_cookie(&mut self, url: &str, cookie: &str) {
        CookieEnv::set_cookie(&mut *self.0.lock().expect("cookie 锁"), url, cookie)
    }

    fn replace_cookie(&mut self, url: &str, cookie: &str) {
        CookieEnv::replace_cookie(&mut *self.0.lock().expect("cookie 锁"), url, cookie)
    }

    fn remove_cookie(&mut self, url: &str) {
        CookieEnv::remove_cookie(&mut *self.0.lock().expect("cookie 锁"), url)
    }

    fn get_cookie_key(&mut self, url: &str, key: &str) -> String {
        CookieEnv::get_cookie_key(&mut *self.0.lock().expect("cookie 锁"), url, key)
    }
}

/// `java.getWebViewUA()`。真身读 `WebSettings.getDefaultUserAgent`,桌面没有
/// WebView —— 给一个**真实存在**的形态,而不是空串:书源会把它当请求头发出去,
/// 空串比假串更容易被站点判成爬虫。
pub const WEB_VIEW_UA: &str = "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";

/// `cache.putFile/getFile` 落 `caches` 表时的键前缀。真身那里这张表是
/// ACache(另一个目录),这边两张表同住一处,靠它把命名空间分开 ——
/// 不分就等于把 `put/get` 与 `putFile/getFile` 又合成了一张。
const ACACHE_PREFIX: &str = "acache_";

/// **产品侧 JS 宿主的装配**(差分侧是 difftest 的 StubHost / js_case_runner,
/// 两者永不互换)。
///
/// 一次引擎调用建一个,内部的十来个 `AnalyzeRule` 由它出宿主。三件事在这里
/// 才成立,从前都是空的:
///
/// 1. **网络面** —— `java.ajax` / `connect` / `get`/`head`/`post` 真的发请求
///    (见 [`crate::js_net::live_net`])。从前 `net: None`,`java.ajax` 返回字面串
///    `«net-unsupported»`,不报错不落日志,会被当正文拼进去;
/// 2. **环境常量** —— `androidId`(一装一个,落库复用)与 `getWebViewUA`。
///    从前全是空串:登录信息那条的 AES 密钥长度为 0,`putLoginInfo` 恒 false、
///    `getLoginInfo` 恒 null,静默失效;
/// 3. **cache / cookie 共享** —— 真身那两个是进程级单例,而这边一条规则一个宿主。
///    从前 `java.put` / `sourceVariable_*` / `loginHeader_*` 写完就随宿主一起丢。
///    现在同一次调用内的宿主共用同一份(跨调用的持久化由调用方 seed/flush,
///    见 [`JsEnv::cache`])。
pub struct JsEnv {
    config: HostConfig,
    /// CacheManager。**同一个 JsEnv 内的全部宿主共用一份**
    cache: SharedMap,
    /// `cache.putFile/getFile` 那张表。真身那里它是 **ACache**(落盘),
    /// 与 `put/get` 的 cacheDao 不是同一处 —— `put` 写的 `getFile` 读不到。
    /// 两张都落 store 的 `caches` 表,靠 [`ACACHE_PREFIX`] 分开命名空间。
    cache_file: SharedMap,
    /// 建环境时 seed 进 `cache` 的那份快照。收工时按它算差量
    /// (只写自己动过的键)—— 并发搜索时整段替换会互相抹掉,
    /// 见 [`JsEnv::cache_delta`] 与 `BookStore::update_cache_prefix`。
    seed: std::collections::HashMap<String, String>,
    /// 与 pipeline 共用的传输层(engine 开库时建的那一个,见
    /// [`crate::js_net::SharedTransport`])。曾经这里自建连接池 ——
    /// `HttpTransport` 还是 `&mut self` 时 pipeline 的那份借不出来。
    transport: crate::js_net::SharedTransport,
    cookies: Arc<Mutex<CookieStore>>,
    source_key: String,
    source_header: Option<Vec<(String, String)>>,
    enabled_cookie_jar: bool,
    cancel: CancelToken,
    /// webView 的平台面:`java.webView*` / `@webjs:` / `{"webView": true}` 的
    /// `java.ajax` 那三条路要它(见 [`crate::js_net::live_net`])
    web_view: Option<Arc<dyn WebViewProvider>>,
    /// 「请用户出手」那三件的策略层(真身 `SourceVerificationHelp`)。
    /// **整个引擎一份**:界面互斥与「同一个盾只弹一次」的归并都靠它跨调用生效
    verify: Option<Arc<net::verification::Verification>>,
    source_name: String,
    source_type: i32,
}

impl JsEnv {
    /// `seed` 是 CacheManager 的既有内容(调用方从 `caches` 表读出来的)。
    /// **必须在算 header 之前灌进去** —— `getLoginHeader()` 与 header 规则里的
    /// `java.get` 都从这张表读。
    pub fn new(
        source: &BookSource,
        cookies: Arc<Mutex<CookieStore>>,
        cancel: CancelToken,
        android_id: String,
        transport: crate::js_net::SharedTransport,
        seed: Vec<(String, String)>,
        web_view: Option<Arc<dyn WebViewProvider>>,
        verify: Option<Arc<net::verification::Verification>>,
    ) -> Result<JsEnv, String> {
        let config = HostConfig {
            android_id,
            web_view_ua: WEB_VIEW_UA.to_string(),
            // 产品侧当然是真随机 —— 定死的那条只在差分侧走
            random: js_host::java_api::RandomSource::System,
        };
        let cache = SharedMap::new();
        let cache_file = SharedMap::new();
        let mut snapshot = std::collections::HashMap::with_capacity(seed.len());
        for (k, v) in seed {
            // ACache 那张表与 CacheManager 那张同住 `caches`,靠前缀分开
            match k.strip_prefix(ACACHE_PREFIX) {
                Some(bare) => cache_file.insert(bare.to_string(), v.clone()),
                None => cache.insert(k.clone(), v.clone()),
            }
            snapshot.insert(k, v);
        }
        let mut env = JsEnv {
            config,
            cache,
            cache_file,
            seed: snapshot,
            transport,
            cookies,
            source_key: source.book_source_url.clone(),
            source_header: None,
            enabled_cookie_jar: source.enabled_cookie_jar == Some(true),
            cancel,
            web_view,
            verify,
            source_name: source.book_source_name.clone(),
            source_type: source.book_source_type,
        };
        env.source_header = Some(env.compute_source_header(source));
        Ok(env)
    }

    /// `BaseSource.getHeaderMap(hasLoginHeader = true)`:header 规则(可能是 JS,
    /// 跑在**第三个宿主**上)+ 默认 UA 注入 + **登录头合并**。
    ///
    /// 算一次、给这个源的全部 JS 网络调用用 —— 真身那里
    /// `AnalyzeUrl(url, source = getSource())` 也是从书源实体拿的。
    /// 登录头从 CacheManager 的 `loginHeader_<key>` 来(352 源有 loginUrl),
    /// 所以 `seed` 必须先灌好。
    fn compute_source_header(&self, source: &BookSource) -> Vec<(String, String)> {
        // 本体在 js-host(与差分侧共用一份,见 `net_face::source_header_for`);
        // 这里只搭宿主:产品配置 + 两张表。
        let mut host = QuickJsHost::with_config(self.config.clone());
        host.cache = self.cache.clone();
        host.cache_file = self.cache_file.clone();
        // `cookie` 绑定接**真 CookieStore**:书源 JS 写的 cookie 要进库、
        // 也要被后续请求带上(此前 js-host 自己攥一张表,写了等于没写)
        host.cookie_store = Some(Rc::new(RefCell::new(SharedCookies(self.cookies.clone()))));
        js_host::net_face::source_header_for(
            &mut host,
            &source.book_source_url,
            &source.book_source_name,
            source.header.as_deref(),
        )
    }

    /// 收工时该往 `caches` 表里写的**差量**:`(变过/新增的, 被 remove 掉的)`。
    /// (CacheManager 的表在建 JsEnv 时 seed 进来,`loginHeader_*` /
    /// `sourceVariable_*` / `userInfo_*` 是跨调用的,真身那边它们本来就落库。)
    ///
    /// 整段替换在并发搜索下会丢数据 —— 每个 worker 各自 seed、各自收工,
    /// 后收工的那个手里的快照根本没有别的源刚写下的键。只写自己动过的,
    /// 跨源就不会互相踩(单线程那三条路上语义完全不变)。
    pub fn cache_delta(&self) -> (Vec<(String, String)>, Vec<String>) {
        let mut now = self.cache.sorted_pairs();
        now.extend(
            self.cache_file
                .sorted_pairs()
                .into_iter()
                .map(|(k, v)| (format!("{ACACHE_PREFIX}{k}"), v)),
        );
        let upserts: Vec<(String, String)> =
            now.iter().filter(|(k, v)| self.seed.get(k) != Some(v)).cloned().collect();
        let live: std::collections::HashSet<&str> = now.iter().map(|(k, _)| k.as_str()).collect();
        let mut deletes: Vec<String> =
            self.seed.keys().filter(|k| !live.contains(k.as_str())).cloned().collect();
        deletes.sort();
        (upserts, deletes)
    }

    /// 登录页给 WebView 的请求头。它与四步抓取共用
    /// `BaseSource.getHeaderMap(hasLoginHeader = true)` 的结果:书源 header 规则、
    /// 默认 UA、既有登录头三层都已经合并。
    pub fn source_header(&self) -> Vec<(String, String)> {
        self.source_header.clone().unwrap_or_default()
    }

    /// `BaseSource.evalJS` 的产品入口。登录表单的渲染、按钮 action 与最终
    /// `login()` 都走这一个宿主,因此能继续使用同一份 cache/cookie/网络面，
    /// `java.startBrowserAwait` 也会落到 M3j/M3k 已接好的验证界面。
    pub fn eval_source_js(
        &self,
        source: &BookSource,
        code: &str,
        result: Option<&serde_json::Value>,
    ) -> Result<JsValue, String> {
        let mut data = RuleData::default();
        let mut host = self.host(&data);
        let result = result.map(|v| BoundValue::Java(login_java_value(v)));
        host.eval_js(
            code,
            &JsBindings {
                host: JsHost::Source,
                source_login: true,
                result,
                source: Some(SourceBinding {
                    key: source.book_source_url.clone(),
                    tag: source.book_source_name.clone(),
                    raw: source.raw.clone(),
                }),
                ..Default::default()
            },
            &mut data,
        )
    }

    /// 一个宿主。`data` 是这条规则的变量层 —— `java.ajax` 的 url 里
    /// `{{...}}` 靠它解值(`AnalyzeRule.ajax` 的覆写带 ruleData)。
    pub fn host(&self, data: &RuleData) -> Box<dyn HostEnv> {
        let mut h = QuickJsHost::with_config(self.config.clone());
        h.cache = self.cache.clone();
        h.cache_file = self.cache_file.clone();
        h.cookie_store = Some(Rc::new(RefCell::new(SharedCookies(self.cookies.clone()))));
        // 取消令牌接到 QuickJS 的中断钩子上:用户点「停」时,正在跑的那段
        // 书源 JS 当场停,不必等这一步做完
        let cancel = self.cancel.clone();
        h.cancel = Some(Rc::new(move || cancel.is_cancelled()));
        h.net = Some(Box::new(crate::js_net::live_net(
            self.transport.clone(),
            Box::new(SharedCookies(self.cookies.clone())),
            self.source_key.clone(),
            self.source_header.clone(),
            self.enabled_cookie_jar,
            data.clone(),
            // 嵌套宿主(ajax 的 url 里那段 `<js>`/`@js:`)与本宿主同一个环境:
            // 两张表 + 中断钩子都得跟着走,见 [`crate::js_net::NestedEnv`]
            crate::js_net::NestedEnv {
                config: self.config.clone(),
                cache: self.cache.clone(),
                cache_file: self.cache_file.clone(),
                cancel: h.cancel.clone(),
            },
            self.web_view.clone(),
            self.cancel.as_fn(),
            self.verify.clone(),
            self.source_name.clone(),
            self.source_type,
        )));
        Box::new(h)
    }
}

/// 登录对话框把 Kotlin `HashMap<String, String>` 绑到 `result`。这里保留
/// NativeJavaMap/NativeJavaList 的形态；表单值正常都是字符串，递归处理只是让
/// 动态 loginUi 的默认值或将来 V2 状态不必再开另一条绑定路径。
fn login_java_value(v: &serde_json::Value) -> JavaValue {
    use serde_json::Value;
    match v {
        Value::Null => JavaValue::Null,
        Value::Bool(v) => JavaValue::Bool(*v),
        Value::Number(v) => {
            let text = v.to_string();
            JavaValue::Num(v.as_f64().unwrap_or_default(), text)
        }
        Value::String(v) => JavaValue::Str(v.clone()),
        Value::Array(v) => JavaValue::List(
            v.iter().map(login_java_value).collect(),
            format!("[{}]", v.iter().map(java_value_text).collect::<Vec<_>>().join(", ")),
        ),
        Value::Object(v) => JavaValue::Map(
            v.iter().map(|(k, v)| (k.clone(), login_java_value(v))).collect(),
            format!(
                "{{{}}}",
                v.iter()
                    .map(|(k, v)| format!("{k}={}", java_value_text(v)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
    }
}

fn java_value_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".into(),
        serde_json::Value::String(v) => v.clone(),
        other => other.to_string(),
    }
}
