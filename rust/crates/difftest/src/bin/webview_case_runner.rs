//! webView 策略差分执行器(被测侧)。底下是 `net::webview`(`BackstageWebView`
//! 的策略层移植),平台那一半换成**剧本**——与裁判 `judge/wvharness` 里那份
//! 确定性 WebView 逐字同契约。
//!
//! 契约:fixtures/cases/webview/README.md。输出的字段名、顺序、归一方式
//! 必须与 `wvharness/Main.kt` 逐字一致。

use difftest::wv_script::{ScriptedHost, WvScript};
use net::webview::{BackstageWebView, WebViewCookieSink};
use serde_json::{Map, Value, json};

#[derive(Default)]
struct RecordingCookies(Vec<(String, Option<String>)>);

impl WebViewCookieSink for RecordingCookies {
    fn set_cookie(&mut self, tag: &str, cookie: Option<&str>) {
        self.0.push((tag.to_string(), cookie.map(str::to_string)));
    }
}

fn str_opt(c: &Value, k: &str) -> Option<String> {
    c.get(k).and_then(Value::as_str).map(str::to_string)
}

fn run_case(c: &Value) -> Value {
    let id = c["id"].as_str().unwrap_or_default();
    let mut host = ScriptedHost::new(WvScript::parse(c));
    let mut cookies = RecordingCookies::default();

    let header_map: Vec<(String, String)> = c
        .get("headerMap")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string())).collect()
        })
        .unwrap_or_default();

    let bwv = BackstageWebView {
        url: str_opt(c, "url"),
        html: str_opt(c, "html"),
        encode: str_opt(c, "encode"),
        tag: str_opt(c, "tag"),
        header_map,
        source_regex: str_opt(c, "sourceRegex"),
        override_url_regex: str_opt(c, "overrideUrlRegex"),
        java_script: str_opt(c, "javaScript"),
        delay_time: c.get("delayTime").and_then(Value::as_i64).unwrap_or(0),
        cache_first: c.get("cacheFirst").and_then(Value::as_bool).unwrap_or(false),
        timeout: c.get("timeout").and_then(Value::as_i64),
        result: str_opt(c, "result"),
        is_rule: c.get("isRule").and_then(Value::as_bool).unwrap_or(false),
    };

    let res = bwv.get_str_response(&mut host, &mut cookies);

    let mut out = Map::new();
    out.insert("id".into(), json!(id));
    match res {
        Ok(r) => {
            out.insert("url".into(), json!(r.url));
            out.insert("body".into(), json!(r.body));
            out.insert("isRedirect".into(), json!(r.is_redirect));
        }
        // 差分标签由 `WvError` 的 Display 给(与裁判 errorTag 同一套名字)
        Err(e) => {
            out.insert("error".into(), json!(e.to_string()));
        }
    }
    out.insert(
        "evals".into(),
        Value::Array(host.eval_log.iter().map(|(at, js)| json!({"at": at, "js": js})).collect()),
    );
    let settings = host.settings.clone().unwrap_or_default();
    out.insert("ua".into(), json!(settings.user_agent));
    out.insert("cacheMode".into(), json!(settings.cache_mode));
    out.insert("blockNetworkImage".into(), json!(settings.block_network_image));
    // `loadDataWithBaseURL(baseUrl = null, …)`:真 WebView 拿 `about:blank`
    // 当这一次的请求地址,剧本 WebView 两侧同此
    out.insert("loadedUrl".into(), json!(host.load.as_ref().map(ScriptedHost::request_url)));
    out.insert("loadedHtml".into(), json!(host.load.as_ref().and_then(|l| l.html.clone())));
    out.insert(
        "loadedEncoding".into(),
        // 裁判侧只有 `loadDataWithBaseURL` 那一支才记 encoding
        json!(host.load.as_ref().filter(|l| l.html.is_some()).map(|l| l.encoding.clone())),
    );
    // 请求头按**键名排序**(理由见 wvharness/Main.kt 同一处注释:真身那边是
    // HashMap 的桶序,复刻它没有意义)。空表时真身走不带头的 loadUrl 重载 ——
    // 那一档裁判侧根本不记,这边也给空对象
    let mut headers = Map::new();
    if let Some(l) = host.load.as_ref() {
        if !l.headers.is_empty() {
            let mut hs: Vec<_> = l.headers.clone();
            hs.sort_by(|a, b| a.0.cmp(&b.0));
            for (k, v) in hs {
                headers.insert(k, json!(v));
            }
        }
    }
    out.insert("headers".into(), Value::Object(headers));
    let mut ck = Map::new();
    for (k, v) in &cookies.0 {
        ck.insert(k.clone(), json!(v));
    }
    out.insert("cookies".into(), Value::Object(ck));
    out.insert("released".into(), json!(host.released));
    Value::Object(out)
}

fn main() {
    // 壳(钉差分 UA / 读 cases / 写 jsonl)收在 difftest::run_jsonl;本套不吃快照
    difftest::run_jsonl("webview_case_runner <cases.json> <out.jsonl>", |c, _| run_case(c));
}
