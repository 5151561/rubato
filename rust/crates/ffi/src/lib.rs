//! FFI 的**无关桥接层**:进程级引擎句柄 + 任务注册表。
//!
//! 分工:
//! - 这里放跨 FFI 都需要、但与 flutter_rust_bridge **无关**的状态
//!   (引擎单例、`task_id` ↔ 取消令牌),因此能在 workspace 里跑单测;
//! - DTO 与 `#[frb]` 标注的函数面在 `app/rust/src/api/`(FRB 只扫那棵树,
//!   DTO 必须定义在被扫的 crate 里,所以不放这儿);
//! - PlatformHooks(webView 过盾 / 验证码 / loginUi 回调 Dart)挂在
//!   [`platform`]:webView 那四件原语已经接上(M3d),验证码 / loginUi 还没有。
//!
//! 取消语义(计划书 §1「FRB 无自动取消」):Dart 侧显式拿 `task_id`,
//! 退页时调 [`cancel_task`];粒度见 [`engine::shared::CancelToken`]。

pub mod platform;

// 再导出给 app/rust 用:诊断口(search_one*)直接调 `engine()?.xxx()`,
// 一次性的令牌由调用方自己建
pub use engine::shared::CancelToken;
use engine::{Engine, EngineError};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

static ENGINE: OnceLock<Engine> = OnceLock::new();
static TASKS: OnceLock<Mutex<HashMap<i32, CancelToken>>> = OnceLock::new();
static NEXT_TASK: AtomicI32 = AtomicI32::new(1);

/// 打开(或复用)引擎。重复调用**不会**换库:第一次的 `db_path` 生效,
/// 返回 `false` 表示这次调用被忽略。
pub fn init_engine(db_path: &str, search_threads: Option<usize>) -> Result<bool, EngineError> {
    if ENGINE.get().is_some() {
        return Ok(false);
    }
    // webView 的平台面**开库时就注入**:桥这一头没状态,
    // 「现在有没有平台」由它自己去问(Dart 侧登记那条流之前一律没有)
    let e = Engine::open_with(
        db_path,
        Some(platform::provider()),
        search_threads,
        Some(platform::verify_ui()),
    )?;
    Ok(ENGINE.set(e).is_ok())
}

pub fn engine() -> Result<&'static Engine, EngineError> {
    ENGINE.get().ok_or_else(|| EngineError::NotFound("引擎未初始化".into()))
}

fn tasks() -> &'static Mutex<HashMap<i32, CancelToken>> {
    TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 领一个任务号并登记它的取消令牌
pub fn new_task() -> (i32, CancelToken) {
    let id = NEXT_TASK.fetch_add(1, Ordering::SeqCst);
    let token = CancelToken::new();
    tasks().lock().expect("任务锁").insert(id, token.clone());
    (id, token)
}

/// 取回任务号对应的取消令牌(长任务开跑前拿它)
pub fn cancel_token(id: i32) -> Option<CancelToken> {
    tasks().lock().expect("任务锁").get(&id).cloned()
}

/// 取消一个在跑的任务。任务号不存在(已结束)时返回 false。
pub fn cancel_task(id: i32) -> bool {
    match tasks().lock().expect("任务锁").get(&id) {
        Some(t) => {
            t.cancel();
            true
        }
        None => false,
    }
}

/// 任务收尾:从注册表摘掉,避免长会话里无限堆积
pub fn finish_task(id: i32) {
    tasks().lock().expect("任务锁").remove(&id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_lifecycle() {
        let (id, token) = new_task();
        assert!(!token.is_cancelled());
        assert!(cancel_task(id));
        assert!(token.is_cancelled());
        finish_task(id);
        // 结束后再取消是 no-op,不是错误
        assert!(!cancel_task(id));
    }

    #[test]
    fn engine_is_single_instance() {
        assert!(init_engine(":memory:", None).expect("首次"));
        assert!(!init_engine(":memory:", None).expect("重复"));
        assert!(engine().is_ok());
    }
}
