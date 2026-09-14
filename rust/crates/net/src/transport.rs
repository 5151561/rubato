//! HTTP 传输抽象:client(回放循环)只认这一层。
//! 差分注入 difftest 的快照回放实现;产品侧日后是 reqwest 单跳执行器
//! (重定向/重试/cookie 循环在 client,不在传输层——契约 docs/http-snapshot.md §7)。

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestBody {
    /// body 自带的 media type(okhttp RequestBody.contentType())
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct HttpResponseRaw {
    pub status: u16,
    /// 响应头(保序,可重名——set-cookie)
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// 快照未命中(带 key 与规范化 URL,两侧字符串必须一致)
    SnapshotMiss {
        key: String,
        url: String,
    },
    Io(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::SnapshotMiss { key, url } => write!(f, "snapshot_miss:{key}:{url}"),
            TransportError::Io(e) => write!(f, "io:{e}"),
        }
    }
}

pub trait HttpTransport {
    /// 单跳执行:不跟重定向、不重试、不碰 cookie。
    ///
    /// `&self`:传输层自身无状态(reqwest 的 Client 本来就是 `Sync` 的连接池),
    /// 这让**一个 LiveTransport 走全场**成为可能 —— 曾经的 `&mut self` 逼得
    /// engine 每个书源现造一个 client(= 每源一个 tokio runtime + 后台线程,
    /// 连接池零复用)。需要记录状态的实现(RecordingTransport)用内部可变性。
    fn execute_hop(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&RequestBody>,
    ) -> Result<HttpResponseRaw, TransportError>;
}

// `&self` 的方法面让 Arc 直接当传输用(全场只用 `Arc<dyn HttpTransport + Send + Sync>` 共享)
impl<T: HttpTransport + ?Sized> HttpTransport for std::sync::Arc<T> {
    fn execute_hop(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<&RequestBody>,
    ) -> Result<HttpResponseRaw, TransportError> {
        (**self).execute_hop(method, url, headers, body)
    }
}

pub fn header_get_ci<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}
