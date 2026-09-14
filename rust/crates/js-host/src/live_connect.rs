//! **LiveConnect 的 crypto 一支**:书源绕开 `java.*`,直接调 Java 类。
//!
//! 语料里约 27 个 B 层源走这条路,形状高度一致:
//!
//! ```js
//! var javaImport = new JavaImporter();
//! javaImport.importPackage(Packages.java.lang, Packages.javax.crypto,
//!                          Packages.javax.crypto.spec, Packages.android.util);
//! with (javaImport) {
//!     function decode(word) {
//!         var key = SecretKeySpec(String("…").getBytes(), "AES");
//!         var iv  = IvParameterSpec(java.base64DecodeToByteArray("…"));
//!         var c   = Cipher.getInstance("AES/CBC/PKCS5Padding");
//!         c.init(2, key, iv);
//!         return String(c.doFinal(Base64.decode(String(word).getBytes(), 2)));
//!     }
//! }
//! ```
//!
//! **这一支不经 hutool**,所以错误类别是 JCE 自己的
//! (`IllegalBlockSizeException` / `BadPaddingException` / `InvalidKeyException` /
//! `InvalidAlgorithmParameterException` / `NoSuchAlgorithmException`),
//! 与 `java.aes*`(一律 `CryptoException`)**不是一套**。分组密码本体两条路共用
//! [`java_api::SymmetricCrypto`],差的只有 [`java_api::CryptoFlavor`] 那层皮。
//!
//! 几处由 jsharness 探针实测钉住的方言(用例在 `hand.json` 的 `lc-` 一族):
//!
//! - `typeof Packages.javax.crypto.Cipher === "function"` —— Rhino 把 Java 类
//!   暴露成构造函数,**不带 `new` 直接调也是构造**(`SecretKeySpec(k,"AES")`);
//! - `with (javaImport)` 里 `String` 被 **java.lang.String 遮住** ——
//!   `String(byte[])` 是「按默认字符集解码」而不是 JS 的字符串化;
//! - `String('ab').getBytes()` 是 Java 数组:`Array.isArray` 为 **false**、
//!   元素是**有符号**的(`String('中').getBytes()[0] === -28`)、
//!   `join` 可用、`String(arr)` 给 `[B@…`(归一成 «identity»);
//! - **同名类跨包冲突**(如同时 import `java.util` 与 `android.util` 后取 `Base64`)
//!   在 Rhino 里是 `EvaluatorException` → 与裁判的 errorTag 一样归到
//!   `js:SyntaxError`(探针 `lc-ambiguous`);
//! - `Cipher.getInstance('AES/CBC/NoSuchPadding')` 抛的是
//!   **NoSuchAlgorithmException**(不是 NoSuchPaddingException,探针 `lc-err-nopad`)。
//!
//! **不接的**:`Packages.okhttp3.*`(书源拿它绕过宿主直接发请求)。裁判侧那条路
//! 会真的出网 —— 见 `fixtures/cases/js-host/README.md`「okhttp3 直连」。

use rquickjs::function::{Func, Opt};
use rquickjs::{Ctx, Object, Value};

use crate::java_api::{self, CryptoFlavor};

/// 把 JS 侧的字节序列读回 `Vec<u8>`。
///
/// 元素是**有符号**的(Java byte),故取低 8 位。装箱层给的是真 JS 数组,
/// 但书源也会直接传字面量数组(`PKCS8EncodedKeySpec([48,-126,…])`)。
fn read_bytes(v: &Value<'_>) -> Vec<u8> {
    if let Some(b) = read_byte_seq(v) {
        return b;
    }
    // 传进来的是串(`Base64.decode("…", 2)` 的 String 重载)
    if let Ok(s) = <rquickjs::Coerced<String> as rquickjs::FromJs>::from_js(v.ctx(), v.clone()) {
        return s.0.into_bytes();
    }
    Vec::new()
}

/// 字节序列的两种形态:真 JS 数组(书源写的字面量)与装箱层的 Java 数组
/// (`Object.create(Array.prototype)` 建的**类数组**,`Array.isArray` 为 false)。
/// 两种都要认 —— 后者是 `getBytes()` / `doFinal()` 的返回形态。
pub fn read_byte_seq(v: &Value<'_>) -> Option<Vec<u8>> {
    let to_u8 = |n: f64| (n as i64 & 0xff) as u8;
    if let Some(arr) = v.as_array() {
        return Some(arr.iter::<f64>().filter_map(|r| r.ok()).map(to_u8).collect());
    }
    let obj = v.as_object()?;
    if !obj.contains_key("length").unwrap_or(false) {
        return None;
    }
    let len: usize = obj.get::<_, f64>("length").ok()? as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        out.push(obj.get::<_, f64>(i as i32).map(to_u8).unwrap_or(0));
    }
    Some(out)
}

/// 抛出宿主侧的归一标签(差分只比错误类别,消息不参与)
fn throw(ctx: &Ctx<'_>, msg: &str) -> rquickjs::Error {
    ctx.throw(
        rquickjs::String::from_str(ctx.clone(), msg)
            .map(Value::from_string)
            .unwrap_or(Value::new_null(ctx.clone())),
    )
}

fn opt_str(o: &Opt<Value<'_>>) -> Option<String> {
    let v = o.0.as_ref()?;
    if v.is_undefined() || v.is_null() {
        return None;
    }
    <rquickjs::Coerced<String> as rquickjs::FromJs>::from_js(v.ctx(), v.clone()).ok().map(|c| c.0)
}

/// Rust 侧原语。对象形状(类、静态成员、`with` 的名字解析)全在
/// [`LIVE_CONNECT_JS`] 里搭 —— 与 `java_proxy` 同一分工。
/// LiveConnect 面的**入口**:只装三个懒 getter,真身留到第一次被碰时再装。
///
/// 为什么:整套 LiveConnect(12 个宿主函数 + [`LIVE_CONNECT_JS`] 那 8 KB 类面)
/// 每次求值都要重装一遍,实测 **369µs** —— 占一次 `evalJS` 的四成
/// (docs/engine-perf.md §3②)。而绝大多数书源根本不碰 `Packages` /
/// `JavaImporter` / `android`,这四成是白付的。
///
/// **语义不动**:每次求值仍是全新 Context、全新 LiveConnect 实例,只是
/// 「什么时候装」变了;碰到任何一个名字都会在读到值之前把真身装完。
/// 这与 `SharedJsScope`(复用 topScope)那条**不是**一回事 —— 那条会改跨次
/// 求值的全局可见性,仍然按「先钉语义,再谈性能」压着没做。
pub fn install_live_connect(ctx: &Ctx<'_>) -> rquickjs::Result<()> {
    static LAZY_SHIM_BC: crate::bytecode::Cached = crate::bytecode::Cached::new();
    ctx.globals().set(
        "__installLC",
        Func::from(|ctx: Ctx<'_>| -> rquickjs::Result<()> { install_live_connect_now(&ctx) }),
    )?;
    crate::bytecode::eval_cached(ctx, &LAZY_SHIM_BC, LAZY_SHIM)?;
    Ok(())
}

/// 三个名字的懒 getter。`set` 那一支要有:书源里 `Packages = x` 这种直接赋值
/// 不该被 getter 吞掉(没有 setter 的访问器属性在非严格模式下会静默丢弃赋值)。
const LAZY_SHIM: &str = r#"
(function () {
  var NAMES = ["Packages", "android", "JavaImporter"];
  var done = false;
  function ensure() {
    if (done) return;
    done = true;
    // 先摘掉访问器再装:否则 LIVE_CONNECT_JS 里的 `globalThis.Packages = P`
    // 会走上面那个 setter,多绕一圈
    for (var i = 0; i < NAMES.length; i++) delete globalThis[NAMES[i]];
    __installLC();
  }
  NAMES.forEach(function (n) {
    Object.defineProperty(globalThis, n, {
      configurable: true,
      enumerable: true,
      get: function () { ensure(); return globalThis[n]; },
      set: function (v) {
        Object.defineProperty(globalThis, n,
          { value: v, writable: true, enumerable: true, configurable: true });
      }
    });
  });
})();
"#;

fn install_live_connect_now(ctx: &Ctx<'_>) -> rquickjs::Result<()> {
    let lc = Object::new(ctx.clone())?;

    // ---- java.lang.String 的字节面 ----
    lc.set(
        "strBytes",
        Func::from(
            |ctx: Ctx<'_>,
             s: rquickjs::Coerced<String>,
             cs: Opt<Value<'_>>|
             -> rquickjs::Result<Vec<u8>> {
                java_api::encode_with_charset(&s.0, opt_str(&cs).as_deref())
                    .map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;
    lc.set(
        "bytesStr",
        Func::from(|ctx: Ctx<'_>, b: Value<'_>, cs: Opt<Value<'_>>| -> rquickjs::Result<String> {
            java_api::decode_with_charset(&read_bytes(&b), opt_str(&cs).as_deref())
                .map_err(|e| throw(&ctx, &e))
        }),
    )?;

    // ---- android.util.Base64 / java.util.Base64 ----
    lc.set(
        "b64AndroidDecode",
        Func::from(|ctx: Ctx<'_>, v: Value<'_>| -> rquickjs::Result<Vec<u8>> {
            // 裁判垫片先把 byte[] 按 ISO-8859-1 造串再解 —— 等价于逐字节
            let raw = read_bytes(&v);
            let s: String = raw.iter().map(|&b| b as char).collect();
            java_api::base64_decode_bytes(&s).map_err(|e| throw(&ctx, &e))
        }),
    )?;
    lc.set(
        "b64AndroidEncode",
        Func::from(|v: Value<'_>, flags: i32| -> String {
            java_api::base64_encode_bytes(&read_bytes(&v), flags)
        }),
    )?;
    // java.util.Base64 的基本解码器是**严格**的:表外字符抛 IllegalArgumentException
    // (与 android 那张 MIME 解码器的表不同,书源分得出来)
    lc.set(
        "b64UtilDecode",
        Func::from(|ctx: Ctx<'_>, v: Value<'_>, url_safe: bool| -> rquickjs::Result<Vec<u8>> {
            use base64::Engine as _;
            let raw = read_bytes(&v);
            let s: String = raw.iter().map(|&b| b as char).collect();
            let body = s.trim_end_matches('=');
            let alphabet =
                if url_safe { &base64::alphabet::URL_SAFE } else { &base64::alphabet::STANDARD };
            let engine = base64::engine::general_purpose::GeneralPurpose::new(
                alphabet,
                base64::engine::general_purpose::NO_PAD,
            );
            engine
                .decode(body.as_bytes())
                .map_err(|_| throw(&ctx, "«host:IllegalArgumentException»"))
        }),
    )?;
    lc.set(
        "b64UtilEncode",
        Func::from(|v: Value<'_>, url_safe: bool, padding: bool| -> String {
            let mut flags = java_api::B64_NO_WRAP;
            if url_safe {
                flags |= java_api::B64_URL_SAFE;
            }
            if !padding {
                flags |= java_api::B64_NO_PADDING;
            }
            java_api::base64_encode_bytes(&read_bytes(&v), flags)
        }),
    )?;

    // ---- javax.crypto ----
    // getInstance / init / doFinal **分三步**,错误落在与真身相同的那一步
    lc.set(
        "cipherCheck",
        Func::from(|ctx: Ctx<'_>, t: rquickjs::Coerced<String>| -> rquickjs::Result<()> {
            // 密钥留到 init 才查,这里只认 transformation:给一把合法长度的假钥匙
            java_api::SymmetricCrypto::new_jce(&t.0, &[0u8; 16], None).map(|_| ()).or_else(|e| {
                if e.contains("NoSuchAlgorithm") { Err(throw(&ctx, &e)) } else { Ok(()) }
            })
        }),
    )?;
    lc.set(
        "cipherInit",
        Func::from(
            |ctx: Ctx<'_>,
             t: rquickjs::Coerced<String>,
             key: Value<'_>,
             iv: Value<'_>|
             -> rquickjs::Result<()> {
                let iv = read_bytes(&iv);
                let c = java_api::SymmetricCrypto::new_jce(
                    &t.0,
                    &read_bytes(&key),
                    if iv.is_empty() { None } else { Some(&iv) },
                )
                .map_err(|e| throw(&ctx, &e))?;
                c.check_init().map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;
    lc.set(
        "cipherDoFinal",
        Func::from(
            |ctx: Ctx<'_>,
             t: rquickjs::Coerced<String>,
             encrypt: bool,
             key: Value<'_>,
             iv: Value<'_>,
             data: Value<'_>|
             -> rquickjs::Result<Vec<u8>> {
                let iv = read_bytes(&iv);
                let c = java_api::SymmetricCrypto::new_jce(
                    &t.0,
                    &read_bytes(&key),
                    if iv.is_empty() { None } else { Some(&iv) },
                )
                .map_err(|e| throw(&ctx, &e))?;
                let data = read_bytes(&data);
                if encrypt { c.encrypt(&data) } else { c.decrypt(&data) }
                    .map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;
    lc.set(
        "mac",
        Func::from(
            |ctx: Ctx<'_>,
             alg: rquickjs::Coerced<String>,
             key: Value<'_>,
             data: Value<'_>|
             -> rquickjs::Result<Vec<u8>> {
                java_api::hmac_bytes(
                    &alg.0,
                    &read_bytes(&key),
                    &read_bytes(&data),
                    CryptoFlavor::Jce,
                )
                .map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;
    lc.set(
        "digest",
        Func::from(
            |ctx: Ctx<'_>,
             alg: rquickjs::Coerced<String>,
             data: Value<'_>|
             -> rquickjs::Result<Vec<u8>> {
                java_api::jce_digest(&alg.0, &read_bytes(&data)).map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;

    // ---- java.security 的 RSA 一支(KeyFactory / Signature)----
    lc.set(
        "rsaSign",
        Func::from(
            |ctx: Ctx<'_>,
             alg: rquickjs::Coerced<String>,
             key: Value<'_>,
             data: Value<'_>|
             -> rquickjs::Result<Vec<u8>> {
                java_api::rsa_sign(&alg.0, &read_bytes(&key), &read_bytes(&data))
                    .map_err(|e| throw(&ctx, &e))
            },
        ),
    )?;
    ctx.globals().set("__lc", lc)?;
    static LIVE_CONNECT_BC: crate::bytecode::Cached = crate::bytecode::Cached::new();
    crate::bytecode::eval_cached(ctx, &LIVE_CONNECT_BC, LIVE_CONNECT_JS)?;
    Ok(())
}

/// 类与包的对象面。**装在 PRELUDE 之后** —— 它要覆盖 PRELUDE 里
/// `JavaImporter`/`Packages` 的空壳。
pub const LIVE_CONNECT_JS: &str = r#"
(function () {
  var JS_STRING = globalThis.String;      // with 块里 String 会被 java.lang.String 遮住
  var AMBIGUOUS = "«java-ambiguous-import»";   // Rhino 的 EvaluatorException → js:SyntaxError

  function isBytes(v) { return !!v && v.__javaKind === "array"; }
  function bytes(v) { return __java.bytes(v); }
  // 参数里的「Java 串或 JS 串」一律取原始文本(JavaString 走 Symbol.toPrimitive)
  function text(v) { return v === undefined || v === null ? v : JS_STRING(v); }
  function byteArg(v) { return isBytes(v) || Array.isArray(v) ? v : (v === undefined || v === null ? [] : v); }

  // ---- java.lang.String ----------------------------------------------------
  // Rhino 里 Java 类既是构造函数也是普通函数:`String(x)` 与 `new String(x)` 同义。
  // 收 byte[] 时是**解码**(默认字符集 UTF-8),收别的就是字符串化。
  function JavaLangString(v, charset) {
    if (isBytes(v)) return __java.str(__lc.bytesStr(v, charset === undefined ? undefined : text(charset)));
    return __java.str(v === undefined ? "" : JS_STRING(v));
  }
  JavaLangString.valueOf = function (v) { return JavaLangString(v); };

  // ---- java.util.Arrays ----------------------------------------------------
  var Arrays = function () {};
  Arrays.copyOfRange = function (a, from, to) {
    var out = [];
    for (var i = from; i < to; i++) out.push(i < a.length ? a[i] : 0);
    return bytes(out.map(function (b) { return b < 0 ? b + 256 : b; }));
  };
  Arrays.copyOf = function (a, len) { return Arrays.copyOfRange(a, 0, len); };
  Arrays.toString = function (a) {
    var s = [];
    for (var i = 0; i < a.length; i++) s.push(a[i]);
    return __java.str("[" + s.join(", ") + "]");
  };

  // ---- android.util.Base64 -------------------------------------------------
  var AndroidBase64 = function () {};
  AndroidBase64.DEFAULT = 0;
  AndroidBase64.NO_PADDING = 1;
  AndroidBase64.NO_WRAP = 2;
  AndroidBase64.CRLF = 4;
  AndroidBase64.URL_SAFE = 8;
  AndroidBase64.NO_CLOSE = 16;
  // 解码不看 flags(AOSP 的解码表两套字母表都认),与裁判垫片一致
  AndroidBase64.decode = function (input, flags) {
    return bytes(__lc.b64AndroidDecode(isBytes(input) ? input : text(input)));
  };
  AndroidBase64.encode = function (input, flags) {
    return bytes(__lc.strBytes(__lc.b64AndroidEncode(byteArg(input), flags | 0), "ISO-8859-1"));
  };
  AndroidBase64.encodeToString = function (input, flags) {
    return __java.str(__lc.b64AndroidEncode(byteArg(input), flags | 0));
  };

  // ---- java.util.Base64 ----------------------------------------------------
  // 与 android 那张表**不是一个解码器**:基本解码器严格,表外字符抛
  // IllegalArgumentException;MIME 解码器才跳过表外字符。
  function utilDecoder(urlSafe, mime) {
    return {
      decode: function (v) {
        if (mime) return bytes(__lc.b64AndroidDecode(isBytes(v) ? v : text(v)));
        return bytes(__lc.b64UtilDecode(isBytes(v) ? v : text(v), urlSafe));
      }
    };
  }
  function utilEncoder(urlSafe, padding) {
    var enc = {
      encode: function (v) { return bytes(__lc.strBytes(__lc.b64UtilEncode(byteArg(v), urlSafe, padding), "ISO-8859-1")); },
      encodeToString: function (v) { return __java.str(__lc.b64UtilEncode(byteArg(v), urlSafe, padding)); },
      withoutPadding: function () { return utilEncoder(urlSafe, false); }
    };
    return enc;
  }
  var UtilBase64 = function () {};
  UtilBase64.getDecoder = function () { return utilDecoder(false, false); };
  UtilBase64.getUrlDecoder = function () { return utilDecoder(true, false); };
  UtilBase64.getMimeDecoder = function () { return utilDecoder(false, true); };
  UtilBase64.getEncoder = function () { return utilEncoder(false, true); };
  UtilBase64.getUrlEncoder = function () { return utilEncoder(true, true); };
  UtilBase64.getMimeEncoder = function () { return utilEncoder(false, true); };

  // ---- javax.crypto.spec ---------------------------------------------------
  function SecretKeySpec(key, algorithm) {
    return {
      __keyBytes: byteArg(key),
      __algorithm: text(algorithm) || "",
      getAlgorithm: function () { return __java.str(this.__algorithm); },
      getEncoded: function () { return bytes(this.__keyBytes.map(function (b) { return b < 0 ? b + 256 : b; })); },
      getFormat: function () { return __java.str("RAW"); }
    };
  }
  function IvParameterSpec(iv) {
    return {
      __ivBytes: byteArg(iv),
      getIV: function () { return bytes(this.__ivBytes.map(function (b) { return b < 0 ? b + 256 : b; })); }
    };
  }
  // DESKeySpec(byte[]) 取前 8 字节;不足 8 是 InvalidKeyException(JCE 的规矩)
  function desKeySpec(key, len, algorithm) {
    var k = byteArg(key);
    if (k.length < len) throw "«host:InvalidKeyException»";
    return { __keyBytes: Array.prototype.slice.call(k, 0, len), __algorithm: algorithm };
  }
  function DESKeySpec(key) { return desKeySpec(key, 8, "DES"); }
  function DESedeKeySpec(key) { return desKeySpec(key, 24, "DESede"); }

  // ---- javax.crypto --------------------------------------------------------
  var Cipher = function () {};
  Cipher.ENCRYPT_MODE = 1;
  Cipher.DECRYPT_MODE = 2;
  Cipher.WRAP_MODE = 3;
  Cipher.UNWRAP_MODE = 4;
  Cipher.getInstance = function (transformation) {
    var t = text(transformation);
    __lc.cipherCheck(t);
    return {
      __t: t, __mode: 1, __key: [], __iv: [],
      // init(mode, key[, ivSpec]) —— 密钥长度与 IV 长度在这一步炸,
      // 与真身同一行(InvalidKeyException / InvalidAlgorithmParameterException)
      init: function (mode, key, params) {
        this.__mode = mode | 0;
        this.__key = (key && key.__keyBytes) || byteArg(key);
        this.__iv = (params && params.__ivBytes) || [];
        __lc.cipherInit(this.__t, this.__key, this.__iv);
      },
      doFinal: function (data) {
        return bytes(__lc.cipherDoFinal(this.__t, this.__mode === 1, this.__key, this.__iv, byteArg(data)));
      },
      getIV: function () { return bytes(this.__iv.map(function (b) { return b < 0 ? b + 256 : b; })); },
      getBlockSize: function () { return /^AES/i.test(this.__t) ? 16 : 8; }
    };
  };

  var Mac = function () {};
  Mac.getInstance = function (algorithm) {
    var a = text(algorithm);
    return {
      __a: a, __key: [], __buf: [],
      init: function (key) { this.__key = (key && key.__keyBytes) || byteArg(key); },
      update: function (data) { this.__buf = this.__buf.concat(Array.prototype.slice.call(byteArg(data))); },
      doFinal: function (data) {
        var all = data === undefined ? this.__buf : this.__buf.concat(Array.prototype.slice.call(byteArg(data)));
        return bytes(__lc.mac(this.__a, this.__key, all));
      }
    };
  };

  var SecretKeyFactory = function () {};
  SecretKeyFactory.getInstance = function (algorithm) {
    var a = text(algorithm);
    return {
      generateSecret: function (spec) {
        return { __keyBytes: (spec && spec.__keyBytes) || [], __algorithm: (spec && spec.__algorithm) || a };
      }
    };
  };

  // ---- java.security -------------------------------------------------------
  var MessageDigest = function () {};
  MessageDigest.getInstance = function (algorithm) {
    var a = text(algorithm);
    return {
      __a: a, __buf: [],
      update: function (data) { this.__buf = this.__buf.concat(Array.prototype.slice.call(byteArg(data))); },
      reset: function () { this.__buf = []; },
      digest: function (data) {
        var all = data === undefined ? this.__buf : this.__buf.concat(Array.prototype.slice.call(byteArg(data)));
        this.__buf = [];
        return bytes(__lc.digest(this.__a, all));
      }
    };
  };

  // ---- java.security.spec / KeyFactory / Signature --------------------------
  // 书源真的直接调这一支(语料 2 个源:西瓜小说的 `sign` 请求头)。
  // 形状照真身:`KeyFactory.getInstance("RSA").generatePrivate(spec)` 交回一个
  // **Key 对象**,`Signature` 上再 initSign/update/sign。算在 __lc.rsaSign 里,
  // 这里只搭状态与 DER 字节的传递。**编码错在 generatePrivate 那一步**(真身
  // 是 InvalidKeySpecException),所以 Key 对象只揣着字节、到签名时才炸不行 ——
  // 但真身也确实是懒的(私钥的合法性 KeyFactory 就查),口径差别记在这里:
  // 被测侧把校验推到 sign(),错误类别仍是 InvalidKeySpecException。
  function PKCS8EncodedKeySpec(der) {
    return { __keyDer: byteArg(der), __keyFormat: "PKCS#8",
             getEncoded: function () { return bytes(byteArg(der)); },
             getFormat: function () { return __java.str("PKCS#8"); } };
  }
  function rsaKey(spec, algorithm) {
    return { __keyDer: (spec && spec.__keyDer) || [], __algorithm: algorithm,
             getAlgorithm: function () { return __java.str(algorithm); },
             getEncoded: function () { return bytes(this.__keyDer); },
             getFormat: function () { return __java.str((spec && spec.__keyFormat) || ""); } };
  }
  var KeyFactory = function () {};
  KeyFactory.getInstance = function (algorithm) {
    var a = text(algorithm);
    return {
      generatePrivate: function (spec) { return rsaKey(spec, a); },
      getAlgorithm: function () { return __java.str(a); }
    };
  };
  var Signature = function () {};
  Signature.getInstance = function (algorithm) {
    var a = text(algorithm);
    return {
      __a: a, __key: [], __buf: [],
      initSign: function (key) { this.__key = (key && key.__keyDer) || byteArg(key); this.__buf = []; },
      update: function (data) { this.__buf = this.__buf.concat(Array.prototype.slice.call(byteArg(data))); },
      sign: function () { return bytes(__lc.rsaSign(this.__a, this.__key, this.__buf)); }
    };
  };

  // ---- 包树 ----------------------------------------------------------------
  // Rhino 里 `Packages.java.lang` 与全局 `org.jsoup` 是同一条路;`java` 不是包
  // (它是 AnalyzeRule),所以只有 `Packages.java` 有 lang/util/security。
  var P = {
    java: {
      lang: { String: JavaLangString },
      util: { Base64: UtilBase64, Arrays: Arrays },
      io: {},
      security: {
        MessageDigest: MessageDigest, KeyFactory: KeyFactory, Signature: Signature,
        spec: {
          PKCS8EncodedKeySpec: PKCS8EncodedKeySpec
        }
      }
    },
    javax: {
      crypto: {
        Cipher: Cipher, Mac: Mac, SecretKeyFactory: SecretKeyFactory,
        spec: {
          SecretKeySpec: SecretKeySpec, IvParameterSpec: IvParameterSpec,
          DESKeySpec: DESKeySpec, DESedeKeySpec: DESedeKeySpec
        }
      }
    },
    android: { util: { Base64: AndroidBase64, Log: { d: function () {}, e: function () {}, i: function () {} } } }
  };
  P.java.security.interfaces = {};
  if (globalThis.org) P.org = globalThis.org;      // org.jsoup(install_liveconnect 装的)
  globalThis.Packages = P;
  globalThis.android = P.android;

  // ---- JavaImporter --------------------------------------------------------
  // `with (javaImport)` 靠 Proxy 的 has/get 解析名字 —— 与 Rhino 一样是**懒的**:
  // 同名类冲突要等到真去取那个名字才炸(探针 lc-ambiguous)。
  function importerTarget() {
    var pkgs = [], classes = {};
    var self = {
      importPackage: function () {
        for (var i = 0; i < arguments.length; i++) if (arguments[i]) pkgs.push(arguments[i]);
      },
      importClass: function () {
        for (var i = 0; i < arguments.length; i++) {
          var c = arguments[i];
          if (c && c.__javaClassName) classes[c.__javaClassName] = c;
        }
      },
      __lookup: function (name) {
        if (Object.prototype.hasOwnProperty.call(classes, name)) return { v: classes[name] };
        var found, n = 0;
        for (var i = 0; i < pkgs.length; i++) {
          var p = pkgs[i];
          if (p && Object.prototype.hasOwnProperty.call(p, name)) {
            if (found !== p[name]) { n += 1; found = p[name]; }
          }
        }
        if (n > 1) throw AMBIGUOUS;
        return n === 1 ? { v: found } : null;
      }
    };
    return self;
  }
  globalThis.JavaImporter = function () {
    var t = importerTarget();
    t.importPackage.apply(t, arguments);
    return new Proxy(t, {
      has: function (target, key) {
        if (typeof key !== "string") return false;          // Symbol.unscopables 等
        if (key in target) return true;
        return !!target.__lookup(key);
      },
      get: function (target, key) {
        if (typeof key !== "string") return undefined;
        if (key in target) return target[key];
        var hit = target.__lookup(key);
        return hit ? hit.v : undefined;
      }
    });
  };
})();
"#;
