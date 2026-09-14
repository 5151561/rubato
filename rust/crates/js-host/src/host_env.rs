//! 产品侧的 [`HostEnv`] 实现:rquickjs 上跑书源 JS。
//!
//! **Phase 2 起口径变了**:本文件的行为由 `js-host` 差分套钉
//! (裁判 `judge/jsharness` 上的 `JsExtensions` 真身,契约见
//! `fixtures/cases/js-host/README.md`)。两处随之改掉的 Phase 1 决定:
//! ① 求值走**非严格模式**(Rhino 跑书源是 sloppy mode);
//! ② `java.ajax`/`connect` 这类未接的网络能力**不再抛异常,而是记一条日志并
//! 返回标记串** —— 真身就是「吞掉异常、返回栈字符串」,差分要求连吞不吞都一致。
//! 等网络面接上,标记串消失。
//!
//! **范围**:Phase 1 只要跑通「A 层源可用」这条线。A 层的定义是无
//! `<js>`/`@js:`/jsLib(tools/extract_js.py 的 classify),但 `searchUrl`
//! 里的 `{{key}}`/`{{page}}` 模板在真身里同样走 `evalJS`
//! (AnalyzeUrl.replaceKeyPageJs → innerRule → evalJS),所以**没有 JS 引擎
//! 就搜不了书**。这里提供的正是这条最小面:
//!
//! - 绑定 `key` / `page` / `result` / `baseUrl` / `src` / `title` /
//!   `nextChapterUrl` / `fromBookInfo`;
//! - `java` 对象:`put`/`get`(打到 rule-engine 的变量层)、`log`、
//!   md5/base64/hex、`encodeURI`/`utf8ToGbk` 之类的纯函数;
//! - 未接的面(`java.ajax`/`webView`/`cookie`/`cache`/`book`/`source`
//!   对象、jsLib、SharedJsScope)**显式抛异常**,让规则以「求值失败」
//!   的形式暴露,而不是静默给空串 —— 补齐是 Phase 2 的 js-host 差分范围。
//!
//! 注意:**它不参与差分**。差分侧永远是确定性桩
//! (difftest::stub_host,契约 fixtures/cases/rule-engine/README.md);
//! 真 JS 与 Rhino 的方言差异要等 Phase 2 建 js-host 差分套才算数。

use crate::java_api::{self, HostConfig, SymOp};
use crate::{format_js_error, md5_hex};
use rquickjs::function::Func;
use rquickjs::{Context, Ctx, FromJs, Object, Runtime, Value};
use rubato_core::host::{
    BoundValue, ElementHandle, HostEnv, JsBindings, JsHost, JsRuleEnv, JsValue, NET_UNSUPPORTED,
    NetError, NetProvider, NetResponse,
};
use std::cell::RefCell;
use std::rc::Rc;

/// `java.*` 的桥:把 `&mut dyn JsRuleEnv`(只活在一次 `eval_js` 调用里)
/// 借给 JS 闭包。变量层(`put`/`get`)与规则反调面(`getString`/`getElements`/
/// `setContent`)是**同一个对象** —— 真身里 `java` 就是那一个 AnalyzeRule 实例。
///
/// # 安全性
/// 裸指针只在 `eval_js` 的函数体内有效:Context 与其上的闭包都在这次调用里
/// 创建、在返回前 drop,QuickJS 不会让函数逃出被销毁的 Context;JS 单线程,
/// 同一时刻只有一个闭包在解引用。
///
/// **还有一条别名不变量**:同一个 `QuickJsHost` 不能在自己的 `eval_inner`
/// 里被再次 `&mut` —— 那会让两条 `VarPtr` 同时活着。挡住它的有两道:
/// 外层是 rule-engine 的 `NoHost`(求值期间把 host 换出去,反调进来即报错),
/// 内层是 [`QuickJsHost::in_eval`] 自检(不依赖调用方,给 `RuleHost` 新加一条
/// 会回调 JS 的方法也不会破)。改这两处任何一处前先回来读这段。
#[derive(Clone, Copy)]
struct VarPtr(*mut (dyn JsRuleEnv + 'static));

impl VarPtr {
    /// # Safety
    /// 调用者保证 `v` 在本 `VarPtr` 的全部使用期间存活且无其他别名。
    unsafe fn new(v: &mut dyn JsRuleEnv) -> VarPtr {
        let p: *mut (dyn JsRuleEnv + '_) = v;
        VarPtr(unsafe {
            std::mem::transmute::<*mut (dyn JsRuleEnv + '_), *mut (dyn JsRuleEnv + 'static)>(p)
        })
    }

    #[allow(clippy::mut_from_ref)]
    fn get(&self) -> &mut dyn JsRuleEnv {
        unsafe { &mut *self.0 }
    }
}

/// `cache`(CacheManager)/ `cookie`(CookieStore)两张表。
///
/// **共享句柄,不是裸 HashMap**:真身那两个都是**进程级单例**,而被测侧一条规则
/// 一个 `AnalyzeRule`、一个 `QuickJsHost`(engine 的 host_factory)。表若长在
/// host 上,`java.put` / `sourceVariable_*` / `loginHeader_*` 写完就随 host 一起丢
/// —— 352 源有 loginUrl、274 源有 header,那些路径整条是空的。
/// 同一个源的多个宿主 `clone()` 同一个句柄即可共用。
///
/// 这里曾经是一个裸指针(`CacheP`),连带一条「ctx 在本函数内 drop」的不变量;
/// 换成 `Rc<RefCell<…>>` 之后那条不变量不用再守,借用也只在方法体内成立
/// (**不要**把 `borrow_mut()` 跨 JS 回调持有)。
#[derive(Clone, Default)]
pub struct SharedMap(Rc<RefCell<std::collections::HashMap<String, String>>>);

impl SharedMap {
    pub fn new() -> SharedMap {
        SharedMap::default()
    }

    pub fn get_str(&self, k: &str) -> Option<String> {
        self.0.borrow().get(k).cloned()
    }

    pub fn insert(&self, k: String, v: String) {
        self.0.borrow_mut().insert(k, v);
    }

    pub fn remove(&self, k: &str) {
        self.0.borrow_mut().remove(k);
    }

    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }

    /// 按键排序的全部内容(差分输出 / 调试用)
    pub fn sorted_pairs(&self) -> Vec<(String, String)> {
        let m = self.0.borrow();
        let mut v: Vec<(String, String)> = m.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }
}

/// 网络提供方的桥,手法同 [`VarPtr`]:实现活在 `QuickJsHost` 上,
/// 闭包只在一次 `eval_js` 调用里解引用。
///
/// # 安全性
/// 同 [`VarPtr`](含**重入**那一条)。
#[derive(Clone, Copy)]
struct NetP(*mut (dyn NetProvider + 'static));

impl NetP {
    /// # Safety
    /// 调用者保证 `n` 在本 `NetP` 的全部使用期间存活且无其他别名。
    unsafe fn new(n: &mut dyn NetProvider) -> NetP {
        let p: *mut (dyn NetProvider + '_) = n;
        NetP(unsafe {
            std::mem::transmute::<*mut (dyn NetProvider + '_), *mut (dyn NetProvider + 'static)>(p)
        })
    }

    #[allow(clippy::mut_from_ref)]
    fn get(&self) -> &mut dyn NetProvider {
        unsafe { &mut *self.0 }
    }
}

/// 每次求值开一个新 Runtime/Context。书源之间不共享 scope
/// (真身的 SharedJsScope/jsLib 是 Phase 2)。
#[derive(Default)]
pub struct QuickJsHost {
    pub logs: Vec<String>,
    /// 环境常量(androidId / WebView UA / 随机源);差分侧注入裁判垫片的固定值
    pub config: HostConfig,
    /// `cache` 绑定(真身的 `CacheManager`)。裁判每个 case 前 `clearAll()`,
    /// 差分侧对应「host 实例活多久,cache 活多久」。
    /// **产品侧要与同一个源的其他宿主共享**(见 [`SharedMap`]);
    /// 落 store 的 `caches` 表(带 saveTime TTL)仍是后续。
    pub cache: SharedMap,
    /// `cache.putFile/getFile` 那一支的表 —— **与 [`QuickJsHost::cache`] 是
    /// 两张表**,不是同一张。真身 `CacheManager.put/get` 打 `cacheDao` + 内存,
    /// 而 `putFile/getFile` 打的是 `ACache`(落盘,CacheManager.kt L151/L155):
    /// `cache.put('k', v)` 之后 `cache.getFile('k')` 在真身里读不到东西。
    /// 合成一张会让「写内存、读磁盘」这类书源在两侧走岔。
    pub cache_file: SharedMap,
    /// `cookie` 绑定的**退化实现**:一张按域名键的裸表。
    /// 只在没有 [`QuickJsHost::cookie_store`] 时用(js-host 差分套的老口径)。
    pub cookies: SharedMap,
    /// `cookie` 绑定的**真实存储**。真身的 `cookie` 就是 `CookieStore` 单例,
    /// 与网络层写的是同一张表 —— JS 里 `cookie.setCookie(...)` 存下的,下一跳
    /// 请求就带得上。此前 js-host 自己攥着一张 [`SharedMap`],于是书源
    /// `cookie.setCookie` 写的东西**既不进 cookiesDb、也带不上后续请求**
    /// (差分与产品同病,pipeline-corpus-b pb01192 照出来的)。
    pub cookie_store: Option<Rc<RefCell<dyn rubato_core::host::CookieEnv>>>,
    /// **执行闸**。书源是用户从网上导入的第三方代码,`while(true)` 会把引擎线程
    /// 挂死 —— `CancelToken` 只在步与步之间生效,救不了。见 [`JsLimits`]。
    pub limits: JsLimits,
    /// 外部取消钩子(engine 注入 `CancelToken`)。返回 `true` 即刻中断当前求值,
    /// **不必等到下一步**。差分侧不装。
    pub cancel: Option<Rc<dyn Fn() -> bool>>,
    /// 重入自检:`eval_inner` 期间置位。见 [`VarPtr`] 的安全性说明。
    /// `Rc` 是为了让复位守卫不借住 `self`(求值本身要 `&mut self`)。
    in_eval: Rc<std::cell::Cell<bool>>,
    /// **差分专用**:两侧共享的确定性垫片(冻 `Date` 与 `Math.random`)。
    /// 产品侧恒为空 —— 真实运行当然要用真时钟。
    /// 文本来自 `fixtures/cases/js-host/determinism.js`,裁判侧读的是同一份。
    pub determinism: Option<String>,
    /// **网络面**(`java.ajax` / `connect` / `get`/`head`/`post`)。
    /// `None` = 没接网络:那几个名字仍走 PRELUDE 里的「未接」标记,
    /// 行为与从前一致(rule-engine 的确定性桩、单元测试都走这条)。
    /// 产品侧由 engine 注入真实 okhttp;差分侧由 difftest 注入 HTTP 录放。
    pub net: Option<Box<dyn NetProvider>>,
}

/// 一次求值的资源上限。裁判自己是拿**可弃线程 + 5 秒**卡的
/// (`jsharness/Main.kt`),产品侧从前什么都没有。
#[derive(Debug, Clone, Copy)]
pub struct JsLimits {
    /// 墙钟上限。到点由 QuickJS 的中断钩子抬起,**在字节码层生效** ——
    /// 死循环也拦得住。`None` = 不限(差分侧走这条:裁判那边有自己的闸,
    /// 两侧的超时口径不必也不该由这里对齐)。
    pub timeout: Option<std::time::Duration>,
    /// QuickJS 堆上限(字节)。`None` = 不限。
    pub memory: Option<usize>,
    /// JS 栈上限(字节)。`None` = 用 rquickjs 的默认 256 KiB。
    pub stack: Option<usize>,
}

impl Default for JsLimits {
    fn default() -> JsLimits {
        // 30 秒 / 256 MiB:够任何正经书源(实测一次求值是毫秒级),
        // 又不至于让一段 `while(true)` 把线程占到天荒地老。
        JsLimits {
            timeout: Some(std::time::Duration::from_secs(30)),
            memory: Some(256 * 1024 * 1024),
            stack: None,
        }
    }
}

impl JsLimits {
    /// 全不限。单测用(要观察「没闸会怎样」的那几条)。
    pub fn unlimited() -> JsLimits {
        JsLimits { timeout: None, memory: None, stack: None }
    }

    /// **差分侧的闸**。两侧的超时口径不该由这里对齐 —— 裁判自己是
    /// 可弃线程 + 5 秒(`jsharness/Main.kt`),被测侧同样在外面卡 5 秒。
    /// 但那条线程杀不掉:宿主自己不设闸的话,一段 `while(true)` 会**空转到
    /// 进程退出**,白占一个核。这里给一个明显宽于外层的 60 秒纯粹当泄漏兜底
    /// —— 5 秒都超了的 case 早已判成 `error: "timeout"`,口径不受影响。
    pub fn difftest() -> JsLimits {
        JsLimits { timeout: Some(std::time::Duration::from_secs(60)), memory: None, stack: None }
    }
}

impl QuickJsHost {
    pub fn new() -> QuickJsHost {
        QuickJsHost::default()
    }

    pub fn with_config(config: HostConfig) -> QuickJsHost {
        QuickJsHost { logs: Vec::new(), config, ..Default::default() }
    }

    /// 差分侧的建法:环境常量 + 定死随机源 + **只留泄漏兜底的那道闸**
    /// (见 [`JsLimits::difftest`])
    pub fn for_difftest(config: HostConfig) -> QuickJsHost {
        QuickJsHost { config, limits: JsLimits::difftest(), ..Default::default() }
    }
}

/// 差分用的详细求值结果:除了产品要的 [`JsValue`],还带
/// **原始 JSON**(数组元素的类型在 `JsValue::List` 里被抹成字符串了)
/// 与「完成值是函数」这一位。产品路径不用它。
pub struct EvalDetail {
    pub value: JsValue,
    /// 数组/对象的原始 JSON(JSON.stringify 的口径,对齐裁判的 NativeJSON.stringify)
    pub json: Option<serde_json::Value>,
    pub is_function: bool,
    /// 完成值是**解包后的 Java 对象**(List / 数组,不含 String)时的形态位。
    /// 真身在顶层解包 `Wrapper`,于是裁判侧 `typeOf` 落到 `other`、
    /// `str` 是 Java 的 `toString()`、`norm` 是 GSON 的 pretty JSON。
    /// 见 java_proxy.rs 与 README「Java 对象代理层」。
    pub java_object: Option<JavaObject>,
}

/// 解包后的 Java 对象在差分输出里的形态位
pub struct JavaObject {
    /// 哪一种 Java 对象:`list` / `array` / `elements`
    pub kind: String,
    /// Java 的 `toString()`(List → `[a, b]`;数组 → 身份形态;
    /// Elements → 各 outerHtml 以换行相连)
    pub str: String,
    /// GSON 的序列化结果(pretty,2 空格缩进)。
    /// `None` = GSON 序列化不了(jsoup 的 Elements → `host:JsonIOException`)
    pub json: Option<serde_json::Value>,
}

impl QuickJsHost {
    /// 同 [`HostEnv::eval_js`],但多带差分要看的形态位。
    pub fn eval_js_detailed(
        &mut self,
        code: &str,
        b: &JsBindings,
        env: &mut dyn JsRuleEnv,
    ) -> Result<EvalDetail, String> {
        self.eval_inner(code, b, env)
    }

    fn eval_inner(
        &mut self,
        code: &str,
        b: &JsBindings,
        env: &mut dyn JsRuleEnv,
    ) -> Result<EvalDetail, String> {
        // 重入自检(见 VarPtr 的安全性说明)。走到这里说明有人在求值期间又拿到了
        // 同一个 host 的 `&mut` —— 那会让两条 VarPtr 同时活着,是 UB 的前一步。
        if self.in_eval.get() {
            return Err("eval_js 重入:同一个 QuickJsHost 正在求值中".into());
        }
        // 复位走 Drop:求值路径上任何 panic(rquickjs 会 resume_unwind 穿出来)
        // 若跳过复位,这个 host 此后每次求值都报「重入」,真因被盖住。
        struct InEvalGuard(Rc<std::cell::Cell<bool>>);
        impl Drop for InEvalGuard {
            fn drop(&mut self) {
                self.0.set(false);
            }
        }
        self.in_eval.set(true);
        let _g = InEvalGuard(self.in_eval.clone());
        self.eval_guarded(code, b, env)
    }

    fn eval_guarded(
        &mut self,
        code: &str,
        b: &JsBindings,
        env: &mut dyn JsRuleEnv,
    ) -> Result<EvalDetail, String> {
        let rt = Runtime::new().map_err(|e| format!("runtime: {e}"))?;
        // **执行闸**:书源是第三方代码。中断钩子在字节码层被周期性调用,
        // 所以 `while(true)` 也拦得住(rquickjs 会把它变成一条不可捕获的异常)。
        if let Some(m) = self.limits.memory {
            rt.set_memory_limit(m);
        }
        if let Some(st) = self.limits.stack {
            rt.set_max_stack_size(st);
        }
        {
            let deadline = self.limits.timeout.map(|d| std::time::Instant::now() + d);
            let cancel = self.cancel.clone();
            if deadline.is_some() || cancel.is_some() {
                rt.set_interrupt_handler(Some(Box::new(move || {
                    if let Some(c) = &cancel {
                        if c() {
                            return true;
                        }
                    }
                    deadline.is_some_and(|d| std::time::Instant::now() >= d)
                })));
            }
        }
        let ctx = Context::full(&rt).map_err(|e| format!("context: {e}"))?;
        // SAFETY:见 VarPtr 的安全性说明——ctx 在本函数内 drop,重入由 in_eval 挡
        let vp = unsafe { VarPtr::new(env) };
        // 两张表是共享句柄(Rc),不再是裸指针 —— 见 SharedMap
        let cache_p = self.cache.clone();
        let cache_file_p = self.cache_file.clone();
        let cookie_p = self.cookies.clone();
        let cookie_store = self.cookie_store.clone();
        // SAFETY:同上;实现活在 self 上,闭包不逃出本次求值
        let net_p = self.net.as_mut().map(|n| unsafe { NetP::new(n.as_mut()) });
        let logs = Rc::new(RefCell::new(Vec::<String>::new()));

        // 固定脚本走字节码缓存(见 bytecode.rs):文本不变,解析只付一次
        static JAVA_PROXY_BC: crate::bytecode::Cached = crate::bytecode::Cached::new();
        static PRELUDE_BC: crate::bytecode::Cached = crate::bytecode::Cached::new();
        static BOX_RETURNS_BC: crate::bytecode::Cached = crate::bytecode::Cached::new();

        let out = ctx.with(|ctx| -> Result<EvalDetail, String> {
            // Java 对象代理层要在 install_java 之前装(装箱包装器要用到 __java)
            crate::bytecode::eval_cached(&ctx, &JAVA_PROXY_BC, crate::java_proxy::JAVA_PROXY)
                .map_err(|e| format_js_error(&ctx, e, "java-proxy"))?;
            install_java(
                &ctx,
                vp,
                logs.clone(),
                &self.config,
                b,
                net_p,
                cookie_p.clone(),
                cookie_store.clone(),
                cache_p.clone(),
            )
            .map_err(|e| format!("install java: {e}"))?;
            install_bindings(&ctx, b, vp, cache_p.clone(), &self.config)
                .map_err(|e| format!("install bindings: {e}"))?;
            install_cookie_cache(&ctx, cache_p, cache_file_p, cookie_p, cookie_store)
                .map_err(|e| format!("install cookie/cache: {e}"))?;
            crate::bytecode::eval_cached(&ctx, &PRELUDE_BC, PRELUDE)
                .map_err(|e| format_js_error(&ctx, e, "prelude"))?;
            // LiveConnect 的类面(JavaImporter / Packages / javax.crypto…)。
            // 在 PRELUDE 之后 —— `Packages.org` 要接 install_liveconnect 装的 `org`。
            crate::live_connect::install_live_connect(&ctx)
                .map_err(|e| format!("install live-connect: {e}"))?;
            // 装箱包装器最后装:PRELUDE 里还会重写 java.get / java.ajax 等
            crate::bytecode::eval_cached(
                &ctx,
                &BOX_RETURNS_BC,
                crate::java_proxy::box_returns_js(),
            )
            .map_err(|e| format_js_error(&ctx, e, "java-box"))?;
            if let Some(d) = &self.determinism {
                // 文本不是 'static(逐 host 注入),缓存位按内容取 —— 见 determinism_cell
                crate::bytecode::eval_cached(&ctx, determinism_cell(d), d)
                    .map_err(|e| format_js_error(&ctx, e, "determinism"))?;
            }
            // **非严格模式**:rquickjs 默认 strict=true,而真身 Rhino 跑书源是
            // sloppy mode —— 八进制转义 '\101'、未声明赋值、arguments.callee
            // 这些 Annex B 面书源里真的会用(js-host 差分抓到的)
            let eval_opts = || {
                let mut o = rquickjs::context::EvalOptions::default();
                o.global = true;
                o.strict = false;
                o
            };
            // **完成值:以 `if`/`for`/`while`/`try` 收尾的脚本**。规范说这类语句的
            // 完成值为空时整段留用上一条(`x=1; if(false){2}` → 1,Rhino 与 V8
            // 都如此),而 quickjs-ng 给 undefined —— 被测侧少一块能力,书源会
            // 因此空手而归。切成几段顺序求值、取最后一个非 undefined 的完成值,
            // 见 dialect::split_value_tail。**先把每段编译一遍**(`if(0){…}`:
            // 不执行,但语法错当场报、`var` 提升也照做),有一段编不过就整段
            // 退回原来的单次求值 —— 那时一个字节都还没跑,不会重复副作用。
            let segments = crate::dialect::split_value_tail(code).filter(|segs| {
                segs.iter().all(|s| {
                    ctx.eval_with_options::<(), _>(
                        format!("if(0){{\n{s}\n}}").into_bytes(),
                        eval_opts(),
                    )
                    .inspect_err(|_| {
                        ctx.catch();
                    })
                    .is_ok()
                })
            });
            if let Some(segs) = segments {
                let mut last: Option<Value> = None;
                for seg in &segs {
                    let v: Value = ctx
                        .eval_with_options(seg.as_bytes().to_vec(), eval_opts())
                        .map_err(|e| format_js_error(&ctx, e, "eval"))?;
                    if !v.is_undefined() {
                        last = Some(v);
                    }
                }
                return finish_eval(
                    &ctx,
                    last.unwrap_or_else(|| Value::new_undefined(ctx.clone())),
                );
            }
            let v: Value = match ctx.eval_with_options(code.as_bytes().to_vec(), eval_opts()) {
                Ok(v) => v,
                Err(e) => {
                    // **Rhino 方言的第二次机会**:真身收而规范不收的写法
                    // (没加括号的解构箭头参,见 dialect.rs)。只在已经报了
                    // SyntaxError 之后重写一次再跑;重写还是错就报**原来那条**
                    // —— 垫片不许把真因盖住。
                    let first = format_js_error(&ctx, e, "eval");
                    let retry = if first.contains("SyntaxError") {
                        crate::dialect::parenthesize_arrow_params(code)
                    } else {
                        None
                    };
                    match retry {
                        Some(fixed) => {
                            match ctx.eval_with_options(fixed.as_bytes().to_vec(), eval_opts()) {
                                Ok(v) => v,
                                Err(e2) => {
                                    let _ = format_js_error(&ctx, e2, "eval"); // 清掉挂起的异常
                                    return Err(first);
                                }
                            }
                        }
                        None => return Err(first),
                    }
                }
            };
            finish_eval(&ctx, v)
        });

        self.logs.extend(logs.borrow().iter().cloned());
        out
    }
}

/// determinism 垫片的缓存位:文本随 host 注入、不是 `'static`,而 [`crate::bytecode::Cached`]
/// 设一次就不再看源码 —— 所以按**内容**取缓存位,不同文本各占一个(泄漏的 `Cached`
/// 以进程内不同的 determinism 文本数为上界,实际是 1)。
fn determinism_cell(src: &str) -> &'static crate::bytecode::Cached {
    static CELLS: std::sync::LazyLock<
        std::sync::Mutex<std::collections::HashMap<String, &'static crate::bytecode::Cached>>,
    > = std::sync::LazyLock::new(Default::default);
    let mut m = CELLS.lock().unwrap();
    m.entry(src.to_string()).or_insert_with(|| Box::leak(Box::new(crate::bytecode::Cached::new())))
}

impl HostEnv for QuickJsHost {
    fn eval_js(
        &mut self,
        code: &str,
        b: &JsBindings,
        env: &mut dyn JsRuleEnv,
    ) -> Result<JsValue, String> {
        self.eval_inner(code, b, env).map(|d| d.value)
    }

    /// `@webjs:` —— 交给网络面(书源那半边的 headerMap / tag 在它手上,
    /// webView 的平台原语也是)。没装网络面就是「未接」。
    fn web_js(&mut self, req: &rubato_core::host::WebJsRequest<'_>) -> Result<String, String> {
        let Some(net) = self.net.as_mut() else {
            return Err(rubato_core::host::NET_UNSUPPORTED.to_string());
        };
        net.web_js(req).map_err(|e| match e {
            rubato_core::host::NetError::Request(m) => m,
            other => format!("{other:?}"),
        })
    }

    fn log(&mut self, msg: &str) {
        self.logs.push(msg.to_string());
    }
}

/// Rhino 方言垫片 + 未接能力的显式报错壳
const PRELUDE: &str = r#"
// Rhino 方言的**实测**面(judge/jsharness 探针,见 fixtures/cases/js-host/README.md):
//   typeof JavaImporter === "function";typeof Packages === "object";
//   typeof importClass === "undefined"(真身没有,别自作主张补);
//   java.lang 是 undefined —— `java` 是 AnalyzeRule 实例,不是 Java 包
// JsExtensions.toast/longToast 返回 Unit(且走 toastOnUi,不落 Debug 日志)
java.toast = function (msg) {};
java.longToast = function (msg) {};
// **`[native code]` 的缩进**:Rhino 用一个制表符,quickjs-ng 用四个空格。
// 书源会把函数对象直接拼进字符串(`"主播:"+result.anchor` —— `anchor` 取不到
// 就落到 `String.prototype.anchor` 上,js-corpus-503b71f5bc),于是这点空白
// 直接进了正文。只归一**内建函数**那一行,用户函数的源码形态不动
// (那一档两侧本来就不同,语料里也没有源去看它)。
(function () {
    var ts = Function.prototype.toString;
    Function.prototype.toString = function () {
        return String(ts.call(this)).replace("{\n    [native code]\n}", "{\n\t[native code]\n}");
    };
})();
// 未接的网络能力。真身 ajax/connect **吞异常**:记一条日志 + 返回异常的栈字符串,
// 所以这里也返回标记串而不是抛 —— 两侧要连「吞不吞」都一致
// (标记契约见 fixtures/cases/js-host/README.md)
var NET_UNSUPPORTED = "«net-unsupported»";
// 注意 `get` 不在里面:`java` 是 AnalyzeRule,`java.get(key)` 是**变量读取**,
// 而 JsExtensions 的 `get(url, headers)` 是 jsoup 请求 —— 真身靠 arity 分派,
// 这里也照做(见下面对 java.get 的包装)
// 真身 `ajax` 失败时走 **AppLog.put**(崩溃日志),**不是 Debug.log** ——
// 差分比的 logs 是 Debug 那一条,所以这里不能落日志。
// 书源自己在 catch 里 `java.log(e)` 的才算(裁判侧同样)。
// **只在没接网络时**顶上:接了网络的宿主已经把这些装成了真实现
// (见 install_java 的「网络面」一段),这里不许覆盖回去。
["ajax", "ajaxAll", "connect", "head", "post"].forEach(function (n) {
    if (typeof java[n] !== "function") {
        java[n] = function () { return NET_UNSUPPORTED; };
    }
});
(function () {
    var varGet = java.get;
    var netGet = java.__netGet;   // 接了网络才有
    java.get = function (a, b) {
        if (arguments.length >= 2) {   // JsExtensions.get(urlStr, headers[, timeout])
            return netGet ? netGet(a, b) : NET_UNSUPPORTED;
        }
        return varGet(a);             // AnalyzeRule.get(key)
    };
})();
delete java.__netGet;
// 这些真身不吞,异常直接穿出去
// Book/BookChapter 的 putVariable/getVariable 打的是 java.put/get 的同一层
// (探针 js-ch-put-variable / js-ch-book-put-variable)。
// **返回的是 `true`**:真身 `BaseBook.putVariable` 与 `BookChapter.putVariable`
// 都无条件 `return true`(RuleDataInterface 那个默认实现的返回值被它俩吃掉了),
// 而 `java.put` 返回的是**值本身** —— 转接时别把返回值一起转过来。
//
// `chapter` 这一份此前是**缺的**:章节位的 `chapter.putVariable(...)` 直接
// 「not a function」。语料里就一个源踩得到(就去看网 `ruleToc.chapterUrl`),
// 而 js 套的裁判从不 setChapter,于是两侧都是 null、两侧都抛、差分照过 ——
// 面没接上,判据却是绿的。M3r 把裁判/被测两侧的 chapter 绑定一起补上了。
(function () {
    // **先把函数抓在闭包里**:下面两个内部名马上就 delete 掉(书源不该看见),
    // 转接函数是在**调用时**才跑的,照着 `java.__entityPut` 去找就晚了。
    var ep = java.__entityPut, eg = java.__entityGet;
    if (typeof ep !== "function") { return; }   // url / source 宿主没有这一层
    [["book", typeof book !== "undefined" ? book : null],
     ["chapter", typeof chapter !== "undefined" ? chapter : null]].forEach(function (p) {
        var which = p[0], o = p[1];
        if (o === null || typeof o !== "object") { return; }
        o.putVariable = function (k, v) { ep(which, k, v); return true; };
        o.getVariable = function (k) { return eg(which, k); };
    });
})();
delete java.__entityPut;
delete java.__entityGet;
// 「未接」清单(Phase 3 的手工表)。**装上了就不覆盖**:webView 三兄弟在接了
// 网络的宿主上已经是真实现(BackstageWebView 的策略层,见 install_java 的
// 「webView 三兄弟」一段);余下几个哪个宿主都没装,守卫对它们是空过。
["webView", "webViewGetSource", "webViewGetOverrideUrl", "startBrowserAwait",
 "getVerificationCode", "downloadFile", "importScript", "cacheFile",
 "getZipStringContent", "queryTTF", "queryBase64TTF", "replaceFont"
].forEach(function (n) {
    if (typeof java[n] !== "function") {
        java[n] = function () { throw new Error(NET_UNSUPPORTED); };
    }
});
"#;

/// 绑定面**按宿主分**(见 [`JsHost`]):真身的两个 evalJS 各绑各的,
/// 缺席的名字是**未声明**(ReferenceError)而不是 null —— 这一位是有语义的,
/// 书源里 `typeof src != "undefined"` 之类的探测靠它。
fn install_bindings(
    ctx: &Ctx<'_>,
    b: &JsBindings,
    vp: VarPtr,
    cache: SharedMap,
    cfg: &HostConfig,
) -> rquickjs::Result<()> {
    let g = ctx.globals();
    // **BaseSource.evalJS 的绑定面最窄**(BaseSource.kt L396-403):只有
    // java / source / sourceApi / baseUrl / cookie / cache。`result`、`book`、
    // `chapter`、`page`、`key` 在这里是**未声明**(ReferenceError),不是 null。
    // 而且三个名字指的是**同一个对象** —— `java === source === sourceApi`,
    // 都是书源实体本身。
    if b.host == JsHost::Source {
        let java: Object = g.get("java")?;
        // 把 BookSource 的字段面并到 java 上(真身里 `java` 就是那个实体,
        // `java.bookSourceUrl` 与 `source.bookSourceUrl` 是同一处)
        install_source(ctx, &g, b.source.as_ref(), cache, cfg)?;
        if let Ok(src) = g.get::<_, Object>("source") {
            for kv in src.props::<String, Value>() {
                let (k, v) = kv?;
                // java 自己那一面优先(getKey/getTag 已在 install_base_source 装过)
                if !java.contains_key(&k)? {
                    java.set(k, v)?;
                }
            }
        }
        g.set("baseUrl", b.source.as_ref().map(|s| s.key.clone()).unwrap_or_default())?;
        g.set("source", java.clone())?;
        g.set("sourceApi", java)?;
        if b.source_login {
            // `SourceLoginDialog` 在 BaseSource.evalJS 自带的窄绑定之外，借
            // buildScriptBindings 额外放进来的四位。普通 Source 宿主不走这里，
            // 仍保持 `result` 等名字未声明的差分契约。
            set_bound(ctx, &g, vp, "result", b.result.as_ref())?;
            install_book(ctx, &g, b.book.as_ref())?;
            install_chapter(ctx, &g, b.chapter.as_ref())?;
            g.set("isLongClick", false)?;
        }
        return Ok(());
    }
    // 两个宿主都绑的:result / baseUrl / page
    // 缺席的绑定给 null(真身是 Java null),不是 undefined ——
    // `result == null` 这类判断在书源里很常见
    set_bound(ctx, &g, vp, "result", b.result.as_ref())?;
    set_opt(&g, "baseUrl", b.base_url.as_deref())?;
    match b.page {
        Some(p) => g.set("page", p)?,
        None => g.set("page", Value::new_null(ctx.clone()))?,
    }
    // book / source / chapter:两个宿主都绑。缺席给 null(真身是 Java null,
    // 探针 chapter-null:`chapter === null` 为真,而 `typeof chapter` 是 "object")
    install_book(ctx, &g, b.book.as_ref())?;
    install_source(ctx, &g, b.source.as_ref(), cache, cfg)?;
    install_chapter(ctx, &g, b.chapter.as_ref())?;
    g.set("rssArticle", Value::new_null(ctx.clone()))?;

    match b.host {
        // AnalyzeRule.evalJS(AnalyzeRule.kt L894-911)
        JsHost::Rule => {
            set_bound(ctx, &g, vp, "src", b.src.as_ref())?;
            set_opt(&g, "title", b.title.as_deref())?;
            set_opt(&g, "nextChapterUrl", b.next_chapter_url.as_deref())?;
            g.set("fromBookInfo", b.from_book_info)?;
        }
        // AnalyzeUrl.evalJS(AnalyzeUrl.kt L378-393):多 key/speakText/speakSpeed,
        // 少 src/title/nextChapterUrl/fromBookInfo
        JsHost::Url => {
            set_opt(&g, "key", b.key.as_deref())?;
            g.set("speakText", Value::new_null(ctx.clone()))?;
            g.set("speakSpeed", Value::new_null(ctx.clone()))?;
        }
        // 上面已提前 return
        JsHost::Source => unreachable!("Source 宿主在函数开头就返回了"),
    }
    Ok(())
}

/// `Book` 实体上、书源 JS 会读而 [`BookBinding`](rubato_core::host::BookBinding)
/// 没带的字段。gson 反序列化不到的字段在 Kotlin 侧是 **null**,JS 里读出来是
/// Java null(`String(x)` → `"null"`),不是 undefined。
#[rustfmt::skip] // 手排的分列表:rustfmt 会拆成一行一个
const BOOK_NULL_FIELDS: &[&str] = &[
    "intro", "coverUrl", "customCoverUrl", "customIntro", "charset", "group",
    "latestChapterTitle", "lastCheckTime", "totalChapterNum", "durChapterTitle",
    "durChapterIndex", "durChapterPos", "durChapterTime", "wordCount", "canUpdate",
    "order", "originName", "originOrder", "variable", "readConfig", "type",
];

/// `BookSource` 上同理。
#[rustfmt::skip] // 手排的分列表:rustfmt 会拆成一行一个
const SOURCE_NULL_FIELDS: &[&str] = &[
    "bookSourceComment", "bookSourceGroup", "loginUrl", "loginUi", "loginCheckJs",
    "coverDecodeJs", "bookUrlPattern", "header", "searchUrl", "exploreUrl",
    "ruleSearch", "ruleExplore", "ruleBookInfo", "ruleToc", "ruleContent",
    "ruleReview", "variableComment", "concurrentRate", "jsLib", "lastUpdateTime",
    "respondTime", "weight", "customOrder",
];

/// `java.__entityPut` / `__entityGet` 的第一个参数 → 层
fn var_entity(which: &str) -> rubato_core::host::VarEntity {
    match which {
        "chapter" => rubato_core::host::VarEntity::Chapter,
        _ => rubato_core::host::VarEntity::Book,
    }
}

/// `book` 绑定(`Book` 实体)。字段是 Java String → 走代理层装箱
/// (探针 bind-book-name-typeof:`typeof book.name === "object"`)。
/// `putVariable`/`getVariable` 打到的是 `java.put/get` 的同一层
/// (探针 bind-book-variable),由 PRELUDE 里转接到 `java`。
fn install_book<'js>(
    ctx: &Ctx<'js>,
    g: &Object<'js>,
    book: Option<&rubato_core::host::BookBinding>,
) -> rquickjs::Result<()> {
    let Some(b) = book else {
        return g.set("book", Value::new_null(ctx.clone()));
    };
    let o = Object::new(ctx.clone())?;
    // 字段是 Java String → 走代理层装箱(探针 bind-book-name-typeof:
    // `typeof book.name === "object"`)
    let boxer: rquickjs::Function = ctx.globals().get::<_, Object>("__java")?.get("str")?;
    for (k, v) in [
        ("name", &b.name),
        ("author", &b.author),
        ("bookUrl", &b.book_url),
        ("origin", &b.origin),
        ("tocUrl", &b.toc_url),
    ] {
        let boxed: Value = boxer.call((v.as_str(),))?;
        o.set(k, boxed)?;
    }
    // `kind` 是 `String?`:没填就是 Java null(不是空串)
    match &b.kind {
        Some(v) => {
            let boxed: Value = boxer.call((v.as_str(),))?;
            o.set("kind", boxed)?;
        }
        None => o.set("kind", Value::new_null(ctx.clone()))?,
    }
    // `Book` 实体上其余书源会读的字段:值是 **Java null**(→ `String(x)` 给 "null"),
    // 不是 undefined。语料里 `book.intro` / `book.coverUrl` 这类拼串很常见。
    for k in BOOK_NULL_FIELDS {
        o.set(*k, Value::new_null(ctx.clone()))?;
    }
    // `Book.setReverseToc(b)` / `getReverseToc()`(Book.kt L206):真身写的是
    // `book.config.reverseToc` —— **目录页的 UI 开关,WebBook 四步一个字都不读**
    // (只有 TocActivity 用)。所以这里只保住「调得动、读得回」,不写回实体:
    // 落地要等 `Book.config` 那一层(Phase 3/4 的 UI 面),届时连着 readConfig
    // 的序列化一起做。语料里 3 个源调它,不给方法就是整条 TypeError。
    // 值存在 Rust 侧的 Cell 里:**闭包不许捕获 `Object<'js>`** —— 捕获了它就逃出
    // QuickJS 的 GC 记账,`JS_FreeRuntime` 那里 `gc_obj_list` 非空直接 abort
    // (M2k 踩过一次,规矩写在 pipeline-corpus-b 的 README 里)。
    let flag = std::rc::Rc::new(std::cell::Cell::new(false));
    {
        let f = flag.clone();
        o.set("setReverseToc", Func::from(move |v: bool| f.set(v)))?;
    }
    {
        let f = flag.clone();
        o.set("getReverseToc", Func::from(move || f.get()))?;
    }
    // Rhino 把 Java 的 bean 属性也暴露成属性名(`book.reverseToc` 走
    // `getReverseToc()`),这里用访问器对齐
    o.prop(
        "reverseToc",
        rquickjs::object::Accessor::new(
            {
                let f = flag.clone();
                move || f.get()
            },
            move |v: bool| flag.set(v),
        ),
    )?;
    g.set("book", o)
}

/// `BookChapter` 实体上、书源 JS 会读而 [`ChapterBinding`] 没带的字段:
/// 值是 **Java null**,不是 undefined。
#[rustfmt::skip] // 手排的分列表:rustfmt 会拆成一行一个
const CHAPTER_NULL_FIELDS: &[&str] = &[
    "resourceUrl", "variable", "wordCount", "start", "end",
    "startFragmentId", "endFragmentId",
];

/// `chapter` 绑定(`BookChapter` 实体)。字符串字段与 book 同样走装箱层。
///
/// 缺席时给 **null**(真身 `AnalyzeRule.chapter` 没 setChapter 过就是 null,
/// 探针 chapter-null:`chapter === null` 为真而 `typeof chapter` 是 "object")。
fn install_chapter<'js>(
    ctx: &Ctx<'js>,
    g: &Object<'js>,
    chapter: Option<&rubato_core::host::ChapterBinding>,
) -> rquickjs::Result<()> {
    let Some(c) = chapter else {
        return g.set("chapter", Value::new_null(ctx.clone()));
    };
    let o = Object::new(ctx.clone())?;
    let boxer: rquickjs::Function = ctx.globals().get::<_, Object>("__java")?.get("str")?;
    for (k, v) in
        [("url", &c.url), ("title", &c.title), ("baseUrl", &c.base_url), ("bookUrl", &c.book_url)]
    {
        let boxed: Value = boxer.call((v.as_str(),))?;
        o.set(k, boxed)?;
    }
    match &c.tag {
        Some(t) => {
            let boxed: Value = boxer.call((t.as_str(),))?;
            o.set("tag", boxed)?;
        }
        None => o.set("tag", Value::new_null(ctx.clone()))?,
    }
    o.set("index", c.index)?;
    // **三个布尔位是两个名字**:Kotlin 的 `var isVip: Boolean` 生成的访问器就叫
    // `isVip()`,于是 Rhino 的 JavaBean 内省把**属性**认成 `vip`,而 `chapter.isVip`
    // 拿到的是**方法对象**(`typeof` 是 "function",恒真 —— 书源写
    // `if (chapter.isVip)` 会永远走进去,这是真身的样子,不是我们的)。
    // 探针 js-ch-bool-*。
    for (getter, prop, v) in [
        ("isVolume", "volume", c.is_volume),
        ("isVip", "vip", c.is_vip),
        ("isPay", "pay", c.is_pay),
    ] {
        o.set(prop, v)?;
        o.set(getter, Func::from(move || v))?;
    }
    for k in CHAPTER_NULL_FIELDS {
        o.set(*k, Value::new_null(ctx.clone()))?;
    }
    g.set("chapter", o)
}

/// `source` 绑定(`BookSource`)。除了 `getKey()`/`getTag()` 两个方法,
/// 书源 JS 还会直接读任意字段(`source.bookSourceUrl` / `source.ruleSearch`),
/// 故把整份 JSON 摊平上去。
fn install_source<'js>(
    ctx: &Ctx<'js>,
    g: &Object<'js>,
    src: Option<&rubato_core::host::SourceBinding>,
    cache: SharedMap,
    cfg: &HostConfig,
) -> rquickjs::Result<()> {
    let Some(s) = src else {
        return g.set("source", Value::new_null(ctx.clone()));
    };
    let o = Object::new(ctx.clone())?;
    if let Some(serde_json::Value::Object(map)) = &s.raw {
        for (k, v) in map {
            match v {
                serde_json::Value::String(t) => o.set(k.as_str(), t.as_str())?,
                serde_json::Value::Bool(t) => o.set(k.as_str(), *t)?,
                serde_json::Value::Number(n) => o.set(k.as_str(), n.as_f64().unwrap_or(0.0))?,
                _ => {}
            }
        }
    }
    // BookSource 上没出现在 JSON 里的字段:Java null(gson 不填的字段就是 null),
    // 不是 undefined —— 语料里 `source.bookSourceComment` 这类拼串会看出差别
    for k in SOURCE_NULL_FIELDS {
        if !o.contains_key(*k).unwrap_or(false) {
            o.set(*k, Value::new_null(ctx.clone()))?;
        }
    }
    // `source` 绑定就是 BookSource 实体,**BaseSource 的方法面在三个宿主上都有** ——
    // `source.getVariable()` / `setVariable()` / `getLoginHeader()` 这些在
    // searchUrl 的 `@js:` 里真的会用(差分抓到:少了它们直接 TypeError)。
    // getKey/getTag 也在里面。
    let b = JsBindings { source: Some(s.clone()), ..Default::default() };
    install_base_source(ctx, &o, &b, cache, cfg)?;
    g.set("source", o)
}

fn set_opt(g: &Object<'_>, name: &str, v: Option<&str>) -> rquickjs::Result<()> {
    match v {
        Some(s) => g.set(name, s),
        None => g.set(name, Value::new_null(g.ctx().clone())),
    }
}

/// `result` / `src`:**按真身绑的那个对象的形态**装,不是拍平的串。
///
/// - `Str` —— JS 原生 string(`Context.javaToJS` 对 Kotlin String 的口径,
///   探针实测 `typeof result === "string"`);
/// - `StrList` —— 走代理层的 `javaList`(NativeJavaList:有下标/`size()`/`get()`,
///   没有 `map`/`join`);
/// - `Element` —— 元素代理([`el_proxy`]),`toArray()`/`select()`/`text()`/
///   `parentNode()`/下标都在。
///
/// 缺席给 **null**(真身是 Java null,不是 undefined)。
fn set_bound<'js>(
    ctx: &Ctx<'js>,
    g: &Object<'js>,
    vp: VarPtr,
    name: &str,
    v: Option<&BoundValue>,
) -> rquickjs::Result<()> {
    match v {
        None => g.set(name, Value::new_null(ctx.clone())),
        Some(BoundValue::Str(s)) => g.set(name, s.as_str()),
        Some(BoundValue::StrList(items)) => {
            let f: rquickjs::Function = ctx.globals().get::<_, Object>("__java")?.get("list")?;
            let boxed: Value = f.call((items.clone(),))?;
            g.set(name, boxed)
        }
        Some(BoundValue::Num(d)) => g.set(name, *d),
        Some(BoundValue::Bool(b)) => g.set(name, *b),
        Some(BoundValue::Element(h)) => g.set(name, el_proxy(ctx, vp, *h)?),
        // **Java 对象**(jayway 的 Map/List、gson 的 LinkedTreeMap、List<JXNode>):
        // Rhino 的 NativeJavaMap / NativeJavaList,见 java_value_js
        Some(BoundValue::Java(jv)) => g.set(name, java_value_js(ctx, vp, jv)?),
        // **NativeObject / NativeArray**:`<js>` 自己造的对象 —— 真 JS 值,
        // 不装箱(`String(result)` 是 `[object Object]`、`hasOwnProperty` 在)
        Some(BoundValue::Native { json, .. }) => g.set(name, ctx.json_parse(json.to_string())?),
        // `loginCheckJs` 的 `result`:真身绑的是 **StrResponse 实例**,
        // 而那段 JS 的返回值会被 `as StrResponse` 强转(WebBook.kt L79)。
        // `__strResponse` 是给被测侧认「交回来的还是它」用的 —— JS 里看得见
        // 一个多余的属性,与真身不同,但书源不会去读它(真身那边同名属性
        // 根本不存在,读到的是 undefined,这里读到 true)。
        Some(BoundValue::Response(r)) => {
            let o = str_response_object(ctx, r)?;
            o.set("__strResponse", true)?;
            g.set(name, o)
        }
    }
}

/// 网络调用**失败**时两侧看到的串。
///
/// 真身 `ajax`/`connect` 在这里返回的是**异常的栈字符串**(`it.stackTraceStr`),
/// 两个引擎的栈文本不可能逐字相同 —— 被测侧统一给这个标记。
/// 裁判那边是 `java.lang.IllegalArgumentException: …\n\tat okhttp3.…` —— 全是
/// Java 的类名与行号,被测侧不可能也不该逐字复现,故两侧归一成同一个标记
/// (裁判侧 `jsharness/Main.kt` 的 `JAVA_STACK`)。**比的是「失败了」,不是
/// 「怎么描述失败」**;「是不是在同一处失败」由 `hops` 那一面钉。
const NET_ERROR: &str = "«java-stack»";

/// `JsExtensions.ajax(url)` 的第一参:真身是 `if (url is List) url.firstOrNull().toString()
/// else url.toString()` —— 书源里传数组的写法真的存在。
fn coerce_ajax_url(v: &Value<'_>) -> String {
    if let Some(arr) = v.as_array() {
        return arr
            .iter::<Value>()
            .next()
            .and_then(|r| r.ok())
            .and_then(|first| coerce_opt_string(&first))
            .unwrap_or_else(|| "null".to_string());
    }
    // Rhino 把 JS 的 undefined 包成 `Undefined.instance`,`toString()` 是
    // "undefined";JS 的 null 过去是 Java null,`url.toString()` 给 "null"。
    // 差一个字就会让后面的 URL 拼装两侧走岔(实测 js-corpus-16bb9a0d01)。
    if v.is_undefined() {
        return "undefined".to_string();
    }
    coerce_opt_string(v).unwrap_or_else(|| "null".to_string())
}

/// JS 值 → 字符串。对象走 `JSON.stringify`(真身那边 `parseJsRequestHeaders`
/// 收的是 `NativeObject`,GSON 按同样的形状读);null/undefined 给 `None`。
///
/// **例外是 java_proxy 的箱**:`java.ajax(book.bookUrl)` 里那个实参在真身那边是
/// `NativeJavaObject(java.lang.String)`,交给 String 形参时 Rhino **直接解包**;
/// 照 JSON.stringify 走会给它加一对引号,URL 当场非法(js-corpus-9fe35bde6e:
/// 裁判发得出这一跳、被测侧 `ajax("https://…") error`)。
fn coerce_opt_string(v: &Value<'_>) -> Option<String> {
    if v.is_null() || v.is_undefined() {
        return None;
    }
    if v.is_object() && !v.is_function() && !is_java_box(v) {
        if let Ok(Some(s)) = v.ctx().json_stringify(v.clone()) {
            return s.to_string().ok();
        }
    }
    v.get::<rquickjs::Coerced<String>>().ok().map(|c| c.0)
}

/// `getString(ruleStr, mContent, isUrl)` 的第二个实参 → 「拿哪份内容跑」。
///
/// 真身有两个重载:`(String?, Any?, Boolean)` 与 `(String?, Boolean)`。传**布尔**
/// 时 Rhino 挑后者(Boolean→Boolean 比 Boolean→Any 更具体),那一支是 `unescape`
/// 而不是内容;别的类型一律落到 mContent。缺席 / null / undefined = 用宿主自己的
/// content。
fn m_content(v: &Option<Value<'_>>) -> Option<String> {
    let v = v.as_ref()?;
    if v.is_bool() {
        return None; // `getString(rule, true)` 是 unescape 那一支
    }
    coerce_opt_string(v)
}

/// 是不是 java_proxy 的**箱**(`JavaString` / `javaList` / `javaArray` / 元素代理)。
/// 它们在 JS 里都是对象,而真身那边是 NativeJavaObject / NativeJavaList /
/// NativeJavaArray —— 不是 NativeObject。判据只有这一份,别在别处另抄标记名。
fn is_java_box(v: &Value<'_>) -> bool {
    let Some(o) = v.as_object() else { return false };
    // `__javaKind` = javaList/javaArray/元素代理;`__v` = JavaString 的内部串
    ["__javaKind", "__v"].iter().any(|k| o.get::<_, Value>(*k).is_ok_and(|x| !x.is_undefined()))
}

/// 返回 **java.lang.String** 的那些成员(`StrResponse.body()` / `url()`、
/// jsoup `Response.body()` …)。真身里它们的声明返回类型是 Kotlin `String`,
/// 经 LiveConnect 装箱 —— 于是 `typeof r.body()` 是 **"object"**、
/// `r.body().length` 是**方法对象**而不是数字(探针 r-connect-body / -len)。
/// 给回裸 JS 串会让书源里 `body().length` 这类写法两侧走岔。
fn java_str_fn<'js>(
    ctx: &Ctx<'js>,
    v: Option<String>,
) -> rquickjs::Result<rquickjs::Function<'js>> {
    // **闭包里只留 Rust 数据**:JS 值捕获进闭包会逃出 QuickJS 的 GC 记账
    rquickjs::Function::new(ctx.clone(), move |ctx: Ctx<'js>| -> rquickjs::Result<Value<'js>> {
        let Some(s) = v.clone() else { return Ok(Value::new_null(ctx)) };
        let f: rquickjs::Function = ctx.globals().get::<_, Object>("__java")?.get("str")?;
        f.call((s,))
    })
}

/// `StrResponse` 在 JS 里的成员面。书源看得到的按语料取:`body()` / `code()` /
/// `url()` / `headers()` / `raw()`,外加 `toString()`(真身是 okhttp
/// `Response.toString()`)。`java.connect` 与 `loginCheckJs` 的 `result` 绑定
/// 共用这一份 —— 真身那边它们本来就是同一个类。
fn str_response_object<'js>(ctx: &Ctx<'js>, r: &NetResponse) -> rquickjs::Result<Object<'js>> {
    let o = Object::new(ctx.clone())?;
    o.set("body", java_str_fn(ctx, r.body.clone())?)?;
    let code = r.code;
    o.set("code", Func::from(move || code))?;
    o.set("url", java_str_fn(ctx, Some(r.url.clone()))?)?;
    o.set("headers", headers_fn(ctx, &r.headers)?)?;
    // okhttp `Response.toString()`:`Response{protocol=…, code=…, message=…, url=…}`
    // 回放固定 HTTP/1.1、message 空(见 jsharness/ReplayHttp 的 Response.Builder)
    let s = format!("Response{{protocol=http/1.1, code={}, message=, url={}}}", r.code, r.url);
    o.set("toString", Func::from(move || s.clone()))?;
    // `raw()` —— 真身的 StrResponse 底下就是 okhttp 的 Response,
    // 书源用 `raw().request().url()` 取**最终请求**的地址
    // (跟完重定向;okhttp 那里 request() 是链上最后一次请求)。
    o.set("raw", raw_response_fn(ctx, r)?)?;
    Ok(o)
}

/// `StrResponse.raw()` → okhttp `Response`:语料只用到 `request().url()`、
/// `code()`、`headers()`、`body()`(后者在 okhttp 里是 ResponseBody,取到就
/// `string()`;这里按书源实际写法给串)。
fn raw_response_fn<'js>(
    ctx: &Ctx<'js>,
    r: &NetResponse,
) -> rquickjs::Result<rquickjs::Function<'js>> {
    let url = r.url.clone();
    let code = r.code;
    let headers = r.headers.clone();
    let body = r.body.clone();
    rquickjs::Function::new(ctx.clone(), move |ctx: Ctx<'js>| -> rquickjs::Result<Object<'js>> {
        let raw = Object::new(ctx.clone())?;
        // **闭包里不许留 JS 值**:把 `Object<'js>` 捕获进 Rust 闭包会让它逃出
        // QuickJS 的 GC 记账,`JS_FreeRuntime` 那里 `gc_obj_list` 非空直接 abort。
        // 每次调用现建,捕获的只有 Rust 的 String。
        let u = url.clone();
        raw.set(
            "request",
            Func::from(move |ctx: Ctx<'js>| -> rquickjs::Result<Object<'js>> {
                let req = Object::new(ctx.clone())?;
                let u1 = u.clone();
                req.set("url", Func::from(move || u1.clone()))?;
                let u2 = u.clone();
                req.set(
                    "toString",
                    Func::from(move || format!("Request{{method=GET, url={u2}}}")),
                )?;
                Ok(req)
            }),
        )?;
        raw.set("code", Func::from(move || code))?;
        raw.set("headers", headers_fn(&ctx, &headers)?)?;
        raw.set("body", java_str_fn(&ctx, body.clone())?)?;
        let u3 = url.clone();
        raw.set(
            "toString",
            Func::from(move || {
                format!("Response{{protocol=http/1.1, code={code}, message=, url={u3}}}")
            }),
        )?;
        Ok(raw)
    })
}

/// 网络响应在 JS 里的形态。**不能**让闭包直接返回 `Value<'js>` —— rquickjs 的
/// `Value` 对 `'js` 不变,闭包签名里推不出来;走 `IntoJs` 是这类返回值的正路。
enum NetJs {
    /// `java.connect` → 真身的 `StrResponse`(okhttp 包装)
    Str(NetResponse),
    /// `java.get/head/post` → jsoup 的 `Connection.Response`;
    /// `Err` 直接在 JS 里抛(真身这三个**不吞异常**)
    Jsoup(Result<NetResponse, NetError>),
}

impl<'js> rquickjs::IntoJs<'js> for NetJs {
    fn into_js(self, ctx: &Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        match self {
            // 书源看得到的成员按语料取:`body()` / `code()` / `url()` / `headers()`,
            // 外加 `toString()`(真身是 okhttp `Response.toString()`)
            NetJs::Str(r) => Ok(str_response_object(ctx, &r)?.into_value()),
            // 语料里用到的是 `body()` / `headers()` / `cookies()` / `statusCode()`
            NetJs::Jsoup(Ok(r)) => {
                let o = Object::new(ctx.clone())?;
                o.set("body", java_str_fn(ctx, r.body.clone())?)?;
                let code = r.code;
                o.set("statusCode", Func::from(move || code))?;
                o.set("url", java_str_fn(ctx, Some(r.url.clone()))?)?;
                o.set("headers", headers_fn(ctx, &r.headers)?)?;
                o.set("cookies", headers_fn(ctx, &r.cookies)?)?;
                Ok(o.into_value())
            }
            NetJs::Jsoup(Err(e)) => {
                Err(ctx
                    .throw(rquickjs::String::from_str(ctx.clone(), &e.to_string())?.into_value()))
            }
        }
    }
}

/// `headers()` / `cookies()`:真身返回 Java Map,JS 里 `$.token` 这样取 ——
/// 故这里是一个**调用后给对象**的函数,不是对象本身。
fn headers_fn<'js>(
    ctx: &Ctx<'js>,
    pairs: &[(String, String)],
) -> rquickjs::Result<rquickjs::Function<'js>> {
    // **闭包里不许留 JS 值**:把 `Object<'js>` 捕获进 Rust 闭包会让它逃出 QuickJS
    // 的 GC 记账,`JS_FreeRuntime` 那里 `gc_obj_list` 非空就直接 abort
    // (pb01106:`loginCheckJs` 的 `result` 是**装绑定时就建好**的响应对象,
    //  于是每次求值都留一个)。捕获 Rust 数据、每次调用现建。
    let pairs: Vec<(String, String)> = pairs.to_vec();
    rquickjs::Function::new(ctx.clone(), move |ctx: Ctx<'js>| -> rquickjs::Result<Object<'js>> {
        let map = Object::new(ctx.clone())?;
        for (k, v) in &pairs {
            map.set(k.as_str(), v.as_str())?;
        }
        Ok(map)
    })
}

/// JS 侧的 key / iv:字符串走 UTF-8 取字节(`encodeToByteArray()`),
/// 数组(装箱过的 Java byte[],真身里就是 `ByteArray` 重载)按元素取低 8 位。
fn js_bytes(v: &Value<'_>) -> Vec<u8> {
    if let Some(b) = crate::live_connect::read_byte_seq(v) {
        return b;
    }
    coerce_opt_string(v).unwrap_or_default().into_bytes()
}

/// `createSymmetricCrypto` 的返回:hutool `SymmetricCrypto` 的可观察面。
/// 语料里用到的是 `decryptStr` / `encryptBase64`,其余按真身补齐同名的几个。
///
/// 与 [`NetJs`] 同一套路:闭包返回不了带 `'js` 的类型,走 `IntoJs`。
struct SymCryptoJs(java_api::SymmetricCrypto);

impl<'js> rquickjs::IntoJs<'js> for SymCryptoJs {
    fn into_js(self, ctx: &Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        symmetric_crypto_object(ctx, self.0).map(|o| o.into_value())
    }
}

fn symmetric_crypto_object<'js>(
    ctx: &Ctx<'js>,
    c: java_api::SymmetricCrypto,
) -> rquickjs::Result<Object<'js>> {
    let o = Object::new(ctx.clone())?;
    let c = Rc::new(c);
    {
        let c = c.clone();
        o.set(
            "decryptStr",
            Func::from(
                move |ctx: Ctx<'_>, data: rquickjs::Coerced<String>| -> rquickjs::Result<String> {
                    java_api::sym_decode_input(&data.0)
                        .and_then(|b| c.decrypt(&b))
                        .map(|b| String::from_utf8_lossy(&b).into_owned())
                        .map_err(|e| throw(&ctx, &e))
                },
            ),
        )?;
    }
    {
        let c = c.clone();
        o.set(
            "decrypt",
            Func::from(
                move |ctx: Ctx<'_>, data: rquickjs::Coerced<String>| -> rquickjs::Result<Vec<u8>> {
                    java_api::sym_decode_input(&data.0)
                        .and_then(|b| c.decrypt(&b))
                        .map_err(|e| throw(&ctx, &e))
                },
            ),
        )?;
    }
    {
        let c = c.clone();
        o.set(
            "encryptBase64",
            Func::from(
                move |ctx: Ctx<'_>, data: rquickjs::Coerced<String>| -> rquickjs::Result<String> {
                    c.encrypt(data.0.as_bytes())
                        // SymmetricCryptoAndroid 覆写成 EncoderUtils.base64Encode(默认 NO_WRAP)
                        .map(|b| java_api::base64_encode_bytes(&b, java_api::B64_NO_WRAP))
                        .map_err(|e| throw(&ctx, &e))
                },
            ),
        )?;
    }
    {
        let c = c.clone();
        o.set(
            "encryptHex",
            Func::from(
                move |ctx: Ctx<'_>, data: rquickjs::Coerced<String>| -> rquickjs::Result<String> {
                    c.encrypt(data.0.as_bytes()).map(hex::encode).map_err(|e| throw(&ctx, &e))
                },
            ),
        )?;
    }
    o.set(
        "encrypt",
        Func::from(
            move |ctx: Ctx<'_>, data: rquickjs::Coerced<String>| -> rquickjs::Result<Vec<u8>> {
                c.encrypt(data.0.as_bytes()).map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;
    Ok(o)
}

fn install_java<'js>(
    ctx: &Ctx<'js>,
    vp: VarPtr,
    logs: Rc<RefCell<Vec<String>>>,
    cfg: &HostConfig,
    b: &JsBindings,
    net: Option<NetP>,
    cookies: SharedMap,
    cookie_store: Option<Rc<RefCell<dyn rubato_core::host::CookieEnv>>>,
    cache: SharedMap,
) -> rquickjs::Result<()> {
    let host = b.host;
    let java = Object::new(ctx.clone())?;

    // `java.put/get` 打哪一层**看宿主**:
    // - Rule/Url → AnalyzeRule/AnalyzeUrl 的变量层(chapter → book → ruleData → source);
    // - Source   → **CacheManager**(`v_<sourceKey>_<key>`,BaseSource.kt L345/L354)。
    //   同名不同层,写岔了会让 header/loginUrl 那条路的变量互相看不见。
    if host == JsHost::Source {
        let key = b.source.as_ref().map(|s| s.key.clone()).unwrap_or_default();
        let (k1, k2) = (key.clone(), key.clone());
        let (c1, c2) = (cache.clone(), cache.clone());
        java.set(
            "put",
            Func::from(
                move |k: rquickjs::Coerced<String>, v: rquickjs::Coerced<String>| -> String {
                    c1.insert(format!("v_{k1}_{}", k.0), v.0.clone());
                    v.0
                },
            ),
        )?;
        java.set(
            "get",
            Func::from(move |k: rquickjs::Coerced<String>| -> String {
                c2.get_str(&format!("v_{k2}_{}", k.0)).unwrap_or_default()
            }),
        )?;
    } else {
        java.set(
            "put",
            Func::from(
                move |k: rquickjs::Coerced<String>, v: rquickjs::Coerced<String>| -> String {
                    vp.get().put(&k.0, &v.0)
                },
            ),
        )?;
        java.set(
            "get",
            Func::from(move |k: rquickjs::Coerced<String>| -> String { vp.get().get(&k.0) }),
        )?;
        // `book.putVariable` / `chapter.putVariable` 的落点:**实体自己那一层**,
        // 不是 `java.put` 的优先级链(章节在场时两者不是一回事)。PRELUDE 把
        // 这两个内部名转接到 book / chapter 上,随后就地 delete。
        java.set(
            "__entityPut",
            Func::from(
                move |which: rquickjs::Coerced<String>,
                      k: rquickjs::Coerced<String>,
                      v: rquickjs::Coerced<String>| {
                    vp.get().put_entity(var_entity(&which.0), &k.0, &v.0);
                },
            ),
        )?;
        java.set(
            "__entityGet",
            Func::from(
                move |which: rquickjs::Coerced<String>, k: rquickjs::Coerced<String>| -> String {
                    vp.get().get_entity(var_entity(&which.0), &k.0)
                },
            ),
        )?;
    }
    {
        let l = logs.clone();
        java.set(
            "log",
            Func::from(move |msg: rquickjs::Coerced<String>| -> String {
                l.borrow_mut().push(msg.0.clone());
                msg.0
            }),
        )?;
    }

    // ---- 网络面(接了才装;没接就落到 PRELUDE 的「未接」标记)----
    //
    // 真身 `ajax`/`connect` **吞异常**:失败时记一条 AppLog(**不是** Debug 日志)
    // 再返回异常的栈字符串。两个引擎的栈文本不可能逐字相同,故这里返回同一个
    // 标记串 —— 契约见 fixtures/cases/js-host/README.md「不可差分面」。
    if let Some(np) = net {
        // **两个宿主的 ajax 不是同一个方法**:`AnalyzeRule` 覆写了它
        // (AnalyzeRule.kt L951)—— 带 ruleData,且失败**记 Debug 日志**;
        // `AnalyzeUrl` 那边用的是 `JsExtensions.ajax`,失败走 AppLog(不可见)。
        // 这一位是差分抓出来的,别按「同一个 ajax」实现。
        let from_rule = host == JsHost::Rule;
        {
            let l = logs.clone();
            java.set(
                "ajax",
                Func::from(
                    move |ctx: Ctx<'_>,
                          url: Value<'_>,
                          timeout: rquickjs::function::Opt<i64>|
                          -> rquickjs::Result<String> {
                        // 真身:`url is List` 取首项,否则 `url.toString()`
                        let u = coerce_ajax_url(&url);
                        let r = np.get().ajax(&u, timeout.0, from_rule);
                        // 嵌套 AnalyzeUrl 自己落的 Debug 日志(典型:option JSON
                        // 只有宽松档解得开)。真身落全局 Debug,外层看得见
                        l.borrow_mut().extend(np.get().take_logs());
                        match r {
                            Ok(body) => Ok(body),
                            // 构造 AnalyzeUrl 就抛了 —— 那一步在 runCatching **之外**,
                            // 异常穿到 JS,不吞(见 NetError 的说明)
                            Err(NetError::Construct(tag)) => Err(ctx.throw(
                                rquickjs::String::from_str(ctx.clone(), &tag)?.into_value(),
                            )),
                            Err(NetError::Request(msg)) => {
                                // 「还没接的能力」(webView 等)与「请求失败」在真身里
                                // 是同一条路(都被吞成栈字符串),但两侧要收敛到**不同**
                                // 的标记:未接的那条比的是「两边都到不了这里」。
                                let tag = if msg.contains(NET_UNSUPPORTED) {
                                    NET_UNSUPPORTED
                                } else {
                                    NET_ERROR
                                };
                                if from_rule {
                                    // AnalyzeRule.kt L967:`log("ajax($urlStr) error\n" + 栈)`
                                    l.borrow_mut().push(format!("ajax({u}) error\n{tag}"));
                                }
                                Ok(tag.to_string())
                            }
                        }
                    },
                ),
            )?;
        }
        // `java.ajaxAll(urlList[, skipRateLimit])`(JsExtensions.kt L154)。
        // **不吞异常**(那边没有 runCatching)、**不带 ruleData**,返回的是
        // `StrResponse` 的 **Java 数组** —— 书源写 `res[i][j].body()`,
        // 所以既要下标也要 `.body()`。装箱走 `__java.objArray`(与 bytes 同一支)。
        java.set(
            "ajaxAll",
            Func::from(
                move |ctx: Ctx<'js>,
                      urls: Value<'js>,
                      _skip: rquickjs::function::Opt<Value<'js>>|
                      -> rquickjs::Result<Value<'js>> {
                    // 真身的形参是 `Array<String>`:Rhino 把 JS 数组逐项转成
                    // java.lang.String,非串元素走 toString
                    let mut list: Vec<String> = Vec::new();
                    if let Some(a) = urls.as_object().and_then(|o| o.as_array()) {
                        for v in a.iter::<Value<'js>>() {
                            list.push(coerce_ajax_url(&v?));
                        }
                    }
                    let r = np.get().ajax_all(&list);
                    match r {
                        Ok(rs) => {
                            let arr = rquickjs::Array::new(ctx.clone())?;
                            for (i, resp) in rs.iter().enumerate() {
                                arr.set(i, str_response_object(&ctx, resp)?)?;
                            }
                            let f: rquickjs::Function =
                                ctx.globals().get::<_, Object>("__java")?.get("objArray")?;
                            f.call((arr, "[Lio.legado.app.help.http.StrResponse;"))
                        }
                        Err(NetError::Construct(tag)) | Err(NetError::Request(tag)) => {
                            Err(ctx
                                .throw(rquickjs::String::from_str(ctx.clone(), &tag)?.into_value()))
                        }
                    }
                },
            ),
        )?;
        let lc = logs.clone();
        java.set(
            "connect",
            Func::from(
                move |url: rquickjs::Coerced<String>,
                      header: rquickjs::function::Opt<Value<'_>>,
                      timeout: rquickjs::function::Opt<i64>|
                      -> NetJs {
                    let h = header.0.as_ref().and_then(coerce_opt_string);
                    let r = np.get().connect(&url.0, h.as_deref(), timeout.0);
                    lc.borrow_mut().extend(np.get().take_logs());
                    match r {
                        Ok(r) => NetJs::Str(r),
                        // 真身失败时给的是 `StrResponse(analyzeUrl.url, 栈字符串)`,
                        // 也就是**仍然有对象**,body 是那段栈 —— 形态要一样
                        Err(_) => NetJs::Str(NetResponse {
                            url: url.0.clone(),
                            body: Some(NET_ERROR.to_string()),
                            ..Default::default()
                        }),
                    }
                },
            ),
        )?;
        for (name, method) in [("head", "HEAD"), ("post", "POST")] {
            java.set(
                name,
                Func::from(move |args: rquickjs::function::Rest<Value<'_>>| -> NetJs {
                    // head(url, headers[, timeout]) / post(url, body, headers[, timeout])
                    let url = args.0.first().and_then(coerce_opt_string).unwrap_or_default();
                    let (body, headers) = if method == "POST" {
                        (args.0.get(1).and_then(coerce_opt_string), args.0.get(2))
                    } else {
                        (None, args.0.get(1))
                    };
                    let h = headers.and_then(coerce_opt_string);
                    NetJs::Jsoup(np.get().jsoup(method, &url, h.as_deref(), body.as_deref()))
                }),
            )?;
        }
        // ---- webView 三兄弟(JsExtensions.kt L245-328)----
        //
        // 三个入口只差**哪一个正则非空**:`webView` 两个都不给,
        // `webViewGetSource` 给 sourceRegex,`webViewGetOverrideUrl` 给
        // overrideUrlRegex。实参按真身的重载铺开(cacheFirst / delayTime 可省)。
        // 返回 `getStrResponse().body`(可空);失败**不吞** —— 真身那三处
        // 都没有 runCatching,异常直接穿到 JS。
        for (name, kind) in
            [("webView", 0u8), ("webViewGetSource", 1), ("webViewGetOverrideUrl", 2)]
        {
            java.set(
                name,
                Func::from(
                    move |ctx: Ctx<'_>,
                          args: rquickjs::function::Rest<Value<'_>>|
                          -> rquickjs::Result<Option<String>> {
                        let arg = |i: usize| args.0.get(i).and_then(coerce_opt_string);
                        // webView(html, url, js[, cacheFirst])
                        // webViewGet*(html, url, js, regex[, cacheFirst[, delayTime]])
                        let (regex, rest) = if kind == 0 { (None, 3) } else { (arg(3), 4) };
                        let html = arg(0);
                        let url = arg(1);
                        let js = arg(2);
                        let cache_first =
                            args.0.get(rest).and_then(|v| v.as_bool()).unwrap_or(false);
                        let delay_time =
                            args.0.get(rest + 1).and_then(|v| v.as_number()).unwrap_or(0.0) as i64;
                        let req = rubato_core::host::WebViewReq {
                            html: html.as_deref(),
                            url: url.as_deref(),
                            js: js.as_deref(),
                            source_regex: if kind == 1 { regex.as_deref() } else { None },
                            override_url_regex: if kind == 2 { regex.as_deref() } else { None },
                            cache_first,
                            delay_time,
                        };
                        match np.get().web_view(&req) {
                            Ok(body) => Ok(body),
                            Err(NetError::Construct(tag)) | Err(NetError::Request(tag)) => Err(ctx
                                .throw(
                                    rquickjs::String::from_str(ctx.clone(), &tag)?.into_value(),
                                )),
                        }
                    },
                ),
            )?;
        }
        // ---- 「请用户出手」那三件(JsExtensions.kt L352-400)----
        //
        // 底下都是 `SourceVerificationHelp`(移植在 `net::verification`):
        // 弹界面 → 等用户 → 回结果。三个入口的差别只在
        // 「等不等」与「弹哪一种界面」。失败**不吞**(真身那三处都没有
        // runCatching),异常直接穿到 JS。
        java.set(
            "startBrowser",
            Func::from(
                move |ctx: Ctx<'_>,
                      args: rquickjs::function::Rest<Value<'_>>|
                      -> rquickjs::Result<()> {
                    let arg = |i: usize| args.0.get(i).and_then(coerce_opt_string);
                    let url = arg(0).unwrap_or_default();
                    let title = arg(1).unwrap_or_default();
                    // startBrowser(url, title[, html])
                    match np.get().start_browser(&url, &title, arg(2).as_deref()) {
                        Ok(()) => Ok(()),
                        Err(NetError::Construct(tag)) | Err(NetError::Request(tag)) => {
                            Err(ctx
                                .throw(rquickjs::String::from_str(ctx.clone(), &tag)?.into_value()))
                        }
                    }
                },
            ),
        )?;
        java.set(
            "startBrowserAwait",
            Func::from(move |args: rquickjs::function::Rest<Value<'_>>| -> NetJs {
                let arg = |i: usize| args.0.get(i).and_then(coerce_opt_string);
                let url = arg(0).unwrap_or_default();
                let title = arg(1).unwrap_or_default();
                // startBrowserAwait(url, title[, refetchAfterSuccess[, html]])
                let refetch = args.0.get(2).and_then(|v| v.as_bool()).unwrap_or(true);
                // 与 `connect` 同一种交回形态(真身也是 StrResponse)——
                // 但**不吞异常**,所以走 Jsoup 那一支的错误通道
                NetJs::Jsoup(np.get().start_browser_await(&url, &title, refetch, arg(3).as_deref()))
            }),
        )?;
        java.set(
            "getVerificationCode",
            Func::from(
                move |ctx: Ctx<'_>,
                      image_url: rquickjs::Coerced<String>|
                      -> rquickjs::Result<String> {
                    match np.get().get_verification_code(&image_url.0) {
                        Ok(code) => Ok(code),
                        Err(NetError::Construct(tag)) | Err(NetError::Request(tag)) => {
                            Err(ctx
                                .throw(rquickjs::String::from_str(ctx.clone(), &tag)?.into_value()))
                        }
                    }
                },
            ),
        )?;
        // `java.get(url, headers)` 是 jsoup 请求,`java.get(key)` 是**变量读取** ——
        // 真身靠 Java 重载的 arity 分派。PRELUDE 里那层包装照做,这里只提供网络那一支。
        java.set(
            "__netGet",
            Func::from(
                move |url: rquickjs::Coerced<String>,
                      headers: rquickjs::function::Opt<Value<'_>>|
                      -> NetJs {
                    let h = headers.0.as_ref().and_then(coerce_opt_string);
                    NetJs::Jsoup(np.get().jsoup("GET", &url.0, h.as_deref(), None))
                },
            ),
        )?;
    }

    // ---- 摘要 / 编码(纯函数,契约见 java_api.rs)----
    java.set("md5Encode", Func::from(|s: rquickjs::Coerced<String>| md5_hex(&s.0)))?;
    java.set(
        "md5Encode16",
        Func::from(|s: rquickjs::Coerced<String>| md5_hex(&s.0)[8..24].to_string()),
    )?;
    // base64Encode(str[, flags]):真身默认 flags = NO_WRAP(2)
    java.set(
        "base64Encode",
        Func::from(|s: rquickjs::Coerced<String>, flags: rquickjs::function::Opt<i32>| {
            java_api::base64_encode_bytes(s.0.as_bytes(), flags.0.unwrap_or(java_api::B64_NO_WRAP))
        }),
    )?;
    // base64Decode(str) / (str, charset) / (str, flags) —— Rhino 按实参类型挑重载,
    // JS 侧没有类型,故按「第二个实参是不是数字」手工分发
    java.set("base64Decode", Func::from(base64_decode_js))?;
    java.set(
        "hexEncodeToString",
        Func::from(|s: rquickjs::Coerced<String>| java_api::hex_encode(&s.0)),
    )?;
    java.set(
        "hexDecodeToString",
        Func::from(|ctx: Ctx<'_>, s: rquickjs::Coerced<String>| -> rquickjs::Result<String> {
            java_api::hex_decode_to_string(&s.0).map_err(|e| throw(&ctx, &e))
        }),
    )?;
    // ---- 摘要 / HMAC(hutool 的 DigestUtil / HMac)----
    java.set(
        "digestHex",
        Func::from(
            |ctx: Ctx<'_>,
             data: rquickjs::Coerced<String>,
             algorithm: rquickjs::Coerced<String>|
             -> rquickjs::Result<String> {
                java_api::digest_hex(&data.0, &algorithm.0).map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;
    java.set(
        "HMacHex",
        Func::from(
            |ctx: Ctx<'_>,
             data: rquickjs::Coerced<String>,
             algorithm: rquickjs::Coerced<String>,
             key: rquickjs::Coerced<String>|
             -> rquickjs::Result<String> {
                java_api::hmac_hex(&data.0, &algorithm.0, &key.0).map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;

    java.set(
        "HMacBase64",
        Func::from(
            |ctx: Ctx<'_>,
             data: rquickjs::Coerced<String>,
             algorithm: rquickjs::Coerced<String>,
             key: rquickjs::Coerced<String>|
             -> rquickjs::Result<String> {
                // JsEncodeUtils.kt L506:`Base64.encodeToString(HMac(...).digest(data), NO_WRAP)`
                java_api::hmac_bytes(
                    &algorithm.0,
                    key.0.as_bytes(),
                    data.0.as_bytes(),
                    java_api::CryptoFlavor::Hutool,
                )
                .map(|b| java_api::base64_encode_bytes(&b, 2))
                .map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;

    // ---- 对称加解密(hutool SymmetricCrypto;契约见 java_api.rs 那一节)----
    // 书源里 aes*/des*/createSymmetricCrypto 全汇到同一处,这里也只有一份核心。
    // key/iv 的**取字节方式**按重载分:传串走 `encodeToByteArray()`(UTF-8),
    // 传字节数组就原样(JsEncodeUtils.kt L45-76)。
    for (name, kind) in [
        // `aesDecodeToString` 名字里说的是**原始数据**,但真身同样走 `decryptStr`:
        // hutool 的 `decrypt(String)` 一律先过 `SecureUtil.decode`,行为与下面同臂
        ("aesDecodeToString", SymOp::DecryptStr),
        ("aesBase64DecodeToString", SymOp::DecryptStr),
        ("desDecodeToString", SymOp::DecryptStr),
        ("desBase64DecodeToString", SymOp::DecryptStr),
        ("aesEncodeToString", SymOp::EncryptStr),
        ("desEncodeToString", SymOp::EncryptStr),
        ("aesEncodeToBase64String", SymOp::EncryptBase64),
        ("desEncodeToBase64String", SymOp::EncryptBase64),
    ] {
        java.set(
            name,
            Func::from(
                move |ctx: Ctx<'_>,
                      data: rquickjs::Coerced<String>,
                      key: rquickjs::Coerced<String>,
                      transformation: rquickjs::Coerced<String>,
                      iv: rquickjs::function::Opt<rquickjs::Coerced<String>>|
                      -> rquickjs::Result<Option<String>> {
                    let iv = iv.0.map(|c| c.0).unwrap_or_default();
                    java_api::symmetric_str_op(
                        kind,
                        &data.0,
                        key.0.as_bytes(),
                        &transformation.0,
                        iv.as_bytes(),
                    )
                    .map_err(|e| throw(&ctx, &e))
                },
            ),
        )?;
    }
    // createSymmetricCrypto(transformation, key[, iv]):返回一个**对象**,
    // 书源在它上面接着调 decryptStr / encryptBase64 / encrypt / decrypt。
    java.set(
        "createSymmetricCrypto",
        Func::from(
            |ctx: Ctx<'_>,
             transformation: rquickjs::Coerced<String>,
             key: Value<'_>,
             iv: rquickjs::function::Opt<Value<'_>>|
             -> rquickjs::Result<SymCryptoJs> {
                let key = js_bytes(&key);
                let iv = iv.0.as_ref().map(js_bytes).unwrap_or_default();
                java_api::SymmetricCrypto::new(
                    &transformation.0,
                    &key,
                    if iv.is_empty() { None } else { Some(&iv) },
                )
                .map(SymCryptoJs)
                .map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;

    // ---- 字节数组面 ----
    // 返回值装箱成 Java 数组(RETURN_KINDS 里 kind = "bytes"):真身的
    // NativeJavaArray 挂 Array.prototype,只有 toString 是 `[B@hash` 的身份形态。
    java.set(
        "strToBytes",
        Func::from(
            |ctx: Ctx<'_>,
             s: rquickjs::Coerced<String>,
             charset: rquickjs::function::Opt<rquickjs::Coerced<String>>|
             -> rquickjs::Result<Vec<u8>> {
                java_api::encode_with_charset(&s.0, charset.0.as_ref().map(|c| c.0.as_str()))
                    .map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;
    java.set(
        "bytesToStr",
        Func::from(
            // 收的是**装箱过的 Java 数组**(类数组,不是真 JS 数组)——
            // 走 js_bytes 而不是 rquickjs 的 Vec<u8> 转换
            |ctx: Ctx<'_>,
             bytes: Value<'_>,
             charset: rquickjs::function::Opt<rquickjs::Coerced<String>>|
             -> rquickjs::Result<String> {
                java_api::decode_with_charset(
                    &js_bytes(&bytes),
                    charset.0.as_ref().map(|c| c.0.as_str()),
                )
                .map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;
    // base64DecodeToByteArray(str[, flags]):真身对空白串给 **null**(不是空数组)
    java.set(
        "base64DecodeToByteArray",
        Func::from(
            |ctx: Ctx<'_>,
             s: rquickjs::function::Opt<rquickjs::Coerced<String>>,
             _flags: rquickjs::function::Opt<i32>|
             -> rquickjs::Result<Option<Vec<u8>>> {
                let Some(s) = s.0 else { return Ok(None) };
                if s.0.trim().is_empty() {
                    return Ok(None);
                }
                java_api::base64_decode_bytes(&s.0).map(Some).map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;

    // ---- cookie(JsExtensions.getCookie,与 `cookie` 绑定同一张表)----
    // 真身:`getCookie(tag)` → CookieStore.getCookie(tag);
    //       `getCookie(tag, key)` → CookieStore.getKey(tag, key)。
    // key 是 `NetworkUtils.getSubDomain(url)` —— 二级域,不是整个 host。
    let ck = cookies.clone();
    let st = cookie_store.clone();
    java.set(
        "getCookie",
        Func::from(
            move |tag: rquickjs::Coerced<String>,
                  key: rquickjs::function::Opt<Value<'_>>|
                  -> String {
                let k = key.0.as_ref().and_then(coerce_opt_string);
                if let Some(s) = &st {
                    return match k {
                        Some(k) => s.borrow_mut().get_cookie_key(&tag.0, &k),
                        None => s.borrow_mut().get_cookie(&tag.0),
                    };
                }
                let all = ck.get_str(&cookie_domain(&tag.0)).unwrap_or_default();
                match k {
                    Some(k) => cookie_to_map(&all).get(&k).unwrap_or_default().to_string(),
                    None => all,
                }
            },
        ),
    )?;

    // ---- 随机 ----
    // 来源由 `HostConfig::random` 定:产品侧是系统熵源,差分侧是与裁判
    // `jsharness/DeterministicRandom` 同一条字节流定义、计数器每 case 从 0 起
    // (裁判侧 Main 每 case `reset()`)。每次 `UUID.randomUUID()` 恰好吃掉一个 block。
    {
        let uuid_counter = Rc::new(RefCell::new(0u64));
        let random = cfg.random;
        java.set(
            "randomUUID",
            Func::from(move |ctx: Ctx<'_>| -> rquickjs::Result<String> {
                let mut c = uuid_counter.borrow_mut();
                let block = random.bytes_at(*c).map_err(|e| throw(&ctx, &e))?;
                *c += 1;
                let mut b = [0u8; 16];
                b.copy_from_slice(&block[..16]);
                Ok(java_api::uuid_from_bytes(&b))
            }),
        )?;
    }

    // ---- 章节号 ----
    java.set(
        "toNumChapter",
        Func::from(|s: rquickjs::function::Opt<rquickjs::Coerced<String>>| -> Option<String> {
            // 真身第一行就是 `s ?: return null`
            s.0.map(|s| java_api::to_num_chapter(&s.0))
        }),
    )?;

    java.set(
        "encodeURI",
        Func::from(
            |s: rquickjs::Coerced<String>,
             enc: rquickjs::function::Opt<rquickjs::Coerced<String>>| {
                java_api::encode_uri(&s.0, enc.0.as_ref().map(|c| c.0.as_str()))
            },
        ),
    )?;

    // ---- 时间 ----
    java.set(
        "timeFormat",
        Func::from(|t: rquickjs::Coerced<f64>| java_api::time_format(t.0 as i64)),
    )?;
    java.set(
        "timeFormatUTC",
        Func::from(
            |t: rquickjs::Coerced<f64>,
             f: rquickjs::Coerced<String>,
             sh: rquickjs::Coerced<f64>| {
                java_api::time_format_utc(t.0 as i64, &f.0, sh.0 as i32)
            },
        ),
    )?;

    // ---- 环境常量(差分侧由 HostConfig 注入裁判垫片的固定值)----
    {
        let id = cfg.android_id.clone();
        java.set("androidId", Func::from(move || id.clone()))?;
    }
    {
        let ua = cfg.web_view_ua.clone();
        java.set("getWebViewUA", Func::from(move || ua.clone()))?;
    }

    // ---- 简繁转换:plan §6 砍单,契约「转换关」,恒等映射(裁判垫片同口径)----
    java.set("t2s", Func::from(|s: rquickjs::Coerced<String>| s.0))?;
    java.set("s2t", Func::from(|s: rquickjs::Coerced<String>| s.0))?;

    // ---- `java.htmlFormat(str[, redirectUrl])`(JsExtensions.kt L721/L726)----
    // 本体是 `HtmlFormatter.formatKeepImg`,与四步流水线的正文那条路**同一份**
    // (html-format crate)。第二个实参是 `URL(redirectUrl)`,**解不出来给 null**
    // (真身 runCatching().getOrNull()),而不是报错。
    java.set(
        "htmlFormat",
        Func::from(
            move |s: rquickjs::Coerced<String>,
                  redirect: rquickjs::function::Opt<rquickjs::Coerced<String>>|
                  -> String {
                let url = redirect
                    .0
                    .as_ref()
                    .and_then(|u| rubato_core::java_url::JavaUrl::parse(&u.0).ok());
                html_format::format_keep_img(Some(&s.0), url.as_ref())
            },
        ),
    )?;

    // ---- 规则反调面(**只有 AnalyzeRule 宿主有**)----
    // 真身的 `java` 就是 AnalyzeRule 本身,JS 里能反过来跑规则;
    // AnalyzeUrl 宿主没有这些方法,调到就是 TypeError(见 JsHost)。
    if host == JsHost::Rule {
        install_rule_callbacks(&java, vp)?;
    }
    // BaseSource 自己那一面(登录信息 / 源变量 / 登录头),只有 Source 宿主有
    if host == JsHost::Source {
        install_base_source(ctx, &java, b, cache, cfg)?;
    }
    // LiveConnect 是 Rhino 的全局面,三个宿主都有
    install_liveconnect(ctx, vp)?;

    ctx.globals().set("java", java)?;
    Ok(())
}

/// `BaseSource` 在 JS 里露出的那一面(BaseSource.kt)。`java` 绑定在这个宿主上
/// **就是书源实体**,所以 `java.getLoginInfo()` / `java.getVariable()` 这些
/// 都在同一个对象上;`source` 与 `sourceApi` 指的也是它(见 install_bindings)。
///
/// 存储都落 **CacheManager**(差分侧就是 `QuickJsHost.cache` 那张表,裁判侧是
/// CacheManager 垫片),键名逐字对齐真身:
/// `v_<key>_<k>` / `sourceVariable_<key>` / `loginHeader_<key>` / `userInfo_<key>`。
fn install_base_source<'js>(
    _ctx: &Ctx<'js>,
    java: &Object<'js>,
    b: &JsBindings,
    cache: SharedMap,
    cfg: &HostConfig,
) -> rquickjs::Result<()> {
    let key = b.source.as_ref().map(|s| s.key.clone()).unwrap_or_default();
    let tag = b.source.as_ref().map(|s| s.tag.clone()).unwrap_or_default();
    {
        let k = key.clone();
        java.set("getKey", Func::from(move || k.clone()))?;
    }
    java.set("getTag", Func::from(move || tag.clone()))?;
    // `getVariable()`:取不到给**空串**(BaseSource.kt L338,与 getLoginHeader 不同)
    {
        let (k, cache) = (key.clone(), cache.clone());
        java.set(
            "getVariable",
            Func::from(move || -> String {
                cache.get_str(&format!("sourceVariable_{k}")).unwrap_or_default()
            }),
        )?;
    }
    // `putVariable(v)` / `setVariable(v)`:真身两个方法同体,签名是
    // `(variable: String?)`(BaseSource.kt L312 / L325),**只有 null 是删除**。
    // 非 null 的实参由 Rhino 按 Java String 形参转换,也就是 JS 的 ToString:
    // `0`→"0"、`1.5`→"1.5"、`true`→"true"、`[1,2]`→"1,2"、
    // `{}`→"[object Object]"、**`undefined`→"undefined"**(Rhino 的怪癖)。
    // 逐条由探针 `js-src-setvar-*` / `js-src-putvar-number` 钉住。
    //
    // 此前这里只认 JS 字符串,别的一律走 None 分支**把变量删掉** —— 与真身
    // 正好相反。`source.setVariable(0)` 是书源里的常见写法(top100 名单里的
    // 「番茄小说2」就这么写),于是产品上那个源的变量永远存不进去。
    for name in ["putVariable", "setVariable"] {
        let (k, cache) = (key.clone(), cache.clone());
        java.set(
            name,
            Func::from(move |ctx: Ctx<'js>, v: Value<'js>| -> rquickjs::Result<()> {
                let key = format!("sourceVariable_{k}");
                if v.is_null() {
                    cache.remove(&key);
                } else {
                    cache.insert(key, rquickjs::Coerced::<String>::from_js(&ctx, v)?.0);
                }
                Ok(())
            }),
        )?;
    }
    // `getLoginHeader()`:CacheManager.get 直出,取不到是 **Java null**(不是空串)
    {
        let (k, cache) = (key.clone(), cache.clone());
        java.set(
            "getLoginHeader",
            Func::from(move |ctx: Ctx<'js>| -> Value<'js> {
                match cache.get_str(&format!("loginHeader_{k}")) {
                    Some(v) => rquickjs::String::from_str(ctx.clone(), &v)
                        .map(Value::from_string)
                        .unwrap_or(Value::new_null(ctx.clone())),
                    None => Value::new_null(ctx),
                }
            }),
        )?;
    }
    // `putLoginHeader(header)`:真身还会把 Cookie 项塞进 CookieStore ——
    // 那一步要 cookie 表,本套的登录头用例不走它,故只落缓存(README 记着)
    {
        let (k, cache) = (key.clone(), cache.clone());
        java.set(
            "putLoginHeader",
            Func::from(move |h: rquickjs::Coerced<String>| {
                cache.insert(format!("loginHeader_{k}"), h.0);
            }),
        )?;
    }
    // 登录信息:AES(androidId 前 16 字节) + base64。真身 put 走
    // `SymmetricCryptoAndroid("AES", key).encryptBase64`(= AES/ECB/PKCS5Padding),
    // get 走 hutool `AES(key).decryptStr` —— 解不开时**吞异常给 null**。
    let aes_key: Vec<u8> = cfg.android_id.as_bytes().iter().copied().take(16).collect();
    {
        let (k, ak, cache) = (key.clone(), aes_key.clone(), cache.clone());
        java.set(
            "getLoginInfo",
            Func::from(move |ctx: Ctx<'js>| -> Value<'js> {
                let Some(enc) = cache.get_str(&format!("userInfo_{k}")) else {
                    return Value::new_null(ctx);
                };
                let plain = java_api::sym_decode_input(&enc).and_then(|bytes| {
                    java_api::SymmetricCrypto::new("AES", &ak, None)?
                        .decrypt(&bytes)
                        .and_then(|d| String::from_utf8(d).map_err(|e| e.to_string()))
                });
                match plain {
                    Ok(s) => rquickjs::String::from_str(ctx.clone(), &s)
                        .map(Value::from_string)
                        .unwrap_or(Value::new_null(ctx.clone())),
                    Err(_) => Value::new_null(ctx),
                }
            }),
        )?;
    }
    {
        let (k, ak, cache) = (key.clone(), aes_key.clone(), cache.clone());
        java.set(
            "putLoginInfo",
            Func::from(move |info: rquickjs::Coerced<String>| -> bool {
                let Ok(c) = java_api::SymmetricCrypto::new("AES", &ak, None) else { return false };
                match c.encrypt(info.0.as_bytes()) {
                    Ok(enc) => {
                        cache.insert(
                            format!("userInfo_{k}"),
                            java_api::base64_encode_bytes(&enc, 2),
                        );
                        true
                    }
                    Err(_) => false,
                }
            }),
        )?;
    }
    // `getLoginInfoMap()`:解密后的 JSON 对象在真身里是 Gson 的
    // `MutableMap<String,String>`，Rhino 看到 NativeJavaMap。不能直接交一个 JS
    // 原生对象：书源会同时用 `map.key` 与 `map.get(key)`（Phase 3 清单里的
    // “番茄小说2”就是前一种）。取不到/解不开/不是对象都给空 Map。
    {
        let (k, ak, cache) = (key.clone(), aes_key.clone(), cache.clone());
        java.set(
            "getLoginInfoMap",
            Func::from(move |ctx: Ctx<'js>| -> rquickjs::Result<Value<'js>> {
                let map = cache
                    .get_str(&format!("userInfo_{k}"))
                    .and_then(|enc| {
                        java_api::sym_decode_input(&enc).ok().and_then(|bytes| {
                            java_api::SymmetricCrypto::new("AES", &ak, None)
                                .ok()?
                                .decrypt(&bytes)
                                .ok()
                        })
                    })
                    .and_then(|plain| serde_json::from_slice::<serde_json::Value>(&plain).ok())
                    .and_then(|v| v.as_object().cloned())
                    .unwrap_or_default();
                let entries = rquickjs::Array::new(ctx.clone())?;
                let boxer: rquickjs::Function =
                    ctx.globals().get::<_, Object>("__java")?.get("str")?;
                for (i, (name, value)) in map.iter().enumerate() {
                    let pair = rquickjs::Array::new(ctx.clone())?;
                    pair.set(0, name.as_str())?;
                    let text =
                        value.as_str().map(str::to_string).unwrap_or_else(|| value.to_string());
                    let boxed: Value = boxer.call((text.as_str(),))?;
                    pair.set(1, boxed)?;
                    entries.set(i, pair)?;
                }
                let text = format!(
                    "{{{}}}",
                    map.iter()
                        .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or_default()))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                let factory: rquickjs::Function =
                    ctx.globals().get::<_, Object>("__java")?.get("nativeJavaMap")?;
                factory.call((entries, text, false))
            }),
        )?;
    }
    Ok(())
}

/// [`JavaValue`] → JS 值:Rhino 的 **NativeJavaMap / NativeJavaList** 与
/// 装箱的 `java.lang.String` / `Integer`。形态与理由见 java_proxy.rs 的
/// `nativeJavaMap`,契约由 jsharness 探针 `js-bind-*` 钉住。
fn java_value_js<'js>(
    ctx: &Ctx<'js>,
    vp: VarPtr,
    v: &rubato_core::host::JavaValue,
) -> rquickjs::Result<Value<'js>> {
    use rubato_core::host::JavaValue;
    let helper = |name: &str| -> rquickjs::Result<rquickjs::Function<'js>> {
        ctx.globals().get::<_, Object>("__java")?.get(name)
    };
    Ok(match v {
        JavaValue::Str(s) => helper("str")?.call((s.as_str(),))?,
        JavaValue::Num(d, s) => helper("num")?.call((*d, s.as_str()))?,
        JavaValue::Bool(b) => helper("bool")?.call((*b,))?,
        JavaValue::Null => Value::new_null(ctx.clone()),
        JavaValue::Element(h) => el_proxy(ctx, vp, *h)?.into_value(),
        JavaValue::Map(entries, s) => {
            let arr = rquickjs::Array::new(ctx.clone())?;
            for (i, (k, val)) in entries.iter().enumerate() {
                let pair = rquickjs::Array::new(ctx.clone())?;
                pair.set(0, k.as_str())?;
                pair.set(1, java_value_js(ctx, vp, val)?)?;
                arr.set(i, pair)?;
            }
            helper("nativeJavaMap")?.call((arr, s.as_str(), v.to_json().is_none()))?
        }
        JavaValue::List(items, s) => {
            let arr = rquickjs::Array::new(ctx.clone())?;
            for (i, it) in items.iter().enumerate() {
                arr.set(i, java_value_js(ctx, vp, it)?)?;
            }
            helper("nativeJavaList")?.call((arr, s.as_str(), v.to_json().is_none()))?
        }
    })
}

/// 句柄 → JS 值。**先分岔**:句柄背后不是 jsoup 的元素时(jayway 读出来的
/// Map/List、Kotlin 的 String/Double…),真身那边它是 `NativeJavaMap` /
/// `NativeJavaList` / 装箱的 `java.lang.String`,而**不是** Elements ——
/// 按键取属性、`toString()` 是 Java 的、过得了 GSON,三处都与 Elements 相反。
/// 见 [`rubato_core::host::RuleHost::el_json`]。
fn el_value<'js>(ctx: &Ctx<'js>, vp: VarPtr, h: ElementHandle) -> rquickjs::Result<Value<'js>> {
    // 规则引擎手上有 RuleValue,给得出**逐层**的 Java 形态(容器的 toString
    // 随出身、值各自装箱);给不出的宿主(差分桩)落回下面按 JSON 现搭的浅一层。
    if let Some(jv) = vp.get().el_java(h) {
        return java_value_js(ctx, vp, &jv);
    }
    let Some(j) = vp.get().el_json(h) else {
        return Ok(el_proxy(ctx, vp, h)?.into_value());
    };
    let s = vp.get().el_to_string(h);
    match &j {
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            let parsed = ctx.json_parse(j.to_string())?;
            let f: rquickjs::Function = ctx.globals().get::<_, Object>("__java")?.get("javaObj")?;
            f.call((parsed, s))
        }
        // 标量:真身是装箱的 java.lang.String / Double / Boolean
        serde_json::Value::String(_) => {
            let f: rquickjs::Function = ctx.globals().get::<_, Object>("__java")?.get("str")?;
            f.call((s,))
        }
        serde_json::Value::Number(n) => {
            Ok(rquickjs::Value::new_number(ctx.clone(), n.as_f64().unwrap_or(0.0)))
        }
        serde_json::Value::Bool(b) => Ok(rquickjs::Value::new_bool(ctx.clone(), *b)),
        serde_json::Value::Null => Ok(Value::new_null(ctx.clone())),
    }
}

// 元素句柄 → JS 代理对象
fn el_proxy<'js>(ctx: &Ctx<'js>, vp: VarPtr, h: ElementHandle) -> rquickjs::Result<Object<'js>> {
    let o = Object::new(ctx.clone())?;
    o.set("__el", h)?;
    // 完成值形态位:jsoup 的 Element/Elements 解包后裁判侧 typeOf 落在 `other`
    o.set("__javaKind", "elements")?;
    o.set("toString", Func::from(move || vp.get().el_to_string(h)))?;
    o.set("text", Func::from(move || vp.get().el_text(h)))?;
    o.set("html", Func::from(move || vp.get().el_html(h)))?;
    o.set("outerHtml", Func::from(move || vp.get().el_to_string(h)))?;
    o.set("attr", Func::from(move |k: rquickjs::Coerced<String>| vp.get().el_attr(h, &k.0)))?;
    // `Element.hasClass(name)`:书源用它分辨 vip/锁定章节
    // (`result.select('em').hasClass('vip') ? '🔒' : ''`)
    o.set(
        "hasClass",
        Func::from(move |k: rquickjs::Coerced<String>| {
            rubato_core::host::jsoup_has_class(&vp.get().el_attr(h, "class"), &k.0)
        }),
    )?;
    o.set(
        "first",
        Func::from(move |ctx: Ctx<'js>| -> rquickjs::Result<Value<'js>> {
            match vp.get().el_get(h, 0) {
                Some(c) => Ok(el_proxy(&ctx, vp, c)?.into_value()),
                None => Ok(Value::new_null(ctx)),
            }
        }),
    )?;
    o.set("size", Func::from(move || vp.get().el_size(h) as i32))?;
    o.set("length", vp.get().el_size(h) as i32)?;
    // `Node.remove()` 在 jsoup 里返回 **void**(实测 1.16.2)—— Rhino 那边是 undefined
    o.set("remove", Func::from(move || vp.get().el_remove(h)))?;
    // jsoup:`Node.parentNode()` 与 `Element.parent()` 在这里是同一个东西
    // (往上到 Document 为止,再往上是 null)。书源用它从命中的节点爬回去
    // 取标题:`result.parentNode().parentNode().select('h2').text()`。
    for name in ["parentNode", "parent"] {
        o.set(
            name,
            Func::from(move |ctx: Ctx<'js>| -> rquickjs::Result<Value<'js>> {
                match vp.get().el_parent(h) {
                    Some(pa) => Ok(el_proxy(&ctx, vp, pa)?.into_value()),
                    None => Ok(Value::new_null(ctx)),
                }
            }),
        )?;
    }
    o.set(
        "toArray",
        Func::from(move |ctx: Ctx<'js>| -> rquickjs::Result<Value<'js>> {
            let arr = rquickjs::Array::new(ctx.clone())?;
            let n = vp.get().el_size(h);
            for i in 0..n {
                if let Some(c) = vp.get().el_get(h, i) {
                    arr.set(i, el_proxy(&ctx, vp, c)?)?;
                }
            }
            Ok(arr.into_value())
        }),
    )?;
    o.set(
        "get",
        Func::from(move |ctx: Ctx<'js>, i: i32| -> rquickjs::Result<Value<'js>> {
            match vp.get().el_get(h, i.max(0) as usize) {
                Some(c) => Ok(el_proxy(&ctx, vp, c)?.into_value()),
                None => Ok(Value::new_null(ctx)),
            }
        }),
    )?;
    o.set(
        "select",
        Func::from(
            move |ctx: Ctx<'js>, css: rquickjs::Coerced<String>| -> rquickjs::Result<Value<'js>> {
                // 选择器不合法时 jsoup **抛**(不是返回空)—— 穿出去
                let hs = vp.get().el_select(h, &css.0).map_err(|e| throw(&ctx, &e))?;
                els_proxy(&ctx, vp, hs).map(|o| o.into_value())
            },
        ),
    )?;
    // `Iterable.forEach(Consumer)` —— jsoup 的 Elements 是 ArrayList,真身有这个
    o.set(
        "forEach",
        Func::from(move |ctx: Ctx<'js>, f: rquickjs::Function<'js>| -> rquickjs::Result<()> {
            for i in 0..vp.get().el_size(h) {
                if let Some(c) = vp.get().el_get(h, i) {
                    f.call::<_, Value>((el_proxy(&ctx, vp, c)?, i as i32))?;
                }
            }
            Ok(())
        }),
    )?;
    // 下标访问:Elements 在真身里是 List,`els[0]` 可用
    let n = vp.get().el_size(h);
    for i in 0..n {
        if let Some(c) = vp.get().el_get(h, i) {
            o.set(i as i32, el_proxy(ctx, vp, c)?)?;
        }
    }
    seal_proxy(ctx, o)
}

/// `AnalyzeRule.getElements` 的返回 → JS 值。**容器形态由规则模式决定**:
/// `Mode::Default` 那条给的是 jsoup 的 `Elements`,其余几条(jayway `getList`、
/// `List<JXNode>`、正则、JS)给的都是普通 `ArrayList` —— 两者在
/// `toString()` 与「过不过得了 GSON」上都不同,见
/// [`rubato_core::host::ElementList`]。
fn element_list_value<'js>(
    ctx: &Ctx<'js>,
    vp: VarPtr,
    list: rubato_core::host::ElementList,
) -> rquickjs::Result<Value<'js>> {
    if list.jsoup {
        return els_proxy(ctx, vp, list.items).map(|o| o.into_value());
    }
    let hs = list.items;
    let o = Object::new(ctx.clone())?;
    // 完成值形态位:裁判侧 typeOf 落在 `other`(解包后是个 Java 对象)
    o.set("__javaKind", "javalist")?;
    for (i, h) in hs.iter().enumerate() {
        o.set(i as i32, el_value(ctx, vp, *h)?)?;
    }
    let n = hs.len() as i32;
    o.set("length", n)?;
    o.set("size", Func::from(move || n))?;
    o.set("isEmpty", Func::from(move || n == 0))?;
    // Kotlin/Java `ArrayList.toString()` → `[a, b]`(项各自的 toString)
    let parts: Vec<String> = hs.iter().map(|h| vp.get().el_to_string(*h)).collect();
    let joined = format!("[{}]", parts.join(", "));
    o.set("toString", Func::from(move || joined.clone()))?;
    // 过 GSON:项里只要有一个是 jsoup 元素,整表就抛(XPath 那条 `List<JXNode>`
    // 正是这一档 —— toString 是 `[a, b]` 而 norm 是 JsonIOException)
    let items_json: Option<Vec<serde_json::Value>> =
        hs.iter().map(|h| vp.get().el_json(*h)).collect();
    match items_json {
        Some(items) => {
            let text = serde_json::Value::Array(items).to_string();
            o.set(
                "toJSON",
                Func::from(move |ctx: Ctx<'js>| -> rquickjs::Result<rquickjs::Value<'js>> {
                    ctx.json_parse(text.clone())
                }),
            )?;
        }
        None => o.set("__javaNoJson", true)?,
    }
    let hs_get = hs.clone();
    o.set(
        "get",
        Func::from(move |ctx: Ctx<'js>, i: i32| -> rquickjs::Result<rquickjs::Value<'js>> {
            match hs_get.get(i.max(0) as usize) {
                Some(h) => el_value(&ctx, vp, *h),
                None => Ok(Value::new_null(ctx)),
            }
        }),
    )?;
    o.set(
        "forEach",
        Func::from(move |ctx: Ctx<'js>, f: rquickjs::Function<'js>| -> rquickjs::Result<()> {
            for (i, h) in hs.iter().enumerate() {
                f.call::<_, Value>((el_value(&ctx, vp, *h)?, i as i32))?;
            }
            Ok(())
        }),
    )?;
    seal_proxy(ctx, o).map(|o| o.into_value())
}

/// 一组句柄 → 类 Elements 的代理(`java.getElements` 与 `Element.select` 的返回)
fn els_proxy<'js>(
    ctx: &Ctx<'js>,
    vp: VarPtr,
    hs: Vec<ElementHandle>,
) -> rquickjs::Result<Object<'js>> {
    let o = Object::new(ctx.clone())?;
    // 完成值形态位:裁判侧 typeOf 落在 `other`,norm 走 GSON(对 Elements 会抛)
    o.set("__javaKind", "elements")?;
    for (i, h) in hs.iter().enumerate() {
        o.set(i as i32, el_proxy(ctx, vp, *h)?)?;
    }
    o.set("length", hs.len() as i32)?;
    let n = hs.len() as i32;
    o.set("size", Func::from(move || n))?;
    let strs: Vec<String> = hs.iter().map(|h| vp.get().el_to_string(*h)).collect();
    // jsoup Elements.toString() = 各 outerHtml 以换行相连(空项不产生分隔符,
    // 见 jsoup_join)
    let joined = rubato_core::host::jsoup_join(&strs, "\n");
    o.set("toString", Func::from(move || joined.clone()))?;
    // jsoup Elements.text() = 各 text 以空格相连
    let texts: Vec<String> = hs.iter().map(|h| vp.get().el_text(*h)).collect();
    let text_joined = rubato_core::host::jsoup_join(&texts, " ");
    o.set("text", Func::from(move || text_joined.clone()))?;
    // jsoup `Elements.attr(k)`:取**第一个**有该属性的元素
    let hs_attr = hs.clone();
    o.set(
        "attr",
        Func::from(move |k: rquickjs::Coerced<String>| -> String {
            hs_attr
                .iter()
                .map(|h| vp.get().el_attr(*h, &k.0))
                .find(|a| !a.is_empty())
                .unwrap_or_default()
        }),
    )?;
    // `Elements.hasClass(name)`:**其中任意一个**有这个 class 就为真
    let hs_cls = hs.clone();
    o.set(
        "hasClass",
        Func::from(move |k: rquickjs::Coerced<String>| -> bool {
            hs_cls
                .iter()
                .any(|h| rubato_core::host::jsoup_has_class(&vp.get().el_attr(*h, "class"), &k.0))
        }),
    )?;
    let hs_html = hs.clone();
    o.set(
        "html",
        Func::from(move || -> String {
            let parts: Vec<String> = hs_html.iter().map(|h| vp.get().el_html(*h)).collect();
            rubato_core::host::jsoup_join(&parts, "\n")
        }),
    )?;
    let hs_sel = hs.clone();
    o.set(
        "select",
        Func::from(
            move |ctx: Ctx<'js>, css: rquickjs::Coerced<String>| -> rquickjs::Result<Value<'js>> {
                let mut out = Vec::new();
                for h in &hs_sel {
                    out.extend(vp.get().el_select(*h, &css.0).map_err(|e| throw(&ctx, &e))?);
                }
                els_proxy(&ctx, vp, out).map(|o| o.into_value())
            },
        ),
    )?;
    // `Elements.remove()`:把这些节点从文档里摘掉,**交回自己**(实测 jsoup
    // 1.16.2 返回 Elements,而且摘下来的那份 outerHtml 照样打得出)。
    // 书源拿它净化正文:`doc.select("div.text>*").not("h3,p").remove()`。
    let hs_rm = hs.clone();
    o.set(
        "remove",
        Func::from(move |ctx: Ctx<'js>| -> rquickjs::Result<Value<'js>> {
            for h in &hs_rm {
                vp.get().el_remove(*h);
            }
            els_proxy(&ctx, vp, hs_rm.clone()).map(|o| o.into_value())
        }),
    )?;
    let hs_not = hs.clone();
    o.set(
        "not",
        Func::from(
            move |ctx: Ctx<'js>, css: rquickjs::Coerced<String>| -> rquickjs::Result<Value<'js>> {
                let mut out = Vec::new();
                for h in &hs_not {
                    out.extend(vp.get().el_not(*h, &css.0).map_err(|e| throw(&ctx, &e))?);
                }
                els_proxy(&ctx, vp, out).map(|o| o.into_value())
            },
        ),
    )?;
    // `Elements.eachText()` / `eachAttr(k)`:jsoup 交回 `List<String>`
    // (Rhino 那边是 NativeJavaList,有 `size()` / `get(i)` 而**没有** map/join)。
    // `eachText` 只收 `hasText()` 为真的那些(空白不算),`eachAttr` 只收有该属性的。
    for (name, attr) in [("eachText", None), ("eachAttr", Some(()))] {
        let hs_each = hs.clone();
        o.set(
            name,
            Func::from(
                move |ctx: Ctx<'js>,
                      k: rquickjs::function::Opt<rquickjs::Coerced<String>>|
                      -> rquickjs::Result<Value<'js>> {
                    let items: Vec<String> = hs_each
                        .iter()
                        .map(|h| match (attr, &k.0) {
                            (Some(()), Some(k)) => vp.get().el_attr(*h, &k.0),
                            _ => vp.get().el_text(*h),
                        })
                        .filter(|t| !t.trim().is_empty())
                        .collect();
                    let f: rquickjs::Function =
                        ctx.globals().get::<_, Object>("__java")?.get("list")?;
                    f.call((items,))
                },
            ),
        )?;
    }
    let hs_first = hs.clone();
    o.set(
        "first",
        Func::from(move |ctx: Ctx<'js>| -> rquickjs::Result<Value<'js>> {
            match hs_first.first() {
                Some(h) => Ok(el_proxy(&ctx, vp, *h)?.into_value()),
                None => Ok(Value::new_null(ctx)),
            }
        }),
    )?;
    let hs_last = hs.clone();
    o.set(
        "last",
        Func::from(move |ctx: Ctx<'js>| -> rquickjs::Result<Value<'js>> {
            match hs_last.last() {
                Some(h) => Ok(el_proxy(&ctx, vp, *h)?.into_value()),
                None => Ok(Value::new_null(ctx)),
            }
        }),
    )?;
    let hs_eq = hs.clone();
    o.set(
        "eq",
        Func::from(move |ctx: Ctx<'js>, i: i32| -> rquickjs::Result<Value<'js>> {
            match hs_eq.get(i.max(0) as usize) {
                Some(h) => Ok(el_proxy(&ctx, vp, *h)?.into_value()),
                None => Ok(Value::new_null(ctx)),
            }
        }),
    )?;
    let n_empty = hs.len();
    o.set("isEmpty", Func::from(move || n_empty == 0))?;
    let hs2 = hs.clone();
    o.set(
        "get",
        Func::from(move |ctx: Ctx<'js>, i: i32| -> rquickjs::Result<Value<'js>> {
            match hs2.get(i.max(0) as usize) {
                Some(h) => Ok(el_proxy(&ctx, vp, *h)?.into_value()),
                None => Ok(Value::new_null(ctx)),
            }
        }),
    )?;
    let hs3 = hs.clone();
    o.set(
        "toArray",
        Func::from(move |ctx: Ctx<'js>| -> rquickjs::Result<Value<'js>> {
            let arr = rquickjs::Array::new(ctx.clone())?;
            for (i, h) in hs3.iter().enumerate() {
                arr.set(i, el_proxy(&ctx, vp, *h)?)?;
            }
            Ok(arr.into_value())
        }),
    )?;
    let hs4 = hs.clone();
    o.set(
        "forEach",
        Func::from(move |ctx: Ctx<'js>, f: rquickjs::Function<'js>| -> rquickjs::Result<()> {
            for (i, h) in hs4.iter().enumerate() {
                f.call::<_, Value>((el_proxy(&ctx, vp, *h)?, i as i32))?;
            }
            Ok(())
        }),
    )?;
    seal_proxy(ctx, o)
}

/// 把代理对象的非下标属性压成不可枚举 —— 真身的 NativeJavaList
/// `for (var i in els)` 只给下标,而 Rust 侧 `Object::set` 建的都是可枚举的。
///
/// **M2j 接上了**(`el_proxy` / `els_proxy` 建完都调它)。此前挂着不接的理由是
/// 「语料里没有对 Elements 用 for..in 的写法」—— 那是因为那时 `result` 是拍平的
/// 串,元素**根本进不了 JS**;绑定一走真对象,`for (i in result)` 立刻就有了
/// (pb01214:不接的话枚举出来的是 `text`/`select`/`toString` 这些方法名,
/// 于是 `$[i].select(...)` 报 not a function)。
fn seal_proxy<'js>(ctx: &Ctx<'js>, o: Object<'js>) -> rquickjs::Result<Object<'js>> {
    let f: rquickjs::Function = ctx.globals().get::<_, Object>("__java")?.get("sealProxy")?;
    f.call((o,))
}

/// Rhino LiveConnect 面:`org.jsoup.Jsoup`(以及 `Packages.org.jsoup`)。
///
/// 书源真的会绕开规则层直接调 jsoup(语料里 23 个用例走这条)。探针实测
/// `typeof org.jsoup.Jsoup === "function"`(Rhino 把 Java 类暴露成构造函数),
/// `typeof org.jsoup.Jsoup.parse(html) === "object"`。
///
/// 解析走 [`RuleHost::rule_parse_html`] 的反调,拿到的 Document 与
/// `getElement(s)` 共用同一套句柄与操作。**两个宿主都装** —— 它是 Rhino 的
/// 全局面,不是 AnalyzeRule 的方法。
///
/// 产品侧注意:`net` 的 AnalyzeUrl 宿主目前传的是 `RuleData`(没有规则面),
/// 那条路上 `org.jsoup` 会报 `NO_RULE_HOST`。searchUrl 里用 jsoup 的源要等
/// net 侧也接上解析能力。
fn install_liveconnect<'js>(ctx: &Ctx<'js>, vp: VarPtr) -> rquickjs::Result<()> {
    let g = ctx.globals();
    let jsoup_cls = rquickjs::Function::new(ctx.clone(), || {})?;
    for name in ["parse", "parseBodyFragment"] {
        jsoup_cls.set(
            name,
            Func::from(
                move |ctx: Ctx<'js>,
                      html: rquickjs::Coerced<String>|
                      -> rquickjs::Result<Value<'js>> {
                    let h = vp.get().rule_parse_html(&html.0).map_err(|e| throw(&ctx, &e))?;
                    Ok(el_proxy(&ctx, vp, h)?.into_value())
                },
            ),
        )?;
    }
    let jsoup_pkg = Object::new(ctx.clone())?;
    jsoup_pkg.set("Jsoup", jsoup_cls)?;
    let org = Object::new(ctx.clone())?;
    org.set("jsoup", jsoup_pkg)?;
    g.set("org", org)?;
    Ok(())
}

/// `java.getString` / `getStringList` / `getElement(s)` / `setContent`。
///
/// 元素不跨接口传值,只过**句柄**(见 [`ElementHandle`]):JS 侧拿到的是一层
/// 代理对象,方法转回 `RuleHost` 上。Elements 的可观察面按 jsoup:
/// `size()`/`get(i)`/下标/`text()`/`html()`/`attr()`/`select()`,
/// `toString()` 是各 outerHtml 以 `\n` 相连。
fn install_rule_callbacks<'js>(java: &Object<'js>, vp: VarPtr) -> rquickjs::Result<()> {
    java.set(
        "getString",
        Func::from(
            move |ctx: Ctx<'js>,
                  rule: rquickjs::Coerced<String>,
                  content: rquickjs::function::Opt<Value<'js>>|
                  -> rquickjs::Result<String> {
                vp.get()
                    .rule_get_string(&rule.0, m_content(&content.0).as_deref())
                    .map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;
    java.set(
        "getStringList",
        Func::from(
            move |ctx: Ctx<'js>,
                  rule: rquickjs::Coerced<String>,
                  content: rquickjs::function::Opt<Value<'js>>|
                  -> rquickjs::Result<Value<'js>> {
                let v = vp
                    .get()
                    .rule_get_string_list(&rule.0, m_content(&content.0).as_deref())
                    .map_err(|e| throw(&ctx, &e))?;
                // 真身返回 Kotlin List<String> → 走代理层的 list 箱
                let items = v.unwrap_or_default();
                let arr = rquickjs::Array::new(ctx.clone())?;
                for (i, s) in items.iter().enumerate() {
                    arr.set(i, s.as_str())?;
                }
                let f: rquickjs::Function =
                    ctx.globals().get::<_, Object>("__java")?.get("list")?;
                f.call((arr,))
            },
        ),
    )?;
    java.set(
        "getElement",
        Func::from(
            move |ctx: Ctx<'js>, rule: rquickjs::Coerced<String>| -> rquickjs::Result<Value<'js>> {
                match vp.get().rule_get_element(&rule.0).map_err(|e| throw(&ctx, &e))? {
                    // **不一定是 jsoup 元素**:`getElement('$.list')` 交回来的是
                    // jayway 的容器 —— toString 是 JSON 文本、而且过得了 GSON
                    Some(h) => el_value(&ctx, vp, h),
                    None => Ok(Value::new_null(ctx)),
                }
            },
        ),
    )?;
    java.set(
        "getElements",
        Func::from(
            move |ctx: Ctx<'js>, rule: rquickjs::Coerced<String>| -> rquickjs::Result<Value<'js>> {
                let list = vp.get().rule_get_elements(&rule.0).map_err(|e| throw(&ctx, &e))?;
                element_list_value(&ctx, vp, list)
            },
        ),
    )?;
    // setContent(content[, baseUrl]) 返回 AnalyzeRule 本身(真身可链式:
    // 探针 sc-chain `java.setContent(h).getString(r)`)——这里返回 java 自己
    java.set(
        "setContent",
        Func::from(
            move |ctx: Ctx<'js>,
                  content: rquickjs::Coerced<String>,
                  base: rquickjs::function::Opt<rquickjs::Coerced<String>>|
                  -> rquickjs::Result<Value<'js>> {
                vp.get()
                    .rule_set_content(&content.0, base.0.as_ref().map(|c| c.0.as_str()))
                    .map_err(|e| throw(&ctx, &e))?;
                ctx.globals().get::<_, Value>("java")
            },
        ),
    )?;
    Ok(())
}

/// `cookie` 绑定(真身 `CookieStore`)与 `cache` 绑定(真身 `CacheManager`)。
///
/// 差分侧不联网:`cookie` 只会走到「读到空串」那条路径,写进去的也只活在本次
/// 求值里(裁判每 case 前 `CacheManager.clearAll()`,两侧同口径)。
/// **产品侧 Phase 2 后续要把这两个接到 store**(`cookies` / `caches` 两张表),
/// 届时 `getCookie` 的二级域名归一等语义由 store 侧的 `CookieEnv` 负责。
fn install_cookie_cache<'js>(
    ctx: &Ctx<'js>,
    cache: SharedMap,
    cache_file: SharedMap,
    cookies: SharedMap,
    store: Option<Rc<RefCell<dyn rubato_core::host::CookieEnv>>>,
) -> rquickjs::Result<()> {
    let g = ctx.globals();

    let cookie = Object::new(ctx.clone())?;
    // 有真存储就走它(真身的 `cookie` 就是 CookieStore 单例,与网络层同一张表),
    // 没有才退回那张按域名键的裸表。**闭包里只捕获 Rc,不捕获 JS 值**。
    let st = store.clone();
    let ck = cookies.clone();
    cookie.set(
        "getCookie",
        Func::from(move |url: rquickjs::Coerced<String>| -> String {
            match &st {
                Some(s) => s.borrow_mut().get_cookie(&url.0),
                None => ck.get_str(&cookie_domain(&url.0)).unwrap_or_default(),
            }
        }),
    )?;
    let st = store.clone();
    let ck = cookies.clone();
    cookie.set(
        "setCookie",
        Func::from(move |url: rquickjs::Coerced<String>, v: rquickjs::Coerced<String>| match &st {
            Some(s) => s.borrow_mut().set_cookie(&url.0, &v.0),
            None => ck.insert(cookie_domain(&url.0), v.0),
        }),
    )?;
    let st = store.clone();
    let ck = cookies.clone();
    cookie.set(
        "replaceCookie",
        Func::from(move |url: rquickjs::Coerced<String>, v: rquickjs::Coerced<String>| match &st {
            Some(s) => s.borrow_mut().replace_cookie(&url.0, &v.0),
            None => ck.insert(cookie_domain(&url.0), v.0),
        }),
    )?;
    let st = store.clone();
    let ck = cookies.clone();
    cookie.set(
        "removeCookie",
        Func::from(move |url: rquickjs::Coerced<String>| match &st {
            Some(s) => s.borrow_mut().remove_cookie(&url.0),
            None => ck.remove(&cookie_domain(&url.0)),
        }),
    )?;
    // CookieStore.getKey(url, key):从合并后的 cookie 串里取一项,取不到给空串
    let st = store.clone();
    let ck = cookies.clone();
    cookie.set(
        "getKey",
        Func::from(
            move |url: rquickjs::Coerced<String>, key: rquickjs::Coerced<String>| -> String {
                match &st {
                    Some(s) => s.borrow_mut().get_cookie_key(&url.0, &key.0),
                    None => {
                        let all = ck.get_str(&cookie_domain(&url.0)).unwrap_or_default();
                        cookie_to_map(&all).get(&key.0).unwrap_or_default().to_string()
                    }
                }
            },
        ),
    )?;
    cookie.set(
        "cookieToMap",
        Func::from(
            move |ctx: Ctx<'js>, c: rquickjs::Coerced<String>| -> rquickjs::Result<Object<'js>> {
                let o = Object::new(ctx.clone())?;
                for (k, v) in cookie_to_map(&c.0).0 {
                    o.set(k, v)?;
                }
                Ok(o)
            },
        ),
    )?;
    g.set("cookie", cookie)?;

    let cache_obj = Object::new(ctx.clone())?;
    let ca = cache.clone();
    // CacheManager.put(key, value, saveTime=0):saveTime 本套不参与(无时钟推进)
    cache_obj.set(
        "put",
        Func::from(move |k: rquickjs::Coerced<String>, v: rquickjs::Coerced<String>| {
            ca.insert(k.0, v.0);
        }),
    )?;
    let ca = cache.clone();
    cache_obj.set(
        "putMemory",
        Func::from(move |k: rquickjs::Coerced<String>, v: rquickjs::Coerced<String>| {
            ca.insert(k.0, v.0);
        }),
    )?;
    // 取不到给 **Java null**(探针 cache-getint:`String(cache.get('nope'))` → "null")。
    // `getFile` **不在这一组**:它读的是另一张表(ACache),见下。
    for name in ["get", "getFromMemory"] {
        let ca = cache.clone();
        cache_obj.set(
            name,
            Func::from(move |ctx: Ctx<'js>, k: rquickjs::Coerced<String>| -> Value<'js> {
                match ca.get_str(&k.0) {
                    Some(v) => rquickjs::String::from_str(ctx.clone(), &v)
                        .map(Value::from_string)
                        .unwrap_or(Value::new_null(ctx.clone())),
                    None => Value::new_null(ctx),
                }
            }),
        )?;
    }
    let ca = cache.clone();
    cache_obj.set(
        "getInt",
        Func::from(move |ctx: Ctx<'js>, k: rquickjs::Coerced<String>| -> Value<'js> {
            match ca.get_str(&k.0).and_then(|v| v.parse::<i32>().ok()) {
                Some(n) => Value::new_int(ctx, n),
                None => Value::new_null(ctx),
            }
        }),
    )?;
    // `deleteMemory` 只打内存那张(`delete` 三张一起删,见下)
    {
        let ca = cache.clone();
        cache_obj.set(
            "deleteMemory",
            Func::from(move |k: rquickjs::Coerced<String>| {
                ca.remove(&k.0);
            }),
        )?;
    }
    // `putFile/getFile` 打的是 **ACache**(落盘),与 `put/get` 的 cacheDao +
    // 内存表是两张表 —— CacheManager.kt L151/L155。写进去的读不回另一张。
    let cf = cache_file.clone();
    cache_obj.set(
        "putFile",
        Func::from(move |k: rquickjs::Coerced<String>, v: rquickjs::Coerced<String>| {
            cf.insert(k.0, v.0);
        }),
    )?;
    let cf = cache_file.clone();
    cache_obj.set(
        "getFile",
        Func::from(move |ctx: Ctx<'js>, k: rquickjs::Coerced<String>| -> Value<'js> {
            match cf.get_str(&k.0) {
                Some(v) => rquickjs::String::from_str(ctx.clone(), &v)
                    .map(Value::from_string)
                    .unwrap_or(Value::new_null(ctx.clone())),
                None => Value::new_null(ctx),
            }
        }),
    )?;
    // `delete(key)` 三张表一起删(真身 CacheManager.kt L160:cacheDao +
    // deleteMemory + ACache.remove)—— 上面那一组只删了前两张
    let cf = cache_file.clone();
    let ca = cache.clone();
    cache_obj.set(
        "delete",
        Func::from(move |k: rquickjs::Coerced<String>| {
            ca.remove(&k.0);
            cf.remove(&k.0);
        }),
    )?;
    let _ = cache;
    g.set("cache", cache_obj)?;
    Ok(())
}

/// `CookieStore` 的键:真身按 `NetworkUtils.getSubDomain(url)` 归一。
/// 本套只走空 cookie 那条路径,故取 host 即可;接 store 时由 net 侧的
/// 二级域名归一接管(契约见 fixtures/cases/fetch/README.md)。
fn cookie_domain(url: &str) -> String {
    let no_scheme = url.split("://").last().unwrap_or(url);
    no_scheme.split('/').next().unwrap_or(no_scheme).to_string()
}

/// `CookieStore.cookieToMap`:直接用 rubato-core 里逐行对齐的那份。
/// 这里曾自写过一版(BTreeMap):键被**排序**——真身是 LinkedHashMap 插入序,
/// JS 里 `JSON.stringify(cookie.cookieToMap(s))` 的键序会分岔;空白值过滤和
/// `"null"` 特判也漏了。差分套只走空串路径照不到,产品路径能踩到。
use rubato_core::cookies::cookie_to_map;

/// `base64Decode(str) / (str, charset) / (str, flags)`:Rhino 按实参的运行时类型
/// 挑 Java 重载,JS 侧没有类型,故按「第二个实参是不是数字」手工分发。
fn base64_decode_js<'js>(
    ctx: Ctx<'js>,
    s: rquickjs::Coerced<String>,
    arg: rquickjs::function::Opt<Value<'js>>,
) -> rquickjs::Result<String> {
    let charset: Option<String> = match arg.0 {
        // flags 重载:EncoderUtils.base64Decode 固定按平台默认字符集(UTF-8)解
        None => None,
        Some(v) if v.is_number() => None,
        Some(v) => {
            let c: rquickjs::Coerced<String> = rquickjs::FromJs::from_js(&ctx, v)?;
            Some(c.0)
        }
    };
    java_api::base64_decode_str(&s.0, charset.as_deref()).map_err(|e| throw(&ctx, &e))
}

/// 把宿主侧的错误标签抛成 JS 异常(差分只比错误类别,消息文本不参与)
fn throw(ctx: &Ctx<'_>, msg: &str) -> rquickjs::Error {
    ctx.throw(
        rquickjs::String::from_str(ctx.clone(), msg)
            .map(Value::from_string)
            .unwrap_or(Value::new_null(ctx.clone())),
    )
}

/// 完成值 → [`EvalDetail`]。求值本身有两条路(整段一次、或按完成值切段顺序求),
/// 收尾这一段两条共用。
fn finish_eval<'js>(ctx: &Ctx<'js>, v: Value<'js>) -> Result<EvalDetail, String> {
    let is_function = v.is_function();
    // **顶层解包**:真身的 RhinoScriptEngine.eval 会把 Wrapper 解包,
    // 所以 `java.md5Encode('a')` 作为完成值又是普通字符串(探针 ret-md5)
    let (v, java_object) = unwrap_java_completion(ctx, v)?;
    let is_function = is_function && java_object.is_none();
    let json = if v.is_object() && !is_function {
        ctx.json_stringify(v.clone())
            .ok()
            .flatten()
            .and_then(|s| s.to_string().ok())
            .and_then(|s| serde_json::from_str(&s).ok())
    } else {
        None
    };
    let value = to_js_value(ctx, v).map_err(|e| format!("convert: {e}"))?;
    Ok(EvalDetail { value, json, is_function, java_object })
}

/// **顶层解包**:真身的 `RhinoScriptEngine.eval` 在返回前把 `Wrapper` 解包,
/// 装箱只在表达式中间可观察(见 java_proxy.rs)。
///
/// - `JavaString` → 解成 JS 原生串,与未装箱时同形;
/// - Java List / 数组 → 值保持原样交给 `to_js_value`,另外带回
///   [`JavaObject`] 形态位(裁判侧 `type` 落在 `other`)。
fn unwrap_java_completion<'js>(
    ctx: &Ctx<'js>,
    v: Value<'js>,
) -> Result<(Value<'js>, Option<JavaObject>), String> {
    let Some(obj) = v.as_object() else { return Ok((v, None)) };
    // JavaString:有 __v 且原型链在 String.prototype 上
    if let Ok(kind) = obj.get::<_, Value>("__javaKind") {
        if let Some(k) = kind.as_string().and_then(|s| s.to_string().ok()) {
            let str_form: String = obj
                .get::<_, rquickjs::Function>("toString")
                .and_then(|f| f.call((rquickjs::function::This(v.clone()),)))
                .map_err(|e| format!("java toString: {e}"))?;
            // GSON 的口径:List → 元素数组;byte[] → 数字数组
            let json = ctx
                .json_stringify(v.clone())
                .ok()
                .flatten()
                .and_then(|s| s.to_string().ok())
                .and_then(|s| serde_json::from_str(&s).ok());
            // jsoup 的 Element/Elements 过不了 GSON(裁判侧 normError =
            // host:JsonIOException);普通 ArrayList 装了 jsoup 元素时同样,
            // 由建箱的一侧挂 `__javaNoJson`(见 element_list_value)。
            let no_json = obj.get::<_, bool>("__javaNoJson").unwrap_or(false);
            let json = if k == "elements" || no_json { None } else { json };
            return Ok((v, Some(JavaObject { kind: k, str: str_form, json })));
        }
    }
    if obj.contains_key("__v").unwrap_or(false) {
        if let Ok(inner) = obj.get::<_, Value>("__v") {
            // JavaString 的串、装箱数的数、装箱布尔的布尔(见 java_proxy)
            if inner.is_string() || inner.is_number() || inner.is_bool() {
                return Ok((inner, None));
            }
        }
    }
    Ok((v, None))
}

/// 一个元素代理对象上的句柄:`el_proxy` 建的带 `__el`;`els_proxy` 建的没有,
/// 它的句柄在下标子项上(`length` 个)。都不是 → `None`。
fn proxy_handles<'js>(v: &Value<'js>) -> rquickjs::Result<Option<Vec<ElementHandle>>> {
    let Some(obj) = v.as_object() else { return Ok(None) };
    match obj.get::<_, Value>("__javaKind") {
        Ok(k) if k.as_string().and_then(|s| s.to_string().ok()).as_deref() == Some("elements") => {}
        _ => return Ok(None),
    }
    if let Ok(h) = obj.get::<_, ElementHandle>("__el") {
        return Ok(Some(vec![h]));
    }
    let n: i32 = obj.get("length").unwrap_or(0);
    let mut out = Vec::new();
    for i in 0..n.max(0) {
        let item: Value = obj.get(i)?;
        match item.as_object().and_then(|o| o.get::<_, ElementHandle>("__el").ok()) {
            Some(h) => out.push(h),
            // 下标上不是元素代理 —— 不是本函数认识的形态,交回给通用那条路
            None => return Ok(None),
        }
    }
    Ok(Some(out))
}

/// 完成值是元素(或一整数组元素)时的 [`JsValue::Elements`];否则 `None`。
///
/// **混装的数组不走这条**(元素与串混在一起):真身那边它是一个混装
/// NativeArray,`getElements` 会原样留着两种;被测侧还没有能同时装两种的
/// 形态,故落回旧口径(整个数组拍平成串)。语料里没有这种写法。
fn element_completion<'js>(
    ctx: &Ctx<'js>,
    v: &Value<'js>,
) -> Result<Option<JsValue>, rquickjs::Error> {
    // 单个 `el_proxy` 才有 `__el`;`els_proxy`(jsoup Elements)与 JS 数组都没有
    let is_list = v.as_array().is_some()
        || v.as_object()
            .is_some_and(|o| o.get::<_, Value>("__el").map(|x| x.is_undefined()).unwrap_or(true));
    let handles = if let Some(arr) = v.as_array() {
        let mut out = Vec::new();
        for item in arr.iter::<Value>() {
            match proxy_handles(&item?)? {
                Some(hs) => out.extend(hs),
                None => return Ok(None),
            }
        }
        // 空数组按普通空列表走(两侧都是「什么都没有」,不必绕元素这条)
        if out.is_empty() {
            return Ok(None);
        }
        out
    } else {
        match proxy_handles(v)? {
            Some(hs) => hs,
            None => return Ok(None),
        }
    };
    // 串形态给不认识句柄的消费方;与本变体存在之前逐字一致(见 JsValue::Elements)
    let mut strings = Vec::with_capacity(handles.len());
    if let Some(arr) = v.as_array() {
        for item in arr.iter::<Value>() {
            let s: rquickjs::Coerced<String> = rquickjs::FromJs::from_js(ctx, item?)?;
            strings.push(s.0);
        }
    } else {
        let s: rquickjs::Coerced<String> = rquickjs::FromJs::from_js(ctx, v.clone())?;
        strings.push(s.0);
    }
    Ok(Some(JsValue::Elements { handles, strings, list: is_list }))
}

/// 数组元素是不是**真 NativeObject**(决定整个数组走 [`JsValue::Native`] 还是
/// [`JsValue::List`])。
///
/// 「是 JS 对象」不够:java_proxy 的三种箱(`JavaString` / `javaList` /
/// `javaArray`)与元素代理在 JS 里都是对象,而真身那边它们是
/// NativeJavaObject / NativeJavaList / NativeJavaArray —— 不是 NativeObject,
/// 装着它们的数组仍走 `List`(`l.push(java.getString(r))` 是最常见的写法)。
/// 嵌套数组同理:元素是 NativeArray,不是 NativeObject。
fn is_native_object(v: &Value<'_>) -> bool {
    if !v.is_object() || v.is_function() || v.as_array().is_some() {
        return false;
    }
    !is_java_box(v)
}

/// JS 完成值 → [`JsValue`](AnalyzeRule 按 String/Double/List/null 分支处理)
fn to_js_value<'js>(ctx: &Ctx<'js>, v: Value<'js>) -> Result<JsValue, rquickjs::Error> {
    if v.is_undefined() || v.is_null() {
        return Ok(JsValue::Null);
    }
    // **元素穿过 JS**:代理对象(或一整个装着代理的数组)回来时不拍平成串 ——
    // 真身那边 `getElements` 认 NativeArray 里的 NativeJavaObject(Element),
    // 下游按元素继续跑规则。见 [`JsValue::Elements`]。
    if let Some(e) = element_completion(ctx, &v)? {
        return Ok(e);
    }
    if let Some(b) = v.as_bool() {
        return Ok(JsValue::Bool(b));
    }
    if let Some(i) = v.as_int() {
        return Ok(JsValue::Num(i as f64));
    }
    if let Some(f) = v.as_float() {
        return Ok(JsValue::Num(f));
    }
    if let Some(s) = v.as_string() {
        return Ok(JsValue::Str(s.to_string()?));
    }
    if let Some(arr) = v.as_array() {
        // **元素是对象的数组不拍平**(见 [`JsValue::Native`]):真身那边元素还是
        // NativeObject,下游 `chapterName: "n"` 靠它走「键值直接访问」那条分支。
        let has_object = arr.iter::<Value>().filter_map(Result::ok).any(|x| is_native_object(&x));
        if has_object {
            if let Ok(Some(json)) = ctx.json_stringify(v.clone()) {
                if let Ok(text) = json.to_string() {
                    if let Ok(serde_json::Value::Array(a)) =
                        serde_json::from_str::<serde_json::Value>(&text)
                    {
                        return Ok(JsValue::Native(a));
                    }
                }
            }
        }
        let mut out = Vec::with_capacity(arr.len());
        for item in arr.iter::<Value>() {
            let item = item?;
            let s: rquickjs::Coerced<String> = rquickjs::FromJs::from_js(ctx, item)?;
            out.push(s.0);
        }
        return Ok(JsValue::List(out));
    }
    if v.is_object() && !v.is_function() {
        if let Ok(Some(json)) = ctx.json_stringify(v.clone()) {
            if let Ok(text) = json.to_string() {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                    return Ok(JsValue::Json(parsed));
                }
            }
        }
    }
    let s: rquickjs::Coerced<String> = rquickjs::FromJs::from_js(ctx, v)?;
    Ok(JsValue::Str(s.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rubato_core::host::VarStore;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MapVars(HashMap<String, String>);
    // 单测只钉变量层与纯函数,不带规则反调面(缺省实现一律报 NO_RULE_HOST)
    impl rubato_core::host::RuleHost for MapVars {}
    impl VarStore for MapVars {
        fn put(&mut self, k: &str, v: &str) -> String {
            self.0.insert(k.into(), v.into());
            v.into()
        }
        fn get(&self, k: &str) -> String {
            self.0.get(k).cloned().unwrap_or_default()
        }
    }

    fn eval(code: &str, b: JsBindings) -> (Result<JsValue, String>, MapVars) {
        let mut h = QuickJsHost::new();
        let mut vars = MapVars::default();
        let r = h.eval_js(code, &b, &mut vars);
        (r, vars)
    }

    /// `key`/`page` 是 **AnalyzeUrl 宿主**的绑定(searchUrl/exploreUrl 那条路径)。
    /// AnalyzeRule 宿主根本不绑 `key` —— 挑错宿主会把语义钉歪,见 [`JsHost`]。
    #[test]
    fn key_and_page_bindings_on_url_host() {
        let b = JsBindings {
            host: JsHost::Url,
            key: Some("斗破".into()),
            page: Some(3),
            ..Default::default()
        };
        assert_eq!(eval("key", b.clone()).0, Ok(JsValue::Str("斗破".into())));
        assert_eq!(eval("page", b.clone()).0, Ok(JsValue::Num(3.0)));
        // searchUrl 里最常见的形态
        assert_eq!(eval("key + '-' + page", b).0, Ok(JsValue::Str("斗破-3".into())));
    }

    /// AnalyzeRule 宿主上 `key` 是**未声明**(不是 null)——真身同样,
    /// 首轮差分里 64 例裁判报的就是 `ReferenceError: "key" 未定义`。
    #[test]
    fn key_is_undeclared_on_rule_host() {
        let (r, _) = eval("typeof key", JsBindings::default());
        assert_eq!(r, Ok(JsValue::Str("undefined".into())));
    }

    /// 绑定**存在但为空**时给 null(真身是 Java null),不是 undefined ——
    /// `result == null` 这类判断在书源里很常见
    #[test]
    fn missing_binding_is_null_not_undefined() {
        let (r, _) = eval("result === null", JsBindings::default());
        assert_eq!(r, Ok(JsValue::Bool(true)));
    }

    #[test]
    fn java_put_get_hits_var_store() {
        let (r, vars) = eval("java.put('a', '1'); java.get('a')", JsBindings::default());
        assert_eq!(r, Ok(JsValue::Str("1".into())));
        assert_eq!(vars.0.get("a").map(String::as_str), Some("1"));
    }

    #[test]
    fn array_becomes_list_object_becomes_json() {
        assert_eq!(
            eval("['a','b']", JsBindings::default()).0,
            Ok(JsValue::List(vec!["a".into(), "b".into()]))
        );
        assert!(matches!(eval("({a:1})", JsBindings::default()).0, Ok(JsValue::Json(_))));
    }

    #[test]
    fn syntax_error_is_err() {
        assert!(eval("(((", JsBindings::default()).0.is_err());
    }

    #[test]
    fn md5_and_base64() {
        assert_eq!(
            eval("java.md5Encode('abc')", JsBindings::default()).0,
            Ok(JsValue::Str("900150983cd24fb0d6963f7d28e17f72".into()))
        );
        assert_eq!(
            eval("java.base64Encode('abc')", JsBindings::default()).0,
            Ok(JsValue::Str("YWJj".into()))
        );
    }
}
