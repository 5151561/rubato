//! **产品侧的 JS 网络面装配**:`java.ajax` / `connect` / `get`/`head`/`post`
//! 真的发请求。本体在 [`js_host::net_face::JsNet`](与差分侧的 ReplayNet 共用,
//! 两者只差注入的四样),这里只做产品侧的注入:
//! - transport 是全引擎共享的那份连接池([`SharedTransport`]);
//! - 嵌套宿主带**取消钩子**(用户点「停」,卡在 ajax url 里的那段 JS 当场停);
//! - 构造错误原样穿出(差分侧才需要归一成裁判的异常类别);
//! - 无 hops 观察面(LiveTransport 没有记录器 —— 将来做书源调试页时要的是
//!   真实跳数,那时给它加记录器,而不是给一份「按调用点猜的」近似列表)。
//!
//! 从前这一面在产品里整个是空的:`QuickJsHost::net` 恒为 `None`,于是
//! `java.ajax` 返回字面串 `«net-unsupported»` —— 不报错、不落日志,**会被当正文
//! 拼进去**。差分侧的标记契约漏进了产品行为,而 `ajax` 是语料 B 层第一名(116 源)。

use js_host::net_face::{JsNet, JsNetIo};
use net::transport::HttpTransport;
use rubato_core::RuleData;
use rubato_core::host::{CookieEnv, NetError, WebViewHost, WebViewProvider};
use std::rc::Rc;
use std::sync::Arc;

/// 全引擎共享的传输层:`HttpTransport::execute_hop` 是 `&self`
/// (reqwest 的 Client 本来就是 `Sync` 的连接池),一个 `LiveTransport`
/// 走全场 —— pipeline、并发搜索的各 worker、JS 网络面共用同一份连接池。
pub type SharedTransport = std::sync::Arc<dyn HttpTransport + Send + Sync>;

/// 嵌套宿主要跟外层同处**一个环境**:两张表是进程级单例的替身,
/// 中断钩子是用户点「停」的那一路。少传一样就是一个洞 ——
/// cache 少传,`java.put` 写在 url 的 `<js>` 里就出门即丢;
/// cancel 少传,卡在 ajax url 里的那段 JS 要等 [`JsLimits`] 的 30 秒默认闸。
///
/// [`JsLimits`]: js_host::host_env::JsLimits
pub struct NestedEnv {
    pub config: js_host::java_api::HostConfig,
    pub cache: js_host::host_env::SharedMap,
    /// `cache.putFile/getFile` 那张(ACache),与 `cache` 是**两张表**
    pub cache_file: js_host::host_env::SharedMap,
    pub cancel: Option<Rc<dyn Fn() -> bool>>,
}

pub type LiveNet = JsNet<SharedTransport, Box<dyn CookieEnv>>;

#[allow(clippy::too_many_arguments)]
pub fn live_net(
    transport: SharedTransport,
    cookies: Box<dyn CookieEnv>,
    source_key: String,
    source_header: Option<Vec<(String, String)>>,
    enabled_cookie_jar: bool,
    rule_data: RuleData,
    nested: NestedEnv,
    web_view: Option<Arc<dyn WebViewProvider>>,
    cancel: rubato_core::host::CancelFn,
    verify: Option<Arc<net::verification::Verification>>,
    source_name: String,
    source_type: i32,
) -> LiveNet {
    // `cancel` 被 webView 那条工厂闭包吃掉,验证那一头也要一份
    let cancel2 = cancel.clone();
    JsNet::new(JsNetIo {
        transport,
        cookies,
        source_key,
        source_header,
        enabled_cookie_jar,
        rule_data,
        // 嵌套宿主**不给网络面**:真身没有这个限制,但语料里没有「ajax url 的
        // `<js>` 里再 ajax」的写法(差分侧 ReplayNet 同样如此)。从前还有一条
        // 硬约束(transport 的 `borrow_mut` 会重入),`&self` 化之后只剩语料这条。
        nested_factory: Box::new(move || {
            let mut h = js_host::host_env::QuickJsHost::with_config(nested.config.clone());
            // 两张表与外层同一份(真身那两个是进程级单例),中断钩子照样接上
            h.cache = nested.cache.clone();
            h.cache_file = nested.cache_file.clone();
            h.cancel = nested.cancel.clone();
            h
        }),
        construct_error: |e| NetError::Construct(format!("{e:?}")),
        hops_fn: None,
        // webView 平台原语(PlatformHooks)。**现在要得到才给** ——
        // 要不到时 `{"webView": true}` 的 `java.ajax` 仍报「未接」,
        // 与从前那一档同串
        web_view: web_view.filter(|p| p.available()).map(
            |p| -> Box<dyn Fn() -> Box<dyn WebViewHost>> {
                Box::new(move || p.acquire(Some(cancel.clone())))
            },
        ),
        // 「请用户出手」那三件(startBrowser / startBrowserAwait /
        // getVerificationCode):界面在 Dart 侧,策略在 net::verification
        verify,
        source_name,
        source_type,
        cancel: Some(cancel2),
    })
}
