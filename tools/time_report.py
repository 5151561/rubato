#!/usr/bin/env python3
"""逐例计时报表:同一条 case,裁判侧(legado 真身,JVM)与被测侧(Rust)各花多久。

数据从哪来:两侧的执行器在 `RUBATO_DIFF_TIME=1` 下给每条输出补一个 `__ns`
(见 difftest::run_jsonl 与三个 harness 的 Main.kt)。**只有差分跑得起来的面
才有数** —— 这份报表的分母天然就是差分的分母,不另建一套 benchmark。

口径(报表自己会骗人的地方,四条):

1. **只比两侧输出一致的例**。输出不一致意味着两边干的活不是同一件
   (一侧早早报错、另一侧跑完全程),那种比值没有意义,单列成「不一致」不进分母。
2. **多轮取每例最小值**,轮数见 `RUBATO_DIFF_ROUNDS`(缺省 3)。裁判侧是 JVM,
   第一轮跑在解释器里 —— 不预热量到的是「JIT 没热」,不是「Kotlin 慢」。
3. **网络是录放的**(docs/http-snapshot.md)。这里量的是**纯规则求值**,
   不含真实 IO;而用户感知的「快」多半在 IO 那头,别拿这个数去讲端到端。
4. **裁判侧挂着 shims**。`ACacheShim` 之类是内存垫片,冷算口径与真身不同;
   webview 那套两侧都是虚拟时钟,量的是策略层记账,不含真的等 900ms。

用法: time_report.py <名字> <judge.jsonl> <rust.jsonl> [<名字> <j> <r> ...] [--json=out.json]
"""
import json
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import compare_jsonl  # noqa: E402  (load() 已经会摘掉 __ns,比较口径与差分同一份)


def load_pair(judge_path, rust_path):
    """按 id 对齐,返回 (逐例 (id, judge_ns, rust_ns) 列表, 各类被剔除的计数)。"""
    # 原始行里要保留 __ns —— compare_jsonl.load 摘的是**比较用**的那一份
    def raw(path):
        out = {}
        with open(path, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line:
                    o = json.loads(line)
                    out[o["id"]] = o
        return out

    jr, rr = raw(judge_path), raw(rust_path)
    jc, rc = compare_jsonl.load(judge_path), compare_jsonl.load(rust_path)
    rows, n_diff, n_untimed = [], 0, 0
    for i in sorted(set(jr) & set(rr)):
        if jc[i] != rc[i]:
            n_diff += 1
            continue
        jn, rn = jr[i].get("__ns"), rr[i].get("__ns")
        if jn is None or rn is None or jn <= 0 or rn <= 0:
            n_untimed += 1
            continue
        rows.append((i, jn, rn))
    return rows, n_diff, n_untimed


def dur(ns):
    if ns < 1_000:
        return f"{ns:.0f}ns"
    if ns < 1_000_000:
        return f"{ns / 1_000:.1f}µs"
    if ns < 1_000_000_000:
        return f"{ns / 1_000_000:.1f}ms"
    return f"{ns / 1_000_000_000:.2f}s"


def summarize(name, judge_path, rust_path):
    rows, n_diff, n_untimed = load_pair(judge_path, rust_path)
    if not rows:
        return {"name": name, "n": 0, "diff": n_diff, "untimed": n_untimed}
    jn = [r[1] for r in rows]
    rn = [r[2] for r in rows]
    ratios = sorted(r[1] / r[2] for r in rows)
    slower = sum(1 for r in rows if r[2] > r[1])
    worst = sorted(rows, key=lambda r: r[1] / r[2])[:3]
    return {
        "name": name,
        "n": len(rows),
        "diff": n_diff,
        "untimed": n_untimed,
        "judge_total_ns": sum(jn),
        "rust_total_ns": sum(rn),
        "total_ratio": sum(jn) / sum(rn),
        "judge_p50_ns": statistics.median(jn),
        "rust_p50_ns": statistics.median(rn),
        "ratio_p50": statistics.median(ratios),
        "ratio_p10": ratios[len(ratios) // 10],
        "ratio_p90": ratios[min(len(ratios) - 1, len(ratios) * 9 // 10)],
        "rust_slower": slower,
        # 被测侧相对最吃亏的三条 —— 报表要能指出「哪里没赢」,否则只是自夸
        "worst": [{"id": i, "judge_ns": a, "rust_ns": b, "ratio": a / b} for i, a, b in worst],
    }


def main():
    out_json = None
    pos = []
    for a in sys.argv[1:]:
        if a.startswith("--json="):
            out_json = a.split("=", 1)[1]
        else:
            pos.append(a)
    if not pos or len(pos) % 3:
        print(__doc__)
        return 2

    stats = [summarize(*pos[i:i + 3]) for i in range(0, len(pos), 3)]

    hdr = f"{'套':<18}{'例数':>7}{'裁判 p50':>11}{'被测 p50':>11}{'倍数 p50':>10}{'倍数 p10/p90':>16}{'总耗时比':>10}{'被测更慢':>9}"
    print(hdr)
    print("─" * 86)
    for s in stats:
        if not s["n"]:
            print(f"{s['name']:<18}{'—':>7}   (没有可比的例:不一致 {s['diff']},没计到时 {s['untimed']})")
            continue
        print(
            f"{s['name']:<18}{s['n']:>7}{dur(s['judge_p50_ns']):>11}{dur(s['rust_p50_ns']):>11}"
            f"{s['ratio_p50']:>9.1f}×{s['ratio_p10']:>7.1f}/{s['ratio_p90']:<8.1f}"
            f"{s['total_ratio']:>9.1f}×{s['rust_slower']:>9}"
        )

    ok = [s for s in stats if s["n"]]
    if ok:
        tj = sum(s["judge_total_ns"] for s in ok)
        tr = sum(s["rust_total_ns"] for s in ok)
        n = sum(s["n"] for s in ok)
        print()
        print(
            f"合计 {len(ok)} 套 {n} 例:裁判 {dur(tj)} / 被测 {dur(tr)} —— 总耗时比 {tj / tr:.1f}×;"
            f"按套的倍数中位数 {statistics.median(s['ratio_p50'] for s in ok):.1f}×"
        )
        print(
            f"(总耗时比被最慢的那几例主导,**别拿它当「快多少倍」** —— 逐例倍数的 p50 才是典型值)"
        )
        # 被测侧输的例:有就必须打出来
        worst = sorted(
            (w | {"suite": s["name"]} for s in ok for w in s["worst"]), key=lambda w: w["ratio"]
        )[:8]
        if worst and worst[0]["ratio"] < 1:
            print("\n被测侧更慢的例(倍数 < 1 就是输):")
            for w in worst:
                if w["ratio"] < 1:
                    print(
                        f"  {w['suite']:<18}{w['id']:<28}裁判 {dur(w['judge_ns'])} / "
                        f"被测 {dur(w['rust_ns'])} = {w['ratio']:.2f}×"
                    )
        excluded = sum(s["diff"] + s["untimed"] for s in stats)
        if excluded:
            print(
                f"\n不进分母的 {excluded} 例:输出不一致 {sum(s['diff'] for s in stats)}、"
                f"没计到时 {sum(s['untimed'] for s in stats)}(超时的那条 callback 没跑到计时)"
            )

    print("\n口径见本文件头部四条:只比输出一致的例 / 多轮取最小 / 网络是录放的 / 裁判侧挂着 shims。")

    if out_json:
        Path(out_json).write_text(
            json.dumps(stats, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(f"机器可读的一份写在 {out_json}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
