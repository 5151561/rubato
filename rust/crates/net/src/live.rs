//! 产品侧的真实单跳执行器(feature `live`)。
//!
//! 契约同 [`crate::transport::HttpTransport`]:**只发一跳** ——
//! 重定向跟进、重试、cookie 循环都在 [`crate::client::execute_call`] 里,
//! 与回放侧共用同一条链路(docs/http-snapshot.md §7)。所以这里必须
//! 关掉 reqwest 自己的 redirect/cookie。
//!
//! Phase 1 未接的面(AnalyzeUrl 已解析但传输层没有入口):per-source
//! 代理 / dnsIp-resolveIp / 不安全 TLS —— 它们要给 `execute_hop` 加参数,
//! 属于 Phase 2 的 net 扩面。

use crate::transport::{HttpResponseRaw, HttpTransport, RequestBody, TransportError};
use std::time::Duration;

pub struct LiveTransport {
    client: reqwest::blocking::Client,
}

impl LiveTransport {
    pub fn with_timeout(timeout: Duration) -> Result<LiveTransport, TransportError> {
        let client = reqwest::blocking::Client::builder()
            // 重定向与 cookie 在 client.rs 的循环里,传输层不许自作主张
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| TransportError::Io(e.to_string()))?;
        Ok(LiveTransport { client })
    }
}

impl HttpTransport for LiveTransport {
    fn execute_hop(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&RequestBody>,
    ) -> Result<HttpResponseRaw, TransportError> {
        let m = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|_| TransportError::Io(format!("bad method: {method}")))?;
        let mut req = self.client.request(m, url);
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        if let Some(b) = body {
            if let Some(ct) = &b.content_type {
                req = req.header("Content-Type", ct.as_str());
            }
            req = req.body(b.bytes.clone());
        }
        let res = req.send().map_err(|e| TransportError::Io(e.to_string()))?;
        let status = res.status().as_u16();
        // 保序 + 可重名(Set-Cookie 靠这个)
        let mut out_headers = Vec::new();
        for (k, v) in res.headers().iter() {
            out_headers
                .push((k.as_str().to_string(), String::from_utf8_lossy(v.as_bytes()).into_owned()));
        }
        let bytes = res.bytes().map_err(|e| TransportError::Io(e.to_string()))?;
        Ok(HttpResponseRaw { status, headers: out_headers, body: bytes.to_vec() })
    }
}
