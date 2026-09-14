//! **固定脚本的字节码缓存**:JAVA_PROXY / PRELUDE / 装箱包装器这批脚本
//! 每次求值都要在全新 Context 里跑一遍,而它们的文本从不变 —— 变的只有
//! 执行时读到的全局。此前每次都重新**解析**(一次求值里光 JAVA_PROXY 就
//! 350µs,docs/engine-perf.md §3②);QuickJS 的序列化字节码
//! (`JS_WriteObject`/`JS_ReadObject`,qjsc 的那条路)**不绑 Runtime**:
//! atom 以字符串形式写出、读入时在目标 Runtime 里重新驻留。于是可以
//! 进程内编译一次,之后每个新 Context 只做「反序列化 + 执行」,解析整个跳过。
//!
//! **语义不动**:仍是每次求值全新 Runtime/Context、脚本仍逐次执行,
//! 执行闸(内存/中断)照常生效 —— 变的只有「文本 → 字节码」这一步在哪付。
//!
//! 两条口径:
//! - 编译 flags 是 `GLOBAL | STRICT`,与这批脚本原来走的 `ctx.eval`
//!   (`EvalOptions::default()`)逐位相同。**flags 编进字节码**,如果哪天
//!   这批脚本改走别的 eval 选项,这里要跟着动,否则缓存的是另一种语义。
//! - 只缓存**编译成功**的;编译错(只可能是我们自己的脚本有语法错)原样上抛,
//!   序列化失败(不该发生)则本次直接执行、不缓存,行为与不缓存完全一致。

use rquickjs::{Ctx, qjs};
use std::ffi::CString;
use std::sync::OnceLock;

/// 一段固定脚本的缓存位。调用方持有 `static`,与脚本文本一一对应 ——
/// **换了文本必须换缓存位**(设一次之后不再看 src)。
pub struct Cached(OnceLock<Vec<u8>>);

impl Cached {
    pub const fn new() -> Cached {
        Cached(OnceLock::new())
    }
}

/// 在 `ctx` 上求值 `src`,等价于 `ctx.eval::<(), _>(src)`(GLOBAL+STRICT),
/// 但解析结果经 `cell` 跨 Context 复用。出错时异常留在 ctx 上
/// (`Err(Error::Exception)`),`format_js_error` 照常取得到。
pub fn eval_cached(ctx: &Ctx<'_>, cell: &Cached, src: &str) -> rquickjs::Result<()> {
    let c = ctx.as_raw().as_ptr();
    // SAFETY:c 来自活着的 Ctx;JS_EvalFunction 消费 func 的所有权,
    // 返回值(脚本完成值,这批脚本都是 undefined)由我们释放。
    unsafe {
        let func = match cell.0.get() {
            Some(bc) => {
                let v = qjs::JS_ReadObject(
                    c,
                    bc.as_ptr(),
                    bc.len() as _,
                    qjs::JS_READ_OBJ_BYTECODE as i32,
                );
                if qjs::JS_IsException(v) {
                    return Err(rquickjs::Error::Exception);
                }
                v
            }
            None => {
                let cstr = CString::new(src)?;
                let flags = (qjs::JS_EVAL_TYPE_GLOBAL
                    | qjs::JS_EVAL_FLAG_STRICT
                    | qjs::JS_EVAL_FLAG_COMPILE_ONLY) as i32;
                // filename 与 rquickjs 的 ctx.eval 同名:异常栈会把它带进
                // format_js_error 的输出,不许因为换了求值路径而变
                let v =
                    qjs::JS_Eval(c, cstr.as_ptr(), src.len() as _, c"eval_script".as_ptr(), flags);
                if qjs::JS_IsException(v) {
                    return Err(rquickjs::Error::Exception);
                }
                let mut size = 0;
                let buf = qjs::JS_WriteObject(c, &mut size, v, qjs::JS_WRITE_OBJ_BYTECODE as i32);
                if buf.is_null() {
                    // 序列化失败只可能带着挂起异常 —— 清掉,退回「本次直接执行」
                    let _ = ctx.catch();
                } else {
                    let bytes = std::slice::from_raw_parts(buf, size as usize).to_vec();
                    qjs::js_free(c, buf.cast());
                    // 并发首跑会都编译一遍,set 输了的一方丢掉自己那份 —— 内容相同
                    let _ = cell.0.set(bytes);
                }
                v
            }
        };
        let ret = qjs::JS_EvalFunction(c, func);
        if qjs::JS_IsException(ret) {
            return Err(rquickjs::Error::Exception);
        }
        qjs::JS_FreeValue(c, ret);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rquickjs::{Context, Runtime};

    fn with_ctx<R>(f: impl FnOnce(&Ctx<'_>) -> R) -> R {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        ctx.with(|ctx| f(&ctx))
    }

    /// 缓存的正当性:同一段脚本,「首跑编译」与「次跑读字节码」在**不同的
    /// Runtime** 上执行,可观察结果一致 —— 跨 Runtime 正是缓存存在的前提。
    #[test]
    fn bytecode_reused_across_runtimes() {
        static CELL: Cached = Cached::new();
        let src = "globalThis.__probe = (typeof __probe === 'undefined' ? 0 : __probe) + 41;
                   var __hoisted = '声明也要进全局';";
        for round in 0..2 {
            with_ctx(|ctx| {
                eval_cached(ctx, &CELL, src).unwrap();
                // 每个 Context 都是新的,__probe 每次都从 0 起算
                let v: i32 = ctx.eval("__probe").unwrap();
                assert_eq!(v, 41, "round {round}");
                let s: String = ctx.eval("__hoisted").unwrap();
                assert_eq!(s, "声明也要进全局", "round {round}");
            });
        }
        assert!(CELL.0.get().is_some(), "首跑之后缓存位必须已填");
    }

    /// 编译错原样上抛,且不污染缓存位(下次换正确脚本仍可用 —— 这里用
    /// 独立缓存位模拟「换了文本必须换缓存位」的口径)。
    #[test]
    fn compile_error_propagates() {
        static BAD: Cached = Cached::new();
        with_ctx(|ctx| {
            let e = eval_cached(ctx, &BAD, "function {").unwrap_err();
            assert!(matches!(e, rquickjs::Error::Exception));
            let _ = ctx.catch();
        });
        assert!(BAD.0.get().is_none(), "编译错不许进缓存");
    }

    /// 运行期异常也要走 Exception 通道(异常对象留在 ctx 上可 catch)。
    #[test]
    fn runtime_error_propagates() {
        static THROWS: Cached = Cached::new();
        for _ in 0..2 {
            // 两轮:首跑(编译路径)与次跑(字节码路径)都要抛
            with_ctx(|ctx| {
                let e = eval_cached(ctx, &THROWS, "throw new Error('runtime')").unwrap_err();
                assert!(matches!(e, rquickjs::Error::Exception));
                let msg: String =
                    ctx.catch().as_exception().and_then(|x| x.message()).unwrap_or_default();
                assert_eq!(msg, "runtime");
            });
        }
    }
}
