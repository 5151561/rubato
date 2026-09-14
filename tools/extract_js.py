#!/usr/bin/env python3
"""从 fixtures/sources/*.json 抽取书源 JS 片段,分层统计,写入 fixtures/corpus-js/。"""
import json, re, hashlib, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import phase3_caps  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "fixtures/sources"
OUT = ROOT / "fixtures/corpus-js"
OUT.mkdir(exist_ok=True)

JS_BLOCK = re.compile(r"<js>(.*?)</js>", re.S | re.I)

def walk(o, path, hits):
    if isinstance(o, dict):
        for k, v in o.items():
            walk(v, f"{path}.{k}", hits)
    elif isinstance(o, str):
        for m in JS_BLOCK.finditer(o):
            hits.append((path, m.group(1)))
        if o.startswith("@js:"):
            hits.append((path, o[4:]))
        if path.endswith(".jsLib") and o.strip():
            hits.append((path, o))

# 分层口径只有一份:tools/phase3_caps.py(**按值判定**,空的 webJs/sourceRegex
# 键不算 C 层)。gen_pipeline_corpus_cases.py 读的是同一份。
classify = phase3_caps.classify

def main():
    tiers = {"A": 0, "B": 0, "C": 0}
    snippets = {}
    total = 0
    for f in sorted(SRC.glob("*.json")):
        data = json.load(open(f))
        if not isinstance(data, list):
            continue
        for s in data:
            if not isinstance(s, dict):
                continue
            total += 1
            tiers[classify(s)] += 1
            hits = []
            walk(s, "", hits)
            for path, code in hits:
                code = code.strip()
                if len(code) < 8:
                    continue
                h = hashlib.md5(code.encode()).hexdigest()[:10]
                if h not in snippets:
                    snippets[h] = (s.get("bookSourceName", "?"), path, code)
    for h, (name, path, code) in snippets.items():
        (OUT / f"{h}.js").write_text(f"// from: {name} {path}\n{code}\n")
    print(f"sources: {total}  tiers: {tiers}")
    print(f"unique js snippets: {len(snippets)} -> {OUT}")

if __name__ == "__main__":
    main()
