#!/usr/bin/env python3
"""js-host 差分用例生成器。

两个来源:
  1. **语料**:fixtures/corpus-js/*.js —— 从 1704 个真实书源抽出的去重 JS 片段
     (tools/extract_js.py 产出)。绑定按片段的来源规则位挑(搜索 URL 给 key/page,
     正文规则给 result/src/baseUrl,等等),见 BIND_BY_PATH。
  2. **手写**:方言探针 + java.* 宿主 API 面。方言探针钉 Rhino(htmlunit-core-js)
     与 QuickJS 的差异点;宿主 API 面按语料频次覆盖(java.ajax 116 源 /
     java.put 92 / java.getString 87 / java.timeFormat 68 / …)。

契约见 fixtures/cases/js-host/README.md。
"""
import hashlib
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent
CORPUS = ROOT / "fixtures/corpus-js"
OUT = ROOT / "fixtures/cases/js-host"

# 片段的来源规则位 → **宿主 + 绑定面**。真身有**两个** JS 宿主,绑定面不同:
#   host="url"  → AnalyzeUrl.evalJS  (searchUrl/exploreUrl):绑 key/page/baseUrl/
#                 book/source/cookie/cache;**没有** src/title/nextChapterUrl/
#                 fromBookInfo,`java` 也**没有** getString/getElement/setContent。
#   host="rule" → AnalyzeRule.evalJS (其余规则位):绑 result/baseUrl/src/title/
#                 nextChapterUrl/chapter/fromBookInfo/page;`java` 是 AnalyzeRule
#                 本身,能反过来调规则求值。
# 挑错宿主会把语义钉歪(searchUrl 片段里的 `key` 在 AnalyzeRule 上是 ReferenceError,
# 而产品里它必须有值)——见 fixtures/cases/js-host/README.md「两个宿主」。
SEARCH_BIND = {
    "host": "url",
    "key": "斗破苍穹",
    "page": "2",
    "baseUrl": "https://difftest.example.com/",
}
LIST_BIND = {
    "result": '{"list":[{"n":"书名A","u":"/b/1"},{"n":"书名B","u":"/b/2"}]}',
    "baseUrl": "https://difftest.example.com/search?k=1",
    "content": "<html><body><div class=item><a href=/b/1>书名A</a></div></body></html>",
}
FIELD_BIND = {
    "result": "第 12 章 混沌雷池",
    "baseUrl": "https://difftest.example.com/book/1",
    "content": "<html><body><div id=info><h1>书名A</h1><p>作者:某某</p></div></body></html>",
}
CONTENT_BIND = {
    "result": "正文第一段\n正文第二段",
    "baseUrl": "https://difftest.example.com/book/1/2.html",
    "content": "<html><body><div id=content>正文第一段<br>正文第二段</div></body></html>",
    "title": "第 12 章 混沌雷池",
    "nextChapterUrl": "https://difftest.example.com/book/1/3.html",
}


def bind_for(path: str) -> dict:
    if path.endswith("searchUrl") or path.endswith("exploreUrl"):
        return dict(SEARCH_BIND)
    if "ruleContent" in path:
        return dict(CONTENT_BIND)
    if path.endswith("bookList") or path.endswith("chapterList"):
        return dict(LIST_BIND)
    return dict(FIELD_BIND)


def corpus_cases() -> list:
    cases = []
    for f in sorted(CORPUS.glob("*.js")):
        text = f.read_text(encoding="utf-8", errors="replace")
        lines = text.splitlines()
        path = ""
        if lines and lines[0].startswith("// from:"):
            path = lines[0].split()[-1]
            lines = lines[1:]
        code = "\n".join(lines).strip()
        if not code:
            continue
        c = {"id": f"js-corpus-{f.stem}", "op": "js", "code": code}
        c.update(bind_for(path))
        c["srcPath"] = path
        cases.append(c)
    return cases


# ---- 手写:方言探针 ----------------------------------------------------------
# 每条钉一个具体的方言点。跑出 FAIL 不等于被测侧错 —— 先看裁判给什么,
# 再决定是移植还是进豁免清单(口径见 docs/plan.md §3)。
DIALECT = [
    # 完成值形态
    ("value-int", "1+1"),
    ("value-float", "1/3"),
    ("value-bigint-ish", "9007199254740993"),
    ("value-neg-zero", "-0"),
    ("value-infinity", "1/0"),
    ("value-nan", "0/0"),
    ("value-exp-large", "1e21"),
    ("value-exp-small", "1e-7"),
    ("value-bool", "1 > 0"),
    # **完成值:以「完成值可能为空」的语句收尾**(规范 §16.1.7 的 UpdateEmpty:
    # 空完成值留用上一条)。quickjs-ng 在 if/for/while/try 上不照办、给 undefined,
    # 被测侧靠 js-host/src/dialect.rs 的分段求值补上 —— 这一族就是那条判据。
    # `;` / `{}` / var / function 四种 quickjs 本来就对,留作对照组。
    ("cv-seq", "1; 2"),
    ("cv-if-false", "x=1; if(false){2}"),
    ("cv-if-true-empty", "x=1; if(true){}"),
    ("cv-if-else-taken", "x=1; if(false){2}else{}"),
    ("cv-if-value", "x=1; if(true){2}"),
    ("cv-if-only", "if(false){2}"),
    ("cv-for-zero", "x=1; for(var i=0;i<0;i++){9}"),
    ("cv-for-value", "x=1; for(var i=0;i<2;i++){9}"),
    ("cv-while-zero", "x=1; while(false){3}"),
    ("cv-try", "x=1; try{}finally{}"),
    ("cv-try-value", "x=1; try{4}finally{}"),
    ("cv-switch-nohit", "x=1; switch(2){case 1: 7}"),
    ("cv-do-while", "x=1; do{}while(false)"),
    ("cv-two-tails", "x=1; if(false){2} if(false){3}"),
    ("cv-tail-after-fn", "a=1;\nfunction f(){ if(a){} }\nif(a==2){}"),
    # 对照组:这四种 quickjs 给对,分段那条路不该动它们
    ("cv-empty-stmt", "x=1; ;"),
    ("cv-block-empty", "x=1; {}"),
    ("cv-var", "x=1; var y=2"),
    ("cv-func-decl", "x=1; function f(){}"),
    # 分段器不许被字符串/正则/注释里的分号骗过去
    ("cv-string-semi", "x='a;if(b){}'"),
    ("cv-regex-semi", "x=1; y=/;if\\(b\\){}/; z=2"),
    ("value-undefined", "undefined"),
    ("value-null", "null"),
    ("value-empty-string", "''"),
    ("value-array", "[1,'a',null]"),
    ("value-nested", "({a:{b:[1,2]},c:'x'})"),
    ("value-date-tostring", "typeof new Date(0).toISOString()"),
    ("value-function", "(function f(){})"),
    ("value-regexp", "/ab+c/gi.source"),
    # 字符串与正则方言
    ("str-replace-fn", "'a1b2'.replace(/\\d/g, function(m){return '['+m+']'})"),
    ("str-replace-dollar", "'ab'.replace(/(a)(b)/, '$2$1')"),
    ("str-match-named", "('2024-05'.match(/(?<y>\\d{4})-(?<m>\\d+)/)||{}).length"),
    ("str-lookbehind", "'abc'.replace(/(?<=a)b/, 'X')"),
    ("str-split-regex", "'a1b2c'.split(/\\d/).join('|')"),
    # **正则字面量里的恒等转义**:非 unicode 模式下 Annex B 允许 `\q` 这类
    # 「转义了但没有含义」的写法当作字面字符。Rhino 对 `\p` / `\P` 例外 ——
    # 它把这两个留给 Unicode 属性转义,**不带 `u` 标志也直接 SyntaxError**;
    # quickjs-ng 按 Annex B 当恒等转义。语料里真有这么写的
    # (`result.replace(/\<|\/|\p|\>/g,"\\n")`,pipeline-corpus-b 的 pb02108
    #  整条 case 因此在裁判侧是 pipeline_error 而被测侧跑得出正文)。
    # `-q` 是对照组:两侧都当恒等转义,PASS。
    ("regex-identity-escape-p", "String(/\\p/.test('p'))"),
    ("regex-identity-escape-q", "String(/\\q/.test('q'))"),
    ("str-trim", "'  x  '.trim()"),
    ("str-padstart", "typeof ''.padStart"),
    ("str-repeat", "'ab'.repeat(3)"),
    ("str-includes", "'abc'.includes('b')"),
    ("str-codepoint", "'𝌆'.length"),
    ("str-normalize", "typeof ''.normalize"),
    ("str-raw", "String.raw`a\\nb`"),
    ("str-localecompare", "'b'.localeCompare('a')"),
    ("str-unicode-escape", "'\\u4e2d\\u6587'"),
    ("str-octal-escape", "'\\101'"),
    # ES 版本面(Rhino 与 QuickJS 支持度不同)
    ("es-arrow", "((x)=>x*2)(3)"),
    ("es-let-const", "let a=1; const b=2; a+b"),
    ("es-template", "var n=3; `a${n}b`"),
    ("es-destructure", "var [a,b]=[1,2]; a+b"),
    ("es-spread", "Math.max.apply(null,[1,5,3])"),
    ("es-spread-syntax", "Math.max(...[1,5,3])"),
    ("es-for-of", "var s=0; for (var v of [1,2,3]) s+=v; s"),
    ("es-default-arg", "(function(a,b){b=b||2;return a+b})(1)"),
    ("es-class", "class A { m(){return 1} } new A().m()"),
    ("es-obj-shorthand", "var x=1; JSON.stringify({x})"),
    ("es-optional-chain", "typeof ({}).a"),
    ("es-array-includes", "[1,2].includes(2)"),
    ("es-object-assign", "JSON.stringify(Object.assign({},{a:1}))"),
    ("es-object-entries", "typeof Object.entries"),
    ("es-array-from", "Array.from('ab').join('|')"),
    ("es-promise-typeof", "typeof Promise"),
    ("es-symbol-typeof", "typeof Symbol"),
    ("es-getter", "var o={get a(){return 7}}; o.a"),
    # Rhino 专属方言(书源里真的会写)
    ("rhino-for-each", "var s=''; for each (var v in [1,2]) s+=v; s"),
    ("rhino-e4x-typeof", "typeof XML"),
    ("rhino-importclass", "typeof importClass"),
    ("rhino-packages", "typeof Packages"),
    ("rhino-javaimporter", "typeof JavaImporter"),
    ("rhino-java-global", "typeof java"),
    ("rhino-liveconnect-string", "String(java.md5Encode('a')).length"),
    ("rhino-conditional-catch", "try { null.x } catch (e) { 'caught' }"),
    # JSON
    ("json-parse", "JSON.parse('{\"a\":1}').a"),
    ("json-stringify-order", "JSON.stringify({b:1,a:2})"),
    ("json-stringify-undefined", "String(JSON.stringify(undefined))"),
    ("json-stringify-nested", "JSON.stringify([1,[2,[3]]])"),
    # 全局对象
    ("global-encodeuri", "encodeURIComponent('中 文')"),
    ("global-decodeuri", "decodeURIComponent('%E4%B8%AD')"),
    ("global-parseint", "parseInt('08')"),
    ("global-parsefloat", "parseFloat('1.5e2')"),
    ("global-isnan", "isNaN('x')"),
    ("global-math-round-half", "Math.round(-0.5)"),
    ("global-date-parse", "isNaN(Date.parse('2024-05-01'))"),
    ("global-tostring-radix", "(255).toString(16)"),
    ("global-tofixed", "(1.005).toFixed(2)"),
    ("global-escape", "escape('中')"),
    ("global-unescape", "unescape('%u4E2D')"),
    ("global-eval", "eval('1+1')"),
]

# ---- 手写:java.* 宿主 API 面 ------------------------------------------------
# 覆盖语料里真实出现的方法(频次见文件头),外加同名重载的 arity 分发。
HOST_API = [
    ("md5-1", "java.md5Encode('abc')"),
    ("md5-16", "java.md5Encode16('abc')"),
    # 装箱探针:JsEncodeUtils 的返回值是 Java String,不转 JS 原生 → typeof "object"
    ("hmac-base64-boxed", "typeof java.HMacBase64('msg','HmacSHA256','key')"),
    ("md5-empty", "java.md5Encode('')"),
    ("md5-cjk", "java.md5Encode('中文')"),
    ("base64-encode", "java.base64Encode('abc')"),
    ("base64-encode-cjk", "java.base64Encode('中文')"),
    ("base64-encode-flags", "java.base64Encode('abc', 2)"),
    ("base64-decode", "java.base64Decode('YWJj')"),
    ("base64-decode-nopad", "java.base64Decode('YWJj_-')"),
    ("base64-decode-charset", "java.base64Decode('YWJj', 'UTF-8')"),
    ("base64-decode-bytes", "String(java.base64DecodeToByteArray('YWJj'))"),
    ("hex-encode", "java.hexEncodeToString('ab')"),
    ("hex-decode", "java.hexDecodeToString('6162')"),
    ("encodeuri-1", "java.encodeURI('中 文')"),
    ("encodeuri-2", "java.encodeURI('中 文', 'GBK')"),
    ("timeformat", "java.timeFormat(1700000000000)"),
    ("timeformat-utc", "java.timeFormatUTC(1700000000000, 'yyyy-MM-dd', 8)"),
    ("htmlformat", "java.htmlFormat('<p>a</p><br>b')"),
    # 本体是 HtmlFormatter.formatKeepImg —— 与四步流水线的正文那条路同一份
    ("htmlformat-nbsp", "java.htmlFormat('a&nbsp;&nbsp;b<br>c')"),
    ("htmlformat-keep-img", "java.htmlFormat('<p>a</p><img src=\"/i.png\"><p>b</p>')"),
    ("htmlformat-redirect",
     "java.htmlFormat('<img src=\"/i.png\">', 'https://difftest.example.com/x/y')"),
    # `URL(redirectUrl)` 解不开时真身 runCatching().getOrNull() → 相对地址原样
    ("htmlformat-redirect-bad", "java.htmlFormat('<img src=\"/i.png\">', 'not a url')"),
    ("htmlformat-empty", "java.htmlFormat('')"),
    ("tonumchapter", "java.toNumChapter('第一百二十三章 标题')"),
    ("androidid", "typeof java.androidId()"),
    ("randomuuid-len", "String(java.randomUUID()).length"),
    ("log-returns", "java.log('hi')"),
    ("toast-returns", "String(java.toast('hi'))"),
    ("put-get", "java.put('k','v'); java.get('k')"),
    ("put-returns", "java.put('k','v')"),
    ("get-missing", "java.get('nope')"),
    ("put-overwrite", "java.put('k','1'); java.put('k','2'); java.get('k')"),
    ("strtobytes", "String(java.strToBytes('ab'))"),
    ("bytestostr", "java.bytesToStr(java.strToBytes('ab'))"),
    ("t2s", "java.t2s('中文')"),
    ("s2t", "java.s2t('中文')"),
    ("getcookie", "java.getCookie('https://difftest.example.com')"),
    ("getcookie-key", "java.getCookie('https://difftest.example.com','k')"),
    ("cache-put-get", "cache.put('k','v'); cache.get('k')"),
    # `put/get` 打 cacheDao + 内存,`putFile/getFile` 打 **ACache**(落盘)——
    # 真身是两张表,写一张读另一张读不到。合成一张会让书源在两侧走岔
    # (js-corpus-02f6203511:`cache.getFile` 取不到才去 ajax)。
    ("cache-file-put-get", "cache.putFile('f','v'); String(cache.getFile('f'))"),
    ("cache-file-miss", "String(cache.getFile('nope'))"),
    ("cache-put-then-getfile", "cache.put('k','v'); String(cache.getFile('k'))"),
    ("cache-putfile-then-get", "cache.putFile('k','v'); String(cache.get('k'))"),
    ("cache-delete-both", "cache.put('k','1'); cache.putFile('k','2');"
     " cache.delete('k'); String(cache.get('k'))+'|'+String(cache.getFile('k'))"),
    ("cookie-tomap", "typeof cookie"),
    ("source-key", "String(source.getKey())"),
    ("source-tag", "String(source.getTag())"),
    ("book-name", "String(book.name)"),
    ("ajax-unsupported", "java.ajax('https://difftest.example.com/a')"),
    # 装箱探针:java.ajax 回的也是 Java String —— typeof 是 object、`.length` 是
    # Java 的方法对象(undefined 的 typeof)、`.replace` 走 Java 的全部替换语义
    ("ajax-boxed-typeof", "typeof java.ajax('https://difftest.example.com/a')"),
    ("ajax-boxed-length", "typeof java.ajax('https://difftest.example.com/a').length"),
    ("ajax-boxed-replace",
     "java.ajax('https://difftest.example.com/a')"
     ".replace('\"','#').indexOf('\"')"),
    ("connect-unsupported", "String(java.connect('https://difftest.example.com/a'))"),
    # `ajaxAll(urlList)`:并发发一组,交回 **StrResponse 的 Java 数组**。
    # 与 ajax 的三处不同(不吞异常 / 不带 ruleData / 返回响应对象)见
    # rubato_core::host::NetProvider::ajax_all
    ("ajaxall-len",
     "java.ajaxAll(['https://difftest.example.com/a','https://difftest.example.com/b']).length"),
    ("ajaxall-body-typeof",
     "typeof java.ajaxAll(['https://difftest.example.com/a'])[0].body()"),
    ("ajaxall-isarray", "Array.isArray(java.ajaxAll(['https://difftest.example.com/a']))"),
    # **响应对象上返回 String 的成员是装箱的**:真身声明的是 Kotlin `String`,
    # 过 LiveConnect 就是 java.lang.String —— `typeof r.body()` 是 "object"、
    # `r.body().length` 是**方法对象**而不是数字。给回裸 JS 串两侧会走岔。
    ("connect-body-typeof", "typeof java.connect('https://difftest.example.com/a').body()"),
    ("connect-body-length-typeof",
     "typeof java.connect('https://difftest.example.com/a').body().length"),
    ("connect-url-typeof", "typeof java.connect('https://difftest.example.com/a').url()"),
    ("connect-code-typeof", "typeof java.connect('https://difftest.example.com/a').code()"),
    ("connect-raw-body-typeof",
     "typeof java.connect('https://difftest.example.com/a').raw().body()"),
    ("connect-headers-typeof", "typeof java.connect('https://difftest.example.com/a').headers()"),
    ("netget-body-typeof", "typeof java.get('https://difftest.example.com/a',{}).body()"),
    ("ajaxall-url-typeof",
     "typeof java.ajaxAll(['https://difftest.example.com/a'])[0].url()"),
    ("ajaxall-tostring",
     "String(java.ajaxAll(['https://difftest.example.com/a'])[0]).slice(0,9)"),
    ("ajaxall-forin",
     "(function(){var r=java.ajaxAll(['https://difftest.example.com/a',"
     "'https://difftest.example.com/b']),k=[];for(var i in r)k.push(i);return k.join(',')})()"),
    # java.getString / getElements:`java` 就是 AnalyzeRule 本身
    ("getstring-css", "java.getString('.t@text')"),
    ("getstring-jsonpath", "java.getString('$.a')"),
    ("getstringlist", "String(java.getStringList('.t@text'))"),
    ("getelements", "String(java.getElements('.t'))"),
    ("setcontent", "java.setContent('<b>x</b>'); java.getString('b@text')"),
    # jayway 的 PathNotFoundException 会原样穿到 JS(内容是 JSON 时)——
    # 与 `getString` 那条路(吞掉、给空串)不同
    ("getelement-jsonpath-miss", "java.setContent('{\"a\":1}'); java.getElement('$.content')"),
    # **jsoup 选择器的错误通道**:`Element.select` 在选择器不合法时**抛**,
    # 不是返回空。空串在 `Selector.select` 头一句 `Validate.notEmpty` 上炸
    # (ValidationException),其余在 QueryParser 里炸,而 QueryParser 把
    # IllegalArgumentException 一律重抛成 SelectorParseException —— 两个类名
    # 书源分得出来。语料里 `"a[href$="+book.tocUrl+"]"` 拼出空值那一档真的踩得到
    # (js-corpus-5175e212a0)。
] + [
    (f"select-{n}", f"org.jsoup.Jsoup.parse('<div><a href=x>t</a></div>').select({q}).size()")
    for n, q in [
        ("empty", "''"),
        ("blank", "'   '"),
        ("unbalanced", r"'a[href~=\\S]:matches(x'"),
        ("attr-empty-val", "'a[href$=]'"),
        ("attr-empty-eq", "'a[href=]'"),
        # `~=` 底下是 AttributeWithValueMatching,**不走** notEmpty —— 空正则合法
        ("attr-empty-regex", "'a[href~=]'"),
        ("attr-unclosed", "'a[href'"),
        ("trailing-comma", "'meta,link,'"),
        ("leading-comma", "',a'"),
        ("double-comma", "'meta,,link'"),
        ("gt-only", "'>'"),
        ("bad-pseudo", "'a:nope(x)'"),
        ("ok", "'a[href$=x]'"),
    ]
] + [
    ("selectnot-empty",
     "org.jsoup.Jsoup.parse('<div><a href=x>t</a></div>').select('a').not('').size()"),
    ("selectnot-bad",
     "org.jsoup.Jsoup.parse('<div><a href=x>t</a></div>').select('a').not('a[href$=]').size()"),
]

# ---- LiveConnect(书源绕开 java.* 直接调 Java 类)----------------------------
# 语料里 ~27 个 B 层源走这条:`new JavaImporter()` + `Packages.javax.crypto.*`。
# 这一支**不经 hutool**,错误类别是 JCE 自己的(与 java.aes* 的 CryptoException
# 不是一套)。下面每一条都由 jsharness 探针实测过,结论写进了
# js-host/src/live_connect.rs 与 README。
_IMP = (
    "var J=new JavaImporter();J.importPackage("
    "Packages.java.lang,Packages.javax.crypto,Packages.javax.crypto.spec,"
    "Packages.android.util,Packages.java.security,Packages.java.security.spec);"
)


def _with(expr: str) -> str:
    """在 `with (javaImport)` 里求值 —— 名字解析走 Rhino 的 JavaImporter。"""
    return f"{_IMP}var __r;with(J){{__r=({expr})}};__r"


_AES_ENC = (
    "(function(){var k=SecretKeySpec(String('0123456789abcdef').getBytes(),'AES');"
    "var iv=IvParameterSpec(String('abcdef0123456789').getBytes());"
    "var c=Cipher.getInstance('AES/CBC/PKCS5Padding');c.init(1,k,iv);"
    "return Base64.encodeToString(c.doFinal(String('hello').getBytes()),2)})()"
)
_AES_DEC_BAD = (
    "(function(){var k=SecretKeySpec(String('0123456789abcdef').getBytes(),'AES');"
    "var iv=IvParameterSpec(String('abcdef0123456789').getBytes());"
    "var c=Cipher.getInstance('AES/CBC/PKCS5Padding');c.init(2,k,iv);"
    "return c.doFinal(String('0123456789abcdef').getBytes())})()"
)

# `java.security` 的 RSA 一支(语料 2 个源:西瓜小说的 `sign` 请求头是
# `SHA256WithRSA` 的签名)。密钥是**为本套现造的** 1024 位测试密钥,只在差分里用;
# DER 按 Java 的有符号 byte 写,与书源里的写法(字节字面量数组)一模一样。
# (验签那半连同 _RSA_PUB 已删,见下面 rsa-err-noalg 前的说明。)
_RSA_PRIV = "[48,-126,2,95,2,1,0,2,-127,-127,0,-25,-4,-46,70,88,-24,100,-19,-10,-107,115,-38,-82,31,29,-77,93,8,-126,-64,-69,-15,97,48,121,25,89,-62,4,-35,-10,-70,118,-70,69,-19,109,74,-115,72,96,-78,66,-54,40,-66,-83,3,-65,-96,7,-105,0,-27,76,94,-52,11,-100,31,54,54,-23,40,-85,-24,103,54,-124,-65,78,-87,42,91,3,77,96,93,34,8,-72,123,70,62,50,-72,-25,49,88,-2,-23,84,103,47,-86,-59,66,42,111,-9,-125,-29,-83,-64,96,88,18,63,61,67,125,-43,-17,93,102,69,80,-102,-85,62,-43,-9,55,-71,60,-80,-78,47,2,3,1,0,1,2,-127,-127,0,-49,-65,63,-117,17,-39,99,113,26,-114,50,6,-42,65,53,54,-57,-116,116,-109,38,123,125,-66,-117,-29,-32,-42,119,-57,82,73,31,67,-90,-107,125,118,-14,-40,-85,7,87,-6,115,-52,117,97,-46,-5,-5,112,5,-76,81,-44,-34,-61,82,-41,60,-108,-91,-20,58,53,-40,-39,-62,-94,-105,113,-83,46,-92,-32,-125,49,-63,-8,53,-108,-92,-110,52,-13,63,4,89,45,-38,121,125,31,72,38,-13,25,113,108,-38,-24,-53,123,-40,39,-78,45,32,-81,-60,-55,-115,-37,-17,-119,1,81,71,-10,37,-59,-42,64,-50,87,-105,-31,2,65,0,-1,-104,24,-33,-99,105,112,-32,-17,-37,-96,-125,-124,-67,15,1,-45,0,116,-32,-103,12,-97,97,118,2,98,120,-128,-128,-94,37,-105,-103,-64,110,-75,-83,-40,9,10,-14,30,75,119,-39,64,40,121,15,66,65,26,47,-126,-81,12,38,-20,10,77,127,80,63,2,65,0,-24,91,32,-72,34,100,3,10,125,17,-108,-28,-46,-58,14,-59,17,66,-11,44,122,81,46,72,126,-109,-59,102,70,1,-55,-77,-128,-31,99,127,-34,-25,-76,-84,-2,47,-39,-109,125,103,82,25,84,-70,-99,-16,52,111,-33,93,-88,69,39,33,-84,108,34,17,2,65,0,-74,87,41,-105,-115,-45,5,38,83,-73,-103,97,122,54,-18,23,-35,17,-4,79,-90,-96,44,-85,-8,-26,102,-99,-108,-1,47,-82,37,-29,24,55,99,1,44,-105,-10,-23,23,-41,-69,30,-113,-8,-49,-76,-91,13,-112,-62,-56,54,93,50,-1,61,-78,95,-37,61,2,65,0,-30,-52,-32,121,27,-120,-121,-73,91,49,40,38,-38,-35,-36,88,-37,106,-126,42,50,18,-65,-100,-97,-128,-39,-13,-28,109,-90,86,2,124,-2,68,56,109,-18,-37,-43,25,27,-59,109,104,-58,-88,87,29,-7,64,23,-49,3,54,70,35,-119,-8,-62,118,-94,33,2,65,0,-44,15,-64,31,-85,65,-15,96,66,-117,-109,-13,112,-88,-31,86,-112,84,82,-84,-93,3,32,17,56,-63,110,96,-74,-96,-60,-76,-105,-33,-124,-62,-76,-30,-21,-80,101,20,55,29,113,20,53,-100,-80,37,-90,60,116,20,82,-72,96,-7,-94,-61,30,12,86,-82]"
LIVE_CONNECT = [
    # —— 名字面 ——
    ("importer-typeof", "typeof new JavaImporter()"),
    ("class-typeof", "typeof Packages.javax.crypto.Cipher"),
    ("pkg-android", "typeof android + '/' + typeof android.util.Base64.decode"),
    # 同名类跨包冲突:Rhino 抛 EvaluatorException(与裁判 errorTag 同归 SyntaxError),
    # 而且是**懒的** —— 取名字时才炸,importPackage 那一步不炸
    ("ambiguous",
     "var J=new JavaImporter();J.importPackage(Packages.java.util,Packages.android.util);"
     "var __r;with(J){__r=(typeof Base64)};__r"),
    # —— java.lang.String:with 块里它遮住 JS 的 String ——
    ("string-shadow", _with("typeof String('ab')")),
    ("string-getbytes", _with("String('ab').getBytes().length")),
    ("string-from-bytes", _with("String(String('hi').getBytes())")),
    ("string-new-charset", _with("new String(String('hi').getBytes(),'UTF-8')")),
    ("string-getbytes-charset", _with("String('中').getBytes('GBK').length")),
    ("string-len-method", _with("String('abc').length()")),
    # —— `with` 块里被 java.lang / java.util **遮住的 JS 内建** ——
    # `String` 是那条大家都知道的(上面几条);同一条规则还会顺手遮掉 `Object`
    # 与(导了 java.util 时的)`Date` —— 而 java.lang.Object 没有 `keys`、
    # java.util.Date 没有 `getFullYear`。**这两条真身自己跑不动**(语料里
    # `Object.keys` 5 个源、`new Date()` 1 个源都在 with 块里),故记豁免不记欠账,
    # 理由逐字写在 exemptions.json。台账 `liveconnect` 面的 Object / Date 两行指向这里。
    ("shadow-object-keys", _with("Object.keys({a:1,b:2}).join(',')")),
    ("shadow-date-getfullyear",
     "var J=new JavaImporter();J.importPackage(Packages.java.lang,Packages.java.util);"
     "var __r;with(J){__r=(typeof new Date().getFullYear)};__r"),
    # 对照:java.lang.Math 与 JS Math 在这一位上给同一个答案,不用豁免
    ("shadow-math-floor", _with("Math.floor(1.5)")),
    # —— Java 数组的形态:有符号、Array.isArray 为 false、join 可用 ——
    ("bytes-signed", _with("String('中').getBytes()[0]")),
    ("bytes-isarray", _with("Array.isArray(String('ab').getBytes())")),
    ("bytes-join", _with("String('ab').getBytes().join('|')")),
    ("bytes-identity", _with("String(String('ab').getBytes()).slice(0,3)")),
    ("arrays-copyofrange", _with("Arrays.copyOfRange(String('abcdef').getBytes(),1,3).length")),
    # —— 两个 Base64:android(MIME 表,宽松)与 java.util(严格)——
    ("b64-android-dec", _with("String(Base64.decode(String('aGk=').getBytes(),2))")),
    ("b64-android-enc", _with("Base64.encodeToString(String('hi').getBytes(),Base64.NO_WRAP)")),
    ("b64-android-flags",
     _with("[Base64.DEFAULT,Base64.NO_WRAP,Base64.URL_SAFE,Base64.NO_PADDING,Base64.CRLF].join(',')")),
    # 表外字符:裁判垫片用 MIME 解码器 → 直接跳过,不抛
    ("b64-android-junk", _with("String(Base64.decode(String('!!!!').getBytes(),2))")),
    ("b64-util-dec",
     "var J=new JavaImporter();J.importPackage(Packages.java.lang,Packages.java.util);"
     "var __r;with(J){__r=(String(Base64.getDecoder().decode(String('aGk='))))};__r"),
    ("b64-util-enc",
     "var J=new JavaImporter();J.importPackage(Packages.java.lang,Packages.java.util);"
     "var __r;with(J){__r=(Base64.getEncoder().encodeToString(String('hi').getBytes()))};__r"),
    # —— Cipher / Mac / MessageDigest:算出来的值要逐字对上 ——
    ("cipher-mode-const", _with("Cipher.ENCRYPT_MODE+'/'+Cipher.DECRYPT_MODE")),
    ("cipher-dofinal-type",
     _with("(function(){var k=SecretKeySpec(String('0123456789abcdef').getBytes(),'AES');"
           "var c=Cipher.getInstance('AES/ECB/PKCS5Padding');c.init(1,k);"
           "var o=c.doFinal(String('hi').getBytes());"
           "return typeof o+'/'+o.length+'/'+Array.isArray(o)})()")),
    ("aes-cbc-enc", _with(_AES_ENC)),
    ("desede-cbc-enc",
     _with("(function(){var k=SecretKeySpec(String('0123456789abcdefABCDEFGH').getBytes(),'DESede');"
           "var iv=IvParameterSpec(String('01234567').getBytes());"
           "var c=Cipher.getInstance('DESede/CBC/PKCS5Padding');c.init(1,k,iv);"
           "return Base64.encodeToString(c.doFinal(String('hello').getBytes()),2)})()")),
    # DESKeySpec + SecretKeyFactory:密钥取前 8 字节
    ("des-keyfactory",
     _with("(function(){var dks=new DESKeySpec(String('KK!%G3JdCHJxpAF3%Vg9pN').getBytes());"
           "var kf=SecretKeyFactory.getInstance('DES');var sk=kf.generateSecret(dks);"
           "var c=Cipher.getInstance('DES/CBC/PKCS5Padding');"
           "var iv=new IvParameterSpec(String('1ae2c94b').getBytes('utf-8'));c.init(1,sk,iv);"
           "return Base64.encodeToString(c.doFinal(String('hello').getBytes()),2)})()")),
    ("mac-hmacsha256",
     _with("(function(){var m=Mac.getInstance('HmacSHA256');"
           "m.init(SecretKeySpec(String('key').getBytes('UTF-8'),'HmacSHA256'));"
           "return Base64.encodeToString(m.doFinal(String('msg').getBytes('UTF-8')),2)})()")),
    ("messagedigest-md5",
     _with("(function(){var d=MessageDigest.getInstance('MD5');"
           "return Base64.encodeToString(d.digest(String('abc').getBytes()),2)})()")),
    # —— 错误类别:JCE 自己的类,且落在与真身相同的那一步 ——
    ("err-blocksize",
     _with("(function(){var k=SecretKeySpec(String('0123456789abcdef').getBytes(),'AES');"
           "var iv=IvParameterSpec(String('abcdef0123456789').getBytes());"
           "var c=Cipher.getInstance('AES/CBC/PKCS5Padding');c.init(2,k,iv);"
           "return c.doFinal(String('abc').getBytes())})()")),
    ("err-badpadding", _with(_AES_DEC_BAD)),
    ("err-keylen",
     _with("(function(){var k=SecretKeySpec(String('short').getBytes(),'AES');"
           "var c=Cipher.getInstance('AES/ECB/PKCS5Padding');c.init(1,k);return 1})()")),
    ("err-ivlen",
     _with("(function(){var k=SecretKeySpec(String('0123456789abcdef').getBytes(),'AES');"
           "var iv=IvParameterSpec(String('abc').getBytes());"
           "var c=Cipher.getInstance('AES/CBC/PKCS5Padding');c.init(1,k,iv);return 1})()")),
    ("err-noalg", _with("Cipher.getInstance('NOPE/CBC/PKCS5Padding')")),
    # 补码名不认识时 JCE 抛的**也是** NoSuchAlgorithmException
    ("err-nopad", _with("Cipher.getInstance('AES/CBC/NoSuchPadding')")),
    # ECB 收到 IvParameterSpec 不是「忽略」而是 **init 当场抛**
    # (`ECB mode cannot use IV`);被测侧此前一路忽略 IV,照样算得出结果
    ("err-ecb-iv",
     _with("(function(){var k=SecretKeySpec(String('0123456789abcdef').getBytes(),'AES');"
           "var iv=IvParameterSpec(String('abcdef0123456789').getBytes());"
           "var c=Cipher.getInstance('AES/ECB/PKCS5Padding');c.init(1,k,iv);return 1})()")),
    # —— 空输入:`doFinal(new byte[0])` 解密是**空数组**,不是 IllegalBlockSize ——
    # 这是 pb02492(🔰笔趣阁.pysmei)那条路的最小复现:书源拿明文页喂
    # `Base64.decode(…, 2)`,表外字符被解码表全跳过 → 0 字节 → 再进 doFinal。
    ("dec-empty",
     _with("(function(){var k=SecretKeySpec(String('OW84U8Eerdb99rtsTXWSILDO').getBytes(),'DESede');"
           "var iv=IvParameterSpec(String('SK8bncVu').getBytes());"
           "var c=Cipher.getInstance('DESede/CBC/PKCS5Padding');c.init(2,k,iv);"
           "return c.doFinal([]).length})()")),
    ("dec-empty-nopadding",
     _with("(function(){var k=SecretKeySpec(String('0123456789abcdef').getBytes(),'AES');"
           "var iv=IvParameterSpec(String('abcdef0123456789').getBytes());"
           "var c=Cipher.getInstance('AES/CBC/NoPadding');c.init(2,k,iv);"
           "return c.doFinal([]).length})()")),
    # 加密那一侧的对照:空输入 PKCS5 补出**整整一个分组**
    ("enc-empty",
     _with("(function(){var k=SecretKeySpec(String('OW84U8Eerdb99rtsTXWSILDO').getBytes(),'DESede');"
           "var iv=IvParameterSpec(String('SK8bncVu').getBytes());"
           "var c=Cipher.getInstance('DESede/CBC/PKCS5Padding');c.init(1,k,iv);"
           "return c.doFinal([]).length})()")),
    # 整条链原样:解不动的明文 → 空 → 解密交空串(书源的 `catch(e){result}` 因此**不触发**)
    ("dec-junk-chain",
     _with("(function(){var k=SecretKeySpec(String('OW84U8Eerdb99rtsTXWSILDO').getBytes(),'DESede');"
           "var iv=IvParameterSpec(String('SK8bncVu').getBytes());"
           "var b=Base64.decode(String('作者甲').getBytes(),2);"
           "var c=Cipher.getInstance('DESede/CBC/PKCS5Padding');c.init(2,k,iv);"
           "return '['+String(c.doFinal(b))+']'})()")),
    # —— java.security:KeyFactory / Signature / PKCS8EncodedKeySpec ——
    ("rsa-sign-sha256",
     _with("(function(){var s=Signature.getInstance('SHA256WithRSA');"
           "s.initSign(KeyFactory.getInstance('RSA').generatePrivate("
           "PKCS8EncodedKeySpec(" + _RSA_PRIV + ")));"
           "s.update(String('hello').getBytes('UTF-8'));"
           "return Base64.encodeToString(s.sign(),2)})()")),
    # 不带 `new` 直接调类是 Rhino 的方言(构造函数即普通函数),书源两种都写
    ("rsa-sign-new-keyspec",
     _with("(function(){var s=Signature.getInstance('SHA256WithRSA');"
           "s.initSign(KeyFactory.getInstance('RSA').generatePrivate("
           "new PKCS8EncodedKeySpec(" + _RSA_PRIV + ")));"
           "s.update(String('hello').getBytes('UTF-8'));"
           "return Base64.encodeToString(s.sign(),2)})()")),
    ("rsa-sign-sha1",
     _with("(function(){var s=Signature.getInstance('SHA1withRSA');"
           "s.initSign(KeyFactory.getInstance('RSA').generatePrivate("
           "PKCS8EncodedKeySpec(" + _RSA_PRIV + ")));"
           "s.update(String('hello').getBytes('UTF-8'));"
           "return Base64.encodeToString(s.sign(),2)})()")),
    ("rsa-sign-len",
     _with("(function(){var s=Signature.getInstance('SHA256WithRSA');"
           "s.initSign(KeyFactory.getInstance('RSA').generatePrivate("
           "PKCS8EncodedKeySpec(" + _RSA_PRIV + ")));"
           "s.update(String('hello').getBytes('UTF-8'));"
           "return s.sign().length})()")),
    # 验签(Signature.initVerify/verify、KeyFactory.generatePublic、
    # X509EncodedKeySpec)那半已删:corpus-js 与 sources 里 `rsaVerify`/`.verify(`
    # 命中 0,唯一用户就是这里的探针 —— 只签不验,签那半(上面几条)保留。
    # 算法名不认得 → NoSuchAlgorithmException;私钥 DER 是坏的 → 真身在
    # `KeyFactory.generatePrivate` 那一步抛 InvalidKeySpecException
    ("rsa-err-noalg",
     _with("(function(){var s=Signature.getInstance('SHA256WithNOPE');"
           "s.initSign(KeyFactory.getInstance('RSA').generatePrivate("
           "PKCS8EncodedKeySpec(" + _RSA_PRIV + ")));"
           "s.update(String('h').getBytes('UTF-8'));return s.sign().length})()")),
    ("rsa-err-badkey",
     _with("(function(){var s=Signature.getInstance('SHA256WithRSA');"
           "s.initSign(KeyFactory.getInstance('RSA').generatePrivate("
           "PKCS8EncodedKeySpec([1,2,3])));"
           "s.update(String('h').getBytes('UTF-8'));return s.sign().length})()")),
]

# ---- hutool 那条对称加解密路(`java.aes*` / `des*` / `createSymmetricCrypto`)--
# 与上面的 LiveConnect 一支**同一份算术、两套皮**:这边一律 `CryptoException`。
# 这一族此前**一条手写探针都没有** —— 只有 corpus 里几条真书源顺带走到,而它们
# 两侧都落在 catch 里,「一致」得毫无信息量(M3q 才照出下面这两条)。
HUTOOL_CRYPTO = [
    # 正对照:DES/CBC 一条能算到底的
    ("des-cbc-enc",
     "java.desEncodeToBase64String('hello','01234567','DES/CBC/PKCS5Padding','01234567')"),
    # ECB + 非空 IV:hutool 只要 iv 非空就传 IvParameterSpec,而 ECB 的 init
    # 收到 params 就抛(`ECB mode cannot use IV`)→ CryptoException。
    # DES 这条**绕开**下面那个裁判环境差(hutool 的 DES 分支走 SecretKeyFactory)
    ("ecb-iv-des",
     "java.desEncodeToBase64String('hello','01234567','DES/ECB/PKCS5Padding','01234567')"),
    # 光写 `AES` 时 transformation 缺省就是 ECB,给了 iv 同样炸
    ("ecb-iv-bare-aes",
     "java.aesEncodeToBase64String('hello','0123456789abcdef','AES','abcdef0123456789')"),
    # 空串:hutool 的 `HexUtil.decodeHex('')` 与 `Base64.decode('')` **都给 null**,
    # 于是 `doFinal(null)` 抛 `Null input buffer` → CryptoException。
    # 与下一条(空白串)、与 LiveConnect 的 `doFinal(new byte[0])` 是**三条挨着的边界**
    ("dec-empty",
     "String(java.createSymmetricCrypto('DESede/CBC/PKCS5Padding',"
     "'OW84U8Eerdb99rtsTXWSILDO','SK8bncVu').decryptStr(''))"),
    # 空白串不是空串:走 base64 的宽松表 → 0 字节 → doFinal 交空数组 → 空串
    ("dec-blank",
     "'['+String(java.createSymmetricCrypto('DESede/CBC/PKCS5Padding',"
     "'OW84U8Eerdb99rtsTXWSILDO','SK8bncVu').decryptStr(' \\n '))+']'"),
    # `*DecodeToString` 那一排薄壳走的是同一条 `decryptStr`,空串同样炸
    ("dec-empty-shell",
     "String(java.desDecodeToString('','01234567','DES/CBC/PKCS5Padding','01234567'))"),
    # **裁判环境差,记豁免**(exemptions.json 里逐字写着理由):hutool 5.8.22 的
    # `KeyUtil.generateKey` 在非 PBE/DES 分支上把**整条 transformation** 当算法名
    # 塞进 `SecretKeySpec` → SunJCE 的 AESCipher 查 `key.getAlgorithm()` 后拒收;
    # Android 的 Conscrypt 只查密钥长度,所以真身在手机上跑得通(语料里 17 个源
    # 就这么写)。被测侧照 Android 的行为放行。
    ("aes-full-transformation",
     "java.aesEncodeToBase64String('hello','0123456789abcdef',"
     "'AES/CBC/PKCS5Padding','abcdef0123456789')"),
]

# ---- 手写:**第三个宿主** BaseSource.evalJS(host="source")-------------------
# source 的 `header` / `loginUrl` / `loginCheckJs` 走它。绑定面最窄:
# 只有 java / source / sourceApi / baseUrl / cookie / cache,
# 而 `java` 就是书源实体本身 —— `java.put/get` 打的是 **CacheManager**
# (`v_<sourceKey>_<key>`),不是 AnalyzeRule 那套四层变量。
# 第三项是本 case 额外要带的字段(预置 CacheManager 之类)。
SOURCE_HOST = [
    # —— 绑定面:缺席的名字是**未声明**,不是 null ——
    ("bind-undeclared",
     "[typeof result,typeof book,typeof chapter,typeof page,typeof key].join(',')"),
    ("bind-typeof",
     "[typeof java,typeof source,typeof sourceApi,typeof cookie,typeof cache].join(',')"),
    # baseUrl 是 getKey()(bookSourceUrl),不是页面地址 —— 与另两个宿主的关键差别
    ("bind-baseurl", "String(baseUrl)"),
    ("bind-same-object", "(java === source) + '/' + (java === sourceApi)"),
    ("bind-source-field",
     "String(source.bookSourceUrl) + '|' + String(source.bookSourceName)"),
    ("bind-java-field", "String(java.bookSourceUrl)"),
    ("getkey-gettag", "String(java.getKey()) + '|' + String(java.getTag())"),
    # —— java.put/get:CacheManager 那一层(观察面是输出里的 `cache`)——
    ("var-put-get", "java.put('k', 'v'); String(java.get('k'))"),
    ("var-get-missing", "String(java.get('nope'))"),
    # —— 源变量(`sourceVariable_<key>`)——
    ("source-variable-empty", "String(java.getVariable())"),
    ("source-variable", "java.putVariable('{\"a\":1}'); String(java.getVariable())"),
    ("source-variable-del",
     "java.putVariable('x'); java.setVariable(null); String(java.getVariable())"),
    # —— 登录头:取不到给 **null**(不是空串)——
    ("login-header-missing", "String(java.getLoginHeader())"),
    ("login-header",
     "String(java.getLoginHeader())",
     {"cache": {"loginHeader_https://difftest.example.com": '{"Cookie":"a=1"}'}}),
    ("login-header-put",
     "java.putLoginHeader('{\"X\":\"1\"}'); String(java.getLoginHeader())"),
    # —— 登录信息:AES(androidId 前 16 字节)+ base64 往返 ——
    ("login-info-missing", "String(java.getLoginInfo())"),
    ("login-info-roundtrip",
     "java.putLoginInfo('{\"u\":\"n\"}') + '/' + String(java.getLoginInfo())"),
    ("login-info-garbage",
     "String(java.getLoginInfo())",
     {"cache": {"userInfo_https://difftest.example.com": "not-base64!!"}}),
    ("login-info-map-empty", "String(java.getLoginInfoMap().size())"),
    ("login-info-map-roundtrip",
     "java.putLoginInfo('{\"u\":\"n\"}');var m=java.getLoginInfoMap();"
     "String(m.u)+'/'+String(m.get('u'))+'/'+JSON.stringify(m)"),
    # —— JsExtensions 那一面照旧在 ——
    ("jsext-md5", "String(java.md5Encode('abc'))"),
    ("jsext-timeformat", "String(java.timeFormat(1788001200000))"),
    ("jsext-androidid-len", "String(java.androidId()).length"),
    # —— 规则反调面**没有**(`java` 不是 AnalyzeRule)——
    ("no-rule-host", "java.getString('.t@text')"),
    ("no-set-content", "java.setContent('<p>x</p>')"),
    # —— 装箱探针:BaseSource 这一面的返回值同样是 Java String ——
    ("getkey-boxed", "typeof java.getKey()"),
    ("gettag-boxed", "typeof java.getTag()"),
    ("getvariable-boxed", "java.putVariable('aXa'); java.getVariable().replace('a','b')"),
    ("login-header-boxed", "typeof java.getLoginHeader()",
     {"cache": {"loginHeader_https://difftest.example.com": '{"Cookie":"a=1"}'}}),
    ("login-info-boxed", "java.putLoginInfo('aXa'); java.getLoginInfo().replace('a','b')"),
    # —— setVariable / putVariable 的**实参不是字符串**时怎么办 ——
    # 真身签名是 `setVariable(variable: String?)`(BaseSource.kt L312),null 是
    # **删除**;非 null 的非字符串由 Rhino 按 Java String 形参转换。书源里
    # `source.setVariable(0)` 是常见写法(top100 名单里就有一个 —— 被测侧此前
    # 只认 JS 字符串,数字被当成 null **把变量删掉了**,与真身相反)。
    ("setvar-number", "java.setVariable(0); '[' + java.getVariable() + ']'"),
    ("setvar-float", "java.setVariable(1.5); '[' + java.getVariable() + ']'"),
    ("setvar-bool", "java.setVariable(true); '[' + java.getVariable() + ']'"),
    ("setvar-object", "java.setVariable({a:1}); '[' + java.getVariable() + ']'"),
    ("setvar-array", "java.setVariable([1,2]); '[' + java.getVariable() + ']'"),
    ("setvar-null-deletes",
     "java.setVariable('x'); java.setVariable(null); '[' + java.getVariable() + ']'"),
    ("setvar-undefined",
     "java.setVariable('x'); java.setVariable(undefined); '[' + java.getVariable() + ']'"),
    ("putvar-number", "java.putVariable(7); '[' + java.getVariable() + ']'"),
]

# ---- 手写:**getElement/getElements 交回来的是什么** -------------------------
# 容器形态由**规则模式**决定,不由内容:`Mode::Default` 走 AnalyzeByJSoup →
# jsoup 的 `Elements`(toString 是各 outerHtml 换行相连、**过不了** GSON);
# jayway 的 `getList` / XPath 的 `List<JXNode>` / 正则 / JS 那几条都是普通
# `ArrayList`(toString 是 `[a, b]`、过得了 GSON,项还能按键取属性)。
# 被测侧此前一律按 Elements 建箱 —— JSON 页上这一整片是错的。
_ELS_JSON = '{"list":[{"n":"书名A","u":"/1"},{"n":"书名B","u":"/2"}],"s":["x","y"]}'
_ELS_HTML = "<html><body><div class=t>t1</div><div class=t>t2</div></body></html>"

ELEMENTS = [
    ("json-list", "java.getElements('$.list')", _ELS_JSON),
    ("json-list-str", "c=java.getElements('$.list'); String(c)", _ELS_JSON),
    ("json-strs", "java.getElements('$.s')", _ELS_JSON),
    # jayway 抛(路径不合法)→ getList 吞掉、交回**空 ArrayList**,不是空 Elements
    ("json-bad-path", "java.getElements('class.a@class.b')", _ELS_JSON),
    # 读出来是标量 → getElements 的收尾 when 落到 `return ArrayList()`
    ("json-scalar", "java.getElements('$.list[0].n')", _ELS_JSON),
    ("json-item-keys",
     "c=java.getElements('$.list'); String(c.get(0))+'|'+String(c[0])+'|'+String(c[0].n)",
     _ELS_JSON),
    ("json-item-size", "c=java.getElements('$.list'); c.length+'|'+c.size()", _ELS_JSON),
    ("json-getelement", "java.getElement('$.list')", _ELS_JSON),
    ("json-getelement-obj", "java.getElement('$.list[0]')", _ELS_JSON),
    ("html-list", "java.getElements('class.t')", _ELS_HTML),
    ("html-list-str",
     "c=java.getElements('class.t'); String(c)+'|'+c.length+'|'+typeof c.get", _ELS_HTML),
    ("html-empty", "java.getElements('class.nope')", _ELS_HTML),
    ("html-getelement-empty", "java.getElement('class.nope')", _ELS_HTML),
    # XPath 那条:容器是 `List<JXNode>`(toString `[a, b]`)而项是 jsoup 元素
    # —— 于是 toString 与 GSON 分属两档,一条用例同时钉住两面
    ("xpath-list", "java.getElements('//div[@class=\"t\"]')", _ELS_HTML),
]

# ---- 手写:**`result` 绑的是 Java 对象**(NativeJavaMap / NativeJavaList)-----
# 产品里 `init: $.xxx` 从 JSON 页选出一个对象,下一步的 `@js:` 里 `result` 就是
# **jayway 读出来的 Java 对象本身**(Map / List),不是它的字符串形态。Rhino 的
# `Context.javaToJS` 把 Map 包成 **NativeJavaMap**(键直接当属性读)、List 包成
# NativeJavaList。被测侧 `AnalyzeRule::bind_value` 对这几支一直按 `to_java_string()`
# 过 —— 于是 `result` 是串、`result.anchor` 撞上 `String.prototype.anchor`
# (pb01652,记在 fixtures/diff-baseline.json 的欠账里)。
#
# 这一族先把**真身的形态**钉住再改被测侧:`typeof` / `String(result)` /
# `for..in` / `JSON.stringify` / Java 方法面(`get`/`size`)/ 值本身是不是装箱串,
# 每一样都是面 —— 「换成 JS 原生对象」只在 `.anchor` 那一格上对,别处会翻车。
#
# `resultRule` 走**真身 getElement**(与产品同一条路径),用例形状见
# fixtures/cases/js-host/README.md。
_BIND_JSON = (
    '{"obj":{"anchor":"玄幻","n":"书名A","u":"/b/1","cnt":7,"ok":true,'
    '"sub":{"k":"v"},"ls":[1,2],"no":false},'
    '"arr":[{"n":"A"},{"n":"B"}],"strs":["x","y"],'
    '"num":7,"txt":"文字","flag":true}'
)

BIND_RESULT = [
    # --- Map(pb01652 那一支):jayway 定路径读出来的 JSONObject
    ("map-typeof", "$.obj", "typeof result"),
    ("map-key", "$.obj", "String(result.n)"),
    # **就是这一条**:真身给 `玄幻`,被测侧此前给 `String.prototype.anchor`
    ("map-anchor", "$.obj", "String(result.anchor)"),
    ("map-index", "$.obj", "String(result['u'])"),
    ("map-missing", "$.obj", "typeof result.nope"),
    ("map-string", "$.obj", "String(result)"),
    ("map-concat", "$.obj", "'' + result"),
    ("map-tostring", "$.obj", "result.toString()"),
    ("map-forin", "$.obj", "var a=[];for(var k in result)a.push(k);a.join(',')"),
    ("map-keys", "$.obj", "Object.keys(result).join(',')"),
    ("map-json", "$.obj", "JSON.stringify(result)"),
    # Java 方法面还在不在(NativeJavaMap 是包着 Map 的,不是拷贝)
    ("map-get", "$.obj", "String(result.get('n'))"),
    ("map-size", "$.obj", "String(result.size())"),
    ("map-length", "$.obj", "typeof result.length"),
    # 值本身:`java.lang.String` 装箱的话 typeof 是 object、`.length` 是方法对象
    ("map-value-typeof", "$.obj", "typeof result.n"),
    # `.length` 是**方法对象**不是数字(Java 的 String.length())。
    # **没钉它自己的 toString** —— 真身是 Rhino 的 Java 签名形态
    # `function length() {/*\nint length()\n*/}\n`,被测侧是那个 JS 函数的源码;
    # 复刻要给每个装箱方法带上 Java 签名(还得把重载全列出来),而语料里
    # **0 处**观察它。钉的是「是方法」与「调得出值」这两位。
    ("map-value-len", "$.obj", "typeof result.n.length + '|' + String(result.n.length())"),
    ("map-num-value", "$.obj", "typeof result.cnt + '|' + String(result.cnt)"),
    ("map-not-string", "$.obj", "typeof result.toUpperCase"),
    ("map-instanceof", "$.obj", "String(result instanceof Object)"),
    # 装箱的串 `!==` JS 串,而 `==` 为真 —— NativeJavaMap 最好认的一处
    ("map-str-eq", "$.obj", "String(result.n === '书名A') + '|' + String(result.n == '书名A')"),
    ("map-num-arith", "$.obj", "String(result.cnt + 1) + '|' + (result.cnt + '')"),
    ("map-bool-value", "$.obj", "typeof result.ok + '|' + String(result.ok)"),
    # **真假值**:装箱的 Boolean 在 `if` 里算真还是算假 —— 装箱能不能做,
    # 全看这一条(JS 的 ToBoolean 对**任何**对象都给真,无从模拟 Rhino 的解包)
    ("map-bool-if-true", "$.obj", "result.ok ? 'T' : 'F'"),
    ("map-bool-if-false", "$.obj", "result.no ? 'T' : 'F'"),
    ("map-str-if-empty", "$.obj", "String(!!result.n)"),
    # 嵌套容器:逐层包装,而**数组的 toString 随出身**(json-smart 是 JSON 文本)
    ("map-sub-typeof", "$.obj", "typeof result.sub + '|' + String(result.sub)"),
    ("map-sub-key", "$.obj", "String(result.sub.k)"),
    ("map-sub-list", "$.obj", "String(result.ls) + '|' + String(result.ls.length)"),
    ("map-hasown", "$.obj", "typeof result.hasOwnProperty"),
    # 完成值就是它本身 —— 一条用例同时钉 type / str(toString)/ norm(GSON)
    ("map-value", "$.obj", "result"),
    # --- List:定路径读出来的 JSONArray
    ("list-typeof", "$.arr", "typeof result"),
    ("list-string", "$.arr", "String(result)"),
    ("list-len", "$.arr", "typeof result.length + '|' + String(result.length)"),
    ("list-size", "$.arr", "String(result.size())"),
    ("list-index", "$.arr", "String(result[0])"),
    ("list-index-key", "$.arr", "String(result[0].n)"),
    ("list-map", "$.arr", "typeof result.map"),
    ("list-json", "$.arr", "JSON.stringify(result)"),
    ("list-forin", "$.arr", "var a=[];for(var k in result)a.push(k);a.join(',')"),
    ("list-value", "$.arr", "result"),
    ("strs-string", "$.strs", "String(result)"),
    ("strs-index", "$.strs", "String(result[0])"),
    ("strs-index-typeof", "$.strs", "typeof result[0]"),
    ("strs-value", "$.strs", "result"),
    # indefinite 路径:jayway **新建**一个 JSONArray,不随模型走
    ("wild-typeof", "$.arr[*]", "typeof result"),
    ("wild-string", "$.arr[*]", "String(result)"),
    ("wild-value", "$.arr[*]", "result"),
    # --- 标量:Rhino 的 javaToJS 对 String/Number/Boolean 原样交回
    ("num-typeof", "$.num", "typeof result + '|' + String(result)"),
    ("num-value", "$.num", "result"),
    ("txt-typeof", "$.txt", "typeof result + '|' + String(result.length)"),
    ("txt-value", "$.txt", "result"),
    ("flag-typeof", "$.flag", "typeof result + '|' + String(result)"),
    ("flag-value", "$.flag", "result"),
    # --- 对照组:**JS 自己造的对象**(Rhino 的 NativeObject)。它与上面那一族
    # 相反 —— 是**真 JS 对象**,不是 Java 对象的包装:有 Object.prototype、
    # `String(x)` 是 `[object Object]`。改 bind_value 时最容易一刀切错的就是这里。
    ("native-typeof", "@js:({a:'A',n:7})", "typeof result"),
    ("native-string", "@js:({a:'A',n:7})", "String(result)"),
    ("native-key", "@js:({a:'A',n:7})", "String(result.a)"),
    ("native-hasown", "@js:({a:'A',n:7})", "typeof result.hasOwnProperty"),
    ("native-json", "@js:({a:'A',n:7})", "JSON.stringify(result)"),
    ("native-value", "@js:({a:'A',n:7})", "result"),
]

# HTML 页那一半:XPath 的 `List<JXNode>` 与 jsoup 的 `Elements` —— 同样是
# 「`result` 不是串」的面,但容器与项的形态都与 jayway 那族不同。
# jsoup 那两条是**对照组**:它们此前就走 BoundValue::Element,改动不许动它们。
_BIND_HTML = (
    "<html><body><ul id=list>"
    "<li><a href=/1>一</a></li><li><a href=/2>二</a></li>"
    "</ul></body></html>"
)

BIND_RESULT_HTML = [
    ("jx-typeof", "//ul[@id='list']/li", "typeof result"),
    ("jx-string", "//ul[@id='list']/li", "String(result)"),
    ("jx-len", "//ul[@id='list']/li", "typeof result.length + '|' + String(result.length)"),
    ("jx-index", "//ul[@id='list']/li", "String(result[0])"),
    ("jx-value", "//ul[@id='list']/li", "result"),
    ("els-typeof", "@css:#list li", "typeof result"),
    ("els-len", "@css:#list li", "typeof result.length + '|' + String(result.length)"),
    ("els-text", "@css:#list li", "String(result.text())"),
    ("els-value", "@css:#list li", "result"),
]

HOST_API_BIND = {
    "content": "<html><body><div class=t>标题一</div><div class=t>标题二</div></body></html>",
    "baseUrl": "https://difftest.example.com/book/1",
    "result": "第 12 章",
}


# `java.webView` / `webViewGetSource` / `webViewGetOverrideUrl`
# (JsExtensions.kt L245-328)—— 走的是 **BackstageWebView 的策略层**,
# 平台那一半是**剧本**(`webview` 字段,契约 fixtures/cases/webview/README.md)。
# 从前这三个名字两侧都是「未接」标记;现在两侧都是真策略。
#
# 每条 = (名字, 代码, 剧本)。剧本给 None 就是**缺省剧本**:页面加载得完、
# 每次求值都回 "null" —— 重试梯子跑满,报「js执行超时」
# (真身 NoStackTraceException,裁判标签 host:NoStackTraceException)。
_WV_URL = "https://difftest.example.com/wv"
WEBVIEW_API = [
    # 取整页(默认 JS 是 document.documentElement.outerHTML)
    ("webview-body", f"java.webView(null,'{_WV_URL}',null)",
     {"evals": ["\"<html>整页</html>\""]}),
    # 自带取值 js;第一拍还没就绪 → 走重试梯子
    ("webview-js", f"java.webView(null,'{_WV_URL}','getIt()')",
     {"evals": ["null", "\"第二拍\""]}),
    # html 非空 → loadDataWithBaseURL(url 可以是 null)
    ("webview-html", "java.webView('<b>本地页</b>',null,null)",
     {"evals": ["\"<b>本地页</b>\""]}),
    # cacheFirst 只改 WebSettings.cacheMode(本套看不见,结果应与第一条同)
    ("webview-cache-first", f"java.webView(null,'{_WV_URL}',null,true)",
     {"evals": ["\"<html>整页</html>\""]}),
    # 一直取不到 → 「js执行超时」
    ("webview-js-timeout", f"java.webView(null,'{_WV_URL}',null)", None),
    # 页面永远加载不完 → withTimeout 到点
    ("webview-never-finishes", f"java.webView(null,'{_WV_URL}',null)",
     {"pageFinished": False}),
    # url 与 html 都是 null:真身 `webView.loadUrl(url!!)` 抛 NPE
    ("webview-null-url", "java.webView(null,null,null)", None),
    # 嗅探:sourceRegex 命中资源 URL,回的是**那个 URL 本身**
    ("webview-source-regex",
     f"java.webViewGetSource(null,'{_WV_URL}',null,'.*\\.mp3')",
     {"loadResources": ["https://cdn.example/a.mp3"]}),
    # 不命中:嗅探客户端只能等超时
    ("webview-source-regex-miss",
     f"java.webViewGetSource(null,'{_WV_URL}',null,'.*\\.mp3')",
     {"loadResources": ["https://cdn.example/a.mp4"]}),
    # 正则编译不了 → PatternSyntaxException(Kotlin 的 matches 是整串匹配)
    ("webview-source-regex-bad",
     f"java.webViewGetSource(null,'{_WV_URL}',null,'(')",
     {"loadResources": ["https://cdn.example/a.mp3"]}),
    # 嗅探:overrideUrlRegex 命中跳转地址
    ("webview-override-url",
     f"java.webViewGetOverrideUrl(null,'{_WV_URL}',null,'.*/final')",
     {"overrideUrls": [{"url": "https://difftest.example.com/final",
                        "isRedirect": False}]}),
]


# ---- 「请用户出手」那三件(JsExtensions.kt L352-400)------------------------
#
# `startBrowser` / `startBrowserAwait` / `getVerificationCode` 底下都是
# `SourceVerificationHelp`(裁判挂的是**真身那 300 行**,被测侧是
# `net::verification` 的移植)。界面那一头两侧都换成**剧本用户**:
# case 的 `verify` 字段说「用户会怎么答」,契约见 fixtures/cases/js-host/README.md。
#
# 语料里这一族的写法几乎只有一种:`result.match(/Just a moment/)` 就弹一下、
# 过完再 `java.ajax` 抓一遍 —— 也就是「盾」那一族的正解。
_VF_URL = "https://difftest.example.com/verify"
# 带 `,{…}` 选项的同一个地址:头里塞了 UA(**过盾的关键**)、一条普通转发头,
# 以及内部网络选项 `CookieJar`(`toWebViewRequestConfig` 不许把它发给网站)
_VF_URL_OPT = (
    _VF_URL + ',{"headers":{"Referer":"https://difftest.example.com/",'
    '"User-Agent":"Source-UA","CookieJar":"1"}}'
)
_VF_URL_POST = _VF_URL + ',{"method":"POST","body":"a=1"}'
_VF_URL_UA = _VF_URL + ',{"headers":{"User-Agent":"Source-UA"}}'

VERIFY_API = [
    # 等用户过盾:回来的是 StrResponse(url 取用户那张页的地址)
    ("verify-browser-await",
     f"var r = java.startBrowserAwait('{_VF_URL}','验证'); r.body() + '|' + r.url()",
     {"result": "<html>过了</html>", "url": "https://difftest.example.com/ok"}),
    # 用户那侧没给地址 → `url2.ifEmpty { url }`:回退到入参那个
    ("verify-browser-await-no-url",
     f"java.startBrowserAwait('{_VF_URL}','验证').url()",
     {"result": "<html>过了</html>"}),
    # 用户把界面关掉 → 真身 checkResult 塞空结果 → 「验证结果为空」
    ("verify-browser-await-closed",
     f"java.startBrowserAwait('{_VF_URL}','验证').body()",
     {"close": True}),
    # 三参重载:refetchAfterSuccess = false(**这一位会关掉归并**,见 flightKey)
    ("verify-browser-await-no-refetch",
     f"java.startBrowserAwait('{_VF_URL}','验证',false).body()",
     {"result": "<html>过了</html>"}),
    # 四参重载:直接给一段 html(不去加载 url)
    ("verify-browser-await-html",
     f"java.startBrowserAwait('{_VF_URL}','验证',true,'<b>本地页</b>').body()",
     {"result": "<html>过了</html>"}),
    # 图片验证码:回的是用户打进去的那串
    ("verify-code", "java.getVerificationCode('https://difftest.example.com/cap.png')",
     {"result": "8Q2K"}),
    # 验证码那边把界面关掉,同样是「验证结果为空」
    ("verify-code-closed", "java.getVerificationCode('https://difftest.example.com/cap.png')",
     {"close": True}),
    # ---- 可见浏览器**怎么加载这一页**(观察面 `browserLoad`)----------------
    # 真身 `WebViewModel.initData` + `WebViewActivity` 那三行:`url,{…}` 由
    # AnalyzeUrl 拆开,UA 从头里单拎出来交给 WebSettings(跳转与子资源才一致),
    # `CookieJar` 是内部网络选项、不许发给网站。UA 这一位是**过盾的关键** ——
    # Cloudflare 的 clearance 认它,可见窗口用了别的 UA,过完照样被拦。
    ("verify-browser-await-url-options",
     f"java.startBrowserAwait('{_VF_URL_OPT}','验证').body()",
     {"result": "<html>过了</html>"}),
    # POST:真身先用 okhttp 那条路(`useWebView = false`)抓一份 HTML
    # **覆盖**入参那份,再 loadDataWithBaseURL —— 于是两侧各多一跳(hops)
    ("verify-browser-await-post",
     f"java.startBrowserAwait('{_VF_URL_POST}','验证').body()",
     {"result": "<html>过了</html>"}),
    # 计划算不出来(真身 `initData` 的 `execute {}` 走 onError:弹个 toast,
    # **界面照开、书源那一侧毫不知情**)—— 于是 `browserLoad` 是 null,
    # 而不是把 AnalyzeUrl 的异常抛回 JS
    ("verify-browser-await-bad-url",
     'java.startBrowserAwait(\'<js>throw new Error("boom")</js>\',\'验证\').body()',
     {"result": "<html>过了</html>"}),
    # 不等结果的那条:返回 undefined,观察面只有「界面弹了什么」
    ("verify-start-browser", f"typeof java.startBrowser('{_VF_URL}','标题')",
     {"result": "没人会读这一句"}),
    # `startBrowser` 那条也带加载计划(真身是同一个 Activity)
    ("verify-start-browser-url-options",
     f"typeof java.startBrowser('{_VF_URL_UA}','标题')",
     {"result": "没人会读这一句"}),
    # 三参重载(html)
    ("verify-start-browser-html",
     f"typeof java.startBrowser('{_VF_URL}','标题','<b>本地页</b>')",
     {"result": "没人会读这一句"}),
]


# ---- 手写:**章节实体在场**(`AnalyzeRule.setChapter`)----------------------
# 这一面此前**整块没判过**:裁判的 `host:"rule"` 从不 setChapter,被测侧照抄了
# 这一条,于是两侧的 `chapter` 都是 null、`chapter.putVariable(...)` 两侧都抛,
# 差分绿得理直气壮 —— 而产品的目录/正文两步是**带着章节实体**跑的。
# 语料里踩得到的是「就去看网」的 `ruleToc.chapterUrl`(见 CHAPTER_CORPUS)。
#
# 第三项是本 case 额外要带的字段;`chapter` 这个键在就是「setChapter 过」。
_CH = {
    "url": "https://difftest.example.com/book/1/2.html",
    "title": "第 12 章 混沌雷池",
    "baseUrl": "https://difftest.example.com/book/1",
    "bookUrl": "https://difftest.example.com/book/1",
    "index": 11,
    "isVip": False,
    "tag": "2026-01-02",
}
# 变量分两层的那一组:书那层放 b,章节那层放 c
_CH_VARS = {**_CH, "vars": {"层": "章节"}}

CHAPTER_HOST = [
    # —— 在场/缺席这一位本身有语义(缺席是 null,不是 undefined)——
    ("typeof", "[typeof chapter, chapter === null].join(',')", {"chapter": _CH}),
    ("absent", "[typeof chapter, chapter === null].join(',')", {}),
    # —— 字段面:与 book 同样走装箱层 ——
    ("fields",
     "[String(chapter.url), String(chapter.title), chapter.index,"
     " String(chapter.tag)].join('|')", {"chapter": _CH}),
    ("field-typeof", "[typeof chapter.title, typeof chapter.index].join(',')",
     {"chapter": _CH}),
    # —— Kotlin 的 `var isVip: Boolean` 生成的访问器叫 `isVip()`,于是 Rhino 的
    #    JavaBean 内省把**属性名**认成 `vip`,而 `chapter.isVip` 拿到的是**方法**
    #    本身(一个函数,恒真)。三个 `is*` 字段都是这个形状。
    ("bool-isvip", "[typeof chapter.isVip, typeof chapter.vip, chapter.vip].join(',')",
     {"chapter": _CH}),
    ("bool-isvip-call", "String(chapter.isVip())", {"chapter": _CH}),
    ("bool-isvolume",
     "[typeof chapter.isVolume, typeof chapter.volume, chapter.volume].join(',')",
     {"chapter": _CH}),
    ("bool-ispay", "[typeof chapter.isPay, typeof chapter.pay, chapter.pay].join(',')",
     {"chapter": _CH}),
    ("bool-truthy", "chapter.isVip ? '真' : '假'", {"chapter": _CH}),
    # —— putVariable:**返回 true**(真身无条件 return true,不是把值还回来)——
    ("put-variable", "[chapter.putVariable('z', '9'), String(java.get('z'))].join('|')",
     {"chapter": _CH}),
    ("put-variable-type", "typeof chapter.putVariable('z', '9')", {"chapter": _CH}),
    ("get-variable", "chapter.putVariable('z', '9'); String(chapter.getVariable('z'))",
     {"chapter": _CH}),
    # —— 章节在场时 book.putVariable 打到哪一层(java.get 读的是链)——
    ("book-put-variable", "[book.putVariable('b', '1'), String(java.get('b'))].join('|')",
     {"chapter": _CH}),
    # —— java.put 的优先级链:章节在场就写章节那层 ——
    ("java-put-layer", "java.put('层', '写'); String(java.get('层'))", {"chapter": _CH_VARS}),
    ("preset-layer", "String(java.get('层'))",
     {"chapter": _CH_VARS, "vars": {"层": "书"}}),
    # —— `java.get("title")` 的特判:章节在场给 chapter.title ——
    ("get-title", "String(java.get('title'))", {"chapter": _CH}),
    ("get-title-absent", "String(java.get('title'))", {}),
]

def hand_cases() -> list:
    cases = []
    for name, code in DIALECT:
        c = {"id": f"js-dialect-{name}", "op": "js", "code": code}
        c.update(FIELD_BIND)
        cases.append(c)
    for name, code in HOST_API:
        c = {"id": f"js-api-{name}", "op": "js", "code": code}
        c.update(HOST_API_BIND)
        cases.append(c)
    for name, code, script in WEBVIEW_API:
        c = {"id": f"js-api-{name}", "op": "js", "code": code}
        c.update(HOST_API_BIND)
        if script:
            c["webview"] = script
        cases.append(c)
    for name, code, script in VERIFY_API:
        c = {"id": f"js-api-{name}", "op": "js", "code": code}
        c.update(HOST_API_BIND)
        c["verify"] = script
        cases.append(c)
    for name, code in LIVE_CONNECT:
        c = {"id": f"js-lc-{name}", "op": "js", "code": code}
        c.update(HOST_API_BIND)
        cases.append(c)
    for name, code in HUTOOL_CRYPTO:
        c = {"id": f"js-hut-{name}", "op": "js", "code": code}
        c.update(HOST_API_BIND)
        cases.append(c)
    for name, rule, code in BIND_RESULT:
        cases.append({
            "id": f"js-bind-{name}", "op": "js", "code": code,
            "resultRule": rule, "content": _BIND_JSON,
            "baseUrl": "https://difftest.example.com/api/book/1",
        })
    for name, rule, code in BIND_RESULT_HTML:
        cases.append({
            "id": f"js-bind-{name}", "op": "js", "code": code,
            "resultRule": rule, "content": _BIND_HTML,
            "baseUrl": "https://difftest.example.com/book/1",
        })
    for name, code, content in ELEMENTS:
        cases.append({
            "id": f"js-els-{name}", "op": "js", "code": code,
            "content": content, "baseUrl": "https://difftest.example.com/search?k=1",
        })
    for name, code, extra in CHAPTER_HOST:
        c = {"id": f"js-ch-{name}", "op": "js", "code": code}
        c.update(FIELD_BIND)
        c.update(extra)
        cases.append(c)
    for entry in SOURCE_HOST:
        name, code = entry[0], entry[1]
        # BaseSource.evalJS 不绑 result/src/…,所以这里**不套** HOST_API_BIND
        c = {"id": f"js-src-{name}", "op": "js", "host": "source", "code": code}
        if len(entry) > 2:
            c.update(entry[2])
        cases.append(c)
    return cases


# ---- 语料:**`@js:` url 面**(op="analyzeUrl")--------------------------------
# 一条真实的 `searchUrl` 整条走 AnalyzeUrl 真身,JS 是真引擎 —— 钉的是
# 「书源写的那些 url JS 跑出什么 url / 方法 / 头 / body」。带上书源自己的
# `header`(headerMapF 不传 → getHeaderMap() 真的跑),于是 header 规则同期进套。
SOURCES = ROOT / "fixtures/sources"

# 书源整份太大,只带 url 与 header 这条路真正读得到的字段(两侧同一份,
# 故不影响判据;书源 JS 读 `source.ruleSearch` 那一面由 op="js" 的用例钉)。
SOURCE_FIELDS = [
    "bookSourceUrl", "bookSourceName", "bookSourceType", "bookSourceGroup",
    "bookSourceComment", "enabled", "enabledCookieJar", "header", "loginUrl",
    "loginUi", "concurrentRate", "jsLib", "variableComment", "searchUrl",
    "respondTime", "weight", "customOrder", "lastUpdateTime",
]

# **确定性垫片进不去 AnalyzeUrl 的内部求值**(`@js:`/`{{}}` 由真身自己 eval,
# 裁判侧没有「先在作用域里跑一段」的入口,而 jsLib 那条路在 jsharness 是桩)。
# 故摸时钟/随机的 url 规则不进本 op —— 语料里只有 25 条,且它们的 JS 片段
# 已经由 op="js" 的用例覆盖(那边垫片是前置拼接的)。
_NONDET = re.compile(r"\bDate\b|Math\.random|randomUUID")


def url_cases() -> list:
    seen = set()
    cases = []
    for f in sorted(SOURCES.glob("*.json")):
        data = json.loads(f.read_text(encoding="utf-8"))
        if not isinstance(data, list):
            continue
        for src in data:
            url = src.get("searchUrl")
            if not isinstance(url, str) or not url.strip():
                continue
            header = src.get("header") or ""
            if not isinstance(header, str):
                header = ""
            key = (url, header)
            if key in seen:
                continue
            trimmed = {k: src[k] for k in SOURCE_FIELDS if k in src}
            # 摸时钟/随机的整条都不进(书源常把 JS 藏在 `bookSourceComment` 里
            # 再 `eval(String(source.bookSourceComment))`,故要扫整份而不是只扫 url)
            if _NONDET.search(url + json.dumps(trimmed, ensure_ascii=False)):
                continue
            seen.add(key)
            h = hashlib.sha1(
                (url + "\u0000" + header).encode("utf-8")
            ).hexdigest()[:10]
            cases.append({
                "id": f"js-url-{h}",
                "op": "analyzeUrl",
                "mUrl": url,
                "baseUrl": src.get("bookSourceUrl") or "",
                "key": "斗破苍穹",
                "page": 2,
                "source": json.dumps(trimmed, ensure_ascii=False),
            })
    return cases


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    hand = hand_cases()
    corpus = corpus_cases()
    urls = url_cases()
    (OUT / "hand.json").write_text(
        json.dumps(hand, ensure_ascii=False, indent=1), encoding="utf-8"
    )
    (OUT / "corpus.json").write_text(
        json.dumps(corpus, ensure_ascii=False, indent=1), encoding="utf-8"
    )
    (OUT / "url.json").write_text(
        json.dumps(urls, ensure_ascii=False, indent=1), encoding="utf-8"
    )
    ids = [c["id"] for c in hand + corpus + urls]
    assert len(ids) == len(set(ids)), "case id 冲突"
    print(
        f"hand {len(hand)} + corpus {len(corpus)} + url {len(urls)} "
        f"= {len(ids)} cases -> {OUT}"
    )


if __name__ == "__main__":
    main()
