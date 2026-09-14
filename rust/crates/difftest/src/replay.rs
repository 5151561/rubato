//! HTTP 录放的被测侧回放传输。key 规范化与快照格式的**唯一契约**是
//! docs/http-snapshot.md(§2-§5);裁判侧等价物 harness/ReplayHttp.kt,
//! 合成侧 tools/http_snapshot.py——三处必须逐字一致。

use base64::Engine;
use net::transport::{HttpResponseRaw, HttpTransport, RequestBody, TransportError};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// §3:key 用的 URL 规范化(输入是已按 okhttp 规范化的 URL 串)。
/// 返回 (规范化 URL, host 目录名)。
pub fn normalize_url_for_key(url: &str) -> (String, String) {
    let (scheme, rest) = match url.split_once("://") {
        Some((s, r)) => (s.to_ascii_lowercase(), r),
        None => (String::new(), url),
    };
    let (authority, tail) = match rest.find(['/', '?', '#']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    // userInfo 丢弃
    let host_port = match authority.rfind('@') {
        Some(i) => &authority[i + 1..],
        None => authority,
    };
    let (mut host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty() => {
            (h.to_string(), Some(p.to_string()))
        }
        _ => (host_port.to_string(), None),
    };
    host = host.to_ascii_lowercase();
    while host.ends_with('.') {
        host.pop();
    }
    let default_port = match scheme.as_str() {
        "http" => Some("80"),
        "https" => Some("443"),
        _ => None,
    };
    let port = port.filter(|p| default_port != Some(p.as_str()));

    let (path_query, _fragment) = match tail.split_once('#') {
        Some((pq, f)) => (pq, Some(f)),
        None => (tail, None),
    };
    let (path, query) = match path_query.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (path_query, None),
    };
    let path = if path.is_empty() { "/" } else { path };
    let query = query.filter(|q| !q.is_empty()).map(|q| {
        let mut segs: Vec<&str> = q.split('&').collect();
        segs.sort_unstable_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        segs.join("&")
    });

    let mut normalized = format!("{scheme}://{host}");
    let mut host_dir = host.clone();
    if let Some(p) = &port {
        normalized.push(':');
        normalized.push_str(p);
        host_dir = format!("{host}_{p}");
    }
    normalized.push_str(path);
    if let Some(q) = &query {
        normalized.push('?');
        normalized.push_str(q);
    }
    (normalized, host_dir)
}

/// §4 白名单
const HEADER_WHITELIST: [&str; 4] = ["user-agent", "referer", "content-type", "x-requested-with"];

/// §2:key = SHA-256(拼接串) 前 16 字节 hex
pub fn snapshot_key(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: Option<&[u8]>,
) -> (String, String, String) {
    let (normalized_url, host_dir) = normalize_url_for_key(url);
    let mut lines: Vec<String> = vec![method.to_uppercase(), normalized_url.clone()];
    let mut hashed: Vec<(String, String)> = headers
        .iter()
        .filter(|(k, _)| HEADER_WHITELIST.contains(&k.to_ascii_lowercase().as_str()))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    hashed.sort_by(|a, b| a.0.cmp(&b.0));
    for (k, v) in hashed {
        lines.push(format!("{k}: {v}"));
    }
    match body {
        None => lines.push(String::new()),
        Some(b) => {
            let mut h = Sha256::new();
            h.update(b);
            lines.push(hex(&h.finalize()));
        }
    }
    let mut h = Sha256::new();
    h.update(lines.join("\n").as_bytes());
    let digest = h.finalize();
    (hex(&digest[..16]), host_dir, normalized_url)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 快照回放传输:单跳 = 一次 key 查找
pub struct ReplayTransport {
    pub root: PathBuf,
    /// §6 回落页名:未命中时读 `<root>/_fallback/<name>.json` 的 response 段
    pub fallback: Option<String>,
}

impl ReplayTransport {
    pub fn with_fallback(root: impl Into<PathBuf>, fallback: Option<&str>) -> Self {
        ReplayTransport { root: root.into(), fallback: fallback.map(str::to_string) }
    }
}

impl HttpTransport for ReplayTransport {
    fn execute_hop(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&RequestBody>,
    ) -> Result<HttpResponseRaw, TransportError> {
        let (key, host_dir, normalized_url) =
            snapshot_key(method, url, headers, body.map(|b| b.bytes.as_slice()));
        let mut path = self.root.join(&host_dir).join(format!("{key}.json"));
        if !path.is_file() {
            // §6 回落页:配了就用它顶上,没配才报 miss
            match &self.fallback {
                Some(name) => {
                    let fb = self.root.join("_fallback").join(format!("{name}.json"));
                    if !fb.is_file() {
                        return Err(TransportError::SnapshotMiss { key, url: normalized_url });
                    }
                    path = fb;
                }
                None => return Err(TransportError::SnapshotMiss { key, url: normalized_url }),
            }
        }
        let text = std::fs::read_to_string(&path).map_err(|_| TransportError::SnapshotMiss {
            key: key.clone(),
            url: normalized_url.clone(),
        })?;
        let v: Value = serde_json::from_str(&text)
            .map_err(|e| TransportError::Io(format!("快照损坏 {path:?}: {e}")))?;
        let resp = &v["response"];
        let status = resp["status"].as_u64().unwrap_or(200) as u16;
        let mut headers_out: Vec<(String, String)> = Vec::new();
        if let Some(obj) = resp["headers"].as_object() {
            for (k, val) in obj {
                match val {
                    Value::Array(items) => {
                        for item in items {
                            if let Some(s) = item.as_str() {
                                headers_out.push((k.clone(), s.to_string()));
                            }
                        }
                    }
                    Value::String(s) => headers_out.push((k.clone(), s.clone())),
                    _ => {}
                }
            }
        }
        let body_out = match resp["bodyBase64"].as_str() {
            Some(b64) => base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| TransportError::Io(format!("快照 body base64 损坏: {e}")))?,
            None => Vec::new(),
        };
        Ok(HttpResponseRaw { status, headers: headers_out, body: body_out })
    }
}

/// 记录每一跳的传输包装(裁判侧等价物:ReplayHttp.hops 全局累计)。
/// `execute_hop` 是 `&self`,hop 表用 `RefCell`(差分单线程)。
pub struct RecordingTransport<T: HttpTransport> {
    pub inner: T,
    hops: std::cell::RefCell<Vec<(String, String)>>,
}

impl<T: HttpTransport> RecordingTransport<T> {
    pub fn new(inner: T) -> Self {
        RecordingTransport { inner, hops: std::cell::RefCell::new(Vec::new()) }
    }

    /// 已发出的 `(method, url)` 序列
    pub fn hops(&self) -> Vec<(String, String)> {
        self.hops.borrow().clone()
    }
}

impl<T: HttpTransport> HttpTransport for RecordingTransport<T> {
    fn execute_hop(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&RequestBody>,
    ) -> Result<HttpResponseRaw, TransportError> {
        let r = self.inner.execute_hop(method, url, headers, body);
        if r.is_ok() {
            // 裁判在 lookup 成功后才记(miss 抛异常不记)
            self.hops.borrow_mut().push((method.to_string(), url.to_string()));
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_normalization() {
        let (u, d) = normalize_url_for_key("HTTP://User@Example.COM.:80/b?x=2&a=1#frag");
        assert_eq!(u, "http://example.com/b?a=1&x=2");
        assert_eq!(d, "example.com");
    }

    #[test]
    fn port_kept_when_non_default() {
        let (u, d) = normalize_url_for_key("https://example.com:8443");
        assert_eq!(u, "https://example.com:8443/");
        assert_eq!(d, "example.com_8443");
    }

    #[test]
    fn key_is_stable() {
        let (k1, _, _) = snapshot_key("get", "http://e.com/?b=2&a=1", &[], None);
        let (k2, _, _) =
            snapshot_key("GET", "http://e.com:80/?a=1&b=2", &[("Accept".into(), "x".into())], None);
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 32);
    }
}
