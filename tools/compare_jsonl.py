#!/usr/bin/env python3
"""结构化比较两份 JSONL 差分输出(按 id 对齐,与键序无关)。

用法: compare_jsonl.py <judge.jsonl> <rust.jsonl> [--show=N] [--exempt=exemptions.json] [--cases=cases.json]
豁免清单条目: {"id": "x042", "reason": "..."} 或 {"pattern": "...", "reason": "..."}
(pattern 形式需要 --cases 提供用例内容;命中豁免的差异单独计数,不算 FAIL)
--cases 同时**钉住分母**:两侧同时漏掉某个 case 时它会算 FAIL,而不是静默
从分母里消失(以前 total 取自两份输出文件的并集,漏成对就看不见了)。
退出码: 0 全部一致(或均被豁免),1 有未豁免差异。
"""
import json
import sys


def load(path):
    out = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            o = json.loads(line)
            # `__ns` 是逐例计时(RUBATO_DIFF_TIME=1)加的旁路字段,不参与比较 ——
            # 两侧的耗时天然不同,不摘掉就会把整套判成 FAIL。
            o.pop("__ns", None)
            out[o["id"]] = o
    return out


def main():
    show = 20
    exempt_path = cases_path = None
    args = []
    for a in sys.argv[1:]:
        if a.startswith("--show="):
            show = int(a.split("=", 1)[1])
        elif a.startswith("--exempt="):
            exempt_path = a.split("=", 1)[1]
        elif a.startswith("--cases="):
            cases_path = a.split("=", 1)[1]
        else:
            args.append(a)
    judge, rust = load(args[0]), load(args[1])

    exempt_ids, exempt_patterns, exempt_fields = {}, {}, []
    if exempt_path:
        for e in json.load(open(exempt_path, encoding="utf-8")):
            if "id" in e:
                exempt_ids[e["id"]] = e["reason"]
            if "pattern" in e:
                exempt_patterns[e["pattern"]] = e["reason"]
            if "fields" in e:
                exempt_fields.append((e["fields"], e["reason"]))
    cases = {}
    if cases_path:
        cases = {c["id"]: c for c in json.load(open(cases_path, encoding="utf-8"))}

    # 分母以 cases 为准(给了 --cases 的话):两侧**同时**漏掉某个 case 时,
    # 老写法的 `set(judge) | set(rust)` 会让它从分母里静默消失 —— 那正是
    # 「判据自己会骗人」的一种。缺席按 FAIL 记。
    ids = sorted(set(judge) | set(rust) | set(cases))
    diffs, exempted = [], []
    for i in ids:
        j, r = judge.get(i), rust.get(i)
        if j is None and r is None:
            # 两侧都没输出这一条(执行器崩了 / 用例被吃掉了)
            j = r = None
        elif j == r:
            continue
        reason = exempt_ids.get(i)
        if reason is None and i in cases:
            reason = exempt_patterns.get(cases[i].get("pattern"))
        if reason is None and i in cases:
            c = cases[i]
            for fields, freason in exempt_fields:
                if all(c.get(k) == v for k, v in fields.items()):
                    reason = freason
                    break
        if reason is not None:
            exempted.append((i, reason))
        else:
            diffs.append((i, j, r))

    total = len(ids)
    tag = f",豁免 {len(exempted)}" if exempted else ""
    print(f"== diff: {total - len(diffs) - len(exempted)} PASS / {len(diffs)} FAIL (共 {total}{tag}) ==")
    for i, reason in exempted:
        print(f"EXEMPT {i}: {reason}")
    for i, j, r in diffs[:show]:
        if j is None and r is None:
            print(f"\nFAIL {i}: 两侧都没有输出(用例在 cases.json 里,但两个执行器都没吐)")
            continue
        print(f"\nFAIL {i}")
        print(f"  judge: {json.dumps(j, ensure_ascii=False)}")
        print(f"  rust : {json.dumps(r, ensure_ascii=False)}")
    if len(diffs) > show:
        print(f"\n... 其余 {len(diffs) - show} 个差异未展示(--show=N 调整)")
    sys.exit(1 if diffs else 0)


if __name__ == "__main__":
    main()
