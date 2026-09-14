#!/usr/bin/env python3
"""**Phase 3 能力探测**:一个书源到底用没用到「C 层长尾」那几样东西。

这份是**唯一口径**:分层(`classify`)与 Phase 3 清单(`gen_phase3_checklist.py`)
读的是同一个探测器,不许再各抄一份。此前 `extract_js.py` 与
`gen_pipeline_corpus_cases.py` 各有一份**裸子串**的 classify:

    has_c = any(w in src_text for w in ("webView", "startBrowser", "webJs",
                                        "getVerificationCode", "queryTTF", "sourceRegex"))

`src_text` 是**整个源的 JSON dump**,于是 `"webJs": ""`、`"sourceRegex": ""`
这两个**空字段的键名**也算数 —— BookSource 的 ContentRule 默认就带这两个键。
实测 1704 源里 **92 个**(29 个本该是 A、63 个本该是 B)是这么被踢进 C 层的,
它们根本没用任何 Phase 3 能力,却因此从 A/B 两套流水线差分里整源缺席。
所以这里一律**按值判定**:键在不算数,值非空(且真值)才算数。

能力名与它对应的真身实现(判据要逐条落到这些实现上):

| 能力 | 真身 | 触发写法 |
|---|---|---|
| `webview.url_option`  | `AnalyzeUrl.useWebView` → `BackstageWebView` | URL 选项 `,{"webView":true}` |
| `webview.source_regex`| `BackstageWebView.SnifferWebClient.onLoadResource` | `ruleContent.sourceRegex` 非空 |
| `webview.web_js`      | `AnalyzeUrl.webJs`(先过 webView 跑这段) | `ruleContent.webJs` 非空 |
| `webview.js_api`      | `JsExtensions.webView/webViewGetSource/webViewGetOverrideUrl` | JS 里调用 |
| `webview.rule_prefix` | `AnalyzeRule` 的 `@webjs:` 前缀 | 规则串前缀 |
| `browser.start`       | `JsExtensions.startBrowser/startBrowserAwait` | JS 里调用 |
| `verify.code`         | `JsExtensions.getVerificationCode` | JS 里调用 |
| `login.url/ui/check`  | `SourceLogin*` | 三个字段非空 |
| `font.ttf`            | `QueryTTF` / `JsExtensions.replaceFont` | JS 里调用 |
| `fs.import_script`    | `JsExtensions.importScript` | JS 里调用 |
| `fs.cache_file`       | `JsExtensions.cacheFile` | JS 里调用 |
| `fs.download`         | `JsExtensions.downloadFile` | JS 里调用 |
| `fs.zip`              | `JsExtensions.getZipStringContent` | JS 里调用 |

**登录三件套不进分层**:`loginUrl`/`loginUi`/`loginCheckJs` 挂在源上,四步
(搜索/详情/目录/正文)一次都不会碰它们 —— 把它们算进 C 层等于把 803 个源
(绝大多数只是带了个空 `loginUrl` 键)踢出 A/B 套。它们仍进 Phase 3 清单,
只是走**另一条验收线**(手工:开登录页 → 存 cookie → 回四步)。
"""
import json
import re

# ---------------------------------------------------------------- 能力探测规则
#
# 每条:能力名 → (正则, 只在这些叶字段上看 / None = 看所有字符串值)。
# 「只在这些叶字段上看」的那几条是**字段本身就是能力**(值非空即触发),
# 正则那几条是**写法**(值里出现这种调用才触发)。

# 值非空即触发的字段(叶名 → 能力)
FIELD_CAPS = {
    "sourceRegex": "webview.source_regex",
    "webJs": "webview.web_js",
    "loginUrl": "login.url",
    "loginUi": "login.ui",
    "loginCheckJs": "login.check_js",
}

# 写法触发的能力(能力 → 正则)。
# `webView` 选项:`,{"webView":true}` / `{webView:true}` / 书源里手抄的**中文引号**
# `{webView:“true”}`(实测 10 处)都要认。真身 `useWebView()` 的假值面是
# null/""/false/"false",语料里**一例假值都没有**(实测 147 处全 true),
# 故这里只认「键出现且值不是显式假」。
PATTERN_CAPS = {
    "webview.url_option": re.compile(
        r'["\'“”]?webView["\'“”]?\s*[:=]\s*(?!["\'“”]?(?:false|null)\b|["\'“”]{2})'
    ),
    "webview.js_api": re.compile(r'\bwebView(?:GetSource|GetOverrideUrl)?\s*\('),
    "webview.rule_prefix": re.compile(r'@webjs:'),
    "browser.start": re.compile(r'\bstartBrowser(?:Await)?\s*\('),
    "verify.code": re.compile(r'\bgetVerificationCode\s*\('),
    "font.ttf": re.compile(r'\b(?:queryTTF|queryBase64TTF|replaceFont)\s*\('),
    "fs.import_script": re.compile(r'\bimportScript\s*\('),
    "fs.cache_file": re.compile(r'\bcacheFile\s*\('),
    "fs.download": re.compile(r'\bdownloadFile\s*\('),
    "fs.zip": re.compile(r'\bgetZipStringContent\s*\('),
}

# 能力 → 它属于哪条验收线(见 fixtures/phase3/README.md)
LINES = {
    "webview.url_option": "webview",
    "webview.source_regex": "webview",
    "webview.web_js": "webview",
    "webview.js_api": "webview",
    "webview.rule_prefix": "webview",
    "browser.start": "browser",
    "verify.code": "verify",
    "login.url": "login",
    "login.ui": "login",
    "login.check_js": "login",
    "font.ttf": "font",
    "fs.import_script": "fs",
    "fs.cache_file": "fs",
    "fs.download": "fs",
    "fs.zip": "fs",
}

# 登录三件套挂在源上,四步走不到 —— 不进分层(理由见模块头注释)
NOT_IN_TIER = {"login.url", "login.ui", "login.check_js"}

# 字段路径的根 → 四步里的哪一步。清单按「哪一步会踩到」分堆,
# 因为验收是**逐步跑**的:webView 在 searchUrl 上和在 ruleToc 上是两条不同的路。
STEP_OF_ROOT = {
    "searchUrl": "search",
    "ruleSearch": "search",
    "exploreUrl": "explore",
    "ruleExplore": "explore",
    "ruleBookInfo": "info",
    "ruleToc": "toc",
    "ruleContent": "content",
    "loginUrl": "login",
    "loginUi": "login",
    "loginCheckJs": "login",
}


def _walk(obj, path=""):
    """(字段路径, 字符串值)。列表下标压成 `[]` —— 路径是给人读的,不是定位器"""
    if isinstance(obj, dict):
        for k, v in obj.items():
            yield from _walk(v, f"{path}.{k}" if path else k)
    elif isinstance(obj, list):
        for v in obj:
            yield from _walk(v, path + "[]")
    elif isinstance(obj, str):
        yield path, obj


def step_of(path):
    return STEP_OF_ROOT.get(path.split(".")[0].split("[")[0], "source")


def hits(src):
    """一个源用到的 Phase 3 能力。返回 [{cap, field, step, snippet}],按字段序。

    `src` 是**书源 dict**(不是 JSON 文本)—— 按值判定要求能分清「键在」与
    「值非空」,文本 dump 分不清,那正是老 classify 的 bug。
    """
    out = []
    for path, value in _walk(src):
        leaf = path.split(".")[-1].split("[")[0]
        cap = FIELD_CAPS.get(leaf)
        if cap is not None:
            if value.strip():
                out.append({"cap": cap, "field": path, "step": step_of(path),
                            "snippet": _snip(value)})
            # 这几个字段名本身不该再被下面的写法正则扫一遍(`webJs` 的值是 JS,
            # 里面出现 `webView(` 才算 js_api —— 那条下面照样能扫到)
        for name, rx in PATTERN_CAPS.items():
            m = rx.search(value)
            if m:
                out.append({"cap": name, "field": path, "step": step_of(path),
                            "snippet": _snip(value, m.start())})
    return out


def _snip(value, at=0, width=120):
    """取触发处附近一小段(清单是给人 review 的,整段规则塞进去没法看)"""
    one = " ".join(value.split())
    if len(one) <= width:
        return one
    start = max(0, min(at, len(one) - width))
    return ("…" if start else "") + one[start:start + width] + "…"


def caps(src):
    """这个源用到的能力集合(有序去重)"""
    seen = []
    for h in hits(src):
        if h["cap"] not in seen:
            seen.append(h["cap"])
    return seen


def classify(src):
    """A / B / C 分层。

    - **C** = 四步路径上用到了 Phase 3 能力(登录三件套不算,见模块头注释);
    - **B** = 其余里带 `<js>` / `@js:` / `jsLib` 的;
    - **A** = 剩下的。

    参数是**书源 dict**。老口径收的是 JSON 文本,那正是空键误判的来源。
    """
    if isinstance(src, str):        # 老调用点的兜底:文本进来就没法按值判定
        raise TypeError("classify() 收书源 dict,不是 JSON 文本(按值判定要分清键在与值非空)")
    if any(c not in NOT_IN_TIER for c in caps(src)):
        return "C"
    text = json.dumps(src, ensure_ascii=False)
    return "B" if ("<js>" in text or "@js:" in text or '"jsLib"' in text) else "A"
