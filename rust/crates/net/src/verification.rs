//! `SourceVerificationHelp`(judge/engine/help/source/SourceVerificationHelp.kt)
//! 的**策略层**逐行移植:书源要用户出手的那三件 ——
//! `java.startBrowser` / `java.startBrowserAwait` / `java.getVerificationCode`。
//!
//! 语料里这一族的写法几乎只有一种(16 处 `startBrowserAwait` 里绝大多数):
//!
//! ```js
//! if (result.match(/Just a moment/)) {          // Cloudflare 拦下来了
//!     cookie.removeCookie(source.bookSourceUrl)
//!     java.startBrowserAwait(url, "验证")        // 弹内置浏览器,人工过盾
//!     result = java.ajax(url)                   // 过完再抓一次(带上新 cookie)
//! }
//! ```
//!
//! 也就是说:**它是「盾」那一族的正解** —— 我们过不去的那些站点,legado 的
//! 答案就是让用户自己点一下。
//!
//! ## 两半:这里是策略,界面在平台侧
//!
//! 真身 `SourceVerificationHelp` 是个进程级 object,它自己**不画界面**:
//! 界面是 `WebViewActivity` / `VerificationCodeActivity`,它们做完之后回头调
//! `setResult(key, result, url)`。这一份照搬那个分工:
//!
//! - **这里**(可移植、可测):挂号表、界面互斥锁、同一目标只弹一次的「飞行表」、
//!   轮询等待、取消、`验证结果为空` 那几条判定;
//! - **平台侧**([`VerifyUi`]):把界面弹出来、把结果塞回挂号表。
//!
//! ## 与真身对齐的几处口径(改之前先读)
//!
//! 1. **界面互斥**(`verificationUiLock`):同一时刻只弹一个。并发搜索会有六个
//!    worker 同时撞上盾,不锁就是六个窗口一起弹出来。
//! 2. **飞行表**(`VerificationFlightRegistry`):`useBrowser && refetchAfterSuccess
//!    && html == null` 这一档,按 `(源类型, 源 key, scheme, host, port)` 归并 ——
//!    第一个进来的弹界面,后面的等它;等到了**不用它的结果**,而是自己重抓一次
//!    ([`VerifyOutcome::Refetch`])。真身如此:过盾之后 cookie 是共享的,
//!    各人重抓各人的地址才对。
//! 3. **结果为空 = 出错**(`验证结果为空`):用户把界面关掉时真身走
//!    `checkResult` 塞一个空串进去,于是等待那一侧当场抛。
//! 4. **等待是轮询**:真身 `LockSupport.parkNanos(1 秒)` 一圈,顺便看协程还活着
//!    没有。这里同样 —— 取消要能进得来(与 webView 那条路同一个道理)。

use rubato_core::host::CancelFn;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// 飞行表的等待上限(真身 `flightWaitTime = 5.minutes`)
const FLIGHT_WAIT: Duration = Duration::from_secs(5 * 60);
/// 轮询一圈的长度(真身 `waitPollTime = 1.seconds`)。取消与「界面还没答复」
/// 都靠它看见 —— 缩短它只是更勤地问一遍,不改语义
const POLL: Duration = Duration::from_millis(200);

/// 要弹的是哪一种界面
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyKind {
    /// `WebViewActivity`:内置浏览器(`startBrowser` / `startBrowserAwait`)
    Browser,
    /// `VerificationCodeActivity`:图片验证码(`getVerificationCode`)
    Code,
}

/// **可见浏览器到底怎么加载这一页**。真身在界面层算(`WebViewModel.initData`
/// 与 `WebViewActivity` 里那三行:`AnalyzeUrl` 拆开 `url,{…}`、
/// `toWebViewRequestConfig` 把 UA 单拎出来、POST 先由 okhttp 抓成 HTML);
/// 我们的界面在 Dart —— 算在宿主网络面(`js-host::net_face::browser_load`),
/// 随这一次「弹界面」一起交过去。
///
/// 策略层不读它,只负责**原样转交**:挂号与返回值用的仍是入参那个 `url`
/// (带 `,{…}` 的原串)。`None` = 算不出来(真身 `initData` 的 `onError`
/// 那一支)或压根不需要(验证码那个界面)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserLoad {
    /// `analyzeUrl.url`(真身 `viewModel.baseUrl`)
    pub url: String,
    /// 转发头:**已经去掉** `User-Agent` / `CookieJar` / `proxy`
    pub headers: Vec<(String, String)>,
    /// `requestConfig.userAgent`——书源没写就是引擎自己那一位(`AppConfig.userAgent`)
    pub user_agent: String,
    /// 加载这一份 HTML(`loadDataWithBaseURL`);`None` = 直接加载 `url`
    pub html: Option<String>,
}

/// 交给平台侧的一次「请用户出手」。字段与真身 `startActivity { putExtra … }`
/// 那几条逐条对应。
#[derive(Debug, Clone)]
pub struct VerifyOpen {
    pub kind: VerifyKind,
    /// 挂号 key;`None` = `startBrowser` 那条**不等结果**的路
    pub key: Option<String>,
    /// 浏览器那条是要打开的地址,验证码那条是图片地址
    pub url: String,
    pub title: String,
    /// `sourceVerificationEnable`:把网页源码存回结果
    pub save_result: bool,
    pub refetch_after_success: bool,
    /// 直接给一段 html(不去加载 url)
    pub html: Option<String>,
    /// `source.getKey()`(书源主键)
    pub source_key: String,
    /// `source.getTag()`(书源名)
    pub source_name: String,
    pub source_type: i32,
    /// `None` 只出现在验证码或不具备 AnalyzeUrl 环境的策略单测。
    pub browser_load: Option<BrowserLoad>,
}

/// 平台侧:把界面弹出来 / 关掉。**它不返回结果** —— 结果由界面那边回头调
/// [`Verification::set_result`](真身的 `SourceVerificationHelp.setResult`)。
pub trait VerifyUi: Send + Sync {
    /// `appCtx.startActivity<…> { putExtra … }`:发出去就走,不等
    fn open(&self, req: &VerifyOpen);

    /// 我们不等了(取消 / 超时):把还开着的那个界面关掉。
    /// 真身 `cancelVerificationAttempt` 走的是同一条。
    fn close(&self, key: &str) {
        let _ = key;
    }

    /// **现在有没有界面**。真身那边界面永远在(它是 app 的一部分),而这边
    /// Dart 侧登记之前一个窗口都弹不出来 —— 那时候该当场报「未接」,
    /// 而不是让书源等一个永远不来的答复(与 webView 的 `available()` 同理)。
    fn available(&self) -> bool {
        true
    }
}

/// 一次等待的结局
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// `VerificationResult.Response`:`(url, 结果)` —— url 空串时调用方回退到入参
    Response(String, String),
    /// `VerificationResult.Refetch`:别人已经过了盾,自己重抓一次
    Refetch,
}

/// 出错时**带着真身的异常类别**:差分侧比的就是那个类名
/// (`js_case_runner` 认 `«host:X»` 标记),产品侧看到的是后面那句话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// `NoStackTraceException`:结果为空 / 等待超时 / source 为 null
    NoStackTrace(String),
    /// `require(…)` 不过 → `IllegalArgumentException`
    IllegalArgument(String),
    /// `CancellationException`:协程被取消(用户点了停)
    Cancelled,
}

impl VerifyError {
    /// 真身抛的是哪一个异常类
    pub fn class(&self) -> &'static str {
        match self {
            VerifyError::NoStackTrace(_) => "NoStackTraceException",
            VerifyError::IllegalArgument(_) => "IllegalArgumentException",
            VerifyError::Cancelled => "CancellationException",
        }
    }
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::NoStackTrace(m) | VerifyError::IllegalArgument(m) => write!(f, "{m}"),
            VerifyError::Cancelled => write!(f, "source verification cancelled"),
        }
    }
}

#[derive(Default)]
struct Attempt {
    /// `(url, 结果)`;`Some(("", ""))` = 用户把界面关掉了(`checkResult`)
    result: Option<(String, String)>,
}

#[derive(Default)]
struct Flight {
    done: bool,
    failed: Option<String>,
    /// 领头的那位自己被取消了 —— 后面等着的重来一轮(真身 `abandoned`)
    abandoned: bool,
    users: u32,
}

#[derive(Default)]
struct Inner {
    attempts: HashMap<String, Attempt>,
    flights: HashMap<String, Flight>,
    /// `verificationUiLock`:同一时刻只弹一个界面
    ui_busy: bool,
    next_key: u64,
}

/// `SourceVerificationHelp` 本体。**不是全局单例**:引擎持一个 `Arc` 传下去,
/// 差分侧每个 case 一个新的 —— 真身那个 object 是进程级的,而我们要的正是
/// 「同一个引擎里共享、不同 case 之间互不影响」。
pub struct Verification {
    ui: Arc<dyn VerifyUi>,
    inner: Mutex<Inner>,
    wake: Condvar,
}

impl Verification {
    /// 现在有没有界面(见 [`VerifyUi::available`])
    pub fn ui_available(&self) -> bool {
        self.ui.available()
    }

    pub fn new(ui: Arc<dyn VerifyUi>) -> Verification {
        Verification { ui, inner: Mutex::new(Inner::default()), wake: Condvar::new() }
    }

    /// 界面那侧填结果(真身 `setResult`)。**先到的算数**(`compareAndSet(null, …)`)
    pub fn set_result(&self, key: &str, url: &str, result: &str) {
        let mut g = self.inner.lock().expect("verify 锁");
        if let Some(a) = g.attempts.get_mut(key) {
            if a.result.is_none() {
                a.result = Some((url.to_string(), result.to_string()));
            }
        }
        self.wake.notify_all();
    }

    /// 界面被用户关掉(真身 `checkResult`):塞一个**空结果**进去 ——
    /// 等待那侧据此抛「验证结果为空」
    pub fn check_result(&self, key: &str) {
        self.set_result(key, "", "");
    }

    /// `JsExtensions.startBrowser`:弹出来就走,不等结果
    pub fn start_browser(
        &self,
        source: &SourceRef<'_>,
        url: &str,
        title: &str,
        html: Option<&str>,
        browser_load: Option<BrowserLoad>,
    ) -> Result<(), VerifyError> {
        check_url_len(url)?;
        self.ui.open(&VerifyOpen {
            kind: VerifyKind::Browser,
            key: None,
            url: url.to_string(),
            title: title.to_string(),
            save_result: false,
            refetch_after_success: true,
            html: html.map(str::to_string),
            source_key: source.key.to_string(),
            source_name: source.name.to_string(),
            source_type: source.kind,
            browser_load,
        });
        Ok(())
    }

    /// `SourceVerificationHelp.getVerificationResult` —— `startBrowserAwait` 与
    /// `getVerificationCode` 都走它,差别只在 `use_browser`。
    pub fn get_verification_result(
        &self,
        source: &SourceRef<'_>,
        url: &str,
        title: &str,
        use_browser: bool,
        refetch_after_success: bool,
        html: Option<&str>,
        browser_load: Option<BrowserLoad>,
        cancel: Option<&CancelFn>,
    ) -> Result<VerifyOutcome, VerifyError> {
        check_url_len(url)?;
        // `canJoinVerificationFlight` + `verificationFlightKey`
        let flight_key = if use_browser && refetch_after_success && html.is_none() {
            flight_key(source.key, source.kind, url)
        } else {
            None
        };
        let Some(flight_key) = flight_key else {
            // 不归并的那一档:自己排队弹界面
            let _ui = self.lock_ui(cancel)?;
            let (u, r) = self.wait_for_verification(
                source,
                url,
                title,
                use_browser,
                refetch_after_success,
                html,
                browser_load.clone(),
                cancel,
            )?;
            return Ok(VerifyOutcome::Response(u, r));
        };

        loop {
            let owner = self.acquire_flight(&flight_key);
            if !owner {
                // 别人正在过同一个盾:等它,然后**自己重抓**
                let waited = self.await_flight(&flight_key, cancel);
                self.release_flight(&flight_key);
                match waited? {
                    FlightWait::Retry => continue,
                    FlightWait::Completed => return Ok(VerifyOutcome::Refetch),
                }
            }
            let out = (|| {
                let _ui = self.lock_ui(cancel)?;
                self.wait_for_verification(
                    source,
                    url,
                    title,
                    use_browser,
                    refetch_after_success,
                    html,
                    browser_load.clone(),
                    cancel,
                )
            })();
            match out {
                Ok((u, r)) => {
                    self.complete_flight(&flight_key, None);
                    self.release_flight(&flight_key);
                    return Ok(VerifyOutcome::Response(u, r));
                }
                Err(e) => {
                    // 取消 = 放弃(等着的那些重来一轮);别的错传给它们
                    match &e {
                        VerifyError::Cancelled => self.abandon_flight(&flight_key),
                        _ => self.complete_flight(&flight_key, Some(e.to_string())),
                    }
                    self.release_flight(&flight_key);
                    return Err(e);
                }
            }
        }
    }

    /// `waitForVerification`:挂号 → 弹界面 → 轮询等结果
    #[allow(clippy::too_many_arguments)]
    fn wait_for_verification(
        &self,
        source: &SourceRef<'_>,
        url: &str,
        title: &str,
        use_browser: bool,
        refetch_after_success: bool,
        html: Option<&str>,
        browser_load: Option<BrowserLoad>,
        cancel: Option<&CancelFn>,
    ) -> Result<(String, String), VerifyError> {
        let key = self.register();
        self.ui.open(&VerifyOpen {
            kind: if use_browser { VerifyKind::Browser } else { VerifyKind::Code },
            key: Some(key.clone()),
            url: url.to_string(),
            title: title.to_string(),
            // 真身:走验证这条路时 `saveResult = true`(网页源码要存回结果)
            save_result: use_browser,
            refetch_after_success,
            html: html.map(str::to_string),
            source_key: source.key.to_string(),
            source_name: source.name.to_string(),
            source_type: source.kind,
            browser_load,
        });
        let out = self.poll_result(&key, cancel);
        if matches!(out, Err(VerifyError::Cancelled)) {
            // `cancelVerificationAttempt`:把还开着的界面关掉
            self.ui.close(&key);
        }
        self.clear(&key);
        out
    }

    fn poll_result(
        &self,
        key: &str,
        cancel: Option<&CancelFn>,
    ) -> Result<(String, String), VerifyError> {
        let mut g = self.inner.lock().expect("verify 锁");
        loop {
            if cancelled(cancel) {
                return Err(VerifyError::Cancelled);
            }
            if let Some((u, r)) = g.attempts.get(key).and_then(|a| a.result.clone()) {
                // 真身:结果为空(用户把界面关了)当错处理
                if r.is_empty() {
                    return Err(VerifyError::NoStackTrace("验证结果为空".into()));
                }
                return Ok((u, r));
            }
            let (guard, _) = self.wake.wait_timeout(g, POLL).expect("verify 锁");
            g = guard;
        }
    }

    // ---------------- 挂号表 ----------------

    fn register(&self) -> String {
        let mut g = self.inner.lock().expect("verify 锁");
        g.next_key += 1;
        // 真身是 UUID;这里够用即可,而且**可复现**(差分侧的观察面里有它)
        let key = format!("verify-{}", g.next_key);
        g.attempts.insert(key.clone(), Attempt::default());
        key
    }

    fn clear(&self, key: &str) {
        self.inner.lock().expect("verify 锁").attempts.remove(key);
    }

    // ---------------- 界面互斥 ----------------

    fn lock_ui(&self, cancel: Option<&CancelFn>) -> Result<UiGuard<'_>, VerifyError> {
        let mut g = self.inner.lock().expect("verify 锁");
        while g.ui_busy {
            if cancelled(cancel) {
                return Err(VerifyError::Cancelled);
            }
            let (guard, _) = self.wake.wait_timeout(g, POLL).expect("verify 锁");
            g = guard;
        }
        g.ui_busy = true;
        Ok(UiGuard(self))
    }

    fn unlock_ui(&self) {
        self.inner.lock().expect("verify 锁").ui_busy = false;
        self.wake.notify_all();
    }

    // ---------------- 飞行表 ----------------

    /// 返回 true = 这一趟由我领头(真身 `Lease.owner`)
    fn acquire_flight(&self, key: &str) -> bool {
        let mut g = self.inner.lock().expect("verify 锁");
        match g.flights.get_mut(key) {
            // 上一趟已经结束了 —— 开新的一趟(真身 `current.done.count == 0L`)
            Some(f) if !f.done => {
                f.users += 1;
                false
            }
            _ => {
                g.flights.insert(key.to_string(), Flight { users: 1, ..Flight::default() });
                true
            }
        }
    }

    fn await_flight(
        &self,
        key: &str,
        cancel: Option<&CancelFn>,
    ) -> Result<FlightWait, VerifyError> {
        let deadline = Instant::now() + FLIGHT_WAIT;
        let mut g = self.inner.lock().expect("verify 锁");
        loop {
            let Some(f) = g.flights.get(key) else {
                // 领头的收摊时把表清了 —— 当它成了
                return Ok(FlightWait::Completed);
            };
            if f.done {
                if let Some(e) = f.failed.clone() {
                    return Err(VerifyError::NoStackTrace(e));
                }
                return Ok(if f.abandoned { FlightWait::Retry } else { FlightWait::Completed });
            }
            if cancelled(cancel) {
                return Err(VerifyError::Cancelled);
            }
            if Instant::now() >= deadline {
                return Err(VerifyError::NoStackTrace("source verification timed out".into()));
            }
            let (guard, _) = self.wake.wait_timeout(g, POLL).expect("verify 锁");
            g = guard;
        }
    }

    fn complete_flight(&self, key: &str, error: Option<String>) {
        let mut g = self.inner.lock().expect("verify 锁");
        if let Some(f) = g.flights.get_mut(key) {
            f.done = true;
            f.failed = error;
        }
        self.wake.notify_all();
    }

    fn abandon_flight(&self, key: &str) {
        let mut g = self.inner.lock().expect("verify 锁");
        if let Some(f) = g.flights.get_mut(key) {
            f.done = true;
            f.abandoned = true;
        }
        self.wake.notify_all();
    }

    fn release_flight(&self, key: &str) {
        let mut g = self.inner.lock().expect("verify 锁");
        if let Some(f) = g.flights.get_mut(key) {
            f.users = f.users.saturating_sub(1);
            if f.users == 0 {
                g.flights.remove(key);
            }
        }
        self.wake.notify_all();
    }
}

enum FlightWait {
    Completed,
    Retry,
}

#[cfg(test)]
impl Verification {
    /// 飞行表上一共挂着几个人(只给测试用:并发那条判据要**确定地**编排
    /// 「第二个已经进来等着了」这一刻,不能靠 sleep 猜)
    fn flight_users(&self) -> u32 {
        self.inner.lock().expect("verify 锁").flights.values().map(|f| f.users).sum()
    }
}

/// 界面互斥锁的守卫(真身 `finally { verificationUiLock.unlock() }`)
struct UiGuard<'a>(&'a Verification);

impl Drop for UiGuard<'_> {
    fn drop(&mut self) {
        self.0.unlock_ui();
    }
}

/// 书源那三位(真身 `BaseSource.getKey/getTag/getSourceType`)
pub struct SourceRef<'a> {
    pub key: &'a str,
    pub name: &'a str,
    pub kind: i32,
}

fn cancelled(cancel: Option<&CancelFn>) -> bool {
    cancel.is_some_and(|c| c())
}

/// 真身 `require(url.length < 64 * 1024)`
fn check_url_len(url: &str) -> Result<(), VerifyError> {
    if url.len() >= 64 * 1024 {
        return Err(VerifyError::IllegalArgument(
            "getVerificationResult parameter url too long".into(),
        ));
    }
    Ok(())
}

/// `verificationFlightKey`:按「源 + 站点」归并 —— **地址里 `,{…}` 那段选项要先切掉**
/// (真身 `AnalyzeUrl.paramPattern`),不然同一个盾会因为选项不同弹两次
fn flight_key(source_key: &str, source_type: i32, url: &str) -> Option<String> {
    let request_url = match crate::param_split(url) {
        Some((start, _)) => &url[..start],
        None => url,
    };
    let http = crate::http_url::HttpUrl::parse(request_url.trim()).ok()?;
    Some(format!(
        "{}\u{0}{}\u{0}{}\u{0}{}\u{0}{}",
        source_type, source_key, http.scheme, http.host, http.port
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Answering {
        /// 界面一弹出来就替用户答上(差分侧的剧本也是这么干的)
        answer: Option<(String, String)>,
        opens: AtomicUsize,
        last_open: Mutex<Option<VerifyOpen>>,
        holder: Mutex<Option<Arc<Verification>>>,
    }

    impl VerifyUi for Answering {
        fn open(&self, req: &VerifyOpen) {
            self.opens.fetch_add(1, Ordering::SeqCst);
            *self.last_open.lock().unwrap() = Some(req.clone());
            let (Some(key), Some((u, r))) = (req.key.as_deref(), self.answer.clone()) else {
                return;
            };
            let v = self.holder.lock().unwrap().clone().expect("已装好");
            v.set_result(key, &u, &r);
        }
    }

    fn wired(answer: Option<(&str, &str)>) -> (Arc<Verification>, Arc<Answering>) {
        let ui = Arc::new(Answering {
            answer: answer.map(|(u, r)| (u.to_string(), r.to_string())),
            opens: AtomicUsize::new(0),
            last_open: Mutex::new(None),
            holder: Mutex::new(None),
        });
        let v = Arc::new(Verification::new(ui.clone()));
        *ui.holder.lock().unwrap() = Some(v.clone());
        (v, ui)
    }

    fn src() -> SourceRef<'static> {
        SourceRef { key: "https://a.example", name: "源", kind: 0 }
    }

    #[test]
    fn browser_await_returns_what_the_user_left_behind() {
        let (v, ui) = wired(Some(("https://a.example/final", "<html>ok</html>")));
        let out = v
            .get_verification_result(
                &src(),
                "https://a.example/p",
                "验证",
                true,
                true,
                None,
                None,
                None,
            )
            .unwrap();
        assert_eq!(
            out,
            VerifyOutcome::Response("https://a.example/final".into(), "<html>ok</html>".into())
        );
        assert_eq!(ui.opens.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn browser_load_plan_reaches_the_platform_unchanged() {
        let (v, ui) = wired(Some(("https://a.example/final", "<html>ok</html>")));
        let load = BrowserLoad {
            url: "https://m.a.example/search".into(),
            headers: vec![("Referer".into(), "https://m.a.example/".into())],
            user_agent: "Source-UA".into(),
            html: Some("<html>challenge</html>".into()),
        };
        v.get_verification_result(
            &src(),
            "https://m.a.example/search,{...}",
            "验证",
            true,
            true,
            None,
            Some(load.clone()),
            None,
        )
        .unwrap();
        assert_eq!(ui.last_open.lock().unwrap().as_ref().unwrap().browser_load, Some(load));
    }

    #[test]
    fn closing_the_page_is_an_error_not_an_empty_string() {
        // 用户把界面关掉 → 真身 `checkResult` 塞空串 → 「验证结果为空」
        let (v, _ui) = wired(Some(("", "")));
        let err = v
            .get_verification_result(
                &src(),
                "https://a.example/p",
                "验证",
                true,
                true,
                None,
                None,
                None,
            )
            .unwrap_err();
        assert_eq!(err, VerifyError::NoStackTrace("验证结果为空".into()));
        assert_eq!(err.class(), "NoStackTraceException");
    }

    #[test]
    fn cancel_gets_out_of_the_wait() {
        let (v, ui) = wired(None); // 界面永远不答
        let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(120));
            f.store(true, Ordering::SeqCst);
        });
        let g = flag.clone();
        let cancel: CancelFn = Arc::new(move || g.load(Ordering::SeqCst));
        let t = Instant::now();
        let err = v
            .get_verification_result(
                &src(),
                "https://a.example/p",
                "验证",
                true,
                true,
                None,
                None,
                Some(&cancel),
            )
            .unwrap_err();
        assert_eq!(err, VerifyError::Cancelled);
        assert!(t.elapsed() < Duration::from_secs(3), "没有当场收摊:{:?}", t.elapsed());
        assert_eq!(ui.opens.load(Ordering::SeqCst), 1);
    }

    /// 同一个盾撞上两次:第二个**不再弹界面**,等第一个过完**自己重抓**。
    ///
    /// 编排是确定的,不靠 sleep:界面那一头**等到第二个真的挂上飞行表**才答复
    /// —— 否则第一个可能在第二个进来之前就收摊了,那时第二个自己成了领头的
    /// (那也是真身的行为,只是不是这条判据要钉的那一支)。
    #[test]
    fn second_caller_joins_the_flight_and_refetches() {
        let holder: Arc<Mutex<Option<Arc<Verification>>>> = Arc::new(Mutex::new(None));
        struct WaitForSecond {
            holder: Arc<Mutex<Option<Arc<Verification>>>>,
            opens: AtomicUsize,
        }
        impl VerifyUi for WaitForSecond {
            fn open(&self, req: &VerifyOpen) {
                self.opens.fetch_add(1, Ordering::SeqCst);
                let Some(key) = req.key.as_deref() else { return };
                let v = self.holder.lock().unwrap().clone().expect("已装好");
                // 等第二个进来(飞行表上两个人)再答
                let t = Instant::now();
                while v.flight_users() < 2 && t.elapsed() < Duration::from_secs(5) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                v.set_result(key, "https://a.example/final", "<html>ok</html>");
            }
        }
        let ui = Arc::new(WaitForSecond { holder: holder.clone(), opens: AtomicUsize::new(0) });
        let v = Arc::new(Verification::new(ui.clone()));
        *holder.lock().unwrap() = Some(v.clone());

        let v2 = v.clone();
        let h = std::thread::spawn(move || {
            v2.get_verification_result(
                &SourceRef { key: "https://a.example", name: "源", kind: 0 },
                // 同一个站点、不同的路径 —— 归并按 (源, scheme, host, port) 算
                "https://a.example/p?x=1",
                "验证",
                true,
                true,
                None,
                None,
                None,
            )
        });
        let first = v
            .get_verification_result(
                &src(),
                "https://a.example/p",
                "验证",
                true,
                true,
                None,
                None,
                None,
            )
            .unwrap();
        let second = h.join().unwrap().unwrap();
        let outs = [first, second];
        // 一个拿到结果、一个被告知「自己重抓」,而**界面只弹过一次**
        assert!(outs.contains(&VerifyOutcome::Refetch), "{outs:?}");
        assert!(outs.iter().any(|o| matches!(o, VerifyOutcome::Response(..))), "{outs:?}");
        assert_eq!(ui.opens.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn flight_key_ignores_the_url_option_and_the_path() {
        let a = flight_key("k", 0, "https://a.example/p?x=1");
        let b = flight_key("k", 0, "https://a.example/q,{\"method\":\"POST\"}");
        assert_eq!(a, b);
        assert_ne!(a, flight_key("k", 0, "https://b.example/p"));
        assert_ne!(a, flight_key("k", 1, "https://a.example/p"));
        // 不是地址 → 不归并(真身 `toHttpUrlOrNull` 为 null 时 flightKey 也是 null)
        assert_eq!(flight_key("k", 0, "小说合集"), None);
    }
}
