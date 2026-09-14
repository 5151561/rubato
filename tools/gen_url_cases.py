#!/usr/bin/env python3
"""生成 java-url 差分用例(NetworkUtils.getAbsoluteURL / java.net.URL)。

两部分:
1. handmade.json:base × spec 的交叉矩阵,覆盖 JDK parseURL 的每条分支
   (继承 scheme、authority 重置 path、queryOnly、相对路径 ./../ 归一、
   未知协议、坏端口、双 @、IPv6、前后空白、`url:` 前缀);
2. corpus.json:从 fixtures/sources 抽真实 bookSourceUrl 作 base、
   真实相对 url 作 spec 的采样交叉。
"""
import json
import random
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = ROOT / "fixtures" / "sources"
OUT_DIR = ROOT / "fixtures" / "cases" / "java-url"

BASES = [
    "http://www.example.com",
    "http://www.example.com/",
    "http://www.example.com/a/b/c.html",
    "http://www.example.com/a/b/",
    "http://www.example.com/a/b/c.html?x=1&y=2",
    "http://www.example.com/a/b/c.html#frag",
    "http://www.example.com:8080/a/b.html",
    "https://user:pw@www.example.com/a/b",
    "https://www.example.com/搜索/книга.html",
    "http://[::1]:8080/a/b",
    "http://192.168.1.1/x",
    "ftp://example.com/a/b",
    "file:///a/b/c.txt",
    "http://example.com?only=query",
]

SPECS = [
    "", " ", "   \t\n ",
    "d.html", "./d.html", "../d.html", "../../d.html", "../../../../d.html",
    "sub/d.html", "sub/../d.html", "sub/./d.html", "a/b/..", "a/b/.", "a/b/../",
    "/abs/path", "/abs/../path", "/../../a", "/./a", "//other.com/x", "///x", "////unc",
    "?q=1", "?", "#top", "#", "d.html?q=1#f", "d.html#", "d.html?",
    "http://other.com/z", "https://other.com/z", "HTTP://Other.com/z",
    "http:d.html", "https:d.html", "HTTP:/d.html", "ftp://other.com/z",
    "data:image/png;base64,AAAA", "data:text/plain,hi",
    "javascript:void(0)", "javascriptx", "JavaScript:x",
    "thunder://xxx", "ed2k://yyy", "magnet:?xt=1", "mailto:a@b.c",
    " spaced.html ", "\tspaced.html\n", "url:d.html", "URL:/d.html", "urlx.html",
    "中文页.html", "a b.html", "a%20b.html", "a+b.html", "a&b=1",
    ":colon", "x:y", "1abc:def", "a-b+c.d:x", "://noproto",
    "http://a:80x/y", "http://a:/y", "http://a:8080", "//u@a@b.com/x", "//@host/x",
    "//[::1]/x", "//[bad/x",
    "?a=b&c=d#e", "e#f?g",
    # scheme 前缀 + query:parseURL 的 indexOf('?') 从 0 起算的怪癖
    "http:?q=1", "https:a?b", "url:?x", "http:/a?b#c", "https://a.com?b",
    # 相对路径归一化的边角
    "../../../x", "a/../../x", "./../x", "a/b/c/../../d", "a//b", "a/", "..", ".",
    "./", "../", "a/..", "./a/../b", "x/./../y", "/a/./b/../c/",
    # 主机非法字符与 {{}} 模板(语料里大量出现)
    "//a b.com/x", "//a{{}}b.com/x", "//a\"b.com/x", "//a[b].com/x", "//a|b.com/x",
    "{{page}}.html", "/list/{{page}}.html", "?page={{page}}",
    # 空白与控制字符
    "\u0001x.html", "a\u0000b.html", "  ", "\n\n",
]


def cross(prefix, bases, specs, ops=("absUrl",)):
    out = []
    n = 0
    for b in bases:
        for s in specs:
            for op in ops:
                out.append({"id": f"{prefix}{n:05d}", "op": op, "base": b, "path": s})
                n += 1
    return out


def corpus():
    bases, specs = set(), set()
    for f in sorted(SRC_DIR.glob("*.json")):
        for src in json.load(open(f, encoding="utf-8")):
            u = src.get("bookSourceUrl")
            if isinstance(u, str) and u.startswith(("http://", "https://")):
                bases.add(u.strip())
            for key in ("searchUrl", "exploreUrl"):
                v = src.get(key)
                if not isinstance(v, str):
                    continue
                # 取 url 段(逗号后是 option JSON,不属于 URL 本体)
                for m in re.finditer(r'"url"\s*:\s*"([^"]{0,200})"', v):
                    specs.add(m.group(1))
                head = v.split(",{")[0].split("\n")[0]
                if head and len(head) < 200:
                    specs.add(head)
    rng = random.Random(20260829)
    bases = sorted(bases)
    specs = sorted(s for s in specs if s and "\\" not in s)
    rng.shuffle(bases)
    rng.shuffle(specs)
    bases, specs = bases[:120], specs[:220]
    out = []
    for i, s in enumerate(specs):
        for j in range(3):
            b = bases[(i * 3 + j) % len(bases)]
            out.append({"id": f"uc{len(out):05d}", "op": "absUrl", "base": b, "path": s})
    return out, bases, specs


def main():
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    hand = cross("uh", BASES, SPECS)
    # String 重载(含逗号切断与空 base)
    n = len(hand)
    for b in ["http://www.example.com/a/b.html", "http://a.com/x,{\"method\":\"POST\"}", "", "not a url"]:
        for s in ["d.html", "/x", "http://o.com/z", "../up.html", " "]:
            hand.append({"id": f"uh{n:05d}", "op": "absUrlStr", "base": b, "path": s})
            n += 1
    (OUT_DIR / "handmade.json").write_text(
        json.dumps(hand, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(hand)} handmade cases -> {OUT_DIR/'handmade.json'}", file=sys.stderr)

    corp, bases, specs = corpus()
    (OUT_DIR / "corpus.json").write_text(
        json.dumps(corp, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"{len(corp)} corpus cases({len(bases)} base × {len(specs)} spec 采样)"
          f" -> {OUT_DIR/'corpus.json'}", file=sys.stderr)


if __name__ == "__main__":
    main()
