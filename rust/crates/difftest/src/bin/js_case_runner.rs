//! js-host 差分执行器(被测侧)。裁判侧是 judge/jsharness(真 Rhino +
//! JsExtensions 真身),契约见 fixtures/cases/js-host/README.md。
//!
//! 这一套钉的是「一段书源 JS 求值出来是什么」:引擎方言 + `java` 宿主 API +
//! 绑定面 + 变量层。**规则的调度与拼接不在这套里**(那是 rule-engine 套,
//! 那边的 JS 是确定性桩)。

use js_host::host_env::{EvalDetail, QuickJsHost};
use js_host::java_api::{HostConfig, RandomSource};
use rubato_core::RuleData;
use rubato_core::host::{BoundValue, JsBindings, JsHost, JsValue, VarStore};
use rubato_core::rule_data::VarLayer;
use serde_json::{Value, json};

/// 未接能力的统一标记(两侧同串,见 judge/jsharness 的 NET_UNSUPPORTED)
const NET_UNSUPPORTED: &str = "«net-unsupported»";
const FS_UNSUPPORTED: &str = "«fs-unsupported»";

/// Java `Double.toString`(Kotlin 的 `Any.toString()` 与 gson 都走它)。
/// 本体在 `rubato_core::java_num` —— 全仓唯一一份。
use rubato_core::java_double_to_string;

/// **异常的栈字符串**(裁判侧 jsharness/Main.kt 的 `JAVA_STACK`,
/// 正则 `^[\w.$]+(Exception|Error)\b`)。两个引擎的异常文本必然不同,
/// 归一成标记 —— 比的是「失败了」,不是「怎么描述失败」。
///
/// 这一条**两侧都要做**:书源在 `catch (e) { java.log(e) }` 里把异常对象打进
/// 日志,裁判给 `com.script.ScriptException: …`,被测侧给 `TypeError: …` ——
/// 同一条正则套上去,两边都落到 `«java-stack»`。
fn is_java_stack(s: &str) -> bool {
    let head: String =
        s.chars().take_while(|c| c.is_alphanumeric() || *c == '.' || *c == '$').collect();
    for suffix in ["Exception", "Error"] {
        if let Some(rest) = head.strip_suffix(suffix) {
            // `[\w.$]+` 至少吃一个字符,且 `\b` 要求后面不是单词字符
            if !rest.is_empty()
                && !s[head.len()..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_')
            {
                return true;
            }
        }
    }
    false
}

/// 裁判侧 `normalizeStr` 的完整口径:身份哈希 + 异常栈两条都归一
fn normalize_str(s: &str) -> String {
    let id = normalize_identity(s);
    if id != s {
        return id;
    }
    if is_java_stack(s) { "«java-stack»".into() } else { s.to_string() }
}

/// Java 对象默认 `toString()` 的身份形态(`[B@1f2e3d4`)。裁判侧同一条正则,
/// 见 jsharness/Main.kt 的 `IDENTITY` —— 逐次运行都不同,两侧归一成标记。
fn normalize_identity(s: &str) -> String {
    let ok = |c: char| c.is_alphanumeric() || ".$;[_".contains(c);
    if let Some((head, tail)) = s.rsplit_once('@') {
        let head = head.strip_prefix('[').unwrap_or(head);
        if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_hexdigit()) && head.chars().all(ok) {
            return "«identity»".into();
        }
    }
    s.to_string()
}

/// 完成值 → 粗类型标签(与裁判 typeOf 同一套)
fn type_of(v: &JsValue) -> &'static str {
    match v {
        JsValue::Null => "null",
        JsValue::Bool(_) => "bool",
        JsValue::Num(_) => "number",
        JsValue::Str(_) => "string",
        JsValue::List(_) => "list",
        JsValue::Json(_) => "object",
        // 裁判侧是 NativeArray(元素数组)或 NativeJavaList(jsoup Elements)——
        // 两者 typeOf 都落在 list 那一档,与本变体出现之前(List)同
        JsValue::Elements { .. } | JsValue::Native(_) => "list",
    }
}

/// AnalyzeRule 的消费口径(AnalyzeRule.kt L798-806)。
/// list/object 在裁判侧走 Kotlin 的 `toString()`:NativeArray 给身份哈希
/// (归一成 `«identity»`),NativeObject 给 `[object Object]`。
fn consume_as_analyze_rule(v: &JsValue) -> Option<String> {
    match v {
        JsValue::Null => None,
        JsValue::Str(s) => Some(s.clone()),
        JsValue::Bool(b) => Some(b.to_string()),
        JsValue::Num(n) => Some(if n.fract() == 0.0 && n.is_finite() {
            format!("{n:.0}")
        } else {
            java_double_to_string(*n)
        }),
        JsValue::List(_) => Some("«identity»".into()),
        JsValue::Json(_) => Some("[object Object]".into()),
        JsValue::Elements { .. } | JsValue::Native(_) => Some("«identity»".into()),
    }
}

/// `JsSourceEngine.normalizeJsResult` 的口径(header JS / @js: 那条路径)。
/// 字符串原样;对象/数组走 JSON 序列化;数字按 Java Double 形态(**归一面**,
/// 见 README「已归一的差异」)。Err = 裁判侧 GSON 会抛的那一类(Infinity/NaN)。
fn normalize_js_result(v: &JsValue, raw: Option<&Value>) -> Result<Option<String>, String> {
    Ok(match v {
        JsValue::Null => None,
        JsValue::Str(s) => Some(s.clone()),
        JsValue::Bool(b) => Some(b.to_string()),
        JsValue::Num(n) => {
            if !n.is_finite() {
                // gson: "Infinity is not a valid double value as per JSON specification"
                return Err("host:IllegalArgumentException".into());
            }
            Some(java_double_to_string(*n))
        }
        // 数组/对象用**原始 JSON**(JsValue::List 把元素类型抹成字符串了)
        JsValue::List(l) | JsValue::Elements { strings: l, .. } => Some(match raw {
            Some(j) => j.to_string(),
            None => Value::Array(l.iter().map(|s| json!(s)).collect()).to_string(),
        }),
        // 对象数组:原始 JSON 就是它自己
        JsValue::Native(a) => {
            Some(raw.cloned().unwrap_or_else(|| Value::Array(a.clone())).to_string())
        }
        JsValue::Json(j) => Some(raw.unwrap_or(j).to_string()),
    })
}

/// 宿主报错 → 归一标签。被测侧的 JS 异常消息与 Rhino 必然不同,只比类别。
///
/// **归一之前先把真因打到 stderr**(`JS_DEBUG=1`):`js:SyntaxError` 这一档
/// 归一之后长得都一样,不打出来就分不清「引擎方言」与「我们自己的缺口」——
/// 与 pipeline 两套的 `PIPELINE_DEBUG` / `JSHARNESS_DEBUG` 同一条规矩。
fn error_tag(msg: &str) -> String {
    if std::env::var_os("JS_DEBUG").is_some() {
        eprintln!("[js] {msg}");
    }
    // Java 方法重载不明确(典型:`javaString.replace(/re/, s)`)。真身是 Rhino 的
    // EvaluatorException,裁判侧 errorTag 把它归到 js:SyntaxError —— 照同一口径。
    // 同一条:LiveConnect 里同名类跨包冲突(`java.util` 与 `android.util` 都有
    // `Base64`)—— Rhino 的 JavaImporter 抛 EvaluatorException,同样归 SyntaxError。
    if msg.contains("«java-ambiguous-overload»") || msg.contains("«java-ambiguous-import»") {
        return "js:SyntaxError".into();
    }
    // 宿主侧直接给出的类别(js-host 抛出来的归一标签)。目前只有一处:
    // `java.ajax` 的 url 里带 `{{$.id}}` 这类 JS 时,AnalyzeUrl 的**构造**就抛,
    // 而那一步在真身的 runCatching 之外 —— 见 rubato_core::host::NetError。
    if let Some(rest) = msg.split("«host:").nth(1) {
        if let Some(name) = rest.split('»').next() {
            return format!("host:{name}");
        }
    }
    // 没有规则面的宿主(AnalyzeUrl / source 宿主)被 JS 调到 `java.getString`
    // 那一族时给的标记。**真身在那里是 TypeError**(找不到函数),见
    // `rubato_core::host::NO_RULE_HOST` 的说明 —— 归到同一档。
    if msg.contains(rubato_core::host::NO_RULE_HOST) {
        return "js:TypeError".into();
    }
    if msg.contains(NET_UNSUPPORTED) || msg.contains("未接入") {
        return "unsupported:net".into();
    }
    // webView 那半边报的错(`java.webView*` 不吞异常,直接穿到 JS)。
    // 裁判侧的标签是**异常类名**,这里把 `net::webview` 的错因归到同一批:
    // - `js执行超时` 真身是 `NoStackTraceException`;
    // - 超时那一档真机上是 `withTimeout` 的 TimeoutCancellationException,
    //   而两边的差分垫片都把它换成了自己的时钟 —— 裁判侧抛 `WebViewTimeout`
    //   (harness/jsharness 的垫片类),这边照同一个名字。
    if let Some(rest) = msg.split("«webview:").nth(1) {
        if let Some(cause) = rest.split('»').next() {
            return match cause {
                "js_timeout" => "host:NoStackTraceException".into(),
                "timeout" => "host:WebViewTimeout".into(),
                other => format!("host:{other}"),
            };
        }
    }
    if msg.contains(FS_UNSUPPORTED) {
        return "unsupported:fs".into();
    }
    for (needle, tag) in [
        ("SyntaxError", "js:SyntaxError"),
        ("ReferenceError", "js:ReferenceError"),
        ("TypeError", "js:TypeError"),
        ("RangeError", "js:RangeError"),
        ("InternalError", "js:InternalError"),
    ] {
        if msg.contains(needle) {
            return tag.into();
        }
    }
    "js:throw".into()
}

/// 两侧共享的确定性垫片文本(见 fixtures/cases/js-host/determinism.js)。
/// 进程启动时从 cases.json 的同级目录读一次。
static DETERMINISM: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// `op:"js"` 与 `op:"analyzeUrl"` 共用的建场(从前两个入口各抄一份逐字相同的
/// 代码,收成这一处)。确定性垫片由调用方给 —— analyzeUrl 那条路**不装**,
/// 理由见 [`run_analyze_url`]。
struct CaseEnv {
    source_json: Value,
    source_url: String,
    source_binding: rubato_core::host::SourceBinding,
    book_binding: rubato_core::host::BookBinding,
    data: RuleData,
    host: QuickJsHost,
    verify_user: VerifyUser,
}

fn case_env(
    c: &Value,
    snapshot_root: Option<&std::path::PathBuf>,
    determinism: Option<String>,
) -> Result<CaseEnv, Value> {
    let id = c["id"].as_str().unwrap_or_default();

    // book / source:与裁判 jsharness/Main.kt 的建法逐字对齐(缺省值同源)
    let source_json: Value = serde_json::from_str(
        c["source"].as_str().unwrap_or(
            r#"{"bookSourceUrl":"https://difftest.example.com","bookSourceName":"difftest","bookSourceType":0,"enabled":true}"#,
        ),
    )
    .unwrap_or(Value::Null);
    let source_url = source_json["bookSourceUrl"].as_str().unwrap_or_default().to_string();
    let source_binding = rubato_core::host::SourceBinding {
        key: source_url.clone(),
        tag: source_json["bookSourceName"].as_str().unwrap_or_default().to_string(),
        raw: Some(source_json.clone()),
    };
    let book_binding = rubato_core::host::BookBinding {
        name: c["bookName"].as_str().unwrap_or("测试书名").to_string(),
        author: c["bookAuthor"].as_str().unwrap_or("测试作者").to_string(),
        book_url: c["bookUrl"]
            .as_str()
            .unwrap_or("https://difftest.example.com/book/1")
            .to_string(),
        origin: source_url.clone(),
        ..Default::default()
    };

    // 变量层:裁判侧建的是 Book(book 层),这里对齐
    let mut data = RuleData { book: Some(VarLayer::default()), ..Default::default() };
    if let Some(vars) = c["vars"].as_object() {
        for (k, v) in vars {
            data.put(k, v.as_str().unwrap_or_default());
        }
    }
    // **章节层要在 `vars` 之后加**:裁判侧的 `vars` 是逐条 `book.putVariable`
    // 打进**书**那一层的,而 `RuleData::put` 只写命中的第一层 —— 先挂上章节层
    // 就会把 `vars` 灌进章节,两侧的起始变量层从此错开。
    if let Some(ch) = chapter_binding(c) {
        let mut layer = VarLayer::default();
        if let Some(vars) = c["chapter"]["vars"].as_object() {
            for (k, v) in vars {
                layer.put(k, v.as_str().unwrap_or_default());
            }
        }
        data.chapter = Some(layer);
        // `java.get("title")` 的特判:章节在场时给的是 `chapter.title`
        data.chapter_title = Some(ch.title);
    }

    // 环境常量对齐裁判垫片(见 fixtures/cases/js-host/README.md「不可差分面」)
    let mut host = QuickJsHost::for_difftest(HostConfig {
        // **正好 16 字符**:BaseSource 的登录信息面拿它当 AES 密钥
        android_id: "rubatodifftest16".into(),
        web_view_ua: "rubato-difftest-ua".into(),
        // 定死的随机流(裁判 jsharness/DeterministicRandom 同一条)
        random: RandomSource::Difftest,
    });
    host.determinism = determinism;
    // 预置 CacheManager(裁判侧同口径:`loginHeader_<key>` / `userInfo_<key>` /
    // `sourceVariable_<key>` 这几条路要先有值才测得到读取那一半)
    if let Some(pre) = c["cache"].as_object() {
        for (k, v) in pre {
            host.cache.insert(k.clone(), v.as_str().unwrap_or_default().to_string());
        }
    }
    // 剧本用户(case 的 `verify`)。`opens` 是观察面之一 —— 界面被要求弹什么
    // (地址 / 标题 / 哪一种 / 要不要存结果)与返回值同样是判据
    let verify_user =
        difftest::verify_script::VerifyScript::parse(c).map(difftest::verify_script::wire);
    // **网络面**:接 HTTP 录放(裁判侧是 jsharness/ReplayHttp,同一份快照)。
    // 回落页按 case 的 `fallback` 挑,缺省 json —— 与裁判 Main.kt 逐字同口径。
    // ReplayNet 自己那份 source header 只用于**嵌套** AnalyzeUrl(java.ajax),
    // 与 analyzeUrl 那条路 getHeaderMap 的那一份不是同一次求值(真身同样各算各的)。
    if let Some(root) = snapshot_root {
        let source_header =
            source_json["header"].as_str().and_then(js_host::net_face::parse_header_json);
        match difftest::js_net::replay_net(
            root.clone(),
            Some(c["fallback"].as_str().unwrap_or("json")),
            source_url.clone(),
            source_header,
            source_json["enabledCookieJar"].as_bool().unwrap_or(false),
            data.clone(),
            // 嵌套宿主与外层同一个环境(config / 垫片 / cache / cookie 两张表)
            difftest::js_net::NestedEnv {
                config: host.config.clone(),
                determinism: host.determinism.clone(),
                cache: host.cache.clone(),
                cache_file: host.cache_file.clone(),
                cookies: host.cookies.clone(),
            },
            // `{"webView": true}` 的 `java.ajax`:剧本来自 case 的 `webview`
            difftest::wv_script::WvScript::parse(c),
            // 「请用户出手」那三件:剧本用户来自 case 的 `verify`
            // (字段缺席 = 这一侧没接界面 → 三件都报「未接」)
            verify_user.as_ref().map(|(v, _)| v.clone()),
            source_json["bookSourceName"].as_str().unwrap_or_default().to_string(),
            source_json["bookSourceType"].as_i64().unwrap_or(0) as i32,
        ) {
            Ok(n) => host.net = Some(Box::new(n)),
            Err(e) => return Err(json!({ "id": id, "error": format!("store:{e}") })),
        }
    }

    Ok(CaseEnv { source_json, source_url, source_binding, book_binding, data, host, verify_user })
}

/// 用例里的 `chapter` 对象 → `ChapterBinding`(裁判侧是一个真的 `BookChapter`,
/// 见 jsharness/Main.kt 的 `host: "rule"` 分支)。没有这个字段就是**不 setChapter**。
fn chapter_binding(c: &Value) -> Option<rubato_core::host::ChapterBinding> {
    let ch = c["chapter"].as_object()?;
    let s = |k: &str, d: &str| ch.get(k).and_then(Value::as_str).unwrap_or(d).to_string();
    Some(rubato_core::host::ChapterBinding {
        url: s("url", "https://difftest.example.com/book/1/2.html"),
        title: s("title", "第 12 章 混沌雷池"),
        base_url: s("baseUrl", "https://difftest.example.com/book/1"),
        book_url: s("bookUrl", "https://difftest.example.com/book/1"),
        index: ch.get("index").and_then(Value::as_i64).unwrap_or(0) as i32,
        is_volume: ch.get("isVolume").and_then(Value::as_bool).unwrap_or(false),
        is_vip: ch.get("isVip").and_then(Value::as_bool).unwrap_or(false),
        is_pay: ch.get("isPay").and_then(Value::as_bool).unwrap_or(false),
        tag: ch.get("tag").and_then(Value::as_str).map(str::to_string),
    })
}

// `op: "exploreKinds"` —— 发现页分类列表(`BookSourceExtensions.exploreKinds()`)
//
// 用例只给整份书源:走哪条路(JSON 数组 / `title::url` 行 / `@js:`+`<js>`)
// 由 `exploreUrl` 自己决定,这正是要比的那一位。**错误只比类别** ——
// 真身出错时交回 `ExploreKind("ERROR:<消息>", <栈>)`,消息与栈两侧不可能逐字
// 一致(与 js 套 `errorTag` 同一条口径,见 `ExploreKindsError::tag`)。
fn run_explore_kinds(c: &Value, snapshot_root: Option<&std::path::PathBuf>) -> Value {
    let id = c["id"].as_str().unwrap_or_default();
    // **确定性垫片不装**(与 `analyzeUrl` 那条同理):裁判侧 `exploreKinds()`
    // 里的 `evalJS` 是真身自己调的,没有「先在作用域里跑一段」的入口 ——
    // 这边装了反而两侧不齐。摸时钟/随机的书源由生成器整条剔除。
    let mut env = match case_env(c, snapshot_root, None) {
        Ok(e) => e,
        Err(v) => return v,
    };
    let explore_url = env.source_json["exploreUrl"].as_str().map(str::to_string);
    let source_binding = env.source_binding.clone();
    // **变量层要包成 `AnalyzeRule`**:发现规则里 `org.jsoup.Jsoup.parse(...)`
    // 是常见写法(LiveConnect),而被测侧的 jsoup 元素是**句柄**,句柄表挂在
    // `AnalyzeRule` 上 —— 只给一层 `RuleData` 的话元素面报 `«no-rule-host»`。
    // 真身那边 `BaseSource.evalJS` 确实没有 AnalyzeRule,但 LiveConnect 的类
    // 本来就是全局的:这里补的是**元素表**,不是 `java` 的反调面
    // (`java.put/get` 在 source 宿主上照旧打 CacheManager)。
    let mut rule = rule_engine::AnalyzeRule::new(Box::new(rule_engine::NoHost), env.data.clone());
    let r = pipeline::explore_kinds::explore_kinds_result(
        explore_url.as_deref(),
        &mut env.host,
        &source_binding,
        &mut rule,
    );
    let mut out = json!({ "id": id });
    match r {
        Ok(kinds) => out["kinds"] = json!(kinds.iter().map(explore_kind_json).collect::<Vec<_>>()),
        // 归一成标签走 `error_tag`(全仓唯一一份口径,连 `JS_DEBUG=1` 的
        // 真因打印一起继承);JSON / Java 那两支的类别由错误本身带着
        Err(e) => {
            use pipeline::explore_kinds::ExploreKindsError as E;
            out["error"] = json!(match &e {
                E::Json(_) => "json".to_string(),
                E::Host(n) => format!("host:{n}"),
                E::Js(m) => error_tag(m),
            });
        }
    }
    if let Some(h) = hops_json(&env.host) {
        out["hops"] = h;
    }
    // CacheManager 终态:JS 那两支可能 `java.put` 过(裁判侧同口径)
    if let Some(m) = map_json(&env.host.cache) {
        out["cache"] = m;
    }
    out
}

/// 一格分类的观察面。`style` 分两位出:**给没给**(`hasStyle`)与**取值**
/// (`style()` 的兜底)—— 合成一位会把「没给」与「给了一份和默认相同的」照成一致。
fn explore_kind_json(k: &rubato_core::entities::ExploreKind) -> Value {
    let st = k.style();
    let mut o = json!({
        "title": k.title,
        "url": k.url,
        "type": k.kind_type,
        "action": k.action,
        "default": k.default_value,
        "viewName": k.view_name,
        "hasStyle": k.style.is_some(),
        "style": {
            // 浮点出成串(裁判侧 `Float.toString`):Rust 的 f32 Display 与
            // JDK 19+ 的 `Float.toString` 都是**最短往返**,形态一致;
            // 而 serde_json 会把 f32 加宽成 f64,`0.29f` 出成 0.28999999165534973
            "layout_flexGrow": rubato_core::java_float_to_string(st.layout_flex_grow),
            "layout_flexShrink": rubato_core::java_float_to_string(st.layout_flex_shrink),
            "layout_alignSelf": st.layout_align_self,
            "layout_flexBasisPercent":
                rubato_core::java_float_to_string(st.layout_flex_basis_percent),
            "layout_wrapBefore": st.layout_wrap_before,
            "layout_justifySelf": st.layout_justify_self,
            "alignSelf()": st.align_self(),
        },
    });
    // `chars` 缺席时**不出这个键**(裁判侧 `k.chars?.let { … }` 同口径):
    // 出成 `null` 会把「没有这个字段」与「有但是 null」照成一致
    if let Some(cs) = &k.chars {
        o["chars"] = json!(cs);
    }
    o
}

fn run_case(c: &Value, snapshot_root: Option<&std::path::PathBuf>) -> Value {
    let id = c["id"].as_str().unwrap_or_default();
    let mut out = json!({ "id": id });

    match c["op"].as_str() {
        Some("js") => {}
        // **`@js:` url 面**:整条 url 规则走 net::AnalyzeUrl,JS 是真 QuickJS。
        // 裁判侧是 AnalyzeUrl 真身 + 真 Rhino,见 jsharness/Main.kt 同名分支。
        Some("analyzeUrl") => return run_analyze_url(c, snapshot_root),
        // **发现页分类列表**:`BookSourceExtensions.exploreKinds()` 那一面。
        // 裁判侧挂的是**那个函数的真身**(jsharness 同名分支),这边走
        // `pipeline::explore_kinds`。
        Some("exploreKinds") => return run_explore_kinds(c, snapshot_root),
        // **四步流水线 × 真 JS**(pipeline-corpus-b)。执行器与 A 层那套共用
        // 一份投影(difftest::pipeline_case),区别只在 JS 走哪条路:
        // 这里是真 QuickJS + 网络面,裁判侧是 :jsharness 的真 Rhino。
        Some("pipeline") => {
            return difftest::pipeline_case::run(
                c,
                snapshot_root,
                difftest::pipeline_case::Js::Real,
            );
        }
        _ => {
            out["error"] = json!(format!("exception:UnknownOp:{}", c["op"]));
            return out;
        }
    }

    // 建场(绑定 / 变量层 / 宿主 / 录放)收在 case_env,与 analyzeUrl 那条路同一份
    let CaseEnv { source_binding, book_binding, data, mut host, verify_user, .. } =
        match case_env(c, snapshot_root, DETERMINISM.get().cloned().flatten()) {
            Ok(env) => env,
            Err(e) => return e,
        };

    // **两个宿主**(见 fixtures/cases/js-host/README.md「两个宿主」):
    // searchUrl/exploreUrl 的 JS 跑 AnalyzeUrl.evalJS,其余跑 AnalyzeRule.evalJS。
    // 绑定面与 `java` 的方法面都不同。
    let host_kind = match c["host"].as_str().unwrap_or("rule") {
        "url" => JsHost::Url,
        "rule" => JsHost::Rule,
        // **第三个宿主**:BaseSource.evalJS(source 的 header / loginUrl /
        // loginCheckJs)。`java` 就是书源实体,绑定面最窄,`java.put/get` 打
        // CacheManager —— 见 rubato_core::host::JsHost::Source
        "source" => JsHost::Source,
        other => panic!("未知 host: {other}"),
    };

    let mut bindings = JsBindings {
        host: host_kind,
        source_login: false,
        book: Some(book_binding),
        source: Some(source_binding),
        // 本套的 `result` / `content` 用例里一直是**串**(裁判侧 `bindings["result"]`
        // 收到的也是 Kotlin String)。元素形态的绑定在 pipeline 那条路上才有,
        // 见 rubato_core::host::BoundValue。
        result: c["result"].as_str().map(|s| BoundValue::Str(s.to_string())),
        // **`url` 宿主的 baseUrl 不是用例里那个原样串**:真身 `AnalyzeUrl.analyzeUrl()`
        // 在 `evalJS` 之前把它改写成 `NetworkUtils.getBaseUrl(url)`(AnalyzeUrl.kt L236)
        // —— 也就是 `scheme://host`,尾部斜杠没了。裁判侧走的是真身,这里照做。
        // (`rule` 宿主没有这一步,`AnalyzeRule` 绑的就是 setBaseUrl 收到的串。)
        base_url: match host_kind {
            JsHost::Url => c["baseUrl"]
                .as_str()
                .map(|b| rubato_core::net_utils::get_base_url(b).unwrap_or_else(|| b.to_string())),
            JsHost::Rule => c["baseUrl"].as_str().map(str::to_string),
            // `source` 宿主的 baseUrl 是 `getKey()`(bookSourceUrl),
            // 由 install_bindings 从 source 绑定里取,这里给什么都不作数
            JsHost::Source => None,
        },
        src: c["content"].as_str().map(|s| BoundValue::Str(s.to_string())),
        title: c["title"].as_str().map(str::to_string),
        next_chapter_url: c["nextChapterUrl"].as_str().map(str::to_string),
        from_book_info: c["fromBookInfo"].as_bool().unwrap_or(false),
        key: c["key"].as_str().map(str::to_string),
        // 裁判侧 page 走 localBindings["page"].toIntOrNull() ?: 原串
        page: c["page"].as_str().and_then(|s| s.parse::<i32>().ok()),
        // 用例带 `chapter` 对象才有章节实体(裁判侧同样只在带它时 setChapter);
        // 不带就是 null —— 真身 `AnalyzeRule.chapter` 没 setChapter 过就是 null。
        chapter: chapter_binding(c),
    };

    // **`java` 就是 AnalyzeRule 本身**:`java.getString` / `getElements` /
    // `setContent` 要反过来跑规则,所以求值环境是一个真的 AnalyzeRule
    // (裁判侧同样,见 jsharness/Main.kt 的 `host: "rule"` 分支)。
    // 它自己的 host 位留空壳:本套只从外面驱动一次求值,不会经 rule.eval_js 进来。
    let mut rule = rule_engine::AnalyzeRule::new(Box::new(rule_engine::NoHost), data);
    match (c["content"].as_str(), c["baseUrl"].as_str()) {
        (Some(content), base) => {
            rule.set_content(rule_engine::value::RuleValue::Str(content.to_string()), base);
        }
        (None, Some(base)) => {
            rule.set_base_url(Some(base));
        }
        (None, None) => {}
    }

    // **`result` 绑的不一定是串**(见 rubato_core::host::BoundValue):产品里
    // `init: $.xxx` 之后那一步的 `result` 是 jayway 从 JSON 页读出来的 Java 对象。
    // `resultRule` 走**同一条产品路径**(getElement → bind_value → evalJS),
    // 裁判侧 jsharness/Main.kt 的 `host: "rule"` 分支逐字同口径。
    if let Some(rr) = c["resultRule"].as_str() {
        // `@js:` 形态的 resultRule 要真跑一段 JS(裁判侧就是同一个 AnalyzeRule 上的
        // 真 Rhino)。本套的 rule 平时挂空壳 —— **只在这一步**临时换上真引擎、
        // 用完换回,免得给另外两千例悄悄改了 `java.getString` 递归那条路的行为。
        // 约束:resultRule 里的 JS 只许是**纯表达式** —— 它跑在另一份 QuickJsHost
        // 上,日志与 cache 不汇进本 case 的观察面(裁判侧是同一份)。
        let prev = std::mem::replace(
            &mut rule.host,
            Box::new(QuickJsHost::for_difftest(host.config.clone())),
        );
        let got = rule.get_element(rr);
        rule.host = prev;
        match got {
            Ok(v) => bindings.result = v.as_ref().and_then(|v| rule.bind_value(Some(v))),
            // 探针用的规则都是取得到的;取不到要**响亮地**红,不许静默换口径
            Err(e) => return json!({ "id": id, "error": format!("resultRule:{e}") }),
        }
    }

    let code = c["code"].as_str().unwrap_or_default();
    match host.eval_js_detailed(code, &bindings, &mut rule) {
        Ok(d) if d.is_function => {
            // 裁判侧函数是 Scriptable:type=object,toString 带身份哈希,
            // 过 GSON 直接抛(序列化不了函数)
            out["type"] = json!("object");
            out["str"] = json!("«identity»");
            out["normError"] = json!("host:IllegalArgumentException");
        }
        // 未接能力被真身**吞成返回值**的那条路径(ajax/connect):值里带标记
        Ok(d) if matches!(&d.value, JsValue::Str(s) if s.contains(NET_UNSUPPORTED) || s.contains(FS_UNSUPPORTED)) =>
        {
            let kind = match &d.value {
                JsValue::Str(s) if s.contains(FS_UNSUPPORTED) => "fs",
                _ => "net",
            };
            out["error"] = json!(format!("unsupported:{kind}"));
        }
        // 完成值是**解包后的 Java 对象**(List / 数组):裁判侧 typeOf 落在 `other`,
        // str 是 Java 的 toString(),norm 走 `GSON.toJson`(pretty,2 空格缩进)。
        // 见 js-host::java_proxy 与 README「Java 对象代理层」。
        Ok(EvalDetail { java_object: Some(jo), .. }) => {
            out["type"] = json!("other");
            out["str"] = json!(normalize_identity(&jo.str));
            match jo.json.as_ref() {
                Some(j) => out["norm"] = json!(serde_json::to_string_pretty(j).unwrap_or_default()),
                // GSON 序列化不了 jsoup 的 Elements(裁判侧 host:JsonIOException)
                None => out["normError"] = json!("host:JsonIOException"),
            }
        }
        Ok(d) => {
            let v = d.value;
            out["type"] = json!(type_of(&v));
            if let Some(s) = consume_as_analyze_rule(&v) {
                out["str"] = json!(normalize_str(&s));
            }
            match normalize_js_result(&v, d.json.as_ref()) {
                // norm 也要过身份归一 —— 裁判侧是 `normalizeStr(n)`。
                // 漏掉这一层会让 byte[] 的 norm 留着 `[B@1`(str 那半已经归一了)
                Ok(Some(n)) => out["norm"] = json!(normalize_str(&n)),
                Ok(None) => {}
                Err(tag) => out["normError"] = json!(tag),
            }
        }
        Err(e) => {
            // 裁判侧 catch 块会清掉除 id 外的所有字段,这里同口径 ——
            // 但 hops 与 verifyOpens 是**清完之后**才挂的,故这里也要带上
            let mut o = json!({ "id": id, "error": error_tag(&e) });
            if let Some(h) = hops_json(&host) {
                o["hops"] = h;
            }
            if let Some(v) = verify_opens(&verify_user) {
                o["verifyOpens"] = v;
            }
            return o;
        }
    }

    let mut vars = serde_json::Map::new();
    if let Some(layer) = &rule.data.book {
        let mut keys: Vec<&String> = layer.vars.keys().collect();
        keys.sort();
        for k in keys {
            vars.insert(k.clone(), json!(layer.vars[k]));
        }
    }
    out["vars"] = Value::Object(vars);
    out["logs"] = json!(normalize_logs(&host.logs));
    // **CacheManager 终态**(裁判侧 CacheManager.snapshot):`source` 宿主的
    // `java.put/get` 打的就是这张表,登录头/登录信息/源变量也落它。空则不出字段。
    if let Some(m) = map_json(&host.cache) {
        out["cache"] = m;
    }
    // **ACache 终态**(`cache.putFile/getFile` 那张,裁判侧 fileSnapshot)。
    // 与上面那张是两处存储 —— 真身 `put` 写的 `getFile` 读不到。
    if let Some(m) = map_json(&host.cache_file) {
        out["cacheFile"] = m;
    }
    if let Some(h) = hops_json(&host) {
        out["hops"] = h;
    }
    if let Some(v) = verify_opens(&verify_user) {
        out["verifyOpens"] = v;
    }
    out
}

/// **界面被要求弹什么**(`java.startBrowser*` / `getVerificationCode`):返回值
/// 只说了一半,另一半是用户看见了什么。一次都没弹就不出这个字段。
type VerifyUser = Option<(
    std::sync::Arc<net::verification::Verification>,
    std::sync::Arc<difftest::verify_script::ScriptedUser>,
)>;

fn verify_opens(user: &VerifyUser) -> Option<Value> {
    let (_, u) = user.as_ref()?;
    let opens = difftest::verify_script::opens_json(u);
    if opens.as_array().is_some_and(Vec::is_empty) {
        return None;
    }
    Some(opens)
}

/// `Debug.log` 的归一(裁判侧 jsharness/Main.kt 的 `normalizeLog`)
fn normalize_logs(logs: &[String]) -> Vec<String> {
    logs.iter()
        .map(|l| {
            if l.contains(NET_UNSUPPORTED) || l.contains("未接入") {
                "«unsupported:net»".to_string()
            } else if l.contains(FS_UNSUPPORTED) {
                "«unsupported:fs»".to_string()
            } else {
                // 裁判侧 normalizeLog:有换行且换行之后是异常栈 → 留首行 + 标记;
                // 整条就是异常栈 → 整条归一(`java.log(e)` 那条路)
                match l.split_once('\n') {
                    Some((head, tail)) if is_java_stack(tail) => format!("{head}\n«java-stack»"),
                    _ if is_java_stack(l) => "«java-stack»".to_string(),
                    _ => l.clone(),
                }
            }
        })
        .collect()
}

/// 一张表 → 按键排序的 JSON 对象;空则不出字段(裁判同口径)
fn map_json(m: &js_host::host_env::SharedMap) -> Option<Value> {
    if m.is_empty() {
        return None;
    }
    let mut o = serde_json::Map::new();
    for (k, v) in m.sorted_pairs() {
        o.insert(k, json!(v));
    }
    Some(Value::Object(o))
}

/// 本 case 发出的请求序列。空则不出这个字段(裁判同口径)。
fn hops_json(host: &QuickJsHost) -> Option<Value> {
    let hops = host.net.as_ref()?.hops();
    if hops.is_empty() {
        return None;
    }
    Some(json!(hops.iter().map(|(m, u)| format!("{m} {u}")).collect::<Vec<_>>()))
}

fn main() {
    // 与裁判读同一份文本(tools/js_diff.sh 复制到 cases.json 的同级目录)
    let det = std::env::args()
        .nth(1)
        .and_then(|p| std::path::Path::new(&p).parent().map(|d| d.join("determinism.js")))
        .and_then(|p| std::fs::read_to_string(p).ok());
    let _ = DETERMINISM.set(det);
    // 壳(钉差分 UA / 读 cases / 写 jsonl)收在 difftest::run_jsonl,三个 runner 同一份
    difftest::run_jsonl(
        "js_case_runner <cases.json> <out.jsonl> [http快照根目录]",
        run_case_guarded,
    );
}

/// 单 case 跑在**可弃线程**上,5 秒闸 + panic 兜底。
///
/// 为什么必须有:裁判那边每个 case 就是这么跑的(`jsharness/Main.kt`:
/// 单线程 executor + `future.get(5, SECONDS)` + `ExecutionException` 兜底),
/// 而这边的主循环从前既无超时也无 `catch_unwind` —— 一次 panic 或死循环会废掉
/// **整轮**的全部输出。判据的鲁棒性得对称:裁判报 `error: "timeout"` 的 case,
/// 被测侧也得拿得出同一个值,否则那一条永远只能靠豁免。
///
/// 超时的线程杀不掉(和裁判的 `future.cancel(true)` 一样杀不掉),就地丢开 ——
/// 它是 daemon 语义:进程退出时随之消失。
fn run_case_guarded(c: &Value, snapshot_root: Option<&std::path::PathBuf>) -> Value {
    let id = c["id"].as_str().unwrap_or_default().to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    let (case, root, tid) = (c.clone(), snapshot_root.cloned(), id.clone());
    let spawned = std::thread::Builder::new()
        // 书源 JS 递归得挺深(嵌套 AnalyzeUrl + 规则反调),默认栈不够宽裕
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_case(&case, root.as_ref())
            }))
            .unwrap_or_else(|e| {
                let msg = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "panic".into());
                // 裁判那边没有对应形态,这一条必然 FAIL —— 就是要它显形,
                // 而不是让整轮输出跟着一起没。
                json!({ "id": tid, "error": format!("host:panic {}", msg.lines().next().unwrap_or("")) })
            });
            let _ = tx.send(out);
        });
    match spawned {
        Ok(_) => match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(v) => v,
            // 与裁判逐字同形:`{"id": …, "error": "timeout"}`
            Err(_) => json!({ "id": id, "error": "timeout" }),
        },
        Err(e) => json!({ "id": id, "error": format!("host:spawn {e}") }),
    }
}

// ---------------------------------------------------------------------------
// `op: "analyzeUrl"` —— **`@js:` url 面**
//
// 一条真实的 url 规则(searchUrl)整条走 `net::AnalyzeUrl`:`@js:` / `<js>` /
// `{{js}}` / `<a,b,c>` 页码 + option JSON + 绝对化 + query/form 编码,
// 而 JS 是**真引擎**(裁判侧真 Rhino,这边 QuickJS)。
//
// 与 `analyze-url` 那套(difftest 的 case_runner,两侧 JS 都是确定性桩)分工:
// 那边钉拼装与编码,这边把 JS 打开。`header` **不预置** —— 走
// `BaseSource.getHeaderMap()` 真路径,于是 header 规则(387 源)一并进套。
// ---------------------------------------------------------------------------

/// 把 `QuickJsHost` 借给 `net::AnalyzeUrl`(它要 `Box<dyn HostEnv>` 且会持有)。
/// 宿主本体留在本函数的栈上 —— 求值完还要读它的 `logs` / `cache` / `net`。
///
/// # 安全性
/// 同 `js-host` 里的 `VarPtr`:指针只在本函数内解引用,`AnalyzeUrl` 在
/// `host` 之前 drop,单线程。
struct HostRef(*mut QuickJsHost);

impl rubato_core::host::HostEnv for HostRef {
    fn eval_js(
        &mut self,
        code: &str,
        b: &JsBindings,
        env: &mut dyn rubato_core::host::JsRuleEnv,
    ) -> Result<JsValue, String> {
        unsafe { (*self.0).eval_js(code, b, env) }
    }

    fn web_js(&mut self, req: &rubato_core::host::WebJsRequest<'_>) -> Result<String, String> {
        unsafe { (*self.0).web_js(req) }
    }

    fn log(&mut self, msg: &str) {
        unsafe { (*self.0).log(msg) }
    }
}

fn run_analyze_url(c: &Value, snapshot_root: Option<&std::path::PathBuf>) -> Value {
    let id = c["id"].as_str().unwrap_or_default();

    // 建场收在 case_env,与 `op:"js"` 那条路同一份。**不装确定性垫片**
    // (determinism 给 None):这条路上的 JS 由 AnalyzeUrl 真身自己 eval,
    // 裁判侧没有「先在作用域里跑一段」的入口(jsLib 那条路在 jsharness 是桩),
    // 垫片进不去。只在被测侧冻时钟会造成**单边**差异,反而更糟 —— 故两侧都用
    // 真时钟,摸时钟/随机的书源整条不进本 op(见 tools/gen_js_cases.py 的 `_NONDET`)。
    let CaseEnv {
        source_json,
        source_url,
        source_binding,
        book_binding,
        data,
        mut host,
        verify_user,
    } = match case_env(c, snapshot_root, None) {
        Ok(env) => env,
        Err(e) => return e,
    };

    // `BaseSource.getHeaderMap(hasLoginHeader = true)`:header 规则(可能是 JS,
    // 跑在**第三个宿主**上)+ 默认 UA 注入 + 登录头合并。真身在 AnalyzeUrl 的
    // init 里、headerMapF 缺席时调它。
    let login_header: Option<Vec<(String, String)>> = host
        .cache
        .get_str(&format!("loginHeader_{source_url}"))
        .and_then(|s| js_host::net_face::parse_header_json(&s));
    let mut header_vars = RuleData::default();
    let source_header = {
        let hp: *mut QuickJsHost = &mut host;
        let mut href = HostRef(hp);
        net::source_header::source_header_map(
            source_json["header"].as_str(),
            // AppConfig.userAgent(main 里已钉成裁判垫片 LegadoConfigShims 的固定值)
            net::client::user_agent(),
            login_header.as_deref(),
            &mut href,
            &source_binding,
            &mut header_vars,
        )
    };

    // **元素面**:`org.jsoup.Jsoup.parse(...)` 是 Rhino 的 LiveConnect 全局面,
    // 与 `java` 是谁无关 —— searchUrl 的 `@js:` 里 parse 回来的 HTML 很常见。
    // net 建不出 AnalyzeRule(依赖方向),由这里注入(见 net::SplitEnv)。
    let rule_host: Box<dyn rubato_core::host::RuleHost> =
        Box::new(rule_engine::AnalyzeRule::new(Box::new(rule_engine::NoHost), RuleData::default()));

    let args = net::UrlArgs {
        rule_host: Some(rule_host),
        m_url: c["mUrl"].as_str().unwrap_or_default(),
        base_url: c["baseUrl"].as_str().unwrap_or_default(),
        key: c["key"].as_str(),
        page: c["page"].as_i64().map(|p| p as i32),
        source_header: Some(source_header),
        source_key: Some(&source_url),
        enabled_cookie_jar: source_json["enabledCookieJar"].as_bool().unwrap_or(false),
        book: Some(book_binding),
        source: Some(source_binding.clone()),
        ..Default::default()
    };

    let built = {
        let hp: *mut QuickJsHost = &mut host;
        net::AnalyzeUrl::new(args, Box::new(HostRef(hp)), data)
    };

    let mut out = match built {
        Ok(au) => {
            let mut o = json!({
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
            });
            let mut hm = serde_json::Map::new();
            for (k, v) in &au.header_map {
                hm.insert(k.clone(), json!(normalize_identity(v)));
            }
            o["headers"] = Value::Object(hm);
            let mut vars = serde_json::Map::new();
            if let Some(layer) = &au.data.book {
                let mut keys: Vec<&String> = layer.vars.keys().collect();
                keys.sort();
                for k in keys {
                    vars.insert(k.clone(), json!(layer.vars[k]));
                }
            }
            o["vars"] = Value::Object(vars);
            o["logs"] = json!(normalize_logs(&host.logs));
            o
        }
        // 裁判侧 catch 会清掉除 id 外的字段,这里同口径(hops/cache 在清完后挂)
        Err(e) => json!({ "id": id, "error": url_error_tag(&e) }),
    };

    // CacheManager / ACache 终态,同 run_case(裁判 Main.kt 的这两个字段
    // 也覆盖 analyzeUrl 这一 op)
    if let Some(m) = map_json(&host.cache) {
        out["cache"] = m;
    }
    if let Some(m) = map_json(&host.cache_file) {
        out["cacheFile"] = m;
    }
    if let Some(h) = hops_json(&host) {
        out["hops"] = h;
    }
    if let Some(v) = verify_opens(&verify_user) {
        out["verifyOpens"] = v;
    }
    out
}

/// `UrlError` → 与裁判 `errorTag` 同一套标签。
/// JS 求值那一支穿的是 QuickJS 的消息,复用 `error_tag`;
/// 切分/编码/页码那三支是 AnalyzeUrl 自己抛的 Java 异常,按类名对齐。
fn url_error_tag(e: &net::UrlError) -> String {
    match e {
        net::UrlError::Js(msg) => error_tag(msg),
        // RuleAnalyzer 的括号不平衡:真身抛 NoStackTraceException
        net::UrlError::Split(_) => "host:NoStackTraceException".into(),
        // `charset(name)`:java.nio 的 UnsupportedCharsetException
        net::UrlError::UnsupportedCharset(_) => "host:UnsupportedCharsetException".into(),
        net::UrlError::IndexOutOfBounds => "host:IndexOutOfBoundsException".into(),
    }
}
