//! 规则级差分执行器(被测侧),按 op 分发到各 crate。
//! 契约见 fixtures/cases/*/README.md;裁判侧为 judge/harness。

use difftest::stub_host::StubHost;
use json_compat::dsl::AnalyzeByJSonPath;
use json_compat::{JsonPath, ReadResult, java_to_string};
use net::{AnalyzeUrl, UrlArgs};
use regex_compat::{JavaRegex, JavaRegexError, legado_replace_regex};
use rubato_core::net_utils::{get_absolute_url, get_absolute_url_str};
use rubato_core::rule_data::VarLayer;
use rubato_core::{JavaUrl, RuleData};
use rule_engine::AnalyzeRule;
use rule_engine::value::RuleValue;
use rule_syntax::{RuleAnalyzer, SplitError};
use serde_json::{Value, json};
use std::collections::HashMap;

// fr 回调契约,与 judge/harness 一致
fn fr(s: &str) -> Option<String> {
    if s.contains("NULL") {
        None
    } else if s.contains("EMPTY") {
        Some(String::new())
    } else {
        Some(format!("«{s}»"))
    }
}

fn run_case(
    c: &Value,
    docs: &mut HashMap<String, scraper::Html>,
    doc_srcs: &mut HashMap<String, String>,
    snapshot_root: Option<&std::path::PathBuf>,
) -> Value {
    let id = c["id"].as_str().unwrap_or_default();
    let data = c["data"].as_str().unwrap_or_default();

    let res: Result<Value, SplitError> = match c["op"].as_str().unwrap_or_default() {
        "split" => (|| {
            let code = c["code"].as_bool().unwrap_or(false);
            let mut ra = RuleAnalyzer::new(data, code);
            if c["trim"].as_bool().unwrap_or(false) {
                ra.trim()?;
            }
            let seps: Vec<&str> = c["sep"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let r = ra.split_rule(&seps)?;
            Ok(json!({"id": id, "result": r, "elementsType": ra.elements_type()}))
        })(),
        "innerRule" => (|| {
            let mut ra = RuleAnalyzer::new(data, false);
            let start_step = c["startStep"].as_u64().unwrap_or(1) as usize;
            let end_step = c["endStep"].as_u64().unwrap_or(1) as usize;
            let r =
                ra.inner_rule(c["inner"].as_str().unwrap_or_default(), start_step, end_step, fr)?;
            Ok(json!({"id": id, "result": r}))
        })(),
        "innerRuleStr" => (|| {
            let mut ra = RuleAnalyzer::new(data, false);
            let r = ra.inner_rule_str(
                c["startStr"].as_str().unwrap_or_default(),
                c["endStr"].as_str().unwrap_or_default(),
                fr,
            )?;
            Ok(json!({"id": id, "result": r}))
        })(),
        "replaceRegex" => {
            let r = legado_replace_regex(
                data,
                c["pattern"].as_str().unwrap_or_default(),
                c["replacement"].as_str().unwrap_or_default(),
                c["first"].as_bool().unwrap_or(false),
            );
            Ok(json!({"id": id, "result": r}))
        }
        "defineDoc" => {
            let name = c["name"].as_str().unwrap_or_default().to_string();
            let html = c["html"].as_str().unwrap_or_default();
            docs.insert(name.clone(), html_compat::parse(html));
            doc_srcs.insert(name, html.to_string());
            Ok(json!({"id": id, "result": "ok"}))
        }
        "jsoupDsl" => {
            let html = c["doc"]
                .as_str()
                .and_then(|n| doc_srcs.get(n).cloned())
                .unwrap_or_else(|| c["html"].as_str().unwrap_or_default().to_string());
            let rule = c["rule"].as_str().unwrap_or_default();
            let mut analyzer = html_compat::dsl::AnalyzeByJSoup::from_html(&html);
            let dsl_error = json!({"id": id, "error": "dsl_error"});
            Ok(match c["mode"].as_str().unwrap_or_default() {
                "string" => match analyzer.get_string(rule) {
                    Ok(r) => json!({"id": id, "result": r}),
                    Err(_) => dsl_error,
                },
                "string0" => match analyzer.get_string0(rule) {
                    Ok(r) => json!({"id": id, "result": r}),
                    Err(_) => dsl_error,
                },
                "stringList" => match analyzer.get_string_list(rule) {
                    Ok(r) => json!({"id": id, "result": r}),
                    Err(_) => dsl_error,
                },
                "elements" => match analyzer.get_elements(rule) {
                    Ok(ids) => {
                        let outs: Vec<String> =
                            ids.iter().map(|&i| analyzer.outer_html_of(i)).collect();
                        json!({"id": id, "result": outs})
                    }
                    Err(_) => dsl_error,
                },
                _ => json!({"id": id, "error": "unknown_mode"}),
            })
        }
        "xpathDsl" => {
            // AnalyzeByXPath(JsoupXpath 2.5.5)差分。契约见
            // fixtures/cases/xpath/README.md。
            let html = c["doc"]
                .as_str()
                .and_then(|n| doc_srcs.get(n).cloned())
                .unwrap_or_else(|| c["html"].as_str().unwrap_or_default().to_string());
            let rule = c["rule"].as_str().unwrap_or_default();
            let xpath_error = json!({"id": id, "error": "xpath_error"});
            let analyzer = match c["contentType"].as_str().unwrap_or("string") {
                "document" => Some(xpath_compat::AnalyzeByXPath::from_document(&html)),
                "element" => {
                    // 裁判取 select(at).first(),再进 Element 分支
                    let doc = html_compat::parse(&html);
                    let at = c["at"].as_str().unwrap_or_default();
                    match html_compat::select::parse(at) {
                        Ok(ev) => html_compat::select::select(doc.tree.root(), &ev)
                            .first()
                            .map(|n| n.id())
                            .map(|first| {
                                xpath_compat::AnalyzeByXPath::from_elements(
                                    html_compat::parse(&html),
                                    vec![first],
                                )
                            }),
                        Err(_) => None,
                    }
                }
                _ => Some(xpath_compat::AnalyzeByXPath::from_content_string(&html)),
            };
            let Some(analyzer) = analyzer else {
                return xpath_error;
            };
            Ok(match c["mode"].as_str().unwrap_or_default() {
                "string" => match analyzer.get_string(rule) {
                    Ok(r) => json!({"id": id, "result": r}),
                    Err(_) => xpath_error,
                },
                "stringList" => match analyzer.get_string_list(rule) {
                    Ok(r) => json!({"id": id, "result": r}),
                    Err(_) => xpath_error,
                },
                "elements" => match analyzer.get_elements(rule) {
                    Ok(None) => json!({"id": id, "result": Value::Null}),
                    Ok(Some(ns)) => {
                        let vals: Vec<String> = ns
                            .iter()
                            .map(|n| xpath_compat::node_as_string(analyzer.document(), n))
                            .collect();
                        let kinds: Vec<&str> =
                            ns.iter().map(|n| if n.is_element() { "el" } else { "str" }).collect();
                        json!({"id": id, "result": vals, "kinds": kinds})
                    }
                    Err(_) => xpath_error,
                },
                _ => json!({"id": id, "error": "unknown_mode"}),
            })
        }
        "cssSelect" => {
            let owned;
            let doc = match c["doc"].as_str().and_then(|n| docs.get(n)) {
                Some(d) => d,
                None => {
                    owned = html_compat::parse(c["html"].as_str().unwrap_or_default());
                    &owned
                }
            };
            let selector = c["selector"].as_str().unwrap_or_default();
            let action = c["action"].as_str().unwrap_or_default();
            match html_compat::select_extract(doc, selector, action) {
                Ok(items) => Ok(json!({"id": id, "result": items})),
                Err(_) => Ok(json!({"id": id, "error": "selector_error"})),
            }
        }
        "analyzeUrl" => {
            let mut data = RuleData::default();
            match c["ruleData"].as_str().unwrap_or("plain") {
                "none" => {}
                "book" => {
                    data.book = Some(VarLayer::default());
                    data.book_name = Some(c["bookName"].as_str().unwrap_or_default().to_string());
                }
                _ => data.rule_data = Some(VarLayer::default()),
            }
            let to_book = data.book.is_some();
            if let Some(m) = c["vars"].as_object() {
                for (k, v) in m {
                    let layer = if to_book { &mut data.book } else { &mut data.rule_data };
                    if let Some(l) = layer {
                        l.put(k, v.as_str().unwrap_or_default());
                    }
                }
            }
            if c["chapter"].as_bool().unwrap_or(false) {
                let mut ch = VarLayer::default();
                if let Some(m) = c["chapterVars"].as_object() {
                    for (k, v) in m {
                        ch.put(k, v.as_str().unwrap_or_default());
                    }
                }
                data.chapter = Some(ch);
                data.chapter_title = Some(c["title"].as_str().unwrap_or_default().to_string());
            }
            let headers: Option<Vec<(String, String)>> = c["headers"].as_object().map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect()
            });
            let args = UrlArgs {
                m_url: c["mUrl"].as_str().unwrap_or_default(),
                base_url: c["baseUrl"].as_str().unwrap_or_default(),
                key: c["key"].as_str(),
                page: c["page"].as_i64().map(|p| p as i32),
                speak_text: c["speakText"].as_str(),
                speak_speed: c["speakSpeed"].as_i64().map(|p| p as i32),
                header_map_f: headers,
                source_header: None,
                ..Default::default()
            };
            Ok(match AnalyzeUrl::new(args, Box::new(StubHost::default()), data) {
                Err(_) => json!({"id": id, "error": "url_error"}),
                Ok(au) => {
                    let mut headers = serde_json::Map::new();
                    for (k, v) in &au.header_map {
                        headers.insert(k.clone(), Value::String(v.clone()));
                    }
                    let mut vars = serde_json::Map::new();
                    if let Some(l) = au.data.book.as_ref().or(au.data.rule_data.as_ref()) {
                        vars.insert("data".into(), sorted_vars(l));
                    }
                    if let Some(l) = au.data.chapter.as_ref() {
                        vars.insert("chapter".into(), sorted_vars(l));
                    }
                    json!({
                        "id": id,
                        "ruleUrl": au.rule_url,
                        "url": au.url,
                        "urlNoQuery": au.url_no_query,
                        "type": au.type_,
                        "method": au.method.as_str(),
                        "body": au.body,
                        "encodedForm": au.encoded_form,
                        "encodedQuery": au.encoded_query,
                        "charset": au.charset,
                        "proxy": au.proxy,
                        "retry": au.retry,
                        "useWebView": au.use_web_view,
                        "webJs": au.web_js,
                        "bodyJs": au.body_js,
                        "dnsIp": au.dns_ip,
                        "readTimeoutMs": au.read_timeout_ms,
                        "urlTimeoutConfigured": au.url_timeout_configured,
                        "followRedirects": au.follow_redirects,
                        "webViewDelayTime": au.web_view_delay_time,
                        "serverID": au.server_id,
                        "userAgent": au.user_agent(net::client::user_agent()),
                        "isPost": au.is_post(),
                        "headers": Value::Object(headers),
                        "vars": Value::Object(vars),
                    })
                }
            })
        }
        // 四步流水线:执行器在 difftest::pipeline_case(与 pipeline-corpus-b 共用
        // 一份投影)。本套两侧的 JS 都是确定性桩 —— 钉的是 AnalyzeRule 的
        // 调度与拼接,方言差异属 js-host / pipeline-corpus-b 两套。
        "pipeline" => {
            Ok(difftest::pipeline_case::run(c, snapshot_root, difftest::pipeline_case::Js::Stub))
        }
        "parseSource" => {
            use rubato_core::entities::BookSource;
            match BookSource::parse(c["json"].as_str().unwrap_or_default()) {
                Err(_) => Ok(json!({"id": id, "error": "source_error"})),
                Ok(s) => {
                    let sr = |r: &Option<rubato_core::entities::SearchRule>| match r {
                        None => Value::Null,
                        Some(r) => json!({
                            "checkKeyWord": r.check_key_word, "bookList": r.book_list,
                            "name": r.name, "author": r.author, "intro": r.intro,
                            "kind": r.kind, "lastChapter": r.last_chapter,
                            "updateTime": r.update_time, "bookUrl": r.book_url,
                            "coverUrl": r.cover_url, "wordCount": r.word_count,
                        }),
                    };
                    let er = |r: &Option<rubato_core::entities::ExploreRule>| match r {
                        None => Value::Null,
                        Some(r) => json!({
                            "bookList": r.book_list, "name": r.name, "author": r.author,
                            "intro": r.intro, "kind": r.kind, "lastChapter": r.last_chapter,
                            "updateTime": r.update_time, "bookUrl": r.book_url,
                            "coverUrl": r.cover_url, "wordCount": r.word_count,
                        }),
                    };
                    let ir = |r: &Option<rubato_core::entities::BookInfoRule>| match r {
                        None => Value::Null,
                        Some(r) => json!({
                            "init": r.init, "name": r.name, "author": r.author,
                            "intro": r.intro, "kind": r.kind, "lastChapter": r.last_chapter,
                            "updateTime": r.update_time, "coverUrl": r.cover_url,
                            "tocUrl": r.toc_url, "wordCount": r.word_count,
                            "canReName": r.can_re_name, "downloadUrls": r.download_urls,
                        }),
                    };
                    let tr = |r: &Option<rubato_core::entities::TocRule>| match r {
                        None => Value::Null,
                        Some(r) => json!({
                            "preUpdateJs": r.pre_update_js, "chapterList": r.chapter_list,
                            "chapterName": r.chapter_name, "chapterUrl": r.chapter_url,
                            "formatJs": r.format_js, "isVolume": r.is_volume,
                            "isVip": r.is_vip, "isPay": r.is_pay,
                            "updateTime": r.update_time, "nextTocUrl": r.next_toc_url,
                        }),
                    };
                    let cr = |r: &Option<rubato_core::entities::ContentRule>| match r {
                        None => Value::Null,
                        Some(r) => json!({
                            "content": r.content, "subContent": r.sub_content,
                            "title": r.title, "nextContentUrl": r.next_content_url,
                            "webJs": r.web_js, "sourceRegex": r.source_regex,
                            "replaceRegex": r.replace_regex, "imageStyle": r.image_style,
                            "imageDecode": r.image_decode, "payAction": r.pay_action,
                            "callBackJs": r.call_back_js,
                        }),
                    };
                    Ok(json!({
                        "id": id,
                        "bookSourceUrl": s.book_source_url,
                        "bookSourceName": s.book_source_name,
                        "bookSourceGroup": s.book_source_group,
                        "bookSourceType": s.book_source_type,
                        "bookUrlPattern": s.book_url_pattern,
                        "customOrder": s.custom_order,
                        "enabled": s.enabled,
                        "enabledExplore": s.enabled_explore,
                        "jsLib": s.js_lib,
                        "enabledCookieJar": s.enabled_cookie_jar,
                        "concurrentRate": s.concurrent_rate,
                        "header": s.header,
                        "loginUrl": s.login_url,
                        "loginCheckJs": s.login_check_js,
                        "coverDecodeJs": s.cover_decode_js,
                        "mainJs": s.main_js,
                        "lastUpdateTime": s.last_update_time,
                        "respondTime": s.respond_time,
                        "weight": s.weight,
                        "exploreUrl": s.explore_url,
                        "searchUrl": s.search_url,
                        "ruleSearch": sr(&s.rule_search),
                        "ruleExplore": er(&s.rule_explore),
                        "ruleBookInfo": ir(&s.rule_book_info),
                        "ruleToc": tr(&s.rule_toc),
                        "ruleContent": cr(&s.rule_content),
                    }))
                }
            }
        }
        "detectCharset" => {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(c["bytesBase64"].as_str().unwrap_or_default())
                .unwrap_or_default();
            Ok(json!({"id": id, "result": net::encoding_detect::get_html_encode(&bytes)}))
        }
        "fetch" => {
            // HTTP 录放全链路:AnalyzeUrl + getStrResponse
            //(契约 docs/http-snapshot.md;裁判侧 harness 的 fetch op)
            let mut cookies = match store::CookieStore::open_in_memory() {
                Ok(s) => s,
                Err(e) => return json!({"id": id, "error": format!("store:{e}")}),
            };
            if let Some(m) = c["cookies"].as_object() {
                for (domain, v) in m {
                    cookies.set_cookie(domain, v.as_str());
                }
            }
            if let Some(m) = c["sessionCookies"].as_object() {
                for (domain, v) in m {
                    cookies.put_session_cookie(domain, v.as_str().unwrap_or_default());
                }
            }
            let (source_key, enabled_cookie_jar, source_header_rule) = match c["source"].as_object()
            {
                Some(s) => (
                    s.get("url").and_then(Value::as_str),
                    s.get("enabledCookieJar").and_then(Value::as_bool).unwrap_or(false),
                    s.get("header").and_then(Value::as_str),
                ),
                None => (None, false, None),
            };
            let mut data = RuleData::default();
            if c["ruleData"].as_str().unwrap_or("plain") != "none" {
                data.rule_data = Some(VarLayer::default());
            }
            if let Some(m) = c["vars"].as_object() {
                if let Some(l) = data.rule_data.as_mut() {
                    for (k, v) in m {
                        l.put(k, v.as_str().unwrap_or_default());
                    }
                }
            }
            let headers: Option<Vec<(String, String)>> = c["headers"].as_object().map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect()
            });
            // LegadoTeam 基准:source.header 不再被忽略 —— headerMapF 缺席时
            // 走 BaseSource.getHeaderMap()(header 规则 + 默认 UA 注入)
            let source_header = source_key.map(|key| {
                let mut host = StubHost::default();
                let source =
                    rubato_core::host::SourceBinding { key: key.to_string(), ..Default::default() };
                // header 里的 java.put/get 打的是 source 级 CacheManager,不是本例的
                // ruleData —— 用独立实例承接(Phase 1 桩 JS 不写变量)
                let mut vars = RuleData::default();
                net::source_header::source_header_map(
                    source_header_rule,
                    net::client::user_agent(),
                    None,
                    &mut host,
                    &source,
                    &mut vars,
                )
            });
            let args = UrlArgs {
                m_url: c["mUrl"].as_str().unwrap_or_default(),
                base_url: c["baseUrl"].as_str().unwrap_or_default(),
                key: c["key"].as_str(),
                page: c["page"].as_i64().map(|p| p as i32),
                header_map_f: headers,
                source_header,
                source_key,
                enabled_cookie_jar,
                ..Default::default()
            };
            let mut au = match AnalyzeUrl::new(args, Box::new(StubHost::default()), data) {
                Err(_) => return json!({"id": id, "error": "url_error"}),
                Ok(au) => au,
            };
            let transport = difftest::replay::ReplayTransport::with_fallback(
                snapshot_root.cloned().unwrap_or_else(|| "fixtures/http".into()),
                c["fallback"].as_str(),
            );
            // webView 那条路:平台那一半是剧本(case 的 `webview` 字段,
            // 不带就是缺省剧本 —— 见 difftest::wv_script)
            let mut wv =
                difftest::wv_script::ScriptedHost::new(difftest::wv_script::WvScript::parse(c));
            match au.get_str_response(&transport, &mut cookies, None, None, true, Some(&mut wv)) {
                Ok(resp) => {
                    let requests: Vec<Value> =
                        resp.hops.iter().map(|(m, u)| json!([m, u])).collect();
                    let mut headers_out = serde_json::Map::new();
                    for (k, v) in &au.header_map {
                        headers_out.insert(k.clone(), Value::String(v.clone()));
                    }
                    let (db, session) = cookies.dump();
                    let mut db_out = serde_json::Map::new();
                    for (k, v) in db {
                        db_out.insert(k, Value::String(v));
                    }
                    let mut sess_out = serde_json::Map::new();
                    for (k, v) in session {
                        sess_out.insert(k, Value::String(v));
                    }
                    Ok(json!({
                        "id": id,
                        "url": resp.url,
                        "status": resp.status,
                        "body": resp.body,
                        "requests": requests,
                        "headers": Value::Object(headers_out),
                        "cookiesDb": Value::Object(db_out),
                        "cookiesSession": Value::Object(sess_out),
                    }))
                }
                Err(e) => {
                    use net::client::CallError;
                    use net::fetch::FetchError;
                    use net::transport::TransportError;
                    let msg = match &e {
                        FetchError::Call(CallError::Transport(TransportError::SnapshotMiss {
                            key,
                            url,
                        })) => format!("snapshot_miss:{key}:{url}"),
                        FetchError::Call(CallError::TooManyRedirects) => {
                            "too_many_redirects".to_string()
                        }
                        _ => "fetch_error".to_string(),
                    };
                    Ok(json!({"id": id, "error": msg}))
                }
            }
        }
        "analyzeRule" => {
            let mut data = RuleData::default();
            match c["ruleData"].as_str().unwrap_or("plain") {
                "none" => {}
                "book" => {
                    data.book = Some(VarLayer::default());
                    data.book_name = Some(c["bookName"].as_str().unwrap_or_default().to_string());
                }
                _ => data.rule_data = Some(VarLayer::default()),
            }
            let target = data.book.is_some();
            if let Some(m) = c["vars"].as_object() {
                for (k, v) in m {
                    let layer = if target { &mut data.book } else { &mut data.rule_data };
                    if let Some(l) = layer {
                        l.put(k, v.as_str().unwrap_or_default());
                    }
                }
            }
            if c["chapter"].as_bool().unwrap_or(false) {
                let mut ch = VarLayer::default();
                if let Some(m) = c["chapterVars"].as_object() {
                    for (k, v) in m {
                        ch.put(k, v.as_str().unwrap_or_default());
                    }
                }
                data.chapter = Some(ch);
                data.chapter_title = Some(c["title"].as_str().unwrap_or_default().to_string());
            }
            // `@webjs:` 走真策略,平台那一半是剧本(case 的 `webview` 字段)
            let stub = StubHost {
                wv_script: difftest::wv_script::WvScript::parse(c),
                ..Default::default()
            };
            let mut ar = AnalyzeRule::new(Box::new(stub), data);
            ar.set_next_chapter_url(c["nextChapterUrl"].as_str());
            let content_str = c["content"].as_str().unwrap_or_default();
            // contentType=map:gson LinkedTreeMap 内容(裁判用
            // GSON.fromJsonObject<Map<String, Any?>> 构造)。数字按
            // ObjectTypeAdapter + LONG_OR_DOUBLE —— INITIAL_GSON 注册的
            // MapDeserializerDoubleAsIntFix 在这条路上**不生效**,实测见
            // fixtures/cases/rule-engine/README.md
            let content = match c["contentType"].as_str().unwrap_or("string") {
                "map" => match rubato_core::gson::parse_lenient(content_str) {
                    Some(Value::Object(m)) => RuleValue::GsonMap(m),
                    _ => return json!({"id": id, "error": "content_error"}),
                },
                _ => RuleValue::Str(content_str.to_string()),
            };
            ar.set_content(content, c["baseUrl"].as_str());
            if let Some(u) = c["redirectUrl"].as_str() {
                ar.set_redirect_url(u);
            }
            let rule = c["rule"].as_str().unwrap_or_default();
            let is_url = c["isUrl"].as_bool().unwrap_or(false);
            let mode = c["mode"].as_str().unwrap_or_default();
            let outcome: Result<Value, rule_engine::RuleError> = (|| {
                Ok(match mode {
                    "string" => json!(ar.get_string(rule, None, is_url)?),
                    "stringList" => json!(ar.get_string_list(rule, None, is_url)?),
                    "element" => match ar.get_element(rule)? {
                        Some(v) => json!(v.to_java_string()),
                        None => Value::Null,
                    },
                    "elements" => {
                        let items: Vec<String> =
                            ar.get_elements(rule)?.iter().map(RuleValue::to_java_string).collect();
                        json!(items)
                    }
                    _ => return Ok(json!({"__unknown_mode": true})),
                })
            })();
            Ok(match outcome {
                // RUBATO_DIFF_VERBOSE=1 时带上具体错因,便于定位(裁判侧统一为
                // rule_error,所以常规差分必须关掉)
                Err(e) => {
                    if std::env::var_os("RUBATO_DIFF_VERBOSE").is_some() {
                        json!({"id": id, "error": format!("rule_error:{e:?}")})
                    } else {
                        json!({"id": id, "error": "rule_error"})
                    }
                }
                Ok(v) if v.get("__unknown_mode").is_some() => {
                    json!({"id": id, "error": "unknown_mode"})
                }
                Ok(v) => {
                    let mut vars = serde_json::Map::new();
                    if let Some(l) = ar.data.book.as_ref().or(ar.data.rule_data.as_ref()) {
                        vars.insert("data".into(), sorted_vars(l));
                    }
                    if let Some(l) = ar.data.chapter.as_ref() {
                        vars.insert("chapter".into(), sorted_vars(l));
                    }
                    json!({"id": id, "result": v, "vars": Value::Object(vars)})
                }
            })
        }
        "jsonDsl" => (|| {
            let json = c["json"].as_str().unwrap_or_default();
            let rule = c["rule"].as_str().unwrap_or_default();
            // 裁判那边是 `AnalyzeByJSonPath(json)` 的构造,底下 `JsonPath.parse`
            // 是**宽松**的(json-smart PERMISSIVE),但**空串**在它之前就被
            // `notEmpty` 拦下抛 IllegalArgumentException —— 外层归一成
            // `exception:<类名>:<消息>`,这里照抄那一行。
            // 两条消息不一样,别归成一条:空串是 `notEmpty` 拦的,
            // 字面量 `null` 是解析出来之后 `notNull` 拦的。
            let iae = |m: &str| {
                Ok(json!({"id": id, "error": format!("exception:IllegalArgumentException:{m}")}))
            };
            if json.is_empty() {
                return iae("json string can not be null or empty");
            }
            let Ok(a) = AnalyzeByJSonPath::parse_string(json) else {
                return iae("json can not be null");
            };
            Ok(match c["mode"].as_str().unwrap_or_default() {
                "string" => json!({"id": id, "result": a.get_string(rule)?}),
                "stringList" => json!({"id": id, "result": a.get_string_list(rule)?}),
                "list" => {
                    let items: Vec<String> = a.get_list(rule)?.iter().map(java_to_string).collect();
                    json!({"id": id, "result": items})
                }
                "object" => match a.get_object(rule) {
                    // 对 null 调 toString 在裁判侧是 NPE,与 PathNotFound 同归 read_error
                    Some(v) if !v.is_null() => json!({"id": id, "result": java_to_string(&v)}),
                    _ => json!({"id": id, "error": "read_error"}),
                },
                _ => json!({"id": id, "error": "unknown_mode"}),
            })
        })(),
        "absUrl" => {
            let base = c["base"].as_str().unwrap_or_default();
            Ok(match JavaUrl::parse(base) {
                Err(_) => json!({"id": id, "error": "base_malformed"}),
                Ok(u) => {
                    let r = get_absolute_url(Some(&u), c["path"].as_str().unwrap_or_default());
                    json!({"id": id, "result": r})
                }
            })
        }
        "absUrlStr" => {
            let r =
                get_absolute_url_str(c["base"].as_str(), c["path"].as_str().unwrap_or_default());
            Ok(json!({"id": id, "result": r}))
        }
        "jsonRead" => {
            let read_error = json!({"id": id, "error": "read_error"});
            let doc: Result<Value, _> =
                serde_json::from_str(c["json"].as_str().unwrap_or_default());
            let path = c["path"].as_str().unwrap_or_default();
            Ok(match doc {
                Err(_) => read_error,
                Ok(doc) => match JsonPath::compile(path).and_then(|p| p.read(&doc)) {
                    Err(_) => read_error,
                    // 引擎对 null 标量调 toString 会 NPE 被吞 → 同归 read_error
                    Ok(ReadResult::Scalar(Value::Null)) => read_error,
                    // definite 路径命中数组:引擎按 List 分支渲染
                    Ok(ReadResult::Scalar(Value::Array(a))) | Ok(ReadResult::List(a)) => {
                        let items: Vec<String> = a.iter().map(java_to_string).collect();
                        json!({"id": id, "list": items})
                    }
                    Ok(ReadResult::Scalar(v)) => {
                        json!({"id": id, "scalar": java_to_string(&v)})
                    }
                },
            })
        }
        "regexFind" => match JavaRegex::compile(c["pattern"].as_str().unwrap_or_default()) {
            Err(_) => Ok(json!({"id": id, "error": "compile_error"})),
            Ok(re) => match re.find_all_groups(data, 50) {
                Ok(ms) => Ok(json!({"id": id, "result": ms})),
                Err(JavaRegexError::Runtime(_)) => Ok(json!({"id": id, "error": "runtime_error"})),
                Err(e) => Ok(json!({"id": id, "error": format!("exception:{e}")})),
            },
        },
        other => Ok(json!({"id": id, "error": format!("exception:UnknownOp:{other}")})),
    };

    match res {
        Ok(v) => v,
        Err(SplitError::IndexOutOfBounds) => json!({"id": id, "error": "index_out_of_bounds"}),
        Err(SplitError::Unbalanced(m)) => json!({"id": id, "error": m}),
    }
}

/// 变量层按键排序输出(裁判侧用 TreeMap,避开 HashMap 遍历序)
fn sorted_vars(l: &VarLayer) -> Value {
    let mut m = serde_json::Map::new();
    let mut keys: Vec<&String> = l.vars.keys().collect();
    keys.sort();
    for k in keys {
        m.insert(k.clone(), Value::String(l.vars[k].clone()));
    }
    Value::Object(m)
}

fn main() {
    // 壳(钉差分 UA / 读 cases / 写 jsonl)收在 difftest::run_jsonl,三个 runner 同一份
    let mut docs: HashMap<String, scraper::Html> = HashMap::new();
    let mut doc_srcs: HashMap<String, String> = HashMap::new();
    difftest::run_jsonl("case_runner <cases.json> <out.jsonl> [http快照根目录]", |c, root| {
        run_case(c, &mut docs, &mut doc_srcs, root)
    });
}
