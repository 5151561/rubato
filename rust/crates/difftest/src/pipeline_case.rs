//! 四步流水线的差分执行器 —— **两套差分共用一份**。
//!
//! - `pipeline` / `pipeline-corpus`(裁判 :harness):JS 是两侧同契约的确定性桩,
//!   钉 AnalyzeRule 的调度与拼接;
//! - `pipeline-corpus-b`(裁判 :jsharness):JS 是真引擎(被测 QuickJS vs
//!   裁判 Rhino)+ 网络面,B 层书源的 `<js>` / `@js:` 才跑得起来。
//!
//! 分成两套是因为**裁判那边的 com.script 只能有一份**(:harness 是桩、
//! :jsharness 是真 Rhino);被测侧没有这个约束,故投影、错误归一、观察面
//! 在这里只有一份 —— 两套的输出格式因此天然一致。
//!
//! 契约:fixtures/cases/pipeline/README.md(输出格式)、
//! fixtures/cases/pipeline-corpus{,-b}/README.md(语料与回落页)。

use crate::shared_io::{SharedCookies, SharedTransport};
use js_host::java_api::{HostConfig, RandomSource};
use rubato_core::RuleData;
use rubato_core::host::HostEnv;
use serde_json::{Value, json};

/// 这一 case 的 JS 走哪一条路
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Js {
    /// 确定性桩(见 [`crate::stub_host`]),裁判侧是 :harness 的 com.script 桩
    Stub,
    /// 真 QuickJS + 网络面,裁判侧是 :jsharness 的真 Rhino
    Real,
}

/// 环境常量对齐裁判垫片。**androidId 正好 16 字符** —— BaseSource 的登录信息面
/// 拿它当 AES 密钥(裁判垫片曾经只有 15 个,那一面因此恒抛 IndexOutOfBounds)。
///
/// **不冻时钟**:确定性垫片进不去四步内部的求值(`<js>` / `@js:` 是真身自己
/// eval 的,裁判侧没有「先在作用域里跑一段」的入口)。单边冻比不冻更糟 ——
/// 故两侧都用真时钟,摸时钟/随机的书源整条不进套(生成器里剔除,见
/// fixtures/cases/pipeline-corpus-b/README.md)。`java.randomUUID` 是例外:
/// 两侧都接在定死的字节流上。
fn difftest_host_config() -> HostConfig {
    HostConfig {
        android_id: "rubatodifftest16".into(),
        web_view_ua: "rubato-difftest-ua".into(),
        random: RandomSource::Difftest,
    }
}

/// 真 JS 那条路的宿主工厂。一次 case 内的全部宿主共用 cache / cookie 两张表
/// 与同一份传输、同一个 cookie 库 —— 真身里它们都是进程级单例。
struct RealJs {
    config: HostConfig,
    cache: js_host::host_env::SharedMap,
    /// `cache.putFile/getFile` 那张(ACache),与 `cache` 是**两张表**
    cache_file: js_host::host_env::SharedMap,
    js_cookies: js_host::host_env::SharedMap,
    transport: SharedTransport,
    cookies: SharedCookies,
    source_key: String,
    source_header: Option<Vec<(String, String)>>,
    enabled_cookie_jar: bool,
    /// `java.ajax` 里 `{"webView": true}` 那条路的剧本(与四步用的是同一份)
    wv_script: crate::wv_script::WvScript,
}

impl RealJs {
    /// `BaseSource.getHeaderMap(hasLoginHeader = true)`:header 规则(可能是 JS,
    /// 跑在第三个宿主 `BaseSource.evalJS` 上)+ 默认 UA 注入 + 登录头合并。
    /// 与 `engine::shared::JsEnv::compute_source_header` 同一条路。
    ///
    /// 算 header 的这个宿主**不给网络面**:header 规则里再发请求会绕回还没建好的
    /// self;真身那里也是先有 header 再有请求。
    fn compute_source_header(
        &self,
        source: &rubato_core::entities::BookSource,
    ) -> Vec<(String, String)> {
        // 本体在 js-host(与产品侧 `engine::shared::JsEnv` 共用一份,见
        // `net_face::source_header_for`);这里只搭宿主:差分配置 + 两张表。
        let mut host = js_host::host_env::QuickJsHost::for_difftest(self.config.clone());
        host.cache = self.cache.clone();
        host.cache_file = self.cache_file.clone();
        host.cookies = self.js_cookies.clone();
        // `cookie` 绑定接**真 CookieStore**(裁判那边它就是与网络层同一个单例):
        // 书源 JS `cookie.setCookie(...)` 存下的要进 cookiesDb、也要被后续请求带上
        host.cookie_store = Some(self.cookies.0.clone());
        js_host::net_face::source_header_for(
            &mut host,
            &source.book_source_url,
            &source.book_source_name,
            source.header.as_deref(),
        )
    }

    /// 一个宿主。`data` 是这条规则的变量层 —— `java.ajax` 的 url 里
    /// `{{…}}` 靠它解值(`AnalyzeRule.ajax` 的覆写带 ruleData)。
    fn host(&self, data: &RuleData) -> Box<dyn HostEnv> {
        let mut h = js_host::host_env::QuickJsHost::for_difftest(self.config.clone());
        h.cache = self.cache.clone();
        h.cache_file = self.cache_file.clone();
        h.cookies = self.js_cookies.clone();
        h.cookie_store = Some(self.cookies.0.clone());
        // 建不出 ReplayNet 的路径不存在(共享句柄已经建好);真出现就不装网络面,
        // 让 `java.ajax` 走「未接」那条,不静默
        if let Ok(n) = crate::js_net::replay_net_with_io(
            self.transport.clone(),
            self.cookies.clone(),
            self.source_key.clone(),
            self.source_header.clone(),
            self.enabled_cookie_jar,
            data.clone(),
            crate::js_net::NestedEnv {
                config: self.config.clone(),
                // 四步内部的求值进不去确定性垫片(见 [`difftest_host_config`]),
                // 嵌套宿主同样 —— 两侧都是真时钟
                determinism: None,
                cache: self.cache.clone(),
                cache_file: self.cache_file.clone(),
                cookies: self.js_cookies.clone(),
            },
            self.wv_script.clone(),
            // 四步那几套没有「用户出手」的剧本(语料里那 11 个源踩的是搜索/正文
            // 规则里的 startBrowserAwait,进不了自动差分)—— 保持「未接」
            None,
            // 界面根本不弹,书源名/类型在这条路上读不到(观察面里没有它们)
            String::new(),
            0,
        ) {
            h.net = Some(Box::new(n));
        }
        Box::new(h)
    }
}

/// 一个 case。`js` 挑 JS 走哪条路(见 [`Js`])。
pub fn run(c: &Value, snapshot_root: Option<&std::path::PathBuf>, js: Js) -> Value {
    use pipeline::{PipelineEnv, PipelineError};
    use rubato_core::entities::{Book, BookChapter, BookSource, EntityVars, SearchBook};

    let id = c["id"].as_str().unwrap_or_default();
    let Ok(source) = BookSource::parse(c["source"].as_str().unwrap_or_default()) else {
        return json!({"id": id, "error": "source_error"});
    };
    // 传输与 cookie 库是**本 case 内共享的**:四步这条链与书源 JS 里
    // `java.ajax` 那条链落在同一张 hop 表、同一个 cookie 库上
    // (裁判侧它们本来就是进程级单例)。见 [`crate::shared_io`]。
    let transport = SharedTransport::new(crate::replay::RecordingTransport::new(
        crate::replay::ReplayTransport::with_fallback(
            snapshot_root.cloned().unwrap_or_else(|| "fixtures/http".into()),
            c["fallback"].as_str(),
        ),
    ));
    let cookies = match SharedCookies::open_in_memory() {
        Ok(s) => s,
        Err(e) => return json!({"id": id, "error": e}),
    };

    fn vars_json(v: &EntityVars) -> Value {
        if !v.present {
            return Value::Null;
        }
        if let Some(raw) = &v.raw_unparsed {
            return Value::String(raw.clone());
        }
        let mut m = serde_json::Map::new();
        let mut keys: Vec<&String> = v.map.keys().collect();
        keys.sort();
        for k in keys {
            m.insert(k.clone(), Value::String(v.map[k].clone()));
        }
        Value::Object(m)
    }
    fn search_book_json(b: &SearchBook) -> Value {
        json!({
            "name": b.name, "author": b.author, "kind": b.kind,
            "coverUrl": b.cover_url, "intro": b.intro, "wordCount": b.word_count,
            "latestChapterTitle": b.latest_chapter_title, "bookUrl": b.book_url,
            "origin": b.origin, "type": b.type_, "vars": vars_json(&b.vars),
            "infoHtml": b.info_html,
        })
    }
    fn book_json(b: &Book) -> Value {
        json!({
            "name": b.name, "author": b.author, "kind": b.kind,
            "wordCount": b.word_count, "latestChapterTitle": b.latest_chapter_title,
            "intro": b.intro, "coverUrl": b.cover_url, "tocUrl": b.toc_url,
            "bookUrl": b.book_url, "type": b.type_,
            "durChapterTitle": b.dur_chapter_title,
            "totalChapterNum": b.total_chapter_num,
            "vars": vars_json(&b.vars),
            "tocHtml": b.toc_html, "infoHtml": b.info_html,
        })
    }
    fn chapter_json(ch: &BookChapter) -> Value {
        json!({
            "title": ch.title, "url": ch.url, "tag": ch.tag,
            "wordCount": ch.word_count,
            "isVolume": ch.is_volume, "isVip": ch.is_vip, "isPay": ch.is_pay,
            "index": ch.index, "vars": vars_json(&ch.vars),
        })
    }
    /// 缺字段/类型不对的字段一律落回 `Book::default()` 的值(逐字段回落,
    /// 不是整体回落 —— 与裁判侧 gson 的宽松反序列化同口径)
    fn book_from_json(j: Option<&Value>) -> Book {
        let Some(j) = j.and_then(Value::as_object) else { return Book::default() };
        let s = |k: &str| j.get(k).and_then(Value::as_str).map(str::to_string);
        let i = |k: &str| j.get(k).and_then(Value::as_i64).map(|v| v as i32).unwrap_or_default();
        Book {
            book_url: s("bookUrl").unwrap_or_default(),
            name: s("name").unwrap_or_default(),
            author: s("author").unwrap_or_default(),
            toc_url: s("tocUrl").unwrap_or_default(),
            info_html: s("infoHtml"),
            toc_html: s("tocHtml"),
            vars: EntityVars::from_variable(j.get("variable").and_then(Value::as_str)),
            type_: i("type"),
            dur_chapter_index: i("durChapterIndex"),
            total_chapter_num: i("totalChapterNum"),
            ..Book::default()
        }
    }

    let step = c["step"].as_str().unwrap_or_default().to_string();

    // **JS 宿主**。两套差分共用这一份执行器,差别只在这里:
    // - [`Js::Stub`](pipeline / pipeline-corpus):两侧都是确定性桩,钉的是
    //   AnalyzeRule 的调度与拼接,`ruleData` 用不上(桩不联网);
    // - [`Js::Real`](pipeline-corpus-b):真 QuickJS + 网络面,与裁判侧
    //   :jsharness 的真 Rhino 对打 —— B 层书源的 `<js>` / `@js:` 才跑得起来。
    let stub = |_: &RuleData| -> Box<dyn HostEnv> {
        Box::new(crate::stub_host::StubHost {
            // `@webjs:` 的剧本与四步用的是同一份(case 的 `webview` 字段)
            wv_script: crate::wv_script::WvScript::parse(c),
            ..Default::default()
        })
    };
    // 真 JS 那条路上,一次 case 内的全部宿主共用两张表(真身里它们是进程级单例):
    // CacheManager(`java.put` on source 宿主 / `sourceVariable_*` / `loginHeader_*`)
    // 与 JS 侧的 cookie 表。
    let cache = js_host::host_env::SharedMap::new();
    let cache_file = js_host::host_env::SharedMap::new();
    let js_cookies = js_host::host_env::SharedMap::new();
    let mut real = RealJs {
        config: difftest_host_config(),
        cache: cache.clone(),
        cache_file: cache_file.clone(),
        js_cookies: js_cookies.clone(),
        transport: transport.clone(),
        cookies: cookies.clone(),
        source_key: source.book_source_url.clone(),
        source_header: None,
        enabled_cookie_jar: source.enabled_cookie_jar == Some(true),
        wv_script: crate::wv_script::WvScript::parse(c),
    };
    if js == Js::Real {
        // `AnalyzeUrl(url, source = getSource())` 用的头:header 规则(可能是 JS,
        // 跑在第三个宿主上)+ 默认 UA 注入。算一次给这个源的全部 JS 网络调用用,
        // 与 `engine::shared::JsEnv` 同一条路。
        let h = real.compute_source_header(&source);
        real.source_header = Some(h);
    }
    let real_fn = |d: &RuleData| -> Box<dyn HostEnv> { real.host(d) };
    let host_factory: &dyn Fn(&RuleData) -> Box<dyn HostEnv> = match js {
        Js::Stub => &stub,
        Js::Real => &real_fn,
    };
    // webView 那条路的剧本(case 的 `webview` 字段;不带就是缺省剧本 ——
    // 见 crate::wv_script)。每次 `BackstageWebView` 都拿一份新的回放器,
    // 与真身「从池子里 acquire 一个 WebView」对齐。
    let wv_script = crate::wv_script::WvScript::parse(c);
    let wv_factory = move || -> Box<dyn rubato_core::host::WebViewHost> {
        Box::new(crate::wv_script::ScriptedHost::new(wv_script.clone()))
    };
    let result: Result<Value, PipelineError> = (|| {
        let mut transport = transport.clone();
        let mut cookies = cookies.clone();
        let mut env = PipelineEnv {
            transport: &mut transport,
            cookies: &mut cookies,
            host_factory,
            web_view: Some(&wv_factory),
        };
        match step.as_str() {
            "search" => {
                let books = pipeline::search_book(
                    &mut env,
                    &source,
                    c["key"].as_str().unwrap_or(""),
                    c["page"].as_i64().map(|p| p as i32),
                )?;
                Ok(json!({"books": books.iter().map(search_book_json).collect::<Vec<_>>()}))
            }
            "explore" => {
                let books = pipeline::explore_book(
                    &mut env,
                    &source,
                    c["url"].as_str().unwrap_or(""),
                    c["page"].as_i64().map(|p| p as i32),
                )?;
                Ok(json!({"books": books.iter().map(search_book_json).collect::<Vec<_>>()}))
            }
            "info" => {
                let mut book = book_from_json(c.get("book"));
                pipeline::get_book_info(
                    &mut env,
                    &source,
                    &mut book,
                    c["canReName"].as_bool().unwrap_or(true),
                )?;
                Ok(json!({"book": book_json(&book)}))
            }
            "toc" => {
                let mut book = book_from_json(c.get("book"));
                let chapters = pipeline::get_chapter_list(&mut env, &source, &mut book)?;
                Ok(json!({
                    "chapters": chapters.iter().map(chapter_json).collect::<Vec<_>>(),
                    "book": book_json(&book),
                }))
            }
            "content" => {
                let mut book = book_from_json(c.get("book"));
                let mut ch = BookChapter { book_url: book.book_url.clone(), ..Default::default() };
                // 缺字段/类型不对逐字段落回缺省,同 book_from_json
                if let Some(j) = c.get("chapter").and_then(Value::as_object) {
                    let s = |k: &str| j.get(k).and_then(Value::as_str).map(str::to_string);
                    ch.url = s("url").unwrap_or_default();
                    ch.title = s("title").unwrap_or_default();
                    ch.base_url = s("baseUrl").unwrap_or_default();
                    ch.index = j
                        .get("index")
                        .and_then(Value::as_i64)
                        .map(|v| v as i32)
                        .unwrap_or_default();
                    ch.is_volume = j.get("isVolume").and_then(Value::as_bool).unwrap_or_default();
                    ch.tag = s("tag");
                }
                let content = pipeline::get_content(
                    &mut env,
                    &source,
                    &mut book,
                    &mut ch,
                    c["nextChapterUrl"].as_str(),
                )?;
                Ok(json!({"content": content, "chapter": chapter_json(&ch)}))
            }
            _ => Ok(json!({"error": "unknown_step"})),
        }
    })();

    let mut out = match result {
        Ok(mut v) => {
            let obj = v.as_object_mut().expect("对象");
            if !obj.contains_key("error") {
                obj.insert(
                    "requests".into(),
                    json!(transport.hops().iter().map(|(m, u)| json!([m, u])).collect::<Vec<_>>()),
                );
                let (db, _) = cookies.0.borrow_mut().dump();
                let mut db_out = serde_json::Map::new();
                for (k, val) in db {
                    db_out.insert(k, Value::String(val));
                }
                obj.insert("cookiesDb".into(), Value::Object(db_out));
            }
            obj.insert("id".into(), Value::String(id.to_string()));
            v
        }
        Err(e) => {
            // 归一只归**不该逐字比**的东西(两个语言的异常文本必然不同),
            // 但**真因打 stderr** —— 定位 FAIL 时看看是哪一步炸的。
            if std::env::var("PIPELINE_DEBUG").as_deref() == Ok("1") {
                eprintln!("[pipeline] {id}: {e:?}");
            }
            json!({"id": id, "error": e.to_diff_string()})
        }
    };
    // **CacheManager 终态**(裁判侧 `CacheManager.snapshot()`)。空则不出字段。
    // **出错路径上也挂** —— 裁判侧那一段在 try/catch **之外**,炸了照样出;
    // 只在成功分支挂就是一处单边差异(`java.put` 之后才抛的 case 会现形)。
    let snap = cache.sorted_pairs();
    if !snap.is_empty() {
        if let Some(obj) = out.as_object_mut() {
            let mut m = serde_json::Map::new();
            for (k, val) in snap {
                m.insert(k, Value::String(val));
            }
            obj.insert("cache".into(), Value::Object(m));
        }
    }
    out
}
