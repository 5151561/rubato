//! 回放客户端循环:okhttp 调用链里影响请求形态的部分摊平
//! (契约 docs/http-snapshot.md §7.1;裁判侧等价物 harness/ReplayHttp.kt)。

use crate::http_url::HttpUrl;
use crate::transport::{
    HttpResponseRaw, HttpTransport, RequestBody, TransportError, header_get_ci,
};
use rubato_core::host::CookieEnv;
use std::sync::OnceLock;

pub const COOKIE_JAR_HEADER: &str = "CookieJar";
const MAX_FOLLOW_UPS: usize = 20;

/// 产品的默认 UA(`AppConfig.userAgent` 那一格,judge/engine AppConfig.kt L804)。
///
/// 真身默认是
/// `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko)
/// Chrome/<BuildConfig.Cronet_Main_Version> Safari/537.36` —— 版本号来自它自己的
/// 构建常量,复刻不了也不该猜,**形状照抄、版本我们自己钉一个**。
/// 书源是冲着浏览器 UA 调的(不少站点对陌生 UA 直接换一套响应),所以这一格
/// 跟着真身走,而不是自报家门。
///
/// **此前这里是 `"rubato-judge"`** —— 差分的固定值漏进了产品,真实请求都带着
/// 裁判的名字出门。现在两者分开:产品拿这一个,差分在 main 里改成固定值(见下)。
const PRODUCT_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

/// 进程级的 UA 配置。**真身就是这个形状** —— `AppConfig` 是个单例,
/// 两处消费同一格:`BaseSource.getHeaderMap()` 的「没写 UA 就补一个」,
/// 与 okhttp 拦截器里同一句兜底(下面 `execute_call` 开头那段)。
static USER_AGENT: OnceLock<String> = OnceLock::new();

/// 差分侧在 `main` 头一行调它钉成固定值。
///
/// **为什么差分一定要钉**:UA 进 HTTP 快照的 key(docs/http-snapshot.md §3
/// ——「UA / Referer / Content-Type 会真实改变响应形态,必须进 key」),
/// 而裁判侧 UA 由 `AppConfig` 垫片给死(`harness/shims`,`"rubato-judge"`)。
/// 两侧对不上就是两个 key,被测侧命中不了录好的快照 —— 漏调这一句不会静默,
/// `pipeline` 那套的 42 张录制快照会当场红。
pub fn set_user_agent(ua: &str) {
    let _ = USER_AGENT.set(ua.to_string());
}

/// 当前生效的 UA(`AppConfig.userAgent`)
pub fn user_agent() -> &'static str {
    USER_AGENT.get().map(String::as_str).unwrap_or(PRODUCT_UA)
}

#[derive(Debug, Clone)]
pub struct CallResult {
    /// 终态那一跳的请求 URL(okhttp 规范化后)
    pub final_url: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// 每跳 (method, 规范化 URL)——流水线差分的「请求序列」
    pub hops: Vec<(String, String)>,
    /// `raw.priorResponse?.isRedirect == true`(至少跟进过一次重定向)
    pub prior_is_redirect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    Transport(TransportError),
    TooManyRedirects,
    /// urlNoQuery 无法按 okhttp HttpUrl 解析(toHttpUrl 抛 IllegalArgumentException)
    BadUrl(String),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallError::Transport(e) => write!(f, "{e}"),
            CallError::TooManyRedirects => write!(f, "too_many_redirects"),
            CallError::BadUrl(u) => write!(f, "bad_url:{u}"),
        }
    }
}

fn header_remove_ci(headers: &mut Vec<(String, String)>, name: &str) {
    headers.retain(|(k, _)| !k.eq_ignore_ascii_case(name));
}

/// okhttp `Request.Builder.header`:先删同名(大小写不敏感)再加
fn header_set(headers: &mut Vec<(String, String)>, name: &str, value: &str) {
    header_remove_ci(headers, name);
    headers.push((name.to_string(), value.to_string()));
}

/// 执行一个 call(重试 × 逐跳),对应 `getClient().newCallStrResponse(retry) {...}`。
/// `headers` 为 AnalyzeUrl 的 headerMap 经 addHeaders 后的形态(保序)。
pub fn execute_call(
    transport: &dyn HttpTransport,
    cookies: &mut dyn CookieEnv,
    method: &str,
    url: HttpUrl,
    headers: &[(String, String)],
    body: Option<RequestBody>,
    retry: i32,
    // urlOption 的 followRedirects(None = okhttp 默认跟进):真身经
    // buildRequestClient 落在 client 配置上,回放里由调用方传入
    follow_redirects: Option<bool>,
) -> Result<CallResult, CallError> {
    // 应用层拦截器(每 call 一次):UA 默认/删除 + Keep-Alive
    let mut base_headers: Vec<(String, String)> = headers.to_vec();
    match header_get_ci(&base_headers, "User-Agent") {
        None => base_headers.push(("User-Agent".into(), user_agent().into())),
        Some("null") => header_remove_ci(&mut base_headers, "User-Agent"),
        Some(_) => {}
    }
    base_headers.push(("Keep-Alive".into(), "300".into()));
    base_headers.push(("Connection".into(), "Keep-Alive".into()));

    let mut last: Option<CallResult> = None;
    let mut all_hops: Vec<(String, String)> = Vec::new();
    let attempts = retry.max(0) as usize + 1;
    for _ in 0..attempts {
        let r = execute_attempt(
            transport,
            cookies,
            method,
            url.clone(),
            base_headers.clone(),
            body.clone(),
            follow_redirects.unwrap_or(true),
        )?;
        // 请求序列跨重试累计(裁判侧 ReplayHttp.hops 是全局累加的)
        all_hops.extend(r.hops.iter().cloned());
        let successful = (200..300).contains(&r.status);
        last = Some(r);
        if successful {
            break;
        }
    }
    let mut result = last.expect("至少一次");
    result.hops = all_hops;
    Ok(result)
}

fn execute_attempt(
    transport: &dyn HttpTransport,
    cookies: &mut dyn CookieEnv,
    method: &str,
    url: HttpUrl,
    base_headers: Vec<(String, String)>,
    body: Option<RequestBody>,
    follow_redirects: bool,
) -> Result<CallResult, CallError> {
    let mut method = method.to_string();
    let mut url = url;
    let mut headers = base_headers;
    let mut body = body;
    let mut hops: Vec<(String, String)> = Vec::new();

    let mut follow_ups = 0usize;
    loop {
        let url_str = url.to_string();

        // 网络拦截器(逐跳):CookieJar 标记 → 注入/回存 cookie
        let cookie_jar = header_get_ci(&headers, COOKIE_JAR_HEADER).is_some();
        let mut sent = headers.clone();
        header_remove_ci(&mut sent, COOKIE_JAR_HEADER);
        if cookie_jar {
            let request_cookie = header_get_ci(&sent, "Cookie").map(str::to_string);
            if let Some(new_cookie) =
                cookies.load_request_cookie(&url_str, request_cookie.as_deref())
            {
                header_set(&mut sent, "Cookie", &new_cookie);
            }
        }
        // BridgeInterceptor:头缺 Content-Type 时按 body 补
        if let Some(b) = &body {
            if header_get_ci(&sent, "Content-Type").is_none() {
                if let Some(ct) = &b.content_type {
                    sent.push(("Content-Type".into(), ct.clone()));
                }
            }
        }

        let response = transport
            .execute_hop(&method, &url_str, &sent, body.as_ref())
            .map_err(CallError::Transport)?;
        hops.push((method.clone(), url_str.clone()));

        if cookie_jar {
            // 注意:即使响应没有 Set-Cookie 也要走 saveResponse——真身会给该域
            // 写入空 session 条目(updateSessionCookie(domain, ""))
            let set: Vec<rubato_core::host::SetCookie> =
                crate::cookie::parse_all(&url, &response.headers)
                    .into_iter()
                    .map(|c| rubato_core::host::SetCookie {
                        name: c.name,
                        value: c.value,
                        persistent: c.persistent,
                    })
                    .collect();
            cookies.save_response_cookies(&url_str, &set);
        }

        // 重定向跟进(okhttp followUpRequest 摘录)
        match follow_up(&method, &url, &response).filter(|_| follow_redirects) {
            None => {
                return Ok(CallResult {
                    final_url: url_str,
                    status: response.status,
                    headers: response.headers,
                    body: response.body,
                    hops,
                    prior_is_redirect: follow_ups > 0,
                });
            }
            Some((next_method, next_url)) => {
                follow_ups += 1;
                if follow_ups > MAX_FOLLOW_UPS {
                    return Err(CallError::TooManyRedirects);
                }
                // 非 307/308 跟进:方法改 GET → 丢体、删实体头
                if next_method == "GET" && method != "GET" {
                    body = None;
                    header_remove_ci(&mut headers, "Transfer-Encoding");
                    header_remove_ci(&mut headers, "Content-Length");
                    header_remove_ci(&mut headers, "Content-Type");
                }
                // 跨 origin 删 Authorization
                if url.scheme != next_url.scheme
                    || url.host != next_url.host
                    || url.port != next_url.port
                {
                    header_remove_ci(&mut headers, "Authorization");
                }
                method = next_method;
                url = next_url;
            }
        }
    }
}

/// 返回 Some((新方法, 新 URL)) 表示跟进;None 表示响应即终态
fn follow_up(method: &str, url: &HttpUrl, response: &HttpResponseRaw) -> Option<(String, HttpUrl)> {
    let redirect = match response.status {
        300..=303 => true,
        307 | 308 => {
            if method != "GET" && method != "HEAD" {
                return None;
            }
            true
        }
        _ => false,
    };
    if !redirect {
        return None;
    }
    let location = header_get_ci(&response.headers, "Location")?;
    let next_url = url.resolve(location)?;
    if next_url.scheme != "http" && next_url.scheme != "https" {
        return None;
    }
    let next_method = if response.status == 307 || response.status == 308 {
        method.to_string()
    } else {
        // 300/301/302/303:GET/HEAD 保持,其余改 GET(okhttp redirectsToGet)
        if method == "GET" || method == "HEAD" { method.to_string() } else { "GET".to_string() }
    };
    Some((next_method, next_url))
}
