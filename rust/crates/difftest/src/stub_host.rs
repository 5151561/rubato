//! 确定性 JS 桩:与 judge/harness 的 `com.script.StubCompiledScript` 同契约。
//!
//! Phase 1 的被测对象是 AnalyzeRule 的分发/拼接,接真 JS 引擎会把方言差异
//!混进来(那是 Phase 2 js-host 的差分范围)。指令表见
//! fixtures/cases/rule-engine/README.md,两侧必须逐条一致。

use rubato_core::host::{HostEnv, JsBindings, JsValue};

#[derive(Default)]
pub struct StubHost {
    pub logs: Vec<String>,
    /// `@webjs:` 走的是**真策略**(`net::webview`),平台那一半是剧本 ——
    /// case 的 `webview` 字段,不带就是缺省剧本(见 [`crate::wv_script`])
    pub wv_script: crate::wv_script::WvScript,
}

/// 本套没有 CookieStore 观察面(analyzeRule 的输出里没有 cookie 那几位),
/// `setCookie` 丢掉即可 —— 裁判侧写进的是它自己的 CookieStore,同样不进输出。
struct NoCookies;

impl net::webview::WebViewCookieSink for NoCookies {
    fn set_cookie(&mut self, _tag: &str, _cookie: Option<&str>) {}
}

impl HostEnv for StubHost {
    fn eval_js(
        &mut self,
        code: &str,
        b: &JsBindings,
        vars: &mut dyn rubato_core::host::JsRuleEnv,
    ) -> Result<JsValue, String> {
        let s = code.trim();
        Ok(match s {
            "#null" => JsValue::Null,
            // 绑定是**对象**(元素只过号,见 BoundValue),桩的口径不变:
            // 一律取它的 `toString()` 形态 —— 与本类型出现之前逐字一致
            "#result" => opt_str(b.result.as_ref().map(|v| v.to_java_string(vars))),
            "#baseUrl" => opt_str(b.base_url.clone()),
            "#src" => opt_str(b.src.as_ref().map(|v| v.to_java_string(vars))),
            "#title" => opt_str(b.title.clone()),
            "#nextChapterUrl" => opt_str(b.next_chapter_url.clone()),
            "#fromBookInfo" => JsValue::Str(b.from_book_info.to_string()),
            "#key" => opt_str(b.key.clone()),
            "#page" => match b.page {
                Some(p) => JsValue::Str(p.to_string()),
                None => JsValue::Null,
            },
            "#err" => return Err("stub js error".into()),
            _ if s.starts_with("#num:") => JsValue::Num(
                s[5..].parse::<f64>().map_err(|_| format!("NumberFormatException: {}", &s[5..]))?,
            ),
            // Kotlin String.toBoolean():只有 "true"(忽略大小写)为真
            _ if s.starts_with("#bool:") => JsValue::Bool(s[6..].eq_ignore_ascii_case("true")),
            _ if s.starts_with("#echo:") => JsValue::Str(s[6..].to_string()),
            _ if s.starts_with("#list:") => {
                JsValue::List(s[6..].split('|').map(str::to_string).collect())
            }
            _ if s.starts_with("#get:") => JsValue::Str(vars.get(&s[5..])),
            _ if s.starts_with("#put:") => {
                let body = &s[5..];
                let i = body.find('=').ok_or("StringIndexOutOfBoundsException")?;
                JsValue::Str(vars.put(&body[..i], &body[i + 1..]))
            }
            // 不认的指令原样返回源码(**未 trim** 的原串)
            _ => JsValue::Str(code.to_string()),
        })
    }

    /// `getWebJsResult`:**真策略**(此前这里是「原样回传 javaScript」的
    /// Phase 1 桩,与裁判侧 `BackstageWebView` 的桩成对;两侧已一起换掉)。
    ///
    /// 常量逐字对齐真身(AnalyzeRule.kt L184-194):`cacheFirst = true`、
    /// `timeout = 10000`、`isRule = true`、`result = GSON.toJson(result)`。
    /// **本套没有书源**(analyzeRule 的 case 不带 source),于是
    /// `headerMap` / `tag` 都是 null —— 与裁判侧 `getSource()` 为 null 同。
    fn web_js(&mut self, req: &rubato_core::host::WebJsRequest<'_>) -> Result<String, String> {
        let mut host = crate::wv_script::ScriptedHost::new(self.wv_script.clone());
        let bwv = net::webview::BackstageWebView {
            url: req.base_url.map(str::to_string),
            html: Some(req.content.to_string()),
            java_script: Some(req.js.to_string()),
            cache_first: true,
            timeout: Some(10_000),
            result: Some(req.result_json.to_string()),
            is_rule: true,
            // `tag` / `headerMap` 是书源那半边:本套没有书源,全走缺省
            ..Default::default()
        };
        let mut sink = NoCookies;
        let res = bwv.get_str_response(&mut host, &mut sink).map_err(|e| e.to_string())?;
        // `.getStrResponse().body.toString()`
        Ok(res.body.unwrap_or_else(|| "null".to_string()))
    }

    fn log(&mut self, msg: &str) {
        self.logs.push(msg.to_string());
    }
}

fn opt_str(v: Option<String>) -> JsValue {
    match v {
        Some(s) => JsValue::Str(s),
        None => JsValue::Null,
    }
}
