//! 剧本「用户」(差分侧的 [`VerifyUi`]):`java.startBrowserAwait` /
//! `java.getVerificationCode` 那三件要人出手,而差分里没有人。
//!
//! 与 webView 那份剧本([`crate::wv_script`])同一个思路:界面录不下来,
//! 于是 case 里写清「用户会怎么答」,两侧照着答 —— 裁判那边的同一份在
//! `judge/jsharness/VerifyStage.kt`(它顶掉 `startActivity`,替
//! `SourceVerificationHelp` 填结果)。
//!
//! ```json
//! "verify": { "result": "<html>过了</html>", "url": "https://a.example/ok" }
//! "verify": { "close": true }      // 用户把界面关掉 → 「验证结果为空」
//! ```
//!
//! **不带 `verify` 字段 = 这一侧没接界面**:三件都报
//! [`NET_UNSUPPORTED`](rubato_core::host::NET_UNSUPPORTED) —— 与接进来之前
//! 逐字同一档,所以老 case 一例不动。

use net::verification::{Verification, VerifyOpen, VerifyUi};
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// 一次 case 里「用户」的答复
#[derive(Debug, Clone, Default)]
pub struct VerifyScript {
    /// 用户交回来的结果(网页源码 / 验证码)
    pub result: Option<String>,
    /// 交回来的地址(真身 `setResult(key, result, url)` 的 url;空 = 回退到入参)
    pub url: Option<String>,
    /// 用户把界面关掉了(真身 `checkResult`:塞空结果 → 「验证结果为空」)
    pub close: bool,
}

impl VerifyScript {
    /// 从 case 的 `verify` 字段解析;字段缺席 = `None`(这一侧没接界面)
    pub fn parse(case: &Value) -> Option<VerifyScript> {
        let o = case.get("verify")?.as_object()?;
        let text = |k: &str| o.get(k).and_then(Value::as_str).map(str::to_string);
        Some(VerifyScript {
            result: text("result"),
            url: text("url"),
            close: o.get("close").and_then(Value::as_bool).unwrap_or(false),
        })
    }
}

/// 剧本用户:界面一「弹出来」就照剧本答复。
///
/// 真身那边 `WebViewActivity` 是另一个界面、另一条线程,答复自然晚一些;
/// 这里当场答 —— 等待那一侧的循环**第一圈就看见结果**,于是两侧都不必真等
/// (与 webView 剧本「不真的睡」同一个道理)。
pub struct ScriptedUser {
    script: VerifyScript,
    /// 回填结果要用到 [`Verification`] 自己 —— 建好之后由 [`wire`] 装上
    holder: Mutex<Option<Arc<Verification>>>,
    /// 观察面:界面弹了几次、每次是什么(差分要比的就是它)
    pub opens: Mutex<Vec<VerifyOpen>>,
}

impl VerifyUi for ScriptedUser {
    fn open(&self, req: &VerifyOpen) {
        self.opens.lock().expect("verify 剧本锁").push(req.clone());
        // `startBrowser` 那条不等结果 —— 没有挂号 key,答无可答
        let Some(key) = req.key.as_deref() else { return };
        let Some(v) = self.holder.lock().expect("verify 剧本锁").clone() else { return };
        if self.script.close {
            v.check_result(key);
            return;
        }
        if let Some(r) = &self.script.result {
            v.set_result(key, self.script.url.as_deref().unwrap_or(""), r);
        }
        // 两者都没写 = 用户不理会 → 等待那侧只能等到取消/超时(case 别这么写)
    }
}

/// 建一对「策略 + 剧本用户」并互相接上
pub fn wire(script: VerifyScript) -> (Arc<Verification>, Arc<ScriptedUser>) {
    let user =
        Arc::new(ScriptedUser { script, holder: Mutex::new(None), opens: Mutex::new(Vec::new()) });
    let v = Arc::new(Verification::new(user.clone()));
    *user.holder.lock().expect("verify 剧本锁") = Some(v.clone());
    (v, user)
}

/// 观察面的归一形态:**界面被要求弹什么**。
///
/// 两侧的字段来源不同(这边是 [`VerifyOpen`],裁判那边是 `Intent` 的 extras),
/// 归一到同一张表才比得了。挂号 key 本身不比(真身是 UUID)——
/// 只比**要不要等结果**这一位。
pub fn opens_json(user: &ScriptedUser) -> serde_json::Value {
    let list: Vec<serde_json::Value> = user
        .opens
        .lock()
        .expect("verify 剧本锁")
        .iter()
        .map(|o| match o.kind {
            net::verification::VerifyKind::Browser => serde_json::json!({
                "kind": "browser",
                "url": o.url,
                "title": o.title,
                "saveResult": o.save_result,
                "refetchAfterSuccess": o.refetch_after_success,
                "html": o.html,
                "sourceKey": o.source_key,
                "sourceName": o.source_name,
                "sourceType": o.source_type,
                "waits": o.key.is_some(),
                // **可见浏览器到底怎么加载这一页**(真身在 `WebViewModel.initData`
                // 里算,裁判那边由 `jsharness.VerifyStage` 照同一份真身算)
                "browserLoad": o.browser_load.as_ref().map(load_json),
            }),
            // 验证码那个界面**没有** title / saveResult / refetch / html 那几位
            // (真身 VerificationCodeActivity 的 extras 就这几条)
            net::verification::VerifyKind::Code => serde_json::json!({
                "kind": "code",
                "url": o.url,
                "sourceKey": o.source_key,
                "sourceName": o.source_name,
                "sourceType": o.source_type,
                "waits": o.key.is_some(),
            }),
        })
        .collect();
    serde_json::Value::Array(list)
}

/// `BrowserLoad` 的归一形态。头按**对象**出(裁判那边是 `HashMap`,
/// 顺序是哈希序,比不了顺序);空表也照出,`null` 只留给「算不出来」。
fn load_json(l: &net::verification::BrowserLoad) -> serde_json::Value {
    let mut headers = serde_json::Map::new();
    for (k, v) in &l.headers {
        headers.insert(k.clone(), serde_json::json!(v));
    }
    serde_json::json!({
        "url": l.url,
        "userAgent": l.user_agent,
        "headers": serde_json::Value::Object(headers),
        "html": l.html,
    })
}
