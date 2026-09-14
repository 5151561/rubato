//! **PlatformHooks**:webView 的四件原语过 FFI 到 Dart(plan §1 那一行
//! 「Rust 不持有 webview」的落地处)。
//!
//! 策略那一半在 `net::webview`(进了差分,17 套里的第十七套钉它);这里只
//! 负责把 [`WebViewHost`] 的调用**搬到 Dart 那一侧**再把结果搬回来:
//!
//! ```text
//!   worker 线程                主 isolate
//!   ───────────                ─────────
//!   create/load  ──(单向流)──▶  headless webview 照做
//!   eval/cookie  ──(单向流)──▶  照做
//!                ◀─(sync 调用)─  web_view_reply(req_id, json)
//!   next_event   ◀─(sync 调用)─  web_view_event(session, json)
//! ```
//!
//! 三条约定,改之前先读:
//!
//! 1. **只有取值的两件要回复**(`eval` / `page_cookie`)。`create` / `load` /
//!    `eval_void` / `destroy` 在真身那边本来就是异步的(`loadUrl` /
//!    `loadUrl("javascript:…")` / `WebViewPool.release`),这里照样**发了就走**
//!    —— 流是有序的,Dart 侧按 session 串行执行,顺序因此不丢。
//! 2. **虚拟原点是「页面加载完成那一刻」**,不是 `loadUrl` 那一刻。策略层的
//!    时钟从 0 起算、事件不占虚拟时间,而真身 `postDelayed(100 + delayTime)`
//!    的基准是 `onPageFinished` —— 差分侧两种锚法等价(事件都发生在虚拟时刻
//!    0),产品侧只有后者对得上真身的排期。锚错了重试梯子会在真机上瞬间跑完。
//! 3. **等待有上限**:`next_event` 最多等 [`PAGE_WAIT_MS`](对齐真身
//!    `withTimeout(timeout ?: 60000)` 的缺省),等不到就是 `None` ——
//!    策略层据此报 `WvError::Timeout`,与真身「页面一直没加载完」同一档。
//!    **已知偏差**:`@webjs:` 那条路真身的预算是 10 000 ms 而不是 60 000
//!    —— 平台原语面里没有这一位(`WebViewSettings` 是差分判据面,不能为它
//!    加字段),于是那条路的超时会晚到,**报的错仍是同一档**(语料里
//!    `@webjs:` 0 源,见 fixtures/phase3/README.md)。
//!
//! 掉线(Dart 侧 [`close`] 或热重启后重新 [`open`])一律把在等的调用**唤醒**:
//! 挂号被清掉的那一刻,等的人拿到「没有回复」而不是干等到超时。

use net::verification::{VerifyKind, VerifyOpen, VerifyUi};
use rubato_core::host::{
    CancelFn, WebViewEvent, WebViewHost, WebViewLoad, WebViewProvider, WebViewSettings,
};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// `next_event` 的等待上限:真身 `withTimeout(timeout ?: 60000)` 的缺省值
pub const PAGE_WAIT_MS: u64 = 60_000;
/// 取值调用(`eval` / `page_cookie`)等回复的上限。真身那两件是毫秒级的,
/// 这个数只是「Dart 那侧死了」的兜底刹车
pub const CALL_WAIT_MS: u64 = 30_000;

/// 等待切成多长一片。**取消是靠分片看见的**:令牌是别的线程置位的,
/// 条件变量那边没有它的通知口 —— 所以每片醒一次、问一次。
/// 100 ms 对「点了停多久收摊」是够快的,对空转也便宜(一秒十次)。
const SLICE_MS: u64 = 100;

/// 一次要 Dart 做的事。`req_id == 0` = 不用回复
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WvCall {
    pub req_id: i64,
    pub session: i64,
    pub op: WvOp,
    /// 入参 JSON(按 [`WvOp`] 定形,见各处 `json!` 的构造点)
    pub json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WvOp {
    /// `createWebView()`:`{"userAgent":…,"blockNetworkImage":…,"cacheMode":…}`
    Create,
    /// `loadUrl` / `loadDataWithBaseURL`:
    /// `{"url":…|null,"html":…|null,"encoding":…,"headers":[[k,v]…]}`
    Load,
    /// `evaluateJavascript`:`{"js":…}` → 回复是**原样串**的 JSON 编码
    Eval,
    /// `loadUrl("javascript:…")`:`{"js":…}`,不收结果
    EvalVoid,
    /// `CookieManager.getCookie(url)`:`{"url":…}` → 回复是 `null` 或串
    Cookie,
    /// `WebViewPool.release`:`{}`
    Destroy,
}

#[derive(Default)]
struct Bridge {
    /// Dart 侧登记的出口(`None` = 没接平台)
    sink: Option<Box<dyn Fn(WvCall) + Send + Sync>>,
    /// 挂号即插 `None`;**键不在** = 这一号已经作废(掉线),别再等
    replies: HashMap<i64, Option<Value>>,
    events: HashMap<i64, VecDeque<WebViewEvent>>,
}

static NEXT_ID: AtomicI64 = AtomicI64::new(1);
static NEXT_SESSION: AtomicI64 = AtomicI64::new(1);

fn state() -> &'static (Mutex<Bridge>, Condvar) {
    static STATE: OnceLock<(Mutex<Bridge>, Condvar)> = OnceLock::new();
    STATE.get_or_init(|| (Mutex::new(Bridge::default()), Condvar::new()))
}

fn lock() -> std::sync::MutexGuard<'static, Bridge> {
    state().0.lock().expect("platform 锁")
}

/// Dart 侧登记平台出口(打开那条流)。重复调用换掉旧的 —— 热重启就是这一路,
/// 在等旧出口回复的调用当场被唤醒。
pub fn open(sink: Box<dyn Fn(WvCall) + Send + Sync>) {
    let mut b = lock();
    b.sink = Some(sink);
    b.replies.clear();
    b.events.clear();
    state().1.notify_all();
}

/// Dart 侧收摊(流关了 / app 退到后台)。此后 [`available`] 为 false。
pub fn close() {
    let mut b = lock();
    b.sink = None;
    b.replies.clear();
    b.events.clear();
    state().1.notify_all();
}

/// 现在有没有平台接着
pub fn available() -> bool {
    lock().sink.is_some()
}

/// Dart 侧交回一次取值的结果。`json` 是**回复值的 JSON**
/// (`eval` 给串、`page_cookie` 给串或 `null`);解不出来当没回复。
pub fn reply(req_id: i64, json: &str) {
    let v: Value = serde_json::from_str(json).unwrap_or(Value::Null);
    let mut b = lock();
    // 键已经不在 = 这一号作废了(掉线/超时),丢掉即可
    if let Some(slot) = b.replies.get_mut(&req_id) {
        *slot = Some(v);
        state().1.notify_all();
    }
}

/// Dart 侧送一个 WebView 事件上来。形态见 [`parse_event`];
/// 不认识的一律忽略(平台将来多送几种,不该把这一侧带崩)。
pub fn event(session: i64, json: &str) {
    let Some(ev) = serde_json::from_str::<Value>(json).ok().and_then(|v| parse_event(&v)) else {
        return;
    };
    let mut b = lock();
    b.events.entry(session).or_default().push_back(ev);
    state().1.notify_all();
}

/// `{"type":"overrideUrl"|"loadResource"|"pageFinished","url":…,"isRedirect":…}`
fn parse_event(v: &Value) -> Option<WebViewEvent> {
    let url = v.get("url").and_then(Value::as_str).unwrap_or_default().to_string();
    match v.get("type").and_then(Value::as_str)? {
        "overrideUrl" => Some(WebViewEvent::OverrideUrl {
            url,
            is_redirect: v.get("isRedirect").and_then(Value::as_bool).unwrap_or(false),
        }),
        "loadResource" => Some(WebViewEvent::LoadResource { url }),
        "pageFinished" => Some(WebViewEvent::PageFinished { url }),
        _ => None,
    }
}

/// 发一件**不用回复**的事;没接平台就是空操作
fn send(session: i64, op: WvOp, json: Value) {
    let b = lock();
    if let Some(sink) = b.sink.as_ref() {
        sink(WvCall { req_id: 0, session, op, json: json.to_string() });
    }
}

/// 发一件**要回复**的事并等它。`None` = 没接平台 / 掉线 / 等超时。
fn call(session: i64, op: WvOp, json: Value) -> Option<Value> {
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let mut b = lock();
    // 没接平台:一件都发不出去
    b.sink.as_ref()?;
    // 挂号在**锁里**排在发出去之后:Dart 那侧要回复也得先拿这把锁,
    // 所以「回复早于挂号」不可能发生
    b.sink.as_ref().expect("上面判过")(
        WvCall { req_id: id, session, op, json: json.to_string() },
    );
    b.replies.insert(id, None);
    let deadline = Duration::from_millis(CALL_WAIT_MS);
    let start = Instant::now();
    loop {
        // 挂号没了(`?` 这一支)= 掉线时被清掉,别再等
        if b.replies.get(&id)?.is_some() {
            return b.replies.remove(&id).flatten();
        }
        let Some(left) = deadline.checked_sub(start.elapsed()) else {
            b.replies.remove(&id);
            return None;
        };
        let (guard, _) = state().1.wait_timeout(b, left).expect("platform 锁");
        b = guard;
    }
}

/// 产品侧的 [`WebViewProvider`]:桥这一头。`available()` 跟着 Dart 侧
/// 登记与否走 —— 没登记时调用方就该走 `WebViewUnsupported` 那一档。
pub struct PlatformProvider;

impl WebViewProvider for PlatformProvider {
    fn available(&self) -> bool {
        available()
    }

    fn acquire(&self, cancel: Option<CancelFn>) -> Box<dyn WebViewHost> {
        Box::new(PlatformWebView::new(cancel))
    }
}

// ---------------------------------------------------------------- PlatformHooks:请用户出手
//
// `java.startBrowser` / `startBrowserAwait` / `getVerificationCode` 那三件的
// **界面**那一头。策略(挂号、界面互斥、同一个盾只弹一次、等待、取消)在
// `net::verification`,两侧共用;这里只是「把界面弹出来 / 关掉」这两句话过桥。
//
// 与 webView 那条桥的不同:**没有回复**。界面做完之后由 Dart 侧调
// [`verify_result`](FRB `verifySetResult`)把结果塞回策略层的挂号表 ——
// 真身 `WebViewActivity` 回头调 `SourceVerificationHelp.setResult` 就是这个形状。

/// 一次要 Dart 弹的界面
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyCall {
    /// 挂号 key;空串 = `startBrowser` 那条**不等结果**的路
    pub key: String,
    pub op: VerifyOp,
    /// 入参 JSON:`{"url","title","kind","saveResult","refetchAfterSuccess",
    /// "html","sourceKey","sourceName","sourceType","request"}`
    pub json: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyOp {
    /// 弹界面(`kind` 是 `browser` 还是 `code`)
    Open,
    /// 我们不等了(取消 / 超时):把还开着的那个关掉
    Close,
}

#[derive(Default)]
struct VerifyBridge {
    sink: Option<Box<dyn Fn(VerifyCall) + Send + Sync>>,
}

fn verify_state() -> &'static Mutex<VerifyBridge> {
    static S: OnceLock<Mutex<VerifyBridge>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(VerifyBridge::default()))
}

/// Dart 侧登记界面出口(订上那条流)
pub fn verify_open(sink: Box<dyn Fn(VerifyCall) + Send + Sync>) {
    verify_state().lock().expect("verify 桥锁").sink = Some(sink);
}

/// Dart 侧收摊
pub fn verify_close() {
    verify_state().lock().expect("verify 桥锁").sink = None;
}

pub fn verify_available() -> bool {
    verify_state().lock().expect("verify 桥锁").sink.is_some()
}

/// 产品侧的 [`VerifyUi`]:把 `open`/`close` 过桥交给 Dart。
///
/// **没接界面时报「未接」**(`available()` 为假,调用方据此走
/// `NET_UNSUPPORTED`)—— 与 webView 那条同理:弹不出窗还去等,就是让书源
/// 等一个永远不来的答复。
pub struct PlatformVerifyUi;

impl VerifyUi for PlatformVerifyUi {
    fn open(&self, req: &VerifyOpen) {
        let json = json!({
            "kind": match req.kind {
                VerifyKind::Browser => "browser",
                VerifyKind::Code => "code",
            },
            "url": req.url,
            "title": req.title,
            "saveResult": req.save_result,
            "refetchAfterSuccess": req.refetch_after_success,
            "html": req.html,
            "sourceKey": req.source_key,
            "sourceName": req.source_name,
            "sourceType": req.source_type,
            "request": req.browser_load.as_ref().map(|load| json!({
                "url": load.url,
                "headers": load.headers,
                "userAgent": load.user_agent,
                "html": load.html,
            })),
        });
        send_verify(VerifyCall {
            key: req.key.clone().unwrap_or_default(),
            op: VerifyOp::Open,
            json: json.to_string(),
        });
    }

    fn close(&self, key: &str) {
        send_verify(VerifyCall {
            key: key.to_string(),
            op: VerifyOp::Close,
            json: "{}".to_string(),
        });
    }

    fn available(&self) -> bool {
        verify_available()
    }
}

fn send_verify(call: VerifyCall) {
    let b = verify_state().lock().expect("verify 桥锁");
    if let Some(sink) = b.sink.as_ref() {
        sink(call);
    }
}

/// 进程里那一个界面出口
pub fn verify_ui() -> Arc<dyn VerifyUi> {
    static U: OnceLock<Arc<dyn VerifyUi>> = OnceLock::new();
    U.get_or_init(|| Arc::new(PlatformVerifyUi)).clone()
}

/// 进程里那一个(`Arc` 只是为了给注入方 clone,它本身没有状态 ——
/// 状态全在 [`state`] 那张表上)
pub fn provider() -> Arc<dyn WebViewProvider> {
    static P: OnceLock<Arc<dyn WebViewProvider>> = OnceLock::new();
    P.get_or_init(|| Arc::new(PlatformProvider)).clone()
}

/// 一个 webview 会话(真身:从池子里 acquire 出来的那一个 WebView)
pub struct PlatformWebView {
    session: i64,
    /// 时间轴的原点:**`loadUrl` 那一刻**。事件的时刻与策略层的排期
    /// 必须在同一根轴上(见 `net::webview` 那条循环)
    origin: Instant,
    destroyed: bool,
    /// 用户点停了吗(`None` = 这条路上没有取消)
    cancel: Option<CancelFn>,
}

/// 睡到某一刻(已经过了就不睡);中途取消就提前回来
fn sleep_until(target: Instant, cancel: Option<&CancelFn>) {
    while let Some(left) = target.checked_duration_since(Instant::now()) {
        if cancel.is_some_and(|c| c()) {
            return;
        }
        std::thread::sleep(left.min(Duration::from_millis(SLICE_MS)));
    }
}

impl PlatformWebView {
    fn new(cancel: Option<CancelFn>) -> PlatformWebView {
        PlatformWebView {
            session: NEXT_SESSION.fetch_add(1, Ordering::SeqCst),
            origin: Instant::now(),
            destroyed: false,
            cancel,
        }
    }
}

impl WebViewHost for PlatformWebView {
    fn create(&mut self, settings: &WebViewSettings) {
        send(
            self.session,
            WvOp::Create,
            json!({
                "userAgent": settings.user_agent,
                "blockNetworkImage": settings.block_network_image,
                "cacheMode": settings.cache_mode,
            }),
        );
    }

    fn load(&mut self, load: &WebViewLoad) {
        self.origin = Instant::now();
        let headers: Vec<Value> = load.headers.iter().map(|(k, v)| json!([k, v])).collect();
        send(
            self.session,
            WvOp::Load,
            json!({
                "url": load.url,
                "html": load.html,
                "encoding": load.encoding,
                "headers": headers,
            }),
        );
    }

    /// 等到「加载起点 + `until_ms`」,期间 Dart 送上来的事件先交回去。
    ///
    /// **时间轴的原点是 `loadUrl` 那一刻**(不是页面加载完那一刻):排期由策略层
    /// 算(`100 + delayTime`、重试梯子),而「后来的 onPageFinished 会把排期重排」
    /// 这件事要求事件与排期在**同一根轴**上,原点只能有一个。
    ///
    /// `None` = 到 `until_ms` 了也没有新事件(或平台掉线 —— 那时策略层等到
    /// 自己的 deadline 就报超时)。
    fn next_event_until(&mut self, until_ms: i64) -> Option<(i64, WebViewEvent)> {
        let target = Duration::from_millis(until_ms.max(0) as u64);
        let mut b = lock();
        loop {
            // 掉线之后不会再有事件了 —— 但**仍然要等到那一刻**,不然策略层会
            // 把「平台没了」读成「排期到了」,当场去求值
            if b.sink.is_none() {
                drop(b);
                sleep_until(self.origin + target, self.cancel.as_ref());
                return None;
            }
            if self.cancelled() {
                return None;
            }
            if let Some(ev) = b.events.get_mut(&self.session).and_then(VecDeque::pop_front) {
                return Some((self.origin.elapsed().as_millis() as i64, ev));
            }
            let left = target.checked_sub(self.origin.elapsed())?;
            // **分片等**:取消令牌是别的线程置位的,条件变量这边没有它的通知口
            let (guard, _) = state()
                .1
                .wait_timeout(b, left.min(Duration::from_millis(SLICE_MS)))
                .expect("platform 锁");
            b = guard;
        }
    }

    fn eval(&mut self, js: &str) -> String {
        // 回不来(掉线/超时)按「这一轮没结果」算 —— 与页面上 JS 返回 null
        // 同一档,策略层的重试梯子照常往下走
        call(self.session, WvOp::Eval, json!({ "js": js }))
            .as_ref()
            .and_then(Value::as_str)
            .unwrap_or("null")
            .to_string()
    }

    fn eval_void(&mut self, js: &str) {
        send(self.session, WvOp::EvalVoid, json!({ "js": js }));
    }

    fn page_cookie(&mut self, url: &str) -> Option<String> {
        call(self.session, WvOp::Cookie, json!({ "url": url }))
            .as_ref()
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    fn cancelled(&self) -> bool {
        self.cancel.as_ref().is_some_and(|c| c())
    }

    fn destroy(&mut self) {
        if self.destroyed {
            return;
        }
        self.destroyed = true;
        lock().events.remove(&self.session);
        send(self.session, WvOp::Destroy, json!({}));
    }
}

impl Drop for PlatformWebView {
    /// 策略层三条出口都会 `destroy`,但 panic / 提前返回的路它盖不住 ——
    /// 漏一个就是 Dart 那侧一个永不回收的 headless 实例
    fn drop(&mut self) {
        self.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// 桥是**进程级**的一份,测试之间必须串起来跑
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static M: Mutex<()> = Mutex::new(());
        // 前一条测试 panic 会毒化这把锁,而它只是个排队器
        M.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 装一个「Dart」:把收到的调用推进 channel,由测试自己决定怎么回。
    fn fake_dart() -> mpsc::Receiver<WvCall> {
        let (tx, rx) = mpsc::channel();
        open(Box::new(move |c| {
            let _ = tx.send(c);
        }));
        rx
    }

    fn fake_verify_dart() -> mpsc::Receiver<VerifyCall> {
        let (tx, rx) = mpsc::channel();
        verify_open(Box::new(move |c| {
            let _ = tx.send(c);
        }));
        rx
    }

    #[test]
    fn visible_browser_request_keeps_source_ua_and_prefetched_html() {
        let _g = serial();
        let rx = fake_verify_dart();
        PlatformVerifyUi.open(&VerifyOpen {
            kind: VerifyKind::Browser,
            key: Some("k".into()),
            url: "https://m.a.example/search,{...}".into(),
            title: "验证".into(),
            save_result: true,
            refetch_after_success: true,
            html: None,
            source_key: "https://a.example".into(),
            source_name: "源".into(),
            source_type: 0,
            browser_load: Some(net::verification::BrowserLoad {
                url: "https://m.a.example/search".into(),
                headers: vec![("Referer".into(), "https://m.a.example/".into())],
                user_agent: "Source-UA".into(),
                html: Some("<html>challenge</html>".into()),
            }),
        });
        let call = rx.recv().expect("收到验证界面调用");
        let json: Value = serde_json::from_str(&call.json).unwrap();
        assert_eq!(json["request"]["url"], "https://m.a.example/search");
        assert_eq!(json["request"]["userAgent"], "Source-UA");
        assert_eq!(json["request"]["headers"][0][0], "Referer");
        assert_eq!(json["request"]["html"], "<html>challenge</html>");
        verify_close();
    }

    #[test]
    fn no_platform_means_unavailable() {
        let _g = serial();
        close();
        assert!(!available());
        let mut wv = PlatformWebView::new(None);
        // 没接平台时取值调用不阻塞,按「没结果」走
        assert_eq!(wv.eval("1"), "null");
        assert_eq!(wv.page_cookie("http://a/"), None);
        close();
    }

    #[test]
    fn eval_round_trip() {
        let _g = serial();
        let rx = fake_dart();
        let mut wv = PlatformWebView::new(None);
        let h = std::thread::spawn(move || wv.eval("document.title"));
        let call = rx.recv().expect("收到调用");
        assert_eq!(call.op, WvOp::Eval);
        assert!(call.req_id > 0);
        assert_eq!(serde_json::from_str::<Value>(&call.json).unwrap()["js"], "document.title");
        // Dart 侧交回的是**原样串**的 JSON 编码
        reply(call.req_id, "\"\\\"标题\\\"\"");
        assert_eq!(h.join().unwrap(), "\"标题\"");
        close();
    }

    #[test]
    fn cookie_null_is_none() {
        let _g = serial();
        let rx = fake_dart();
        let mut wv = PlatformWebView::new(None);
        let h = std::thread::spawn(move || wv.page_cookie("http://a/"));
        let call = rx.recv().expect("收到调用");
        assert_eq!(call.op, WvOp::Cookie);
        reply(call.req_id, "null");
        assert_eq!(h.join().unwrap(), None);
        close();
    }

    #[test]
    fn events_arrive_in_order_on_one_timeline() {
        let _g = serial();
        let rx = fake_dart();
        let mut wv = PlatformWebView::new(None);
        wv.load(&WebViewLoad { url: Some("http://a/".into()), ..Default::default() });
        let session = rx.recv().expect("load").session;
        event(session, r#"{"type":"overrideUrl","url":"http://b/","isRedirect":true}"#);
        event(session, r#"{"type":"pageFinished","url":"http://c/"}"#);
        event(session, r#"{"type":"nonsense"}"#);
        // 时刻从 `loadUrl` 那一刻起算,**不因页面加载完而重定原点** ——
        // 「后来的 onPageFinished 把求值重排」要求事件与排期在同一根轴上
        let deadline = 60_000;
        assert_eq!(
            wv.next_event_until(deadline).map(|(_, e)| e),
            Some(WebViewEvent::OverrideUrl { url: "http://b/".into(), is_redirect: true })
        );
        assert_eq!(
            wv.next_event_until(deadline).map(|(_, e)| e),
            Some(WebViewEvent::PageFinished { url: "http://c/".into() })
        );
        // 认不出来的那条被丢了,队列空 —— 到点(这里给 0,即「已经过了」)交 None
        assert_eq!(wv.next_event_until(0), None);
        close();
    }

    /// **点了停就别再等**:这一步可以长达 60 秒(等页面加载完),
    /// 而取消在别处是「步与步之间」的粒度 —— 从前点完停还得再等一分钟
    #[test]
    fn cancel_cuts_the_wait_short() {
        let _g = serial();
        let rx = fake_dart();
        let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f = flag.clone();
        let mut wv = PlatformWebView::new(Some(Arc::new(move || f.load(Ordering::SeqCst))));
        wv.load(&WebViewLoad { url: Some("http://a/".into()), ..Default::default() });
        rx.recv().expect("load");
        let g = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(120));
            g.store(true, Ordering::SeqCst);
        });
        let t = Instant::now();
        // 等的是 60 秒,但令牌 120 ms 之后就置位了
        assert_eq!(wv.next_event_until(60_000), None);
        assert!(wv.cancelled());
        assert!(t.elapsed() < Duration::from_secs(3), "没有当场收摊:{:?}", t.elapsed());
        close();
    }

    /// 到点之前不交 None:策略层拿 `None` 当「排期到了,去求值」——
    /// 早交一步就等于把重试梯子瞬间跑完
    #[test]
    fn waits_until_the_deadline_before_saying_nothing_came() {
        let _g = serial();
        let rx = fake_dart();
        let mut wv = PlatformWebView::new(None);
        wv.load(&WebViewLoad { url: Some("http://a/".into()), ..Default::default() });
        rx.recv().expect("load");
        let t = Instant::now();
        assert_eq!(wv.next_event_until(60), None);
        assert!(t.elapsed() >= Duration::from_millis(55), "没等够:{:?}", t.elapsed());
        close();
    }

    #[test]
    fn destroy_is_once_and_drop_covers_the_leak() {
        let _g = serial();
        let rx = fake_dart();
        {
            let mut wv = PlatformWebView::new(None);
            wv.destroy();
            wv.destroy();
        } // Drop 再来一次也不该多发
        let ops: Vec<WvOp> = rx.try_iter().map(|c| c.op).collect();
        assert_eq!(ops, vec![WvOp::Destroy]);
        close();
    }

    #[test]
    fn close_wakes_the_waiter() {
        let _g = serial();
        let rx = fake_dart();
        let mut wv = PlatformWebView::new(None);
        let h = std::thread::spawn(move || wv.eval("1"));
        rx.recv().expect("收到调用");
        close();
        assert_eq!(h.join().unwrap(), "null");
    }
}
