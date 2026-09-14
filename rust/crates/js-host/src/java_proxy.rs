//! **Java 对象代理层**:`java.*` 的返回值在真身里不是 JS 原生值。
//!
//! 真身的 `java` 是 Kotlin 对象,Rhino 通过 LiveConnect 把返回值包成
//! `NativeJavaObject`。观察到的后果(全部由 jsharness 探针实测,契约见
//! `fixtures/cases/js-host/README.md`「Java 对象代理层」):
//!
//! | 表达式 | 真身 | 朴素 JS |
//! |---|---|---|
//! | `typeof java.md5Encode('a')` | `"object"` | `"string"` |
//! | `java.md5Encode('a') === '0cc…'` | `false` | `true` |
//! | `java.md5Encode('a') == '0cc…'` | `true` | `true` |
//! | `java.get('k').replace('a','b')` | Java 语义:**全部替换、字面量** | 只换第一个 |
//! | `java.get('k').charAt(0)` | **数字**(Java `char`) | 单字符串 |
//! | `java.get('k').length` | Java **方法对象** | 数字 |
//! | `java.get('k').length()` | 数字 | TypeError |
//! | `java.md5Encode('a') instanceof String` | `true` | `false` |
//! | `java.get('k').slice(1,3)` | 可用(JS 原型兜底) | 可用 |
//! | `java.getStringList(r).join('|')` | **TypeError**(Java List 没有 join) | 可用 |
//! | `String(java.getStringList(r))` | `[a, b]`(Java List toString) | `a,b` |
//!
//! 即:**Java 方法优先,JS 原型兜底**。Rhino 把 `java.lang.String` 的包装对象的
//! 原型链接到 `String.prototype`,所以 `slice`/`match` 这些 Java 没有的名字还能用,
//! 但 `replace`/`split`/`length` 这些**两边都有**的名字走 Java 的那一份。
//!
//! **完成值不装箱**:`RhinoScriptEngine.eval` 在顶层把 `Wrapper` 解包,所以
//! `java.md5Encode('a')` 作为整段 JS 的完成值时又变回普通字符串(探针 `ret-md5`)。
//! 装箱只在**表达式中间**可观察 —— 这一位是本层存在的全部理由。
//!
//! 被测侧的做法:装箱在 **JS 侧**做(本文件的 [`JAVA_PROXY`]),Rust 只管返回原始值;
//! `java` 上哪个方法返回哪种 Java 类型由 [`RETURN_KINDS`] 这张表说了算。

/// `java.*` 各方法的 **Java 返回类型**,决定装哪种箱。
///
/// 不在表里的名字**不装箱**(返回 JS 原生值)——对应真身里返回
/// `int`/`boolean`/`void` 的那些(Rhino 对 Java 基本类型不包装)。
/// 表要跟着 `host_env::install_java` 一起长。
#[rustfmt::skip] // 手排的分列表:rustfmt 会拆成一行一个
pub const RETURN_KINDS: &[(&str, &str)] = &[
    // → java.lang.String
    ("md5Encode", "str"),
    ("md5Encode16", "str"),
    ("base64Encode", "str"),
    ("base64Decode", "str"),
    ("hexEncodeToString", "str"),
    ("hexDecodeToString", "str"),
    ("encodeURI", "str"),
    ("timeFormat", "str"),
    ("timeFormatUTC", "str"),
    ("androidId", "str"),
    ("getWebViewUA", "str"),
    ("t2s", "str"),
    ("s2t", "str"),
    ("put", "str"),
    ("get", "str"),
    ("log", "str"),
    // 规则反调面(只有 AnalyzeRule 宿主装):getString 返回 java.lang.String
    ("getString", "str"),
    // **网络面**:`JsExtensions.ajax` / `AnalyzeRule.ajax` 都是 `String?`,
    // 和 md5Encode 走完全同一条 LiveConnect 路径(RhinoWrapFactory 没覆写
    // `wrap()`、`javaPrimitiveWrap` 是默认的 true)。于是
    // `java.ajax(u).replace(a,b)` 在真身里是 **Java 的 replace**(全部替换、
    // 按字面量),`.length` 是方法对象而不是数字。
    // 差分套此前照不到:顶层完成值会解包,而 hand.json 里 ajax 只有一条裸完成值。
    ("ajax", "str"),
    // BaseSource 那一面(只有 Source 宿主装):三个都是 Kotlin String / String?
    // (null 由装箱包装器原样放行)
    ("getKey", "str"),
    ("getTag", "str"),
    ("getVariable", "str"),
    ("getLoginHeader", "str"),
    ("getLoginInfo", "str"),
    ("digestHex", "str"),
    ("HMacHex", "str"),
    ("HMacBase64", "str"),
    ("randomUUID", "str"),
    ("getCookie", "str"),
    ("toNumChapter", "str"),
    ("bytesToStr", "str"),
    // 对称加解密的一排薄壳:真身都返回 java.lang.String
    ("aesDecodeToString", "str"),
    ("aesBase64DecodeToString", "str"),
    ("desDecodeToString", "str"),
    ("desBase64DecodeToString", "str"),
    ("aesEncodeToString", "str"),
    ("desEncodeToString", "str"),
    ("aesEncodeToBase64String", "str"),
    ("desEncodeToBase64String", "str"),
    // → byte[]
    ("strToBytes", "bytes"),
    ("base64DecodeToByteArray", "bytes"),
];

/// 装箱层的 JS 实现。在 `install_java` 之后、书源代码之前求值。
///
/// 三种箱:
/// - `str` —— `java.lang.String`:原型链接到 `String.prototype`,Java 方法覆盖同名者;
/// - `list` —— `java.util.List`:有 `size()`/`get()`/`forEach`/下标/`length`,
///   **没有** `join`/`map`(真身在这里就是 TypeError);
/// - `bytes` —— Java 数组:是**真 JS 数组**(`map`/`join`/下标/`length` 都可用,
///   真身的 `NativeJavaArray` 同样挂 `Array.prototype`),
///   只把 `toString` 换成 Java 的身份形态。
pub const JAVA_PROXY: &str = r#"
(function () {
  var IDENTITY_SEQ = 0;
  // Java 对象默认 toString 的形态:`[B@1f2e3d4`。两侧都归一成 «identity»
  // (逐次运行都不同,不可能逐字对齐),见 README「已归一的差异」。
  function identityString(prefix) {
    IDENTITY_SEQ += 1;
    return prefix + "@" + IDENTITY_SEQ.toString(16);
  }

  // ---- java.lang.String ----------------------------------------------------
  function JavaString(v) { this.__v = String(v); }
  // 原型链接到 String.prototype:`instanceof String` 为真,slice/match 这些
  // Java 没有的名字由 JS 兜底(探针 jstr-slice / jstr-match)
  JavaString.prototype = Object.create(String.prototype);
  JavaString.prototype.constructor = JavaString;

  function defJavaString(name, fn) {
    Object.defineProperty(JavaString.prototype, name, {
      value: fn, writable: true, enumerable: false, configurable: true
    });
  }

  // 隐式强转(`String(x)` / `x + ''` / 模板串 / 正则)走 Symbol.toPrimitive ——
  // 真身里那是 Rhino 的 [[DefaultValue]],与 JS 可见的 toString() 不是一条路:
  // `typeof java.get('k').toString()` 在真身里是 **"object"**(Java 的
  // String.toString() 返回 java.lang.String,又被包了一层)。探针 jstr-tostring-typeof。
  Object.defineProperty(JavaString.prototype, Symbol.toPrimitive, {
    value: function () { return this.__v; }, enumerable: false, configurable: true
  });
  defJavaString("toString", function () { return new JavaString(this.__v); });
  // java.lang.String.getBytes([charset]):LiveConnect 那一支处处用它取字节
  defJavaString("getBytes", function (charset) {
    return __java.bytes(__lc.strBytes(this.__v, charset === undefined ? undefined : String(charset)));
  });
  defJavaString("valueOf", function () { return this.__v; });
  // JSON.stringify 给字符串而不是 {__v:…}(探针 jstr-json-stringify)
  defJavaString("toJSON", function () { return this.__v; });

  // 下面这些**两边都有**,真身走 Java 那一份
  defJavaString("length", function () { return this.__v.length; });
  defJavaString("charAt", function (i) { return this.__v.charCodeAt(i); });   // Java char → 数字
  defJavaString("replace", function (a, b) {
    // Java 的 String.replace(CharSequence,CharSequence) 与 replace(char,char)
    // 在实参是 JS 正则时**重载不明确**,Rhino 抛 EvaluatorException(→ js:SyntaxError)
    if (a instanceof RegExp) { throw new Error("«java-ambiguous-overload» replace"); }
    return new JavaString(this.__v.split(String(a)).join(String(b)));  // 字面量、全部替换
  });
  defJavaString("replaceAll", function (re, b) {
    return new JavaString(this.__v.replace(new RegExp(String(re), "g"), String(b)));
  });
  defJavaString("split", function (re) {
    return javaArray(this.__v.split(new RegExp(String(re))), "[Ljava.lang.String;");
  });
  defJavaString("substring", function (a, b) {
    return new JavaString(b === undefined ? this.__v.substring(a) : this.__v.substring(a, b));
  });
  defJavaString("indexOf", function (s, from) { return this.__v.indexOf(String(s), from); });
  defJavaString("lastIndexOf", function (s) { return this.__v.lastIndexOf(String(s)); });
  defJavaString("toUpperCase", function () { return new JavaString(this.__v.toUpperCase()); });
  defJavaString("toLowerCase", function () { return new JavaString(this.__v.toLowerCase()); });
  defJavaString("trim", function () { return new JavaString(this.__v.trim()); });
  defJavaString("concat", function (s) { return new JavaString(this.__v + String(s)); });
  // 这些是 Java 独有的名字
  defJavaString("contains", function (s) { return this.__v.indexOf(String(s)) >= 0; });
  defJavaString("startsWith", function (s) { return this.__v.indexOf(String(s)) === 0; });
  defJavaString("endsWith", function (s) {
    var t = String(s);
    return this.__v.length >= t.length && this.__v.lastIndexOf(t) === this.__v.length - t.length;
  });
  defJavaString("equals", function (o) { return this.__v === String(o); });
  defJavaString("equalsIgnoreCase", function (o) {
    return this.__v.toLowerCase() === String(o).toLowerCase();
  });
  defJavaString("matches", function (re) {
    return new RegExp("^(?:" + String(re) + ")$").test(this.__v);
  });
  defJavaString("isEmpty", function () { return this.__v.length === 0; });
  defJavaString("isBlank", function () { return this.__v.trim().length === 0; });
  defJavaString("compareTo", function (o) {
    var t = String(o);
    return this.__v < t ? -1 : (this.__v > t ? 1 : 0);
  });

  // ---- java.util.List ------------------------------------------------------
  // 真身给的是 NativeJavaList:有下标/length/size()/get()/forEach,
  // 但**没有** Array.prototype —— `join`/`map` 在真身里是 TypeError(探针 gsl-join)
  function javaList(items) {
    var o = {};
    for (var i = 0; i < items.length; i++) o[i] = items[i];
    Object.defineProperty(o, "length", { value: items.length, enumerable: false });
    Object.defineProperty(o, "__javaKind", { value: "list", enumerable: false });
    Object.defineProperty(o, "__items", { value: items, enumerable: false });
    Object.defineProperty(o, "size", { value: function () { return items.length; }, enumerable: false });
    Object.defineProperty(o, "get", { value: function (i) { return items[i]; }, enumerable: false });
    Object.defineProperty(o, "isEmpty", { value: function () { return items.length === 0; }, enumerable: false });
    Object.defineProperty(o, "contains", {
      value: function (x) { return items.indexOf(x) >= 0; }, enumerable: false
    });
    // Java 的 Iterable.forEach(Consumer) —— 这个真身有(探针 gsl-foreach)
    Object.defineProperty(o, "forEach", {
      value: function (f) { for (var i = 0; i < items.length; i++) f(items[i], i); },
      enumerable: false
    });
    // Kotlin List.toString() → `[a, b]`
    Object.defineProperty(o, "toString", {
      value: function () { return "[" + items.join(", ") + "]"; }, enumerable: false
    });
    Object.defineProperty(o, "toJSON", { value: function () { return items; }, enumerable: false });
    Object.defineProperty(o, "toArray", {
      value: function () { return javaArray(items.slice(), "[Ljava.lang.Object;"); },
      enumerable: false
    });
    return o;
  }

  // ---- Rhino 的 NativeJavaMap / NativeJavaList -----------------------------
  // jayway / gson 读出来的 Map/List 交给 JS 时,Rhino 的 WrapFactory **逐层**
  // 包装。与上面那个 `javaList`(java.getStringList 那一支)不同的三处,
  // 全由 jsharness 探针 `js-bind-*` 实测:
  //   1. **没有 Object.prototype** —— `hasOwnProperty` 是 undefined、
  //      `instanceof Object` 为假(故用 Object.create(null));
  //   2. `for..in` 只给键(下标)—— 附加面一律不可枚举;
  //   3. **取出来的每个值又各自装箱** —— `typeof m.n` 是 "object"、
  //      `m.n === '书名A'` 为假而 `==` 为真。
  // `toString()` 按容器的**出身**走不同口径(json-smart 的 JSONArray 是 JSON
  // 文本、gson 的 ArrayList 是 `[a, b]`),故由 Rust 侧算好传进来,不在这里重算。
  function hide(o, name, value) {
    Object.defineProperty(o, name, {
      value: value, writable: true, enumerable: false, configurable: true
    });
  }

  function nativeJavaMap(entries, str, noJson) {
    var o = Object.create(null);
    for (var i = 0; i < entries.length; i++) o[entries[i][0]] = entries[i][1];
    function find(k) {
      k = String(k);
      for (var i = 0; i < entries.length; i++) if (entries[i][0] === k) return entries[i];
      return null;
    }
    hide(o, "toString", function () { return str; });
    hide(o, "size", function () { return entries.length; });
    hide(o, "isEmpty", function () { return entries.length === 0; });
    // Java 的 Map.get:取不到给 **null**(不是 undefined)
    hide(o, "get", function (k) { var e = find(k); return e === null ? null : e[1]; });
    hide(o, "containsKey", function (k) { return find(k) !== null; });
    hide(o, "__javaKind", "javaobj");
    if (noJson) hide(o, "__javaNoJson", true);
    return o;
  }

  function nativeJavaList(items, str, noJson) {
    var o = Object.create(null);
    for (var i = 0; i < items.length; i++) o[i] = items[i];
    hide(o, "length", items.length);
    hide(o, "toString", function () { return str; });
    hide(o, "size", function () { return items.length; });
    hide(o, "isEmpty", function () { return items.length === 0; });
    hide(o, "get", function (i) { return i >= 0 && i < items.length ? items[i] : null; });
    hide(o, "contains", function (x) { return items.indexOf(x) >= 0; });
    hide(o, "forEach", function (f) { for (var i = 0; i < items.length; i++) f(items[i], i); });
    // 没有 Array.prototype 的对象在 JSON.stringify 下会被当**对象**序列化成
    // `{"0":…}` —— 交回一个真数组把形态摆正(项各自的 toJSON 继续递归)
    hide(o, "toJSON", function () { return items.slice(); });
    hide(o, "__javaKind", "javalist");
    if (noJson) hide(o, "__javaNoJson", true);
    return o;
  }

  // 装箱的 java.lang.Boolean:`typeof` 是 "object",而且 **`if` 里恒为真**
  // —— 连 `false` 也是(探针 js-bind-map-bool-if-false 实测裁判给 'T':
  // Rhino 的 ScriptRuntime.toBoolean 在 ECMA1 版本上对 Scriptable 直接 return
  // true,不解包)。JS 的 ToBoolean 对任何对象也给真,两边正好同形。
  function javaBoolean(v) {
    var o = Object.create(null);
    hide(o, "__v", v);
    hide(o, "toString", function () { return v ? "true" : "false"; });
    hide(o, "valueOf", function () { return v; });
    hide(o, "toJSON", function () { return v; });
    hide(o, "booleanValue", function () { return v; });
    return o;
  }

  // 装箱的 java.lang.Integer / Double:`typeof` 是 "object",`String(x)` 走
  // Java 的 toString,算术走 valueOf。顶层完成值由 `__v` 解包(与 JavaString 同)。
  function javaNumber(v, str) {
    var o = Object.create(null);
    hide(o, "__v", v);
    hide(o, "toString", function () { return str; });
    hide(o, "valueOf", function () { return v; });
    hide(o, "toJSON", function () { return v; });
    hide(o, "intValue", function () { return v; });
    hide(o, "longValue", function () { return v; });
    hide(o, "doubleValue", function () { return v; });
    return o;
  }

  // ---- Java 数组 -----------------------------------------------------------
  // 真身的 NativeJavaArray 挂 Array.prototype(探针 arr-split-map / ges-toarray),
  // 只有 toString 是 Java 的身份形态
  function javaArray(items, typeName) {
    // **不是** JS 数组:真身的 NativeJavaArray 在 `Array.isArray` 下为 false
    // (探针 lc-bytes-isarray),但原型是 Array.prototype —— join/map/下标/length
    // 都可用,`for (var i in a)` 只给下标。
    var a = Object.create(Array.prototype);
    for (var i = 0; i < items.length; i++) a[i] = items[i];
    Object.defineProperty(a, "length", { value: items.length, writable: true, enumerable: false });
    var ident = identityString(typeName);
    Object.defineProperty(a, "__javaKind", { value: "array", enumerable: false });
    Object.defineProperty(a, "toString", { value: function () { return ident; }, enumerable: false });
    // GSON 把 byte[] 序列化成数字数组(裁判侧 norm 的口径)
    Object.defineProperty(a, "toJSON", { value: function () { return items.slice(); }, enumerable: false });
    return a;
  }

  // Rust 侧建的元素代理:方法与标记都是可枚举的,而真身的 NativeJavaList
  // `for (var i in els)` 只给下标。建完调一次本函数把非下标属性压成不可枚举。
  function sealProxy(o) {
    Object.getOwnPropertyNames(o).forEach(function (k) {
      if (/^\d+$/.test(k)) return;                 // 下标留给 for..in
      var d = Object.getOwnPropertyDescriptor(o, k);
      if (d && d.configurable) { d.enumerable = false; Object.defineProperty(o, k, d); }
    });
    return o;
  }

  globalThis.__java = {
    sealProxy: sealProxy,
    str: function (v) { return v === null || v === undefined ? v : new JavaString(v); },
    list: function (items) { return javaList(items); },
    // 对象数组(目前只有 `java.ajaxAll` → `Array<StrResponse>`):
    // Rust 侧建好元素、类型名由调用方给,箱子的形态与 bytes 那一支同一份
    objArray: function (items, typeName) { return javaArray(items, typeName); },
    // Rhino 的 **NativeJavaMap / NativeJavaList**:jayway 读出来的 Map/List
    // 在 JS 里按键(下标)取属性、`for..in` 只给键;`toString()` 是 **Java 的**
    // (`{k=v, k2=v2}` / json-smart 的 JSON 文本,由 Rust 侧按真身口径给),
    // 而它与 jsoup 的 Element 相反 —— **过得了 GSON**。
    // `value` 就是解析好的 JS 对象/数组:键与下标是它自己的,附加面全设成不可枚举。
    // 见上:Rhino 的 NativeJavaMap / NativeJavaList / 装箱的数
    nativeJavaMap: function (entries, str, noJson) {
      return nativeJavaMap(entries, str, noJson);
    },
    nativeJavaList: function (items, str, noJson) {
      return nativeJavaList(items, str, noJson);
    },
    num: function (v, str) { return javaNumber(v, str); },
    bool: function (v) { return javaBoolean(v); },
    javaObj: function (value, str) {
      Object.defineProperty(value, "__javaKind", { value: "javaobj", enumerable: false });
      Object.defineProperty(value, "toString", {
        value: function () { return str; }, enumerable: false, configurable: true
      });
      return value;
    },
    // Java 的 byte 是**有符号**的:`String('中').getBytes()[0] === -28`
    // (探针 lc-bytes-signed)。Rust 侧一律给 0..255,签名这一步只此一处。
    bytes: function (items) {
      var out = [];
      for (var i = 0; i < items.length; i++) { var b = items[i]; out.push(b > 127 ? b - 256 : b); }
      return javaArray(out, "[B");
    },
    isProxy: function (v) {
      return v instanceof JavaString || (v && (v.__javaKind === "list" || v.__javaKind === "array"));
    },
    JavaString: JavaString
  };
})();
"#;

/// 按 [`RETURN_KINDS`] 把 `java` 上的方法就地包一层装箱。
/// 在 [`JAVA_PROXY`] 与 `install_java` 之后求值。
/// 装箱包装器的源码。每次求值都要跑一遍,但**串本身只拼一次**
/// (拼串实测 3.7µs/次 —— 不多,可它是纯白付的)。
pub fn box_returns_js() -> &'static str {
    static SRC: std::sync::LazyLock<String> = std::sync::LazyLock::new(build_box_returns_js);
    &SRC
}

fn build_box_returns_js() -> String {
    let mut table = String::from("[");
    for (name, kind) in RETURN_KINDS {
        table.push_str(&format!("[\"{name}\",\"{kind}\"],"));
    }
    table.push(']');
    format!(
        r#"
(function () {{
  var TABLE = {table};
  TABLE.forEach(function (e) {{
    var name = e[0], kind = e[1];
    var orig = java[name];
    if (typeof orig !== "function") return;
    java[name] = function () {{
      var v = orig.apply(java, arguments);
      if (v === null || v === undefined) return v;
      // **只装标量**:`java.get` 按 arity 分派,两个实参那一支返回的是
      // jsoup 的 `Connection.Response`(对象,有 body()/headers()),
      // 不是 String —— 照装就把整个对象拍成 JavaString,`.body()` 成了
      // not a function(pipeline-corpus-b pb00578)。真身那边装箱是按
      // **声明返回类型**走的,重载的两支各装各的。
      if (kind === "str") return typeof v === "string" ? __java.str(v) : v;
      if (kind === "list") return __java.list(v);
      if (kind === "bytes") return __java.bytes(v);
      return v;
    }};
  }});
}})();
"#
    )
}
