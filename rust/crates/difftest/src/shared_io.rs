//! 一次 case 内**共享的** HTTP 传输与 cookie 库。
//!
//! 为什么要有它:裁判侧的 `ReplayHttp.hops` 与 `appDb.cookieDao` 都是**进程级
//! 单例** —— 四步流水线发的请求和书源 JS 里 `java.ajax` 发的请求落在同一张
//! hop 表上,存下的 cookie 也是同一份,后面的跳能带上前面 JS 拿到的 cookie。
//!
//! 被测侧原本不是:`pipeline::PipelineEnv` 握着 `&mut` 的 transport/cookies,
//! 而 `js_net::ReplayNet` 自己另建一份。于是 B 层书源里 `java.ajax` 的那一跳
//! **不进 `requests`**、它存下的 cookie **不进 `cookiesDb`**,也不会被后续请求
//! 带上 —— 三处都是静默的单边差异。用 `Rc<RefCell<…>>` 把两边接到同一份上。
//!
//! (产品侧 `engine::shared::JsEnv` 走的是另一条路:那边 JS 网络面自建连接池,
//! 因为 pipeline 的 transport 在整次调用里被 `&mut` 借着。差分侧不需要连接池,
//! 只需要「同一张 hop 表 + 同一个 cookie 库」,故这里用共享句柄。)

use net::transport::{HttpResponseRaw, HttpTransport, RequestBody, TransportError};
use rubato_core::host::{CookieEnv, SetCookie};
use std::cell::RefCell;
use std::rc::Rc;

use crate::replay::{RecordingTransport, ReplayTransport};

/// 本套的传输本体:录放 + 记 hop
pub type CaseTransport = RecordingTransport<ReplayTransport>;

#[derive(Clone)]
pub struct SharedTransport(pub Rc<CaseTransport>);

impl SharedTransport {
    pub fn new(t: CaseTransport) -> SharedTransport {
        SharedTransport(Rc::new(t))
    }

    /// 本 case 已发出的 `(method, url)` 序列
    pub fn hops(&self) -> Vec<(String, String)> {
        self.0.hops()
    }
}

impl HttpTransport for SharedTransport {
    fn execute_hop(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&RequestBody>,
    ) -> Result<HttpResponseRaw, TransportError> {
        self.0.execute_hop(method, url, headers, body)
    }
}

#[derive(Clone)]
pub struct SharedCookies(pub Rc<RefCell<store::CookieStore>>);

impl SharedCookies {
    pub fn open_in_memory() -> Result<SharedCookies, String> {
        Ok(SharedCookies(Rc::new(RefCell::new(
            store::CookieStore::open_in_memory().map_err(|e| format!("store:{e}"))?,
        ))))
    }
}

impl CookieEnv for SharedCookies {
    fn get_cookie(&mut self, url: &str) -> String {
        self.0.borrow_mut().get_cookie(url)
    }

    fn load_request_cookie(&mut self, url: &str, request_cookie: Option<&str>) -> Option<String> {
        self.0.borrow_mut().load_request_cookie(url, request_cookie)
    }

    fn save_response_cookies(&mut self, url: &str, cookies: &[SetCookie]) {
        self.0.borrow_mut().save_response_cookies(url, cookies)
    }

    fn set_cookie(&mut self, url: &str, cookie: &str) {
        CookieEnv::set_cookie(&mut *self.0.borrow_mut(), url, cookie)
    }

    fn replace_cookie(&mut self, url: &str, cookie: &str) {
        CookieEnv::replace_cookie(&mut *self.0.borrow_mut(), url, cookie)
    }

    fn remove_cookie(&mut self, url: &str) {
        CookieEnv::remove_cookie(&mut *self.0.borrow_mut(), url)
    }

    fn get_cookie_key(&mut self, url: &str, key: &str) -> String {
        CookieEnv::get_cookie_key(&mut *self.0.borrow_mut(), url, key)
    }
}
