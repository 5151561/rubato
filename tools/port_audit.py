#!/usr/bin/env python3
"""**移植对照台账**:真身的每一个面,我们到底是移植了、是桩、是砍了,还是**没比过**。

## 为什么有这个东西

到 M3o 为止,Phase 3 的驱动力一直是**探针**:跑一遍逐源验收 → 照出一处缺口 →
修 → 再跑。三次真缺口里有两次本来是**读得出来**的,不是挖出来的 ——
`onPageFinished` 每次都要重排求值(M3h)明写在 `HtmlWebViewClient` 里,
可见浏览器的 UA(M3n)明写在 `WebViewModel.initData` 里。真身 1159 个 kt 文件
一直躺在 `judge/engine/`,缺的从来不是源码,是**一份能回答「这个面比过没有」的账**。

M3h 那句话是这份台账的全部理由:

> **「差分全绿」不等于「面都比过了」,只等于「比过的面一致」。**

差分能守住「比过的面」,守不住「没进剧本的那一位」。台账守的就是后者。

## 口径

**面**(`FACES`)是**机械枚举**的,不是手写清单 —— 真身加一个方法,
这里下一次就会多一行「台账里没有」。四个面:

| 面 | 枚举源 | 枚举法 |
|---|---|---|
| `jsext`  | `help/JsExtensions.kt`            | `fun <名>(` |
| `urlopt` | `AnalyzeUrl.kt` 的 `class UrlOption` | `private var <名>:` |
| `field`  | `BookSource.kt` + `data/entities/rule/*.kt` | 构造参数 `var/val <名>:` |
| `liveconnect` | **语料**(`fixtures/sources/*.json`) | `Packages.<包>.<类>` 全名 + `with(JavaImporter)` 里的简单名 |

**`liveconnect` 那一面的枚举源为什么是语料**:前三面都能从 kt 文件里捞出全集,
它不能 —— Rhino 的白名单是「整个 classpath 减掉 `RhinoClassShutter` 那张黑名单」,
而 classpath 是 JDK + Android + hutool + okhttp + jsoup 的全部,枚举它既不可能也没意义。
**能枚举的是需求侧**:书源真的引到了哪些类。于是它的「多一行」是
「语料里出现一个新类」,同样机械、同样会自己长出来。细则见 `face_liveconnect` 的注释。

**分母**(`用量`)按 `fixtures/sources/*.json` 实算,**按源计**(不是按次)。
数字一律现算,**别手抄进文档** —— 这仓库栽过两次。

**状态**是人写的,落在 `docs/port-ledger.md`(入库,它是判据不是生成物):

- `移植` —— 逐行照搬且**进了差分**;`依据` 写哪一套。
- `桩`   —— 两侧成对的确定性桩(判据建在桩的契约上),`依据` 写桩在哪。
- `不做` —— 砍单(plan §6)。**用量 > 0 就必须写理由**,不许空着。
- `待做` —— 判过了、**该做、还没做**。`依据` 写分母与真身出处 —— 这一档就是待办本身。
- `未比` —— 实现了,但**这一位差分表达不了**(M3h 那类 bug 的藏身处)。
- `未判` —— 还没人看过。`--sync` 新增的行都是这一档,它就是欠账本身。

`supply` 那一列是**证据不是结论**:在 `rust/crates/*/src` 里搜得到同名符号
而已。台账说了算,工具只负责**对账**:真身有而台账没有、台账有而真身没了、
状态与证据矛盾 —— 这三件 `--check` 会拦。

## 用法

    tools/port_audit.py                 # 打表:面 × 状态 × 用量
    tools/port_audit.py --face=jsext    # 只看一个面
    tools/port_audit.py --todo          # 只看还欠着的(未判 / 未比 / 有用量却不做)
    tools/port_audit.py --sync          # 真身新增的面补进台账(记 未判),真身删掉的标出来
    tools/port_audit.py --check         # 对不上就非零退出(缺项 / 多项 / 矛盾)
"""
import argparse
import json
import pathlib
import re
import sys
from collections import defaultdict

ROOT = pathlib.Path(__file__).resolve().parent.parent
LEGADO = ROOT / "judge/engine/src/main/java/io/legado/app"
CORPUS = sorted((ROOT / "fixtures/sources").glob("*.json"))
LEDGER = ROOT / "docs/port-ledger.md"
RUST = ROOT / "rust/crates"

STATUSES = ("移植", "桩", "不做", "待做", "未比", "未判")

# ------------------------------------------------------------------ 面的枚举
#
# 每个面:从真身源码里机械捞出符号名。手写清单会漂,枚举不会。


# 一行台账的键:单文件的面就是名字本身;字段面按 `实体::字段` ——
# `title` 在 `ExploreKind`(发现分类)与 `ContentRule`(正文标题)里各有一个,
# 合成一行就没法分别判(头一版合过,`style` 那一族当场判错)。


def face_jsext() -> dict[str, dict[str, str]]:
    """`JsExtensions.kt` 的方法面 —— 书源 JS 里 `java.xxx(...)` 能调到的全集。"""
    text = (LEGADO / "help/JsExtensions.kt").read_text(encoding="utf-8")
    names = sorted(set(re.findall(r"^\s*(?:suspend\s+)?fun\s+(\w+)\s*\(", text, re.M)))
    return {n: {"name": n, "origin": "JsExtensions.kt"} for n in names}


def face_urlopt() -> dict[str, dict[str, str]]:
    """`AnalyzeUrl.UrlOption` 的字段面 —— URL 尾巴 `,{...}` 里认得的键。"""
    text = (LEGADO / "model/analyzeRule/AnalyzeUrl.kt").read_text(encoding="utf-8")
    body = text.split("class UrlOption", 1)[1].split("\n    }", 1)[0]
    names = sorted(set(re.findall(r"private var (\w+):", body)))
    return {n: {"name": n, "origin": "AnalyzeUrl.kt"} for n in names}


def face_field() -> dict[str, dict[str, str]]:
    """书源与规则对象的字段面 —— 书源 JSON 里认得的键,**按实体分行**。"""
    files = [LEGADO / "data/entities/BookSource.kt"]
    files += sorted((LEGADO / "data/entities/rule").glob("*.kt"))
    out: dict[str, dict[str, str]] = {}
    for f in files:
        text = f.read_text(encoding="utf-8")
        # 构造参数与属性都是 `var x: T` / `val x: T`
        for n in re.findall(r"^\s*(?:var|val) (\w+):", text, re.M):
            out[f"{f.stem}::{n}"] = {"name": n, "origin": f.name}
    return dict(sorted(out.items()))


# ---- liveconnect:书源绕开 `java.*` 直接调的 Java 类 ------------------------
#
# **这一面的枚举源不是真身源码,是语料** —— 别的面都能从 kt 文件里机械捞出全集,
# 这一面不能:Rhino 的白名单是「整个 classpath 减掉 `RhinoClassShutter` 那张黑名单」,
# 而 classpath 是 JDK + Android + hutool + okhttp + jsoup 的全部。枚举那个没有意义,
# 也不可能穷尽。**能枚举的是需求侧**:书源真的引到了哪些类。于是这一面的
# 「真身加一个面这里就多一行」变成「语料出现一个新类这里就多一行」,同样是机械的。
#
# 两条路都要认(Rhino 的名字解析就是这两条):
#   ① 全限定名 —— `Packages.javax.crypto.Cipher` / `org.jsoup.Jsoup.parse(...)`;
#   ② `importPackage` 进来的简单名 —— 只在 `with (javaImport) { … }` 里解析得到,
#      故只在 `with` 之后的正文里认,且要用在**调用位**(`X(` / `new X(` / `X.m(`)。
# `java.` 后面直接跟大写的**不算**(`java.HMacBase64` 是 AnalyzeRule 宿主上的方法,
# `java` 不是包 —— 与 js-host/src/live_connect.rs 的包树注释同一条)。
# JS 内建重名的按 SHADOWED 那张小表处理:`with` 块里 java.lang.Object 真的会把
# `Object` 遮住(探针 js-lc-shadow-object-keys),不认它这一面就整片看不见。
_LC_ROOTS = r"(?:java|javax|android|androidx|org|okhttp3|okio|cn|com|kotlin|dalvik)"
LC_FQN = re.compile(rf"(?:Packages\.)?\b({_LC_ROOTS}(?:\.[a-z][A-Za-z0-9_]*)*\.[A-Z][A-Za-z0-9_$]*)\b")
LC_PKG = re.compile(rf"Packages\.({_LC_ROOTS}(?:\.[a-z][A-Za-z0-9_]*)+)")
LC_HINT = re.compile(r"Packages\.|JavaImporter|importClass\s*\(|importPackage\s*\(")
LC_WITH = re.compile(r"\bwith\s*\(")
LC_USE = re.compile(r"(?<![.\w$])([A-Z][A-Za-z0-9_$]*)\s*(?:\(|\.\s*[a-z][A-Za-z0-9_$]*\s*\()")
LC_DECL = re.compile(r"\b(?:var|let|const|function|class)\s+([A-Za-z_$][A-Za-z0-9_$]*)")
LC_JS_BUILTIN = set(
    "Array Object String Number Boolean Math JSON Date RegExp Error TypeError SyntaxError "
    "RangeError EvalError URIError Function Symbol Map Set WeakMap WeakSet Promise Proxy "
    "Reflect Int8Array Uint8Array Int16Array Uint16Array Int32Array Uint32Array Float32Array "
    "Float64Array ArrayBuffer DataView JavaImporter Packages".split()
)
# 导了这个包之后,这些 JS 内建的名字会被同名 Java 类**遮住**(Rhino 的 with 语义)
LC_SHADOWED = {
    "java.lang": {"String", "Object", "Number", "Boolean", "Math", "Error"},
    "java.util": {"Date", "Map", "Set"},
}


def _lc_strings(node, out: list[str]) -> None:
    """一个源里所有字符串值(含「字符串包 JSON」那一层)。"""
    if isinstance(node, dict):
        for v in node.values():
            _lc_strings(v, out)
    elif isinstance(node, list):
        for v in node:
            _lc_strings(v, out)
    elif isinstance(node, str):
        out.append(node)
        text = node.strip()
        if text[:1] in ("[", "{"):
            try:
                _lc_strings(json.loads(text), out)
            except ValueError:
                pass


def _lc_names(src: dict) -> dict[str, str]:
    """一个源里引到的 Java 类 → 它是怎么被引到的(`Packages.<包>` 还是 `JavaImporter`)。"""
    hit: dict[str, str] = {}
    values: list[str] = []
    _lc_strings(src, values)
    for v in values:
        if not LC_HINT.search(v):
            continue
        for fq in LC_FQN.findall(v):
            pkg, simple = fq.rsplit(".", 1)
            if pkg == "java":       # `java.HMacBase64` —— 宿主对象上的方法,不是包
                continue
            hit[simple] = pkg
        m = LC_WITH.search(v)
        if not m:
            continue
        declared = set(LC_DECL.findall(v))
        shadowed = {n for p in set(LC_PKG.findall(v)) for n in LC_SHADOWED.get(p, ())}
        for n in set(LC_USE.findall(v[m.end():])):
            if n in declared or (n in LC_JS_BUILTIN and n not in shadowed):
                continue
            hit.setdefault(n, "JavaImporter")
    return hit


def face_liveconnect() -> dict[str, dict[str, str]]:
    """LiveConnect 的类面 —— 用量在这里就一起算了(见 `demand`)。"""
    out: dict[str, dict[str, str]] = {}
    for src in load_sources():
        for name, origin in _lc_names(src).items():
            row = out.setdefault(name, {"name": name, "origin": origin, "used": 0})
            row["used"] += 1
            if row["origin"] == "JavaImporter" and origin != "JavaImporter":
                row["origin"] = origin      # 语料里但凡有一处写全名,就按全名记出处
    return dict(sorted(out.items()))


FACES = {
    "jsext": (face_jsext, "help/JsExtensions.kt", "书源 JS 调得到的宿主方法"),
    "urlopt": (face_urlopt, "AnalyzeUrl.kt::UrlOption", "URL 尾巴 ,{...} 的选项键"),
    "field": (face_field, "data/entities/{BookSource,rule/*}.kt", "书源 JSON 的字段"),
    "liveconnect": (face_liveconnect, "com/script/rhino/RhinoClassShutter.kt(黑名单)",
                    "书源绕开 `java.*` 直接调的 Java 类(**面从语料枚举**,口径见 port_audit.py)"),
}

# ------------------------------------------------------------------ 分母:语料用量
#
# 整份语料只扫一遍,每个源抽出三样东西,再与面求交 —— 逐面逐名去 grep 一遍语料
# 是 170 × 1704 次扫描,那样慢得没法进判据。

CALL_RE = re.compile(r"\.(\w+)\s*\(")
# 真身用 Gson 解这段尾巴,而 **Gson 缺省是宽松的**:键可以不加引号。语料里
# `,{webView:“true”}`(连引号都是中文的)实测 35 个源这么写 —— 只认标准 JSON
# 会把它们整片漏掉(头一版就漏了,63 vs 98)。与 `phase3_caps.py` 同一口径。
OPT_KEY_RE = re.compile(r'["\'“”]?(\w+)["\'“”]?\s*[:=]')


def option_keys(text: str) -> set[str]:
    """一个字符串值里所有 `,{...}` 选项块的**顶层**键。

    三处不能省(每一处都是实测漏出来的):① 一个值里可能有**好几个** `,{...}`
    (`exploreUrl` 一行一个 URL),只取最后一个会漏掉一多半;② 只认**顶层**键 ——
    `headers`/`body` 是嵌套对象,里面的 `User-Agent` 不是选项;③ 键**可以不加
    引号**(见 `OPT_KEY_RE`)。三处补齐之后 `webView` 63 → 98,与
    `phase3_caps.py` 那份独立口径**逐源对上**(0 分歧)。
    """
    keys: set[str] = set()
    for m in re.finditer(r",\{", text):
        depth, i = 0, m.end() - 1
        while i < len(text):
            ch = text[i]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    break
            elif depth == 1 and (ch.isalpha() or ch in "\"'“”"):
                k = OPT_KEY_RE.match(text, i)
                if k:
                    keys.add(k.group(1))
                    i = k.end() - 1
            i += 1
    return keys


def load_sources() -> list[dict]:
    out: list[dict] = []
    for f in CORPUS:
        data = json.loads(f.read_text(encoding="utf-8"))
        out.extend(data if isinstance(data, list) else [data])
    return out


def _walk(node, calls: set, optkeys: set, fields: set) -> None:
    """一个源里:调用名 / URL 选项键 / 非空字段名。"""
    if isinstance(node, dict):
        for k, v in node.items():
            if v not in (None, "", [], {}, False):
                fields.add(k)
            _walk(v, calls, optkeys, fields)
    elif isinstance(node, list):
        for v in node:
            _walk(v, calls, optkeys, fields)
    elif isinstance(node, str):
        calls.update(CALL_RE.findall(node))
        if ",{" in node:
            optkeys.update(option_keys(node))
        # **字段面有一半藏在字符串里**:`exploreUrl` 的发现分类(ExploreKind)与
        # `loginUi` 的表单(RowUi)都是「字符串包 JSON」。不往里走的话这两族
        # 字段的用量恒为 0 —— 而 `exploreUrl` 非空的有近千个源,那个 0 是假的。
        text = node.strip()
        if text[:1] in ("[", "{"):
            try:
                _walk(json.loads(text), calls, optkeys, fields)
            except ValueError:
                pass


def ambiguous(faces) -> dict[str, set[str]]:
    """**同名跨实体**的那些名字:用量是按名算的,`ReviewRule::enabled` 与
    `BookSource::enabled` 会合在一个数里。打表时给它们标个 `*`,别把
    「段评那一族 1682 个源在用」这种话当真。"""
    out = {}
    for face, items in faces.items():
        seen, dup = set(), set()
        for it in items.values():
            (dup if it["name"] in seen else seen).add(it["name"])
        out[face] = dup
    return out


def demand(sources: list[dict], faces: dict) -> dict[str, dict[str, int]]:
    counts = {face: defaultdict(int) for face in FACES}
    for src in sources:
        calls, optkeys, fields = set(), set(), set()
        _walk(src, calls, optkeys, fields)
        for n in calls:
            counts["jsext"][n] += 1
        for n in optkeys:
            counts["urlopt"][n] += 1
        for n in fields:
            counts["field"][n] += 1
    # liveconnect 的用量在枚举那一趟就数过了(它的「面」本来就是从语料捞的)
    for it in faces["liveconnect"].values():
        counts["liveconnect"][it["name"]] = it["used"]
    return counts


# ------------------------------------------------------------------ 证据:被测侧有没有
#
# 只是「搜得到同名符号」,**不是结论**。结论写在台账里。


# 每个面只在**它该在的地方**找 —— 整个 rust/ 搜一遍等于没搜:`name`/`type`/`url`
# 这种名字哪儿都有,头一版把 `ruleReview`(砍了的)之外的字段面全判成「有」。
SUPPLY_SCOPE = {
    "jsext": ["rust/crates/js-host/src"],
    "urlopt": ["rust/crates/net/src"],
    # 字段面此前有一半落在 Dart(`loginUi` 的 RowUi 表单是引擎原样交出去、界面自己解的)。
    # Flutter 前端移除之后这一半没有了 —— 不是把 `app/lib` 留在这儿当摆设:
    # 扫不到的目录会让台账「搜得到就算有」的那一档凭空变绿,判据当场骗人。
    # 受影响的 RowUi 那八行已在 docs/port-ledger.md 里改判为「待做」。
    "field": ["rust/crates/rubato-core/src/entities.rs"],
    "liveconnect": ["rust/crates/js-host/src"],
}

# 名字能以**裸符号**出现的面(方法名、类名);键面(字段/选项)只会是字面量
SYMBOL_FACES = ("jsext", "liveconnect")


def rust_index() -> dict[str, str]:
    index = {}
    for face, scopes in SUPPLY_SCOPE.items():
        parts = []
        for scope in scopes:
            path = ROOT / scope
            files = [path] if path.is_file() else sorted(path.rglob("*.rs"))
            parts += [f.read_text(encoding="utf-8", errors="replace") for f in files]
        index[face] = "\n".join(parts)
    return index


def snake(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def supply(index: dict[str, str], face: str, name: str) -> bool:
    """**证据不是结论**:被测侧那一面里搜得到这个名字(键面字面量或同名符号)。"""
    text = index[face]
    if f'"{name}"' in text or f"'{name}'" in text:
        return True
    # 键面(字段/选项)只会以**字面量**出现;方法/类名面还认同名符号
    # (`java.longToast = function …` / `Cipher: Cipher` 那种裸写法,
    #  与 snake_case 的 rust 函数名)
    if face not in SYMBOL_FACES:
        return False
    return re.search(rf"\b(?:{re.escape(name)}|{re.escape(snake(name))})\b", text) is not None


# ------------------------------------------------------------------ 台账读写

ROW_RE = re.compile(r"^\|\s*`([^`]+)`\s*\|([^|]*)\|([^|]*)\|([^|]*)\|([^|]*)\|\s*$")


def read_ledger() -> dict[str, dict[str, dict[str, str]]]:
    """→ {face: {name: {"rubato":…, "status":…, "why":…}}}"""
    out: dict[str, dict[str, dict[str, str]]] = {f: {} for f in FACES}
    if not LEDGER.exists():
        return out
    face = None
    for line in LEDGER.read_text(encoding="utf-8").splitlines():
        m = re.match(r"^##\s+`(\w+)`", line)
        if m and m.group(1) in FACES:
            face = m.group(1)
            continue
        if face is None:
            continue
        m = ROW_RE.match(line)
        if not m:
            continue
        name, _origin, rubato, status, why = (x.strip() for x in m.groups())
        out[face][name] = {"rubato": rubato, "status": status, "why": why}
    return out


def write_ledger(ledger, faces) -> None:
    lines = [
        "# 移植对照台账",
        "",
        "**这份是判据,不是生成物** —— 状态与理由是人写的,`tools/port_audit.py --check`",
        "只负责对账:真身有而这里没有、这里有而真身没了、状态与证据矛盾。",
        "口径、状态那五档的定义、怎么跑,全在 `tools/port_audit.py` 的文件头。",
        "",
        "**数字不写在这里**(会跟代码悄悄漂):用量跑 `tools/port_audit.py` 现算。",
        "",
    ]
    for face, (_, origin, desc) in FACES.items():
        lines += [
            f"## `{face}` —— {desc}", "", f"真身:`{origin}`", "",
            "| 真身 | 出处 | rubato | 状态 | 依据 |", "|---|---|---|---|---|",
        ]
        rows = ledger.get(face, {})
        for key in sorted(set(rows) | set(faces[face])):
            row = rows.get(key, {"rubato": "", "status": "未判", "why": ""})
            gone = " *(真身已无此面)*" if key not in faces[face] else ""
            src = faces[face].get(key, {}).get("origin", "—")
            lines.append(f"| `{key}` | {src} | {row['rubato']} | {row['status']}{gone} | {row['why']} |")
        lines.append("")
    LEDGER.write_text("\n".join(lines), encoding="utf-8")


# ------------------------------------------------------------------ 对账

def audit(faces, counts, ledger, index):
    """→ [(严重程度, face, name, 一句话)]  严重程度:'err' 拦判据 / 'debt' 只记账"""
    problems = []
    for face in FACES:
        rows = ledger[face]
        for key, item in faces[face].items():
            name, used = item["name"], counts[face].get(item["name"], 0)
            row = rows.get(key)
            if row is None:
                problems.append(("err", face, key, "台账里没有这一行(真身有这个面)"))
                continue
            st = row["status"]
            if st not in STATUSES:
                problems.append(("err", face, key, f"状态 `{st}` 不在那几档里"))
            elif st == "未判":
                problems.append(("debt", face, key, f"还没人判过(用量 {used} 源)"))
            elif st == "待做":
                if not row["why"]:
                    problems.append(("err", face, key, "「待做」得写清分母与真身出处"))
                else:
                    problems.append(("debt", face, key, f"待做(用量 {used} 源):{row['why']}"))
            elif st == "未比":
                problems.append(("debt", face, key, f"实现了但这一位差分表达不了(用量 {used} 源)"))
            elif st == "不做" and used > 0 and not row["why"]:
                problems.append(("err", face, key, f"砍了却有 {used} 个源在用,而且没写理由"))
            elif st in ("移植", "桩") and not supply(index, face, name):
                problems.append(("err", face, key, f"台账说「{st}」,但被测侧那一面里搜不到 `{name}`"))
        for key in rows:
            if key not in faces[face]:
                problems.append(("err", face, key, "台账有这一行,真身已经没有了(基准升级?)"))
    return problems


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--face", choices=list(FACES), help="只看一个面")
    ap.add_argument("--todo", action="store_true", help="只列还欠着的")
    ap.add_argument("--sync", action="store_true", help="把真身新增的面补进台账(记 未判)")
    ap.add_argument("--check", action="store_true", help="对不上就非零退出")
    args = ap.parse_args()

    faces = {name: fn() for name, (fn, _, _) in FACES.items()}
    counts = demand(load_sources(), faces)
    ledger = read_ledger()

    if args.sync:
        write_ledger(ledger, faces)
        print(f"台账已同步:{LEDGER.relative_to(ROOT)}")
        ledger = read_ledger()

    index = rust_index()
    amb = ambiguous(faces)
    problems = audit(faces, counts, ledger, index)

    shown = [f for f in FACES if not args.face or f == args.face]
    if args.todo:
        for sev, face, name, msg in problems:
            if face in shown:
                print(f"{'!!' if sev == 'err' else '  '} {face:7s} {name:28s} {msg}")
    else:
        for face in shown:
            _, origin, desc = FACES[face]
            by_status = defaultdict(list)
            for key, item in faces[face].items():
                by_status[ledger[face].get(key, {}).get("status", "未判")].append(key)
            total_used = sum(1 for it in faces[face].values() if counts[face].get(it["name"], 0) > 0)
            print(f"\n== {face}({desc};真身 {origin})")
            print(f"   面 {len(faces[face])} 个,语料里用得到 {total_used} 个")
            for st in STATUSES:
                names = by_status.get(st, [])
                if not names:
                    continue
                used = sorted(
                    (k for k in names if counts[face].get(faces[face][k]["name"], 0) > 0),
                    key=lambda k: -counts[face][faces[face][k]["name"]],
                )
                tail = "  用量:" + ", ".join(
                    f"{k}({counts[face][faces[face][k]['name']]}"
                    f"{'*' if faces[face][k]['name'] in amb[face] else ''})"
                    for k in used[:6]
                ) if used else ""
                if len(used) > 6:
                    tail += f" …共 {len(used)} 个"
                print(f"   {st}  {len(names):3d}{tail}")

    errs = [p for p in problems if p[0] == "err"]
    debts = [p for p in problems if p[0] == "debt"]
    if any(amb[f] for f in shown):
        print("\n(带 * 的用量是**按名**合并的:同一个名字在几个实体里都有)")
    print(f"\n对账:错 {len(errs)} 条、欠账 {len(debts)} 条(未判 + 待做 + 未比)")
    if args.check:
        for _, face, name, msg in errs:
            print(f"!! {face} {name}:{msg}", file=sys.stderr)
        if errs:
            print("\n台账对不上。真身新增的面跑 --sync 补进去,再逐条判。", file=sys.stderr)
            return 1
        print("移植对照台账:✅(面与台账对得上)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
