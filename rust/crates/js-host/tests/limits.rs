//! 执行闸与重入自检的单测。
//!
//! 这几条是**产品面**的:书源 JS 是用户从网上导入的第三方代码,
//! 一段 `while(true)` 从前会把引擎线程挂死(`CancelToken` 只在步与步之间
//! 生效,救不了)。差分套照不到它们 —— 裁判那边的闸在 harness 上,
//! 不在被比较的输出里。

use js_host::host_env::{JsLimits, QuickJsHost};
use rubato_core::host::{HostEnv, JsBindings, RuleHost, VarStore};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Env(HashMap<String, String>);

impl VarStore for Env {
    fn put(&mut self, k: &str, v: &str) -> String {
        self.0.insert(k.into(), v.into());
        v.into()
    }
    fn get(&self, k: &str) -> String {
        self.0.get(k).cloned().unwrap_or_default()
    }
}
impl RuleHost for Env {}

fn eval(host: &mut QuickJsHost, code: &str) -> Result<rubato_core::host::JsValue, String> {
    let mut env = Env::default();
    host.eval_js(code, &JsBindings::default(), &mut env)
}

#[test]
fn deadline_stops_an_infinite_loop() {
    let mut host = QuickJsHost::new();
    host.limits = JsLimits { timeout: Some(Duration::from_millis(200)), ..JsLimits::default() };
    let t0 = Instant::now();
    let r = eval(&mut host, "while (true) {}");
    assert!(r.is_err(), "死循环必须被闸住,拿到的是 {r:?}");
    // 中断钩子在字节码层生效,不该拖到秒级
    assert!(t0.elapsed() < Duration::from_secs(5), "闸抬得太晚:{:?}", t0.elapsed());
}

#[test]
fn cancel_hook_stops_an_infinite_loop() {
    let mut host = QuickJsHost::new();
    host.limits = JsLimits::unlimited();
    // 「用户已经点了停」:钩子恒为真,第一次回调就该中断
    host.cancel = Some(std::rc::Rc::new(|| true));
    let t0 = Instant::now();
    assert!(eval(&mut host, "while (true) {}").is_err());
    assert!(t0.elapsed() < Duration::from_secs(5));
}

#[test]
fn memory_limit_is_enforced() {
    let mut host = QuickJsHost::new();
    host.limits = JsLimits { memory: Some(4 * 1024 * 1024), ..JsLimits::unlimited() };
    // 一直往数组里塞字符串,4 MiB 之内必然撞墙
    let r = eval(
        &mut host,
        "var a = []; for (var i = 0; i < 1e7; i++) { a.push('xxxxxxxxxxxxxxxx' + i); } a.length",
    );
    assert!(r.is_err(), "内存上限没生效:{r:?}");
}

#[test]
fn limits_do_not_disturb_ordinary_code() {
    let mut host = QuickJsHost::new(); // 缺省 30 秒 / 256 MiB
    let v = eval(&mut host, "1 + 1").expect("正经代码不该被闸住");
    assert_eq!(v, rubato_core::host::JsValue::Num(2.0));
}

/// 同一个宿主嵌套求值会让两条 `VarPtr` 同时活着 —— UB 的前一步。
/// 外层由 rule-engine 的 `NoHost` 挡,这一条钉的是宿主自己的兜底。
#[test]
fn reentrancy_is_refused() {
    struct Reenter(*mut QuickJsHost);
    impl VarStore for Reenter {
        fn put(&mut self, _k: &str, _v: &str) -> String {
            String::new()
        }
        fn get(&self, _k: &str) -> String {
            // 求值途中拿同一个 host 再进一次
            let mut env = Reenter(self.0);
            let r = unsafe { (*self.0).eval_js("1", &JsBindings::default(), &mut env) };
            match r {
                Err(e) => e,
                Ok(_) => "重入居然成功了".into(),
            }
        }
    }
    impl RuleHost for Reenter {}

    let mut host = QuickJsHost::new();
    let hp: *mut QuickJsHost = &mut host;
    let mut env = Reenter(hp);
    let v = host.eval_js("java.get('k')", &JsBindings::default(), &mut env).expect("外层照常返回");
    match v {
        rubato_core::host::JsValue::Str(s) => {
            assert!(s.contains("重入"), "内层应报重入,实际是 {s}");
        }
        other => panic!("形态不对:{other:?}"),
    }
}

/// **同一个 realm 的多次全局 eval 共享绑定**。
///
/// `dialect::split_value_tail` 那条路把脚本切成几段分别 eval,靠的就是这一条:
/// 切开之后 `var` / `let` / `function` / 未声明赋值四种绑定都还看得见。
/// 哪天换引擎或升 rquickjs 把这一条打破,分段求值会静默给出错的完成值 ——
/// 所以钉在这里,而不是留在注释里。
#[test]
fn cross_eval_scope_is_shared() {
    use rubato_core::host::JsValue;
    let mut host = QuickJsHost::new();
    // 分段求值:`x=1; y=x+1;` 是头一段,`if` 收尾的那条是第二段 ——
    // 第二段看得见头一段的绑定,且完成值留用上一条(规范的 UpdateEmpty)
    assert_eq!(eval(&mut host, "x=1; y=x+1; if(false){9}"), Ok(JsValue::Num(2.0)));
    // `var` / `let` / `function` 三种声明跨段同样看得见
    assert_eq!(
        eval(&mut host, "var a=1; let b=2; function f(){return a+b}; z=f(); if(0){}"),
        Ok(JsValue::Num(3.0))
    );
    // 末段真的产生了值时,用的是它自己的(不是退回上一段)
    assert_eq!(eval(&mut host, "x=1; if(true){7}"), Ok(JsValue::Num(7.0)));
}
