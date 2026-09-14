//! 引擎组装:把 store / net / pipeline / js-host 接成一个可被 FFI 调用的门面。
//!
//! 线程与锁的约定:
//! - `stores`(books/chapters/caches/book_sources)一把锁,每次调用短暂持有;
//! - `cookies` 一把锁,由 [`shared::SharedCookies`] 在每次 cookie 读写时短暂持有;
//! - HTTP 传输**全引擎一份**(`execute_hop` 是 `&self`,reqwest 的连接池
//!   自己是线程安全的),开库时建好,pipeline / 各 worker / JS 网络面共用;
//! - 取消是**步间粒度**,见 [`shared::CancelToken`]。
//!
//! 与差分的关系:这一层**不在差分面里**。四步的语义由 pipeline 套与
//! pipeline-corpus 套钉住(docs/plan.md §4),engine 只做装配、落库与并发。

pub mod js_net;
pub mod reader;
pub mod shared;

use net::live::LiveTransport;
use pipeline::{PipelineEnv, PipelineError};
use rubato_core::entities::{Book, BookChapter, BookSource, SearchBook};
use rubato_core::host::{WebViewHost, WebViewProvider};
use shared::{CancelToken, JsEnv, SharedCookies};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use store::book_store::now_ms;
use store::{BookRow, BookStore, CookieStore, HighlightRow, HighlightStore, SourceStore};

/// 书源登录页需要的全部数据。界面只负责渲染与收集输入；header 规则、登录信息
/// 解密、动态 loginUi 都已经在引擎里按 `BaseSource` 语义求值。
#[derive(Debug, Clone, PartialEq)]
pub struct SourceLoginSpec {
    pub source_url: String,
    pub source_name: String,
    /// `true` = `loginUi` 表单；`false` = 可见 WebView 登录。
    pub is_form: bool,
    pub login_url: Option<String>,
    /// RowUi JSON 数组。只有表单登录有值。
    pub rows_json: Option<String>,
    /// 已保存的登录表单 JSON；取不到时是 `{}`。
    pub stored_json: String,
    /// `getHeaderMap(true)` 的结果，给可见 WebView 使用。
    pub headers: Vec<(String, String)>,
}

/// 并发搜索的 worker 数(桌面/真机都够用,再多就是给站点添堵)。
/// 验收探针那种「一口气扫上百个源」的场合可以调高,见 [`Engine::open_with`]。
const SEARCH_THREADS: usize = 6;
/// 调也有个限:再多就是给站点添堵,也压不过单跳超时
const MAX_SEARCH_THREADS: usize = 32;
/// 单跳超时:搜索要在可接受时间内翻完一批源
const HTTP_TIMEOUT_SECS: u64 = 15;
/// 书源 JS 的 CacheManager 在 `caches` 表里的前缀(正文缓存走 `content:`)
const JS_CACHE_PREFIX: &str = "js:";
/// 设备级常量的前缀
const ENV_PREFIX: &str = "env:";

#[derive(Debug)]
pub enum EngineError {
    Db(String),
    NotFound(String),
    BadSource(String),
    Pipeline(String),
    /// 环境级故障(系统熵源之类),与存储/书源无关
    Env(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::Db(e) => write!(f, "存储错误: {e}"),
            EngineError::NotFound(e) => write!(f, "找不到: {e}"),
            EngineError::BadSource(e) => write!(f, "书源错误: {e}"),
            EngineError::Pipeline(e) => write!(f, "抓取失败: {e}"),
            EngineError::Env(e) => write!(f, "环境错误: {e}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<store::rusqlite::Error> for EngineError {
    fn from(e: store::rusqlite::Error) -> EngineError {
        EngineError::Db(e.to_string())
    }
}

fn pipeline_err(e: PipelineError) -> EngineError {
    EngineError::Pipeline(match &e {
        PipelineError::TocEmpty => "目录为空".to_string(),
        PipelineError::ContentEmpty => "正文为空".to_string(),
        PipelineError::Fetch(f) => format!("{f}"),
        PipelineError::Other(m) => m.clone(),
    })
}

struct Stores {
    books: BookStore,
    sources: SourceStore,
}

/// `java.androidId()`。真身读 `Settings.Secure.ANDROID_ID` —— 一台设备一个、
/// 装了就不变。这边**首次开库时生成 16 个十六进制字符并落库**:
/// - 不能是空串:`putLoginInfo` 拿它前 16 字节当 AES 密钥,空串 → 密钥长度 0
///   → 恒 false、`getLoginInfo` 恒 null,静默失效;
/// - 不能每次现算:书源拿它当设备号/会话号拼签名,变一次就是换一台设备。
///
/// **必须在 `Engine::open` 里定死,不能等到用时现查**:并发搜索的 6 个线程会
/// 同时走「查不到就生成 + INSERT OR REPLACE」这条路,各自拿到不同的 id,
/// 最后落库的那个赢。此时若某个源已经用先前那个 id `putLoginInfo`,
/// 密文就再也解不开了 —— 而那条路是**静默吞异常给 null** 的,
/// 用户只看到「登录状态莫名其妙没了」。只影响首次运行,但代价不可逆。
fn resolve_android_id(books: &mut BookStore) -> Result<String, EngineError> {
    let key = format!("{ENV_PREFIX}androidId");
    if let Some(v) = books.get_cache(&key)? {
        if !v.is_empty() {
            return Ok(v);
        }
    }
    let b = js_host::java_api::RandomSource::System.bytes_at(0).map_err(EngineError::Env)?;
    let id: String = b[..8].iter().map(|x| format!("{x:02x}")).collect();
    books.put_cache(&key, &id)?;
    Ok(id)
}

pub struct Engine {
    stores: Mutex<Stores>,
    cookies: Arc<Mutex<CookieStore>>,
    /// 全引擎一份的 HTTP 传输(连接池)。曾经是每源、每 worker 现造一个
    /// `LiveTransport` —— reqwest 的 blocking Client 每个实例自带一条后台线程
    /// 和一个 tokio runtime,搜 500 源就是 500 次起停、连接池零复用。
    transport: js_net::SharedTransport,
    /// `java.androidId()`。**开库时就定死**,见 [`resolve_android_id`]
    android_id: String,
    /// 并发搜索的 worker 数(见 [`SEARCH_THREADS`])
    search_threads: usize,
    /// webView 的平台面(PlatformHooks)。`None` = 这一份引擎根本没有平台
    /// (库里跑的测试、命令行探针);**有也不等于要得到** ——
    /// Dart 侧登记之前 `available()` 是假的,见 [`Engine::web_view`]
    web_view: Option<Arc<dyn WebViewProvider>>,
    /// 「请用户出手」那三件的策略层(真身 `SourceVerificationHelp`)。
    /// **整个引擎一份** —— 界面互斥与「同一个盾只弹一次」的归并要跨调用生效。
    /// `None` = 没有界面那一头(库里跑的测试、命令行探针),三件都报「未接」
    verify: Option<Arc<net::verification::Verification>>,
    /// 发现页分类列表的**进程内缓存**(真身 `exploreKindsMap`)。
    /// 键与真身同源(书源地址 + `exploreUrl`)—— 改了发现规则自动失效。
    /// 上界就是书源数,不另设淘汰。
    explore_kinds:
        Mutex<std::collections::HashMap<String, Vec<rubato_core::entities::ExploreKind>>>,
    /// 阅读排版的 layout session(见 [`reader`])
    reader: Mutex<reader::SessionRegistry>,
    /// 正文预取的准入闸:在飞的 `(bookUrl, index)`。**并发上限就是它的容量**
    prefetch: Mutex<std::collections::HashSet<(String, i32)>>,
}

/// 发现分类缓存的键。真身取的是 `md5(bookSourceUrl + exploreUrl)` —— md5 在那里
/// 只是为了当**文件名**(落盘那层);进程内这份直接用原串,语义一样。
fn explore_kinds_key(source: &BookSource) -> String {
    format!("{}\0{}", source.book_source_url, source.explore_url.as_deref().unwrap_or(""))
}

impl Engine {
    /// `db_path` 是单个 sqlite 文件(`:memory:` 走内存库)。
    /// books/chapters/caches/book_sources/cookies 全在这一个文件里。
    pub fn open(db_path: &str) -> Result<Engine, EngineError> {
        Engine::open_with(db_path, None, None, None)
    }

    /// 带平台面的开库(产品走这一条:`ffi::platform::provider()`)。
    /// 平台面只影响 `{"webView": true}` 那条路 —— 给不出就是
    /// `FetchError::WebViewUnsupported`,与从前一模一样。
    ///
    /// `search_threads`:并发搜索的 worker 数,缺省 [`SEARCH_THREADS`]。
    /// 调高只在「一口气扫上百个源」的场合有意义(Phase 3 的验收探针) ——
    /// 那种场合墙钟时间 ≈ 各源耗时之和 ÷ 这个数,死站点的超时全靠它摊掉。
    pub fn open_with(
        db_path: &str,
        web_view: Option<Arc<dyn WebViewProvider>>,
        search_threads: Option<usize>,
        verify_ui: Option<Arc<dyn net::verification::VerifyUi>>,
    ) -> Result<Engine, EngineError> {
        let mut books = BookStore::open(db_path)?;
        let sources = SourceStore::open(db_path)?;
        let cookies = if db_path == ":memory:" {
            CookieStore::open_in_memory()?
        } else {
            CookieStore::open(db_path)?
        };
        let android_id = resolve_android_id(&mut books)?;
        let transport =
            LiveTransport::with_timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
                .map_err(|e| EngineError::Env(format!("transport: {e}")))?;
        Ok(Engine {
            stores: Mutex::new(Stores { books, sources }),
            cookies: Arc::new(Mutex::new(cookies)),
            transport: Arc::new(transport),
            android_id,
            search_threads: search_threads.unwrap_or(SEARCH_THREADS).clamp(1, MAX_SEARCH_THREADS),
            web_view,
            verify: verify_ui.map(|ui| Arc::new(net::verification::Verification::new(ui))),
            explore_kinds: Mutex::new(std::collections::HashMap::new()),
            reader: Mutex::new(reader::SessionRegistry::default()),
            prefetch: Mutex::new(std::collections::HashSet::new()),
        })
    }

    fn stores(&self) -> std::sync::MutexGuard<'_, Stores> {
        self.stores.lock().expect("stores 锁")
    }

    /// webView 平台原语的工厂,**现在要得到才给**(Dart 侧登记了那条流)。
    /// `None` 一路传到 `AnalyzeUrl` 就是 `FetchError::WebViewUnsupported`
    /// —— 比给一个必然超时的壳诚实。每次 `BackstageWebView` 要一个新的
    /// (真身:从池子里 acquire,用完 release)。
    fn web_view(
        &self,
        cancel: &CancelToken,
    ) -> Option<impl Fn() -> Box<dyn WebViewHost> + use<'_>> {
        let p = self.web_view.as_ref().filter(|p| p.available())?;
        // 取消令牌跟着进平台侧:webView 那一步可以长达 60 秒,
        // 而取消在别处是「步与步之间」的粒度 —— 不给它,点了停还得再等一分钟
        let cancel = cancel.as_fn();
        Some(move || p.acquire(Some(cancel.clone())))
    }

    // ---------------- JS 宿主装配 ----------------

    /// 一次调用的 JS 环境。建完就把 `caches` 表里 `js:` 那一段 seed 进去 ——
    /// `loginHeader_*` / `sourceVariable_*` / `userInfo_*` 是**跨调用**的
    /// (真身的 CacheManager 落库),不 seed 就等于每次都是新装的 app。
    fn js_env(&self, source: &BookSource, cancel: &CancelToken) -> Result<JsEnv, EngineError> {
        let seed = self.stores().books.list_cache_prefix(JS_CACHE_PREFIX)?;
        JsEnv::new(
            source,
            self.cookies.clone(),
            cancel.clone(),
            self.android_id.clone(),
            self.transport.clone(),
            seed,
            self.web_view.clone(),
            self.verify.clone(),
        )
        .map_err(EngineError::Pipeline)
    }

    /// 收工时把 CacheManager 写回。**按差量写**:只碰这个 JsEnv 自己动过的键
    /// —— 并发搜索时 6 个 worker 各自 seed 各自收工,整段替换会把别的源刚写下的
    /// `sourceVariable_*` 抹掉(见 [`JsEnv::cache_delta`])。JS 里 `remove` 掉的
    /// 条目仍会被删,所以「下次不该 seed 回来」照旧成立。
    /// 落库失败不致命(下次重来),故只吞不抛。
    fn flush_js_env(&self, js: &JsEnv) {
        let (upserts, deletes) = js.cache_delta();
        let _ = self.stores().books.update_cache_prefix(JS_CACHE_PREFIX, &upserts, &deletes);
    }

    /// 一次流水线调用的固定装配:建 JS 环境 → 搭 [`PipelineEnv`] → 跑 `f` →
    /// **不论成败**把 CacheManager 差量写回([`Engine::flush_js_env`])。
    /// 六个入口从前各抄一份,这里收成一处;错误层次不变:
    /// 建 JS 环境失败是 [`EngineError`],`f` 里流水线失败按 [`pipeline_err`] 归一。
    fn with_env<T>(
        &self,
        source: &BookSource,
        cancel: &CancelToken,
        f: impl FnOnce(&mut PipelineEnv<'_>) -> Result<T, PipelineError>,
    ) -> Result<T, EngineError> {
        let mut cookies = SharedCookies(self.cookies.clone());
        let js = self.js_env(source, cancel)?;
        let hf = |d: &rubato_core::RuleData| js.host(d);
        let wv = self.web_view(cancel);
        let r = {
            let mut env = PipelineEnv {
                transport: &*self.transport,
                cookies: &mut cookies,
                host_factory: &hf,
                web_view: wv.as_ref().map(|w| w as &dyn Fn() -> Box<dyn WebViewHost>),
            };
            f(&mut env)
        };
        self.flush_js_env(&js);
        r.map_err(pipeline_err)
    }

    // ---------------- 书源 ----------------

    /// 导入书源 JSON(单个对象或数组,legado 的分享格式)。返回入库条数。
    pub fn import_sources(&self, json: &str) -> Result<usize, EngineError> {
        let v: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| EngineError::BadSource(format!("JSON 解析失败: {e}")))?;
        let list = match v {
            serde_json::Value::Array(a) => a,
            other => vec![other],
        };
        Ok(self.stores().sources.save_sources(&list)?)
    }

    pub fn source_count(&self) -> Result<i64, EngineError> {
        Ok(self.stores().sources.count()?)
    }

    pub fn list_sources(&self) -> Result<Vec<BookSource>, EngineError> {
        Ok(self.stores().sources.list_all()?)
    }

    pub fn set_source_enabled(&self, url: &str, enabled: bool) -> Result<(), EngineError> {
        Ok(self.stores().sources.set_enabled(url, enabled)?)
    }

    pub fn delete_source(&self, url: &str) -> Result<(), EngineError> {
        Ok(self.stores().sources.delete_source(url)?)
    }

    /// 打开一个源的登录页。对齐 `SourceLoginActivity` 的分流:
    /// `loginUi` 非空且不等于 `[]` 走表单，否则走可见 WebView。
    pub fn source_login_spec(&self, source_url: &str) -> Result<SourceLoginSpec, EngineError> {
        let source = self.source_of(source_url)?;
        let form_rule = source.login_ui.as_deref().map(str::trim).filter(|s| {
            !s.is_empty() && s.chars().filter(|c| !c.is_whitespace()).collect::<String>() != "[]"
        });
        let login_url = source.login_url.as_deref().map(str::trim).filter(|s| !s.is_empty());
        if form_rule.is_none() && login_url.is_none() {
            return Err(EngineError::BadSource(format!(
                "{} 没有登录配置",
                source.book_source_name
            )));
        }

        let cancel = CancelToken::new();
        let js = self.js_env(&source, &cancel)?;
        let stored_json = match js.eval_source_js(&source, "source.getLoginInfo()", None) {
            Ok(rubato_core::host::JsValue::Str(v))
                if serde_json::from_str::<serde_json::Value>(&v).is_ok() =>
            {
                v
            }
            _ => "{}".to_string(),
        };
        let headers = js.source_header();

        // 少量旧源把一个 URL 错放在 loginUi 里。上游会把它当表单后解析失败；
        // 这里把它归到可见 WebView，至少保留用户实际可完成的登录路径。
        let misplaced_url =
            form_rule.filter(|s| s.starts_with("http://") || s.starts_with("https://"));
        let is_form = form_rule.is_some() && misplaced_url.is_none();
        let rows_json = if is_form {
            let rule = form_rule.expect("上面判过");
            match extract_inline_js(rule) {
                Some(code) => {
                    let state = serde_json::from_str::<serde_json::Value>(&stored_json)
                        .unwrap_or_else(|_| serde_json::json!({}));
                    let login_js = login_url.map(get_login_js).unwrap_or_default();
                    let code = format!("{login_js}\n{code}");
                    let value = js
                        .eval_source_js(&source, &code, Some(&state))
                        .map_err(|e| EngineError::Pipeline(format!("login_ui:{e}")))?;
                    Some(js_value_string(value)?)
                }
                None => Some(rule.to_string()),
            }
        } else {
            None
        };
        let web_url = if is_form {
            None
        } else {
            let raw = misplaced_url.or(login_url).expect("登录配置已判过");
            Some(rubato_core::net_utils::get_absolute_url_str(Some(source_url), raw))
        };
        self.flush_js_env(&js);
        Ok(SourceLoginSpec {
            source_url: source.book_source_url,
            source_name: source.book_source_name,
            is_form,
            login_url: web_url,
            rows_json,
            stored_json,
            headers,
        })
    }

    /// 表单按钮或最终“登录”。`action = None` 时调用书源的 `login()`；无
    /// `loginUrl` 脚本的表单只保存登录信息。保存发生在脚本之前，与真身一致。
    pub fn source_login_action(
        &self,
        source_url: &str,
        data_json: &str,
        action: Option<&str>,
        persist: bool,
    ) -> Result<(), EngineError> {
        let source = self.source_of(source_url)?;
        let data: serde_json::Value = serde_json::from_str(data_json)
            .map_err(|e| EngineError::BadSource(format!("登录数据不是 JSON: {e}")))?;
        if !data.is_object() {
            return Err(EngineError::BadSource("登录数据必须是对象".into()));
        }
        let cancel = CancelToken::new();
        let js = self.js_env(&source, &cancel)?;
        let login_js = source.login_url.as_deref().map(get_login_js).unwrap_or_default();
        let save = if persist {
            "if (!source.putLoginInfo(JSON.stringify(result))) { throw('保存登录信息失败'); }"
        } else {
            ""
        };
        let invoke = action.unwrap_or(
            "if (typeof login=='function') { login.apply(this); } else { throw('Function login not implements!!!'); }",
        );
        // 没有脚本的静态表单仍要能保存，不凭空制造“未实现 login()”错误。
        let invoke = if login_js.trim().is_empty() && action.is_none() { "" } else { invoke };
        let code = format!("{save}\n{login_js}\n{invoke}");
        let out = js.eval_source_js(&source, &code, Some(&data));
        self.flush_js_env(&js);
        out.map(|_| ()).map_err(|e| EngineError::Pipeline(format!("login:{e}")))
    }

    // ---------------- 搜索 ----------------

    /// 在全部启用书源上搜书。结果**逐条回调**(一个源出一批就推一批),
    /// `cancel` 在源与源之间生效。返回实际跑过的源数。
    ///
    /// **取消不是错误**:被取消时提前收工并照常返回跑过的源数
    /// —— 用户主动停搜不该在 UI 上冒一条报错。要知道是不是取消了,
    /// 问手里的那个 [`CancelToken`]。
    pub fn search(
        &self,
        key: &str,
        cancel: &CancelToken,
        on_hit: &mut dyn FnMut(SearchBook),
    ) -> Result<usize, EngineError> {
        let sources = self.stores().sources.list_enabled()?;
        if sources.is_empty() {
            return Ok(0);
        }
        let queue = Arc::new(Mutex::new(sources.into_iter()));
        let (tx, rx) = mpsc::channel::<SearchBook>();
        let done = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for _ in 0..self.search_threads {
                let queue = queue.clone();
                let tx = tx.clone();
                handles.push(scope.spawn(move || {
                    let mut n = 0usize;
                    loop {
                        if cancel.is_cancelled() {
                            break;
                        }
                        let Some(source) = queue.lock().expect("队列锁").next() else { break };
                        n += 1;
                        // JS 环境按源建:cache/cookie 的表、网络面的 source_key
                        // 都是源级的(见 JsEnv)。
                        // 单源失败不影响其他源(真身也是逐源吞异常)
                        let hits = self.with_env(&source, cancel, |env| {
                            pipeline::search_book(env, &source, key, Some(1))
                        });
                        if let Ok(hits) = hits {
                            for h in hits {
                                if tx.send(h).is_err() {
                                    return n;
                                }
                            }
                        }
                    }
                    n
                }));
            }
            drop(tx);
            // 主线程边收边回调,UI 因此能「搜到一个显示一个」
            for hit in rx {
                on_hit(hit);
            }
            handles.into_iter().map(|h| h.join().unwrap_or(0)).sum::<usize>()
        });
        Ok(done)
    }

    /// **单源搜索,错误原样交回**(诊断口)。
    ///
    /// [`Engine::search`] 是**逐源吞异常**的 —— 那是产品该有的行为(一个源炸了
    /// 不该让整次搜索失败),但它把「站点真没这本书」与「这个源当场炸了」
    /// 压成了同一个结果:**空表**。Phase 3 的逐源验收正卡在这上面
    /// (头一遍 98 个源里 80 个落进「0 命中」那一堆,全是「还没查明」)。
    ///
    /// 所以另开这一条:一次只跑一个源,失败就把错误交出来。并发由调用方掌握
    /// (验收探针在 Dart 那侧开池子),将来书源调试页要的也是它。
    pub fn search_source(
        &self,
        source_url: &str,
        key: &str,
        cancel: &CancelToken,
    ) -> Result<Vec<SearchBook>, EngineError> {
        let source = self.source_of(source_url)?;
        self.with_env(&source, cancel, |env| pipeline::search_book(env, &source, key, Some(1)))
    }

    /// **搜索那一步抓回来的页面,原样交出**(诊断口)。
    ///
    /// [`Engine::search_source`] 回答的是「这个源出没出书」,而它出不了书有两种
    /// 完全不同的因:**页面就没拿到该拿的东西**(过盾没过、站点改版、地区拦截 ——
    /// webView 那条路尤其看不出来:加载失败给的是**错误页**,照样算加载完,
    /// 于是与「站点真没这本书」同形),还是**规则选不中**。这一条把中间那张页
    /// 交回去,人(或验收报表)看一眼就分得开。将来书源调试页要的也是它。
    ///
    /// 与搜索**共用同一条抓取路径**(`pipeline::search_fetch`),不另写一份。
    pub fn search_source_page(
        &self,
        source_url: &str,
        key: &str,
        cancel: &CancelToken,
    ) -> Result<(String, Option<String>), EngineError> {
        let source = self.source_of(source_url)?;
        self.with_env(&source, cancel, |env| {
            pipeline::search_fetch(env, &source, key, Some(1)).map(|(_, res)| (res.url, res.body))
        })
    }

    /// **单源发现,错误原样交回**(诊断口)。
    ///
    /// Phase 3 有少量书源只有发现、没有搜索；若探针固定从搜索进入，它们在
    /// 目录/正文上的 webView 能力永远不可达。发现分类的展开仍由调用方完成，
    /// 这里接收用户实际点中的那条 URL，与 `WebBook.exploreBookAwait` 同口径。
    pub fn explore_source(
        &self,
        source_url: &str,
        url: &str,
        cancel: &CancelToken,
    ) -> Result<Vec<SearchBook>, EngineError> {
        let source = self.source_of(source_url)?;
        self.with_env(&source, cancel, |env| pipeline::explore_book(env, &source, url, Some(1)))
    }

    /// **发现页的分类列表**(真身 `BookSourceExtensions.exploreKinds()`)。
    ///
    /// 产品面的入口:书源的 `exploreUrl` 摊成一排格子,用户点中哪一格,
    /// 那一格的 `url` 才交给 [`Engine::explore_source`]。此前引擎只有后半截 ——
    /// 分类 JSON 没人解,发现页也就无从谈起。判据:`tools/explore_diff.sh`。
    ///
    /// **缓存**:真身有两层(进程内 `exploreKindsMap` + 落盘 `ACache`,键是
    /// `md5(bookSourceUrl + exploreUrl)`)。这里只做**进程内**那一层 ——
    /// `@js:` 那一支可能真发请求(语料里就有一个源要先 `java.ajax` 拿域名),
    /// 每次开发现页都重跑一遍既慢又多余;落盘那层等书源编辑页有了「清缓存」
    /// 那个按钮再说。键与真身同源,故改了 `exploreUrl` 会自动失效。
    pub fn explore_kinds(
        &self,
        source_url: &str,
        cancel: &CancelToken,
    ) -> Result<Vec<rubato_core::entities::ExploreKind>, EngineError> {
        let source = self.source_of(source_url)?;
        let key = explore_kinds_key(&source);
        if let Some(hit) = self.explore_kinds.lock().expect("发现分类缓存锁").get(&key) {
            return Ok(hit.clone());
        }
        let source_binding = rubato_core::host::SourceBinding {
            key: source.book_source_url.clone(),
            tag: source.book_source_name.clone(),
            raw: source.raw.clone(),
        };
        let js = self.js_env(&source, cancel)?;
        let mut host = js.host(&rubato_core::RuleData::default());
        // 变量层包成 `AnalyzeRule`:发现规则里 `org.jsoup.Jsoup.parse(...)` 是
        // 常见写法,而元素句柄表挂在 AnalyzeRule 上(理由同 js_case_runner 那处)
        let mut rule = rule_engine::AnalyzeRule::new(
            Box::new(rule_engine::NoHost),
            rubato_core::RuleData::default(),
        );
        let kinds = pipeline::explore_kinds::explore_kinds(
            source.explore_url.as_deref(),
            host.as_mut(),
            &source_binding,
            &mut rule,
        );
        self.flush_js_env(&js);
        self.explore_kinds.lock().expect("发现分类缓存锁").insert(key, kinds.clone());
        Ok(kinds)
    }

    /// **发现那一步抓回来的页面,原样交出**。与 [`Engine::explore_source`]
    /// 共用 `pipeline::explore_fetch`，用于区分“页面不对”和“发现规则选不中”。
    pub fn explore_source_page(
        &self,
        source_url: &str,
        url: &str,
        cancel: &CancelToken,
    ) -> Result<(String, Option<String>), EngineError> {
        let source = self.source_of(source_url)?;
        self.with_env(&source, cancel, |env| {
            pipeline::explore_fetch(env, &source, url, Some(1)).map(|(_, res)| (res.url, res.body))
        })
    }

    /// 界面那一侧填回验证结果(真身 `SourceVerificationHelp.setResult`)。
    /// `result` 空串 = 用户把界面关了 —— 等待那侧据此报「验证结果为空」
    pub fn set_verify_result(&self, key: &str, url: &str, result: &str) {
        if let Some(v) = &self.verify {
            v.set_result(key, url, result);
        }
    }

    /// 界面那一侧把 cookie 抄回来(真身 `WebViewActivity.onPageFinished` 里那句
    /// `CookieStore.setCookie(url, CookieManager.getInstance().getCookie(url))`)。
    ///
    /// **过盾靠的就是这一下**:用户在内置浏览器里过完 Cloudflare,新 cookie 在
    /// 浏览器的 jar 里;书源接着 `java.ajax(url)` 走的是引擎自己的 cookie 表 ——
    /// 不抄过来,过盾等于白过。
    pub fn set_cookie(&self, url: &str, cookie: &str) {
        self.cookies.lock().expect("cookie 锁").set_cookie(url, Some(cookie));
    }

    // ---------------- 书架 ----------------

    pub fn bookshelf(&self) -> Result<Vec<BookRow>, EngineError> {
        Ok(self.stores().books.list_books()?)
    }

    pub fn get_book(&self, book_url: &str) -> Result<BookRow, EngineError> {
        self.stores()
            .books
            .get_book(book_url)?
            .ok_or_else(|| EngineError::NotFound(book_url.to_string()))
    }

    pub fn remove_book(&self, book_url: &str) -> Result<(), EngineError> {
        Ok(self.stores().books.delete_book(book_url)?)
    }

    /// 加书架:补详情 → 抓目录 → 落库。返回入库后的书。
    pub fn add_to_bookshelf(&self, hit: &SearchBook) -> Result<BookRow, EngineError> {
        let source = self.source_of(&hit.origin)?;
        let mut book = search_book_to_book(hit);
        // 这几条入口不领任务号(产品里没有「停」这个按钮),令牌是常假的那一份
        let cancel = CancelToken::new();
        let chapters = self.with_env(&source, &cancel, |env| {
            // 详情失败不致命(搜索结果里通常已有书名/作者),目录失败才致命
            let _ = pipeline::get_book_info(env, &source, &mut book, true);
            pipeline::get_chapter_list(env, &source, &mut book)
        })?;
        let now = now_ms();
        let mut row = BookRow::new(book);
        row.last_check_time = now;
        row.latest_chapter_time = now;
        row.dur_chapter_time = now;
        row.book.total_chapter_num = chapters.len() as i32;
        let book_url = row.book.book_url.clone();
        let mut st = self.stores();
        st.books.save_book(&row)?;
        st.books.save_chapters(&book_url, &normalize_chapters(&book_url, chapters))?;
        drop(st);
        self.get_book(&book_url)
    }

    /// 每一章**有没有缓存过正文**,顺序与 [`Self::chapters`] 一致。
    ///
    /// 设计稿 S-04 的「386 章 · 已缓存 214」与 S-06b 的筛选 chip / 每行状态
    /// 都要它 —— 那三处要么有真数,要么就不该显示,不能编。
    pub fn chapter_cache_flags(&self, book_url: &str) -> Result<Vec<bool>, EngineError> {
        let st = self.stores();
        let chapters = st.books.list_chapters(book_url)?;
        let cached = st.books.cached_chapter_urls(book_url)?;
        Ok(chapters.iter().map(|c| cached.contains(&c.url)).collect())
    }

    /// 产品自己的一点设置(主题档位这类)。**引擎不解释内容**,与
    /// `reader_settings` 同一条路:`caches` 本来就是通用 kv,前缀分家
    /// (这里是 `app:`),为一行设置多拉一个平台插件不划算。
    pub fn app_setting(&self, key: &str) -> Result<Option<String>, EngineError> {
        Ok(self.stores().books.get_cache(&format!("app:{key}"))?)
    }

    pub fn set_app_setting(&self, key: &str, value: &str) -> Result<(), EngineError> {
        self.stores().books.put_cache(&format!("app:{key}"), value)?;
        Ok(())
    }

    /// 搜索结果那一屏的封面。书**还没入库**,所以不能走 `chapter_image` ——
    /// 就地拼成 Book 交给 [`Self::book_image`],链路(书源 header / cookie /
    /// AnalyzeUrl)与正文图片是同一条。
    pub fn hit_image(
        &self,
        hit: &SearchBook,
        src: &str,
        cancel: &CancelToken,
    ) -> Result<Vec<u8>, EngineError> {
        self.book_image(&search_book_to_book(hit), src, cancel)
    }

    // ---------------- 目录与正文 ----------------

    pub fn chapters(&self, book_url: &str) -> Result<Vec<BookChapter>, EngineError> {
        Ok(self.stores().books.list_chapters(book_url)?)
    }

    /// 重新抓目录(书架刷新)。返回新章节数。
    pub fn refresh_chapters(&self, book_url: &str) -> Result<usize, EngineError> {
        let row = self.get_book(book_url)?;
        let source = self.source_of(&row.book.origin)?;
        let mut book = row.book.clone();
        // 这几条入口不领任务号(产品里没有「停」这个按钮),令牌是常假的那一份
        let cancel = CancelToken::new();
        let chapters = self.with_env(&source, &cancel, |env| {
            pipeline::get_chapter_list(env, &source, &mut book)
        })?;
        let old = row.book.total_chapter_num;
        let mut row = row;
        row.book = book;
        row.last_check_time = now_ms();
        row.book.last_check_count = (chapters.len() as i32 - old).max(0);
        row.book.total_chapter_num = chapters.len() as i32;
        let mut st = self.stores();
        st.books.save_book(&row)?;
        st.books.save_chapters(book_url, &normalize_chapters(book_url, chapters))?;
        Ok(row.book.total_chapter_num as usize)
    }

    /// 取某章正文(诊断口:不领任务号,取消由调用方不发起来实现)。
    /// 产品那条路走 [`Engine::chapter_content_with`] —— 阅读页退页要能真的停下来。
    pub fn chapter_content(&self, book_url: &str, index: i32) -> Result<String, EngineError> {
        self.chapter_content_with(book_url, index, &CancelToken::new())
    }

    /// 取某章正文,**取消令牌由调用方给**。命中 `caches` 直接返回;否则抓取后写缓存。
    ///
    /// 从前这条路里的令牌是「常假的那一份」—— 阅读页退了、用户已经翻到别处,
    /// 抓取还在跑到超时为止。预取一开就更明显:后台那几章不该拖着不放手。
    pub fn chapter_content_with(
        &self,
        book_url: &str,
        index: i32,
        cancel: &CancelToken,
    ) -> Result<String, EngineError> {
        let row = self.get_book(book_url)?;
        let (chapter, next_url) = {
            let st = self.stores();
            let chapter = st
                .books
                .get_chapter(book_url, index)?
                .ok_or_else(|| EngineError::NotFound(format!("{book_url}#{index}")))?;
            let next = st.books.get_chapter(book_url, index + 1)?.map(|c| c.url);
            (chapter, next)
        };
        if let Some(cached) = self.stores().books.get_content(book_url, &chapter.url)? {
            return Ok(cached);
        }
        let source = self.source_of(&row.book.origin)?;
        let mut book = row.book.clone();
        let mut chapter = chapter;
        let content = self.with_env(&source, cancel, |env| {
            pipeline::get_content(env, &source, &mut book, &mut chapter, next_url.as_deref())
        })?;
        self.stores().books.put_content(book_url, &chapter.url, &content)?;
        Ok(content)
    }

    /// 阅读进度持久化(翻页热路径,只写三列)
    pub fn save_progress(
        &self,
        book_url: &str,
        index: i32,
        pos: i32,
        title: Option<&str>,
    ) -> Result<(), EngineError> {
        Ok(self.stores().books.save_progress(book_url, index, pos, title, now_ms())?)
    }

    // ---------------- 正文批注(计划书 reader-layout M4c) ----------------

    /// 一章的批注。**按 (bookUrl, chapterIndex) 取** —— 章的 URL 会随书源换而变
    pub fn highlights(
        &self,
        book_url: &str,
        chapter_index: i32,
    ) -> Result<Vec<HighlightRow>, EngineError> {
        Ok(HighlightStore::list(self.stores().books.conn(), book_url, chapter_index)?)
    }

    /// 一本书的全部批注(批注列表那一屏)
    pub fn book_highlights(&self, book_url: &str) -> Result<Vec<HighlightRow>, EngineError> {
        Ok(HighlightStore::list_book(self.stores().books.conn(), book_url)?)
    }

    /// 新增或整条覆盖,回它的主键(= 创建时间)
    pub fn save_highlight(&self, row: &HighlightRow) -> Result<i64, EngineError> {
        Ok(HighlightStore::save(self.stores().books.conn(), row, now_ms())?)
    }

    /// 换样式或笔记,**不动位置**
    pub fn update_highlight(
        &self,
        time: i64,
        style: &str,
        note: &str,
    ) -> Result<bool, EngineError> {
        Ok(HighlightStore::update(self.stores().books.conn(), time, style, note)?)
    }

    pub fn delete_highlight(&self, time: i64) -> Result<bool, EngineError> {
        Ok(HighlightStore::delete(self.stores().books.conn(), time)?)
    }

    fn source_of(&self, origin: &str) -> Result<BookSource, EngineError> {
        self.stores()
            .sources
            .get_source(origin)?
            .ok_or_else(|| EngineError::NotFound(format!("书源 {origin}")))
    }
}

fn extract_inline_js(rule: &str) -> Option<&str> {
    let text = rule.trim();
    if let Some(code) = text.strip_prefix("@js:") {
        return Some(code.trim());
    }
    text.strip_prefix("<js>")?.strip_suffix("</js>").map(str::trim)
}

fn get_login_js(rule: &str) -> &str {
    extract_inline_js(rule).unwrap_or(rule.trim())
}

fn js_value_string(v: rubato_core::host::JsValue) -> Result<String, EngineError> {
    use rubato_core::host::JsValue;
    match v {
        JsValue::Str(v) => Ok(v),
        JsValue::Json(v) => Ok(v.to_string()),
        JsValue::Native(v) => {
            serde_json::to_string(&v).map_err(|e| EngineError::Pipeline(format!("login_ui:{e}")))
        }
        JsValue::List(v) => {
            serde_json::to_string(&v).map_err(|e| EngineError::Pipeline(format!("login_ui:{e}")))
        }
        JsValue::Null => Ok(String::new()),
        other => Err(EngineError::Pipeline(format!("login_ui 返回了不支持的值:{other:?}"))),
    }
}

/// `SearchBook.toBook()`
fn search_book_to_book(sb: &SearchBook) -> Book {
    Book {
        name: sb.name.clone(),
        author: sb.author.clone(),
        kind: sb.kind.clone(),
        book_url: sb.book_url.clone(),
        origin: sb.origin.clone(),
        origin_name: sb.origin_name.clone(),
        type_: sb.type_,
        word_count: sb.word_count.clone(),
        latest_chapter_title: sb.latest_chapter_title.clone(),
        cover_url: sb.cover_url.clone(),
        intro: sb.intro.clone(),
        toc_url: sb.toc_url.clone(),
        origin_order: sb.origin_order,
        vars: sb.vars.clone(),
        info_html: sb.info_html.clone(),
        toc_html: sb.toc_html.clone(),
        ..Default::default()
    }
}

/// 落库前把 bookUrl / index 钉成库里的主键形态
/// (真身在 `BookChapterList` 里已给 index,这里只兜底)
fn normalize_chapters(book_url: &str, mut chapters: Vec<BookChapter>) -> Vec<BookChapter> {
    for (i, c) in chapters.iter_mut().enumerate() {
        c.book_url = book_url.to_string();
        c.index = i as i32;
    }
    chapters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_creates_all_tables() {
        let e = Engine::open(":memory:").expect("open");
        assert_eq!(e.source_count().expect("count"), 0);
        assert!(e.bookshelf().expect("shelf").is_empty());
    }

    #[test]
    fn import_sources_accepts_object_and_array() {
        let e = Engine::open(":memory:").expect("open");
        assert_eq!(e.import_sources(r#"{"bookSourceUrl":"http://a"}"#).expect("单个"), 1);
        assert_eq!(
            e.import_sources(r#"[{"bookSourceUrl":"http://b"},{"bookSourceUrl":"http://c"}]"#)
                .expect("数组"),
            2
        );
        assert_eq!(e.source_count().expect("count"), 3);
        assert!(e.import_sources("不是 JSON").is_err());
    }

    #[test]
    fn search_without_sources_is_noop() {
        let e = Engine::open(":memory:").expect("open");
        let mut hits = Vec::new();
        let n = e.search("斗破", &CancelToken::new(), &mut |h| hits.push(h)).expect("search");
        assert_eq!((n, hits.len()), (0, 0));
    }

    #[test]
    fn cancelled_search_stops_without_erroring() {
        let e = Engine::open(":memory:").expect("open");
        e.import_sources(r#"{"bookSourceUrl":"http://a","searchUrl":"http://a/s"}"#).expect("导入");
        let token = CancelToken::new();
        token.cancel();
        // 一个源都不该跑(联网前先看令牌),也不该报错
        let mut hits = 0;
        assert_eq!(e.search("x", &token, &mut |_| hits += 1).expect("取消不是错误"), 0);
        assert_eq!(hits, 0);
    }

    #[test]
    fn source_login_form_persists_data_and_runs_login_function() {
        let e = Engine::open(":memory:").expect("open");
        e.import_sources(
            r#"{
                "bookSourceUrl":"https://login.example/base/",
                "bookSourceName":"登录源",
                "loginUrl":"function login(){source.putLoginHeader(JSON.stringify({Token:result.user}))}",
                "loginUi":"[{\"name\":\"user\",\"type\":\"text\",\"default\":\"guest\"}]"
            }"#,
        )
        .expect("导入");
        let spec = e.source_login_spec("https://login.example/base/").expect("spec");
        assert!(spec.is_form);
        assert_eq!(spec.source_name, "登录源");
        assert_eq!(
            spec.rows_json.as_deref(),
            Some(r#"[{"name":"user","type":"text","default":"guest"}]"#)
        );
        assert_eq!(spec.stored_json, "{}");
        assert!(spec.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("user-agent")));

        e.source_login_action("https://login.example/base/", r#"{"user":"alice"}"#, None, true)
            .expect("登录");
        let after = e.source_login_spec("https://login.example/base/").expect("after");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&after.stored_json).expect("stored"),
            serde_json::json!({"user":"alice"})
        );
        assert_eq!(
            e.stores()
                .books
                .list_cache_prefix(JS_CACHE_PREFIX)
                .expect("cache")
                .into_iter()
                .find(|(k, _)| k == "loginHeader_https://login.example/base/")
                .map(|(_, v)| v),
            Some(r#"{"Token":"alice"}"#.into())
        );
    }

    #[test]
    fn source_login_web_resolves_relative_url() {
        let e = Engine::open(":memory:").expect("open");
        e.import_sources(
            r#"{"bookSourceUrl":"https://login.example/base/","loginUrl":"../account"}"#,
        )
        .expect("导入");
        let spec = e.source_login_spec("https://login.example/base/").expect("spec");
        assert!(!spec.is_form);
        assert_eq!(spec.login_url.as_deref(), Some("https://login.example/account"));
    }

    #[test]
    fn progress_round_trip() {
        let e = Engine::open(":memory:").expect("open");
        let mut row = BookRow::new(Book {
            book_url: "http://a/1".into(),
            name: "书".into(),
            origin: "http://a".into(),
            ..Default::default()
        });
        row.book.author = "佚名".into();
        e.stores().books.save_book(&row).expect("save");
        e.save_progress("http://a/1", 4, 88, Some("第五章")).expect("progress");
        let got = e.get_book("http://a/1").expect("get");
        assert_eq!((got.book.dur_chapter_index, got.dur_chapter_pos), (4, 88));
    }
}
