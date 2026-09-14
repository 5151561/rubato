//! 书源 JS 宿主:rquickjs(QuickJS)上复刻真身的三个 `evalJS`。
//!
//! - [`host_env`] —— 宿主本体:三个绑定面、`java` 对象、执行闸;
//! - [`java_api`] —— `JsExtensions` / `JsEncodeUtils` 的移植(纯函数面);
//! - [`java_proxy`] —— Rhino 的 LiveConnect 装箱层(`java.*` 的返回值在真身里
//!   不是 JS 原生值);
//! - [`live_connect`] —— `JavaImporter` / `Packages.javax.crypto` 那一面。
//!
//! 口径由 **js-host 差分套**钉(裁判是 judge/jsharness 上的真 Rhino + 真身
//! JsExtensions),契约见 `fixtures/cases/js-host/README.md`。
//!
//! 本文件曾经还挂着 Phase 0 的 spike(`eval_snippet` / `HostState` + 自己那份
//! PRELUDE)。那份 PRELUDE 里补的 `importClass` / `Packages` / `JavaImporter`
//! **已被 M1 实测证伪**(真身里 `importClass` 是 undefined),留着早晚有人照抄,
//! 故随 `bin/s1`、`bin/s1_corpus` 一并删掉 —— 它们的活现在由差分套 + corpus.json 干。

mod bytecode;
mod dialect;
pub mod host_env;
pub mod java_api;
pub mod java_proxy;
pub mod live_connect;
pub mod net_face;

use md5::{Digest, Md5};
use rquickjs::{Ctx, Value};

pub(crate) fn md5_hex(s: &str) -> String {
    let mut h = Md5::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

/// QuickJS 的错误 → 差分口径的错误串。
///
/// 错误名(SyntaxError/TypeError/…)要带上:差分只比**错误类别**,
/// 消息文本两个引擎必然不同(契约见 fixtures/cases/js-host/README.md)。
pub(crate) fn format_js_error(ctx: &Ctx<'_>, e: rquickjs::Error, phase: &str) -> String {
    if matches!(e, rquickjs::Error::Exception) {
        let caught = ctx.catch();
        if let Some(ex) = caught.as_exception() {
            let name: String = ex
                .get::<_, Value>("name")
                .ok()
                .and_then(|v| v.as_string().and_then(|s| s.to_string().ok()))
                .unwrap_or_default();
            return format!(
                "{phase}: {}{}{}",
                if name.is_empty() { String::new() } else { format!("{name}: ") },
                ex.message().unwrap_or_default(),
                ex.stack().map(|s| format!("\n{s}")).unwrap_or_default()
            );
        }
        return format!("{phase}: exception: {caught:?}");
    }
    format!("{phase}: {e}")
}
