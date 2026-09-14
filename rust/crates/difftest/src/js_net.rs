//! js-host 差分的**网络面装配**(被测侧)。本体在
//! [`js_host::net_face::JsNet`](与产品侧的 LiveNet 共用),这里只做差分侧的注入:
//! - transport / cookies 是**共享句柄**:pipeline 那条链与 `java.ajax` 这条链要
//!   落在同一张 hop 表、同一个 cookie 库上(裁判侧它们本来就是进程级单例),
//!   见 [`crate::shared_io`];
//! - 嵌套宿主带**确定性垫片**(config / determinism 决定随机与时钟,否则整套
//!   差分逐次都在变)与外层同一份的 cache / cookie 两张表;
//! - 构造错误按裁判的异常类别归一(见 [`construct_error`]);
//! - hops 观察面接 RecordingTransport(跳数与 URL 拼装是 ajax 类书源最容易
//!   两侧走岔的地方)。
//!
//! 裁判侧对应 `judge/jsharness`:那边挂的是 `AnalyzeUrl` 真身 + `ReplayHttp`;
//! 这边把 `net::AnalyzeUrl`(同一份移植)接到同一份 HTTP 快照上。
//! 契约 docs/http-snapshot.md。

use crate::replay::{RecordingTransport, ReplayTransport};
use crate::shared_io::{SharedCookies, SharedTransport};
use js_host::net_face::{JsNet, JsNetIo};
use rubato_core::RuleData;
use rubato_core::host::NetError;
use std::sync::Arc;

/// 嵌套宿主的环境。**每一样都必须与外层同一份**:config / 确定性垫片决定
/// 随机与时钟,cache / cookie 两张表在真身那里是进程级单例 —— 嵌套的 `<js>`
/// 里 `java.put` 写完,外层得读得到。
pub struct NestedEnv {
    pub config: js_host::java_api::HostConfig,
    pub determinism: Option<String>,
    pub cache: js_host::host_env::SharedMap,
    /// `cache.putFile/getFile` 那张(ACache),与 `cache` 是**两张表**
    pub cache_file: js_host::host_env::SharedMap,
    pub cookies: js_host::host_env::SharedMap,
}

pub type ReplayNet = JsNet<SharedTransport, SharedCookies>;

pub fn replay_net(
    root: std::path::PathBuf,
    fallback: Option<&str>,
    source_key: String,
    source_header: Option<Vec<(String, String)>>,
    enabled_cookie_jar: bool,
    rule_data: RuleData,
    nested: NestedEnv,
    wv_script: crate::wv_script::WvScript,
    verify: Option<Arc<net::verification::Verification>>,
    // 书源名与类型:弹界面时要往上写(裁判那边取的是 `source.getTag()` /
    // `getSourceType()`,两侧同一份 case 的 source)
    source_name: String,
    source_type: i32,
) -> Result<ReplayNet, String> {
    replay_net_with_io(
        SharedTransport::new(RecordingTransport::new(ReplayTransport::with_fallback(
            root, fallback,
        ))),
        SharedCookies::open_in_memory()?,
        source_key,
        source_header,
        enabled_cookie_jar,
        rule_data,
        nested,
        wv_script,
        verify,
        source_name,
        source_type,
    )
}

/// 传输与 cookie 库由调用方给(pipeline 那条链要与本宿主共用同一份)。
#[allow(clippy::too_many_arguments)]
pub fn replay_net_with_io(
    transport: SharedTransport,
    cookies: SharedCookies,
    source_key: String,
    source_header: Option<Vec<(String, String)>>,
    enabled_cookie_jar: bool,
    rule_data: RuleData,
    nested: NestedEnv,
    // `{"webView": true}` 那条路的剧本(case 的 `webview` 字段)
    wv_script: crate::wv_script::WvScript,
    // 「请用户出手」那三件的剧本用户(case 的 `verify` 字段);
    // `None` = 这一侧没接界面,三件都报「未接」
    verify: Option<Arc<net::verification::Verification>>,
    source_name: String,
    source_type: i32,
) -> Result<ReplayNet, String> {
    let hops_transport = transport.clone();
    let cookie_store = cookies.0.clone();
    Ok(JsNet::new(JsNetIo {
        transport,
        cookies,
        source_key,
        source_header,
        enabled_cookie_jar,
        rule_data,
        // url 里的 `<js>` / `@js:` 由一个**干净的**宿主求值:真身那里是同一个
        // Rhino 引擎,但不带网络(嵌套 ajax 不在本套的观察面上)
        nested_factory: Box::new(move || {
            let mut h = js_host::host_env::QuickJsHost::for_difftest(nested.config.clone());
            h.determinism = nested.determinism.clone();
            h.cache = nested.cache.clone();
            h.cache_file = nested.cache_file.clone();
            h.cookies = nested.cookies.clone();
            // `cookie` 绑定接真 CookieStore(与外层同一份,见 pipeline_case)
            h.cookie_store = Some(cookie_store.clone());
            h
        }),
        construct_error,
        hops_fn: Some(Box::new(move || hops_transport.hops())),
        // 「请用户出手」那三件:剧本用户在 `crate::verify_script`
        verify,
        source_name,
        source_type,
        cancel: None,
        // `{"webView": true}` 的 `java.ajax`:走真策略,平台那一半是剧本
        // (`nested.wv_script`,来自 case 的 `webview` 字段)
        web_view: Some(Box::new(move || {
            Box::new(crate::wv_script::ScriptedHost::new(wv_script.clone()))
        })),
    }))
}

/// `initUrl` 抛出来的错 → 裁判侧的异常类别。
///
/// url 里写 `{{$.id}}` 这类东西时,真身在 `replaceKeyPageJs` 里把它当 JS 跑,
/// `$` 未定义 → Rhino 的 `ReferenceError` → 被 `com.script` 包成
/// `ScriptException` → 裁判的 errorTag 走 `WrappedException → hostTag`
/// 落到 `host:ScriptException`(JSHARNESS_DEBUG=1 探出来的)。
fn construct_error(e: net::UrlError) -> NetError {
    match e {
        net::UrlError::Js(_) => NetError::Construct("«host:ScriptException»".into()),
        // 其余(切分/编码/页码越界)裁判侧是别的类;还没有用例踩到,
        // 先按「原样穿出去」处理 —— 真踩到会在差分上现形,不会静默。
        other => NetError::Construct(format!("{other:?}")),
    }
}
