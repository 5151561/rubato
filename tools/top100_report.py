#!/usr/bin/env python3
"""top100 套的**按源**报表 —— plan §4 Phase 2 判据「自选 top-100 常用源 ≥90%」。

为什么另写一份而不是看 compare_jsonl.py 的汇总行:那一行是**按 case** 算的,
分母 446。一个源错一步只丢 1/446,通过率天然好看 —— 判据几乎没有信息量。
「这个源能不能用」的意思是**它的四五步全都对**,所以判据按源算:

    源通过 ⟺ 它名下的每一条 case 都 PASS

**豁免不算通过**,但源仍留在 100 的分母里。豁免买到的是「这一例不算 FAIL、
不进 CI 棘轮」,买不到「这个源算能用」—— 否则把跑不动的那支逐条豁免掉,
判据就被躲掉的部分注水了。报表把「仅因豁免支未过」的源单列出来,看得见。

比较与豁免的判定逻辑直接复用 compare_jsonl.py,不另写一份(两份会漂)。

用法: top100_report.py <judge.jsonl> <rust.jsonl> --selection=selection.json
                        --cases=cases.json [--exempt=exemptions.json]
退出码: 0 达标(≥90/100),1 未达标。
"""
import json
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import compare_jsonl  # noqa: E402

THRESHOLD = 90  # plan §4:≥90%(名单正好 100 源,故就是 90 个源)


def main():
    sel_path = cases_path = exempt_path = None
    args = []
    for a in sys.argv[1:]:
        if a.startswith("--selection="):
            sel_path = a.split("=", 1)[1]
        elif a.startswith("--cases="):
            cases_path = a.split("=", 1)[1]
        elif a.startswith("--exempt="):
            exempt_path = a.split("=", 1)[1]
        elif a.startswith("--"):
            continue  # --show=N 之类给 compare_jsonl.py 的,这里不管
        else:
            args.append(a)
    assert sel_path and cases_path, "--selection= 与 --cases= 都要给"

    judge, rust = compare_jsonl.load(args[0]), compare_jsonl.load(args[1])
    cases = {c["id"]: c for c in json.load(open(cases_path, encoding="utf-8"))}
    selection = json.load(open(sel_path, encoding="utf-8"))

    exempt_ids, exempt_patterns, exempt_fields = {}, {}, []
    if exempt_path and Path(exempt_path).exists():
        for e in json.load(open(exempt_path, encoding="utf-8")):
            if "id" in e:
                exempt_ids[e["id"]] = e["reason"]
            if "pattern" in e:
                exempt_patterns[e["pattern"]] = e["reason"]
            if "fields" in e:
                exempt_fields.append((e["fields"], e["reason"]))

    def verdict(cid):
        """PASS / EXEMPT / FAIL —— 与 compare_jsonl.py 逐字同一套判定"""
        j, r = judge.get(cid), rust.get(cid)
        if j is not None and j == r:
            return "PASS", None
        reason = exempt_ids.get(cid)
        c = cases.get(cid, {})
        if reason is None:
            reason = exempt_patterns.get(c.get("pattern"))
        if reason is None:
            for fields, freason in exempt_fields:
                if all(c.get(k) == v for k, v in fields.items()):
                    reason = freason
                    break
        return ("EXEMPT", reason) if reason is not None else ("FAIL", None)

    rows = []
    for src in selection:
        bad, exempted = [], []
        for cid in src["cases"]:
            v, _ = verdict(cid)
            if v == "FAIL":
                bad.append(cases.get(cid, {}).get("step", cid))
            elif v == "EXEMPT":
                exempted.append(cases.get(cid, {}).get("step", cid))
        rows.append((src, bad, exempted))

    passed = [r for r in rows if not r[1] and not r[2]]
    only_exempt = [r for r in rows if not r[1] and r[2]]
    failed = [r for r in rows if r[1]]

    n = len(rows)
    print(f"== top-100 按源:{len(passed)} PASS / {len(failed) + len(only_exempt)} 未过"
          f"(共 {n};其中 {len(only_exempt)} 源仅因豁免支未过)== "
          f"判据线 {THRESHOLD}/{n}")
    by_tier = Counter(s["tier"] for s, _, _ in rows)
    ok_tier = Counter(s["tier"] for s, _, _ in passed)
    print("   分层:" + " ".join(
        f"{t} {ok_tier.get(t, 0)}/{by_tier[t]}" for t in sorted(by_tier)))

    for src, bad, exempted in failed + only_exempt:
        marks = []
        if bad:
            marks.append("FAIL " + ",".join(Counter(bad)))
        if exempted:
            marks.append("豁免 " + ",".join(Counter(exempted)))
        print(f"   [{src['tier']}] {src['name']}  {src['url']}  ({'; '.join(marks)})")

    sys.exit(0 if len(passed) >= THRESHOLD else 1)


if __name__ == "__main__":
    main()
